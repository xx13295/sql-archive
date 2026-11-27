use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Duration, Utc};
use regex::Regex;
use serde_json::json;

use crate::error::{AppError, AppResult};
use crate::filename::{archive_file_name, channel_for_db_dir, group_key, parse_source_name};
use crate::java_bridge::{BridgeScript, JavaEngine};
use crate::models::{
    ArchivePlan, ParsedScriptName, PlanFile, PlanItem, PlanTotals, RootProfile, ScriptItem,
    ShardingChange, TargetFile, ValidationResult,
};
use crate::scan::scan_product;
use crate::storage::Storage;
use crate::util::{now_rfc3339, sha256_bytes, timestamp_for_backup};
use crate::validation::validate_selected;
use crate::workspace;

pub struct PreviewContext<'a> {
    pub profile: &'a RootProfile,
    pub engine: &'a JavaEngine,
    pub storage: &'a Storage,
    pub data_dir: &'a Path,
    pub product: &'a str,
    pub relative_paths: &'a [String],
    pub module_child: &'a str,
    pub general_child: &'a str,
}

struct SourceFile<'a> {
    relative_path: String,
    item: &'a ScriptItem,
    parsed: ParsedScriptName,
    validation: ValidationResult,
}

pub fn preview_archive(context: PreviewContext<'_>) -> AppResult<ArchivePlan> {
    if context.relative_paths.is_empty() {
        return Err(AppError::user("NO_SCRIPTS", "请先选择需要归档的脚本。"));
    }
    let scan = scan_product(context.profile, context.storage, context.product)?;
    let validations = validate_selected(
        context.profile,
        context.storage,
        Some(context.engine),
        context.product,
        context.relative_paths,
    )?;
    let mut conflicts = Vec::new();
    let mut sources: Vec<SourceFile<'_>> = Vec::new();
    for relative_path in context.relative_paths {
        let Some(item) = scan
            .scripts
            .iter()
            .find(|script| &script.relative_path == relative_path)
        else {
            conflicts.push(format!("{relative_path}：文件不存在或已变更。"));
            continue;
        };
        if item.mark != "passed" {
            conflicts.push(format!("{relative_path}：尚未标记为通过。"));
            continue;
        }
        let validation = validations
            .get(relative_path)
            .cloned()
            .unwrap_or_else(ValidationResult::empty_ok);
        if !validation.errors.is_empty() {
            conflicts.push(format!("{relative_path}：{}", validation.errors.join("；")));
            continue;
        }
        let parsed = match parse_source_name(&item.file_name) {
            Ok(parsed) => parsed,
            Err(message) => {
                conflicts.push(format!("{relative_path}：{message}"));
                continue;
            }
        };
        sources.push(SourceFile {
            relative_path: relative_path.clone(),
            item,
            parsed,
            validation,
        });
    }

    let backup_date = timestamp_for_backup();
    let root = Path::new(&context.profile.root_path);
    let flyway_root: Option<&str> = context.profile.flyway_root.as_deref();

    let plan_id = uuid::Uuid::new_v4().to_string();
    let plan_dir = context.data_dir.join("plans").join(&plan_id);
    fs::create_dir_all(plan_dir.join("targets"))
        .map_err(|error| AppError::io("创建归档计划目录失败", error))?;

    // 按版本分组并扫描基准序号。
    let mut by_version: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, source) in sources.iter().enumerate() {
        by_version
            .entry(source.parsed.version.clone())
            .or_default()
            .push(index);
    }
    let mut sequence_base = BTreeMap::new();
    let mut sequences: HashMap<usize, u32> = HashMap::new();
    if flyway_root.is_some() {
        for (version, indexes) in &by_version {
            let (base, _dirs) = workspace::latest_sequence_for(
                context.profile,
                context.product,
                version,
                context.module_child,
                context.general_child,
            )?;
            sequence_base.insert(version.clone(), base);
            let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
            for index in indexes {
                groups
                    .entry(group_key(&sources[*index].parsed))
                    .or_default()
                    .push(*index);
            }
            let mut next = base;
            for (_key, mut group) in groups {
                next += 1;
                group.sort_by(|left, right| {
                    sources[*left]
                        .parsed
                        .db_type
                        .cmp(&sources[*right].parsed.db_type)
                });
                for index in group {
                    sequences.insert(index, next);
                }
            }
        }
    }

    // 规划目标通道，并按 (源库, 目标通道) 批量翻译。
    let mut target_plans: HashMap<usize, Vec<(String, String)>> = HashMap::new();
    let mut unsupported: HashMap<usize, Vec<String>> = HashMap::new();
    for (index, source) in sources.iter().enumerate() {
        let dirs = if source.parsed.db_type == "ORACLE" {
            &context.profile.oracle_targets
        } else {
            &context.profile.tdsql_targets
        };
        if dirs.is_empty() {
            unsupported
                .entry(index)
                .or_default()
                .push(format!("未配置 {} 目标数据库目录。", source.parsed.db_type));
            continue;
        }
        let mut planned = Vec::new();
        for db_dir in dirs {
            match channel_for_db_dir(db_dir) {
                Some(channel) => planned.push((db_dir.clone(), channel.to_string())),
                None => unsupported
                    .entry(index)
                    .or_default()
                    .push(format!("目标库 {db_dir} 没有对应的引擎转换通道，已跳过。")),
            }
        }
        target_plans.insert(index, planned);
    }

    let mut translation_results: HashMap<usize, HashMap<String, (bool, String)>> = HashMap::new();
    let mut translation_errors: HashMap<usize, Vec<String>> = HashMap::new();
    let mut sharding = context.engine.sharding.clone();
    let mut affected_tables = BTreeSet::new();
    let mut call_groups: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (index, targets) in &target_plans {
        let source_db = sources[*index].parsed.db_type.clone();
        for (_db_dir, channel) in targets {
            call_groups
                .entry((source_db.clone(), channel.clone()))
                .or_default()
                .push(*index);
        }
    }
    for ((source_db, channel), indexes) in call_groups {
        let scripts = indexes
            .iter()
            .map(|index| {
                let path = root
                    .join("temp")
                    .join(context.product)
                    .join(&sources[*index].relative_path);
                let content = crate::util::read_text_file(&path).map(|(text, _)| text)?;
                Ok(BridgeScript {
                    index: *index,
                    file_name: sources[*index].item.file_name.clone(),
                    content,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        match context
            .engine
            .translate_batch_with_sharding(&scripts, &source_db, &channel, &sharding)
        {
            Ok(output) => {
                sharding = output.sharding.clone();
                for index in &indexes {
                    let key = (*index, channel.clone());
                    if let Some(result) = output.translate.get(&key) {
                        if result.ok {
                            if let Some(sql) = &result.sql {
                                translation_results
                                    .entry(*index)
                                    .or_default()
                                    .insert(channel.clone(), (result.flag, sql.clone()));
                                if result.flag {
                                    collect_tables(sql, &mut affected_tables);
                                }
                            } else {
                                translation_errors
                                    .entry(*index)
                                    .or_default()
                                    .push(format!("{channel}：引擎未返回脚本内容。"));
                            }
                        } else {
                            translation_errors
                                .entry(*index)
                                .or_default()
                                .push(format!("{channel}：{}", result.message));
                        }
                    } else {
                        translation_errors
                            .entry(*index)
                            .or_default()
                            .push(format!("{channel}：引擎未返回结果。"));
                    }
                }
            }
            Err(error) => {
                for index in &indexes {
                    translation_errors
                        .entry(*index)
                        .or_default()
                        .push(format!("{channel}：{}", error.message));
                }
            }
        }
    }

    let mut items = Vec::new();
    let mut case_insensitive_targets: BTreeMap<String, usize> = BTreeMap::new();
    for (index, source) in sources.iter().enumerate() {
        let sequence = sequences.get(&index).copied().unwrap_or(0);
        let mut blockers = Vec::new();
        if sequence == 0 {
            blockers.push("未能计算归档序号（未找到 Flyway 目录）。".to_string());
        }
        if flyway_root.is_none() {
            blockers.push("未找到 Flyway 目录。".to_string());
        }
        if let Some(messages) = unsupported.get(&index) {
            blockers.extend(messages.clone());
        }
        if let Some(messages) = translation_errors.get(&index) {
            blockers.extend(messages.clone());
        }
        let archive_name = archive_file_name(
            &source.parsed.version,
            context.module_child,
            "00",
            sequence,
            &source.parsed.description,
            &source.parsed.db_type,
            &source.parsed.extension,
        );
        let backup_path = workspace::backup_product_dir(root, context.product, &backup_date)
            .join(&source.item.file_name);
        if backup_path.exists() {
            blockers.push(format!("备份文件已存在：{}", backup_path.display()));
        }
        let mut targets = Vec::new();
        if let (Some(flyway_root), Some(planned)) = (flyway_root, target_plans.get(&index)) {
            for (target_index, (db_dir, channel)) in planned.iter().enumerate() {
                let Some((_sub, sql)) = translation_results
                    .get(&index)
                    .and_then(|map| map.get(channel))
                else {
                    continue;
                };
                let target_path = workspace::version_general_dir(
                    Path::new(flyway_root),
                    context.product,
                    &source.parsed.version,
                    context.module_child,
                    context.general_child,
                )
                .join(db_dir)
                .join(&archive_name);
                if target_path.exists() {
                    blockers.push(format!("归档文件已存在：{}", target_path.display()));
                }
                let lower = target_path.to_string_lossy().to_lowercase();
                if let Some(previous) = case_insensitive_targets.get(&lower) {
                    if *previous != index {
                        blockers.push(format!(
                            "多个源文件映射到同一归档文件（大小写不敏感）：{}",
                            target_path.display()
                        ));
                    }
                } else {
                    case_insensitive_targets.insert(lower, index);
                }
                let staged_path = plan_dir
                    .join("targets")
                    .join(format!("{index}_{target_index}.groovy"));
                fs::write(&staged_path, sql.as_bytes())
                    .map_err(|error| AppError::io("写入翻译结果失败", error))?;
                targets.push(TargetFile {
                    db_dir: db_dir.clone(),
                    path: target_path.to_string_lossy().to_string(),
                    file_name: archive_name.clone(),
                    channel: channel.clone(),
                    byte_size: sql.len() as u64,
                    sha256: sha256_bytes(sql.as_bytes()),
                    staged_path: Some(staged_path.to_string_lossy().to_string()),
                });
            }
        }
        if targets.is_empty() && blockers.is_empty() {
            blockers.push("没有任何可生成的目标脚本。".to_string());
        }
        items.push(PlanItem {
            group_key: group_key(&source.parsed),
            source_path: root
                .join("temp")
                .join(context.product)
                .join(&source.relative_path)
                .to_string_lossy()
                .to_string(),
            source_repo_relative_path: source.item.repo_relative_path.clone(),
            source_file_name: source.item.file_name.clone(),
            source_db: source.parsed.db_type.clone(),
            source_version: source.parsed.version.clone(),
            sequence,
            backup_path: backup_path.to_string_lossy().to_string(),
            targets,
            tenant_code_warning: source.validation.tenant_code_warning,
            state: if blockers.is_empty() {
                "ready".into()
            } else {
                "blocked".into()
            },
            blockers,
        });
    }
    items.sort_by(|left, right| {
        left.source_version
            .cmp(&right.source_version)
            .then(left.sequence.cmp(&right.sequence))
            .then(left.source_db.cmp(&right.source_db))
    });

    let mut warnings = Vec::new();
    for source in &sources {
        for warning in &source.validation.warnings {
            warnings.push(format!("{}：{warning}", source.relative_path));
        }
    }
    for item in &items {
        if item.tenant_code_warning {
            warnings.push(format!(
                "{} 包含未分表且带租户编码字段，归档前需单独确认。",
                item.source_file_name
            ));
        }
    }
    // 只有检测到分表建表语句时才更新分表模板；引擎对非分表脚本也会归一化 YAML，
    // 不能把这种归一化当成真实变更。
    let sharding_changed =
        !affected_tables.is_empty() && sharding.trim_end() != context.engine.sharding.trim_end();
    let mut sharding_change = None;
    let mut sharding_staged_path = None;
    let mut sharding_after_sha = None;
    if sharding_changed {
        if let Some(path) = context.profile.sharding_path.clone() {
            let staged = plan_dir.join("sharding.yml");
            fs::write(&staged, sharding.as_bytes())
                .map_err(|error| AppError::io("写入分表模板预览失败", error))?;
            let after_sha = sha256_bytes(sharding.as_bytes());
            sharding_after_sha = Some(after_sha.clone());
            sharding_staged_path = Some(staged.to_string_lossy().to_string());
            let before_sha = if Path::new(&path).is_file() {
                crate::util::sha256_file(Path::new(&path)).ok()
            } else {
                None
            };
            sharding_change = Some(ShardingChange {
                path,
                before_sha,
                after_sha: Some(after_sha),
                changed: true,
                affected_tables: affected_tables.into_iter().collect(),
                preview: sharding.chars().take(20000).collect(),
            });
        } else {
            conflicts.push(
                "检测到分表模板需要更新，但当前仓库没有 saas-sharding.yml.vm，无法安全写回。"
                    .into(),
            );
        }
    }

    for item in &items {
        for blocker in &item.blockers {
            conflicts.push(format!("{}：{}", item.source_file_name, blocker));
        }
    }
    conflicts.dedup();
    let versions = by_version.keys().cloned().collect::<Vec<_>>();
    let target_count: usize = items.iter().map(|item| item.targets.len()).sum();
    let plan = ArchivePlan {
        id: plan_id.clone(),
        root_path: context.profile.root_path.clone(),
        product: context.product.to_string(),
        created_at: now_rfc3339(),
        expires_at: (Utc::now() + Duration::minutes(30)).to_rfc3339(),
        items,
        conflicts,
        warnings,
        sharding_change,
        digest: String::new(),
        totals: PlanTotals {
            source_count: sources.len(),
            target_count,
            backup_count: sources.len(),
            versions: versions.clone(),
        },
    };
    let source_hashes = sources
        .iter()
        .map(|source| (source.relative_path.clone(), source.item.sha256.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut file = PlanFile {
        plan,
        staging_dir: plan_dir.to_string_lossy().to_string(),
        flyway_root: flyway_root.unwrap_or_default().to_string(),
        module_child: context.module_child.to_string(),
        general_child: context.general_child.to_string(),
        engine_fingerprint: context.engine.fingerprint.clone(),
        check_rule_sha256: context.engine.check_rule_sha.clone(),
        sharding_before_sha: Some(context.engine.sharding_sha.clone()),
        sharding_changed,
        sharding_staged_path,
        sharding_after_sha,
        sequence_base,
        source_hashes,
        state: "staged".into(),
    };
    file.plan.digest = compute_digest(&file);
    write_plan_file(&plan_dir, &file)?;
    Ok(file.plan)
}

fn collect_tables(sql: &str, tables: &mut BTreeSet<String>) {
    let regex = Regex::new(r#"(?i)create\s+table\s+[`"]?([a-zA-Z0-9_]+)"#).expect("regex");
    for capture in regex.captures_iter(sql) {
        if let Some(name) = capture.get(1) {
            tables.insert(name.as_str().to_string());
        }
    }
}

pub fn compute_digest(file: &PlanFile) -> String {
    let mut clone = file.clone();
    clone.plan.digest = String::new();
    let payload = json!({
        "plan": clone.plan,
        "flywayRoot": clone.flyway_root,
        "moduleChild": clone.module_child,
        "generalChild": clone.general_child,
        "engineFingerprint": clone.engine_fingerprint,
        "checkRuleSha256": clone.check_rule_sha256,
        "shardingBeforeSha": clone.sharding_before_sha,
        "shardingChanged": clone.sharding_changed,
        "shardingAfterSha": clone.sharding_after_sha,
        "sequenceBase": clone.sequence_base,
        "sourceHashes": clone.source_hashes,
    });
    sha256_bytes(payload.to_string().as_bytes())
}

pub fn plan_dir(data_dir: &Path, plan_id: &str) -> AppResult<PathBuf> {
    if plan_id.is_empty()
        || !plan_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return Err(AppError::user("PLAN_INVALID", "归档计划编号无效。"));
    }
    Ok(data_dir.join("plans").join(plan_id))
}

pub fn write_plan_file(dir: &Path, file: &PlanFile) -> AppResult<()> {
    let text = serde_json::to_vec_pretty(file)
        .map_err(|error| AppError::internal("PLAN_SERIALIZE", error.to_string()))?;
    crate::util::atomic_write(&dir.join("plan.json"), &text)
}

pub fn load_plan_file(data_dir: &Path, plan_id: &str) -> AppResult<PlanFile> {
    let dir = plan_dir(data_dir, plan_id)?;
    let text = fs::read(dir.join("plan.json"))
        .map_err(|_| AppError::user("PLAN_NOT_FOUND", "归档计划不存在或已过期。"))?;
    serde_json::from_slice(&text)
        .map_err(|error| AppError::user("PLAN_INVALID", format!("归档计划已损坏：{error}")))
}

pub fn verify_plan(file: &PlanFile, data_dir: &Path) -> AppResult<()> {
    if file.state != "staged" {
        return Err(AppError::user(
            "PLAN_CONSUMED",
            "该归档计划已执行或已失效。",
        ));
    }
    let expired = chrono::DateTime::parse_from_rfc3339(&file.plan.expires_at)
        .map(|value| value < Utc::now())
        .unwrap_or(true);
    if expired {
        return Err(AppError::user(
            "PLAN_EXPIRED",
            "归档计划已过期，请重新生成预览。",
        ));
    }
    if compute_digest(file) != file.plan.digest {
        return Err(AppError::user(
            "PLAN_STALE",
            "归档计划内容与预览不一致，请重新预览。",
        ));
    }
    for (relative_path, expected) in &file.source_hashes {
        let path = Path::new(&file.plan.root_path)
            .join("temp")
            .join(&file.plan.product)
            .join(relative_path);
        let actual = crate::util::sha256_file(&path).map_err(|_| {
            AppError::user(
                "PLAN_STALE",
                format!("源文件 {relative_path} 已被移动或删除，请重新预览。"),
            )
        })?;
        if &actual != expected {
            return Err(AppError::user(
                "PLAN_STALE",
                format!("源文件 {relative_path} 内容已变化，请重新预览。"),
            ));
        }
    }
    for item in &file.plan.items {
        if Path::new(&item.backup_path).exists() {
            return Err(AppError::user(
                "PLAN_STALE",
                format!("备份目标已出现：{}", item.backup_path),
            ));
        }
        for target in &item.targets {
            if Path::new(&target.path).exists() {
                return Err(AppError::user(
                    "PLAN_STALE",
                    format!("归档目标已出现：{}", target.path),
                ));
            }
        }
    }
    if let Some(change) = &file.plan.sharding_change {
        let actual = crate::util::sha256_file(Path::new(&change.path)).ok();
        if actual != change.before_sha {
            return Err(AppError::user(
                "PLAN_STALE",
                "分表模板在预览后被修改，请重新预览。",
            ));
        }
    }
    // 序号基准必须与预览时一致；外部新增归档文件会改变序号。
    for (version, base) in &file.sequence_base {
        let base_dir = workspace::version_general_dir(
            Path::new(&file.flyway_root),
            &file.plan.product,
            version,
            &file.module_child,
            &file.general_child,
        );
        let latest = crate::sequence::latest_sequence(&base_dir)?;
        if latest != *base {
            return Err(AppError::user(
                "PLAN_STALE",
                "归档目录的序号在预览后发生变化，请重新预览。",
            ));
        }
    }
    // 确认 staged 内容仍然存在。
    for item in &file.plan.items {
        for target in &item.targets {
            if let Some(staged) = &target.staged_path {
                if !Path::new(staged).is_file() {
                    return Err(AppError::user(
                        "PLAN_STALE",
                        "预览生成的脚本内容已丢失，请重新预览。",
                    ));
                }
            }
        }
    }
    let _ = data_dir;
    Ok(())
}

pub fn mark_plan_consumed(data_dir: &Path, plan_id: &str) -> AppResult<()> {
    let mut file = load_plan_file(data_dir, plan_id)?;
    file.state = "consumed".into();
    write_plan_file(&plan_dir(data_dir, plan_id)?, &file)
}

pub fn cleanup_expired_plans(data_dir: &Path) {
    let base = data_dir.join("plans");
    let Ok(entries) = fs::read_dir(&base) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if let Ok(file) = load_plan_file(data_dir, name) {
            let expired = Utc::now().to_rfc3339() > file.plan.expires_at;
            if expired || file.state == "consumed" {
                let _ = fs::remove_dir_all(&path);
            }
        }
    }
}
