use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use crate::error::AppResult;
use crate::filename::{compare_versions, group_key, parse_source_name};
use crate::java_bridge::{BridgeScript, JavaEngine};
use crate::models::{RootProfile, ValidationResult};
use crate::storage::{root_id, Storage};
use crate::util::{now_rfc3339, read_text_file};
use crate::workspace;

const MAX_SCRIPT_SIZE: u64 = 64 * 1024;

pub fn validate_selected(
    profile: &RootProfile,
    storage: &Storage,
    engine: Option<&JavaEngine>,
    product: &str,
    relative_paths: &[String],
) -> AppResult<HashMap<String, ValidationResult>> {
    let scan = crate::scan::scan_product(profile, storage, product)?;
    let root = Path::new(&profile.root_path);
    let temp_dir = workspace::temp_product_dir(root, product);
    // 只在产品配置了基线（releaseVersion / baseLineVersion）时做版本下限校验；
    // 未配置时不能用“最大版本目录”推断，否则会误伤仍在归档的旧版本脚本。
    let baseline = workspace::configured_baseline(profile, product);
    let industry = !profile.oracle_targets.is_empty();
    let mut results = HashMap::new();
    let mut engine_inputs: BTreeMap<String, Vec<(usize, String, String)>> = BTreeMap::new();

    for (index, relative_path) in relative_paths.iter().enumerate() {
        let path = temp_dir.join(relative_path);
        let mut result = ValidationResult::empty_ok();
        let Some(item) = scan
            .scripts
            .iter()
            .find(|script| &script.relative_path == relative_path)
        else {
            result.errors.push("文件不在当前产品提交目录中。".into());
            results.insert(relative_path.clone(), result.finalize());
            continue;
        };
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                result.errors.push(format!("无法读取文件：{error}"));
                results.insert(relative_path.clone(), result.finalize());
                continue;
            }
        };
        if bytes.len() as u64 > MAX_SCRIPT_SIZE {
            result
                .errors
                .push(format!("文件大小 {} 字节，超过 64 KiB 限制。", bytes.len()));
        }
        let parsed = match parse_source_name(&item.file_name) {
            Ok(parsed) => Some(parsed),
            Err(message) => {
                result.errors.push(message);
                None
            }
        };
        if let Some(parsed) = &parsed {
            if let Some(baseline) = &baseline {
                match compare_versions(&parsed.version, baseline) {
                    Ordering::Less => result.errors.push(format!(
                        "提交版本 {} 低于当前基线 {}。",
                        parsed.version, baseline
                    )),
                    Ordering::Greater => result.warnings.push(format!(
                        "提交版本 {} 高于当前基线 {}。",
                        parsed.version, baseline
                    )),
                    Ordering::Equal => {}
                }
            }
            if !industry && parsed.db_type == "ORACLE" {
                result
                    .errors
                    .push("运营支撑门户只允许提交 TDSQL 脚本。".into());
            }
            if industry {
                let key = group_key(parsed);
                let group = scan.groups.iter().find(|group| group.key == key);
                let has_both = group
                    .map(|group| group.db_types.len() >= 2)
                    .unwrap_or(false);
                if !has_both {
                    result
                        .errors
                        .push("行业应用要求同一变更同时提交 ORACLE 与 TDSQL 脚本。".into());
                }
            }
            let duplicate = scan
                .scripts
                .iter()
                .filter(|script| {
                    script
                        .parsed
                        .as_ref()
                        .map(|other| {
                            group_key(other) == group_key(parsed) && other.db_type == parsed.db_type
                        })
                        .unwrap_or(false)
                })
                .count();
            if duplicate > 1 {
                result
                    .errors
                    .push("同一变更组内该数据库类型的脚本出现多次，顺序号必须唯一。".into());
            }
            let source_db = parsed.db_type.clone();
            engine_inputs.entry(source_db).or_default().push((
                index,
                relative_path.clone(),
                String::new(),
            ));
        } else {
            result.errors.push("文件名无法解析，不能参与归档。".into());
        }
        results.insert(relative_path.clone(), result);
    }

    if let Some(engine) = engine {
        let sharding_sha = engine.sharding_sha.clone();
        let rule_sha = engine.check_rule_sha.clone();
        let engine_id = engine.fingerprint.clone();
        let root_id = root_id(&profile.root_path);
        for (source_db, mut inputs) in engine_inputs {
            let mut scripts = Vec::new();
            for (index, relative_path, _) in inputs.iter_mut() {
                let path = temp_dir.join(&relative_path);
                match read_text_file(&path) {
                    Ok((content, _)) => {
                        let item = scan
                            .scripts
                            .iter()
                            .find(|script| &script.relative_path == relative_path)
                            .expect("scan item");
                        let cached = storage.get_validation(
                            &root_id,
                            &item.sha256,
                            &source_db,
                            &engine_id,
                            &rule_sha,
                            &sharding_sha,
                        )?;
                        if let Some(cached) = cached {
                            merge(&mut results, relative_path, cached);
                        } else {
                            scripts.push(BridgeScript {
                                index: *index,
                                file_name: item.file_name.clone(),
                                content,
                            });
                        }
                    }
                    Err(error) => {
                        merge_error(&mut results, relative_path, error.message);
                    }
                }
            }
            if scripts.is_empty() {
                continue;
            }
            let outcomes = engine.check_batch(&scripts, &source_db)?;
            for (index, relative_path, _) in inputs.iter_mut() {
                if !scripts.iter().any(|script| script.index == *index) {
                    continue;
                }
                let item = scan
                    .scripts
                    .iter()
                    .find(|script| &script.relative_path == relative_path)
                    .expect("scan item");
                match outcomes.get(index) {
                    Some(outcome) if outcome.ok => {
                        let mut engine_result = ValidationResult::empty_ok();
                        engine_result.tenant_code_warning = outcome.flag;
                        storage.put_validation(
                            &root_id,
                            &item.sha256,
                            &source_db,
                            &engine_id,
                            &rule_sha,
                            &sharding_sha,
                            &engine_result,
                        )?;
                        merge(&mut results, relative_path, engine_result);
                    }
                    Some(outcome) => {
                        let message = if outcome.message.trim().is_empty() {
                            "引擎校验未通过。".to_string()
                        } else {
                            outcome.message.clone()
                        };
                        let mut engine_result = ValidationResult::empty_ok();
                        engine_result.errors.push(message);
                        storage.put_validation(
                            &root_id,
                            &item.sha256,
                            &source_db,
                            &engine_id,
                            &rule_sha,
                            &sharding_sha,
                            &engine_result,
                        )?;
                        merge(&mut results, relative_path, engine_result);
                    }
                    None => {
                        merge_error(&mut results, relative_path, "引擎未返回该校验结果。".into())
                    }
                }
            }
        }
    } else {
        for result in results.values_mut() {
            result
                .warnings
                .push("未执行 Java 引擎校验（引擎不可用）。".into());
        }
    }

    for result in results.values_mut() {
        result.checked_at = now_rfc3339();
        result.status = if !result.errors.is_empty() {
            "error".into()
        } else if !result.warnings.is_empty() || result.tenant_code_warning {
            "warning".into()
        } else {
            "ok".into()
        };
    }
    Ok(results)
}

fn merge(results: &mut HashMap<String, ValidationResult>, key: &str, other: ValidationResult) {
    if let Some(existing) = results.get_mut(key) {
        existing.errors.extend(other.errors);
        existing.warnings.extend(other.warnings);
        existing.tenant_code_warning |= other.tenant_code_warning;
    }
}

fn merge_error(results: &mut HashMap<String, ValidationResult>, key: &str, message: String) {
    if let Some(existing) = results.get_mut(key) {
        existing.errors.push(message);
    }
}
