//! 归档执行器：按计划写入归档文件、备份源文件、更新分表模板，失败时回滚。

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{AppError, AppResult};
use crate::models::{ArchiveRun, ConfirmArchiveInput, RunItem, RunStep};
use crate::plan::{load_plan_file, mark_plan_consumed, verify_plan};
use crate::storage::{RunCounts, Storage};
use crate::util::{atomic_write, now_rfc3339, sha256_bytes, sha256_file};

pub fn confirm_archive(
    storage: &Storage,
    data_dir: &Path,
    input: ConfirmArchiveInput,
) -> AppResult<ArchiveRun> {
    if !input.confirmed {
        return Err(AppError::user(
            "CONFIRM_REQUIRED",
            "需要显式确认后才能执行归档。",
        ));
    }
    let file = load_plan_file(data_dir, &input.plan_id)?;
    verify_plan(&file, data_dir)?;
    if !file.plan.conflicts.is_empty() {
        return Err(AppError::user(
            "PLAN_BLOCKED",
            "归档计划存在冲突或阻断项，请重新预览并处理。",
        ));
    }
    let acked: BTreeSet<&str> = input.tenant_ack_paths.iter().map(String::as_str).collect();
    for item in &file.plan.items {
        if item.state != "ready" || !item.blockers.is_empty() {
            return Err(AppError::user(
                "PLAN_BLOCKED",
                "归档计划包含不可执行的项目。",
            ));
        }
        if item.tenant_code_warning
            && !acked.contains(item.source_file_name.as_str())
            && !acked.contains(item.source_path.as_str())
        {
            return Err(AppError::user(
                "TENANT_ACK_REQUIRED",
                format!("请先确认租户编码警告：{}", item.source_file_name),
            ));
        }
    }

    let run_id = uuid::Uuid::new_v4().to_string();
    let backup_date = file
        .plan
        .items
        .first()
        .map(|item| {
            Path::new(&item.backup_path)
                .parent()
                .and_then(Path::file_name)
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string()
        })
        .unwrap_or_default();
    let sharding_path = file
        .plan
        .sharding_change
        .as_ref()
        .map(|change| change.path.clone());
    let sharding_existed = sharding_path
        .as_deref()
        .map(|path| Path::new(path).is_file())
        .unwrap_or(false);
    let sharding_backup_path = if let Some(path) = &sharding_path {
        let file_name = Path::new(path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("saas-sharding.yml.vm");
        Some(
            data_dir
                .join("sharding-backups")
                .join(&run_id)
                .join(file_name)
                .to_string_lossy()
                .to_string(),
        )
    } else {
        None
    };
    let counts = RunCounts {
        versions: file.plan.totals.versions.clone(),
        source_count: file.plan.totals.source_count as i64,
        target_count: file.plan.totals.target_count as i64,
        sharding_path: sharding_path.clone(),
        sharding_backup_path: sharding_backup_path.clone(),
        sharding_before_sha: file
            .plan
            .sharding_change
            .as_ref()
            .and_then(|change| change.before_sha.clone()),
        sharding_changed: file
            .plan
            .sharding_change
            .as_ref()
            .map(|change| change.changed)
            .unwrap_or(false),
        sharding_existed,
    };
    let run_items = file
        .plan
        .items
        .iter()
        .map(|item| RunItem {
            source_path: item.source_path.clone(),
            source_file_name: item.source_file_name.clone(),
            source_db: item.source_db.clone(),
            sequence: item.sequence,
            state: "pending".into(),
            backup_path: Some(item.backup_path.clone()),
            archive_paths: item
                .targets
                .iter()
                .map(|target| target.path.clone())
                .collect(),
            error: None,
        })
        .collect::<Vec<_>>();
    let steps = [
        "prepare",
        "archive_files",
        "backup",
        "sharding",
        "delete_sources",
    ]
    .into_iter()
    .map(|step| {
        let skipped = step == "sharding" && !counts.sharding_changed;
        RunStep {
            step: step.into(),
            state: if step == "prepare" {
                "done"
            } else if skipped {
                "skipped"
            } else {
                "pending"
            }
            .into(),
            detail: skipped.then(|| "无分表模板变更".to_string()),
            updated_at: now_rfc3339(),
        }
    })
    .collect::<Vec<_>>();
    storage.create_run(
        &run_id,
        &crate::storage::root_id(&file.plan.root_path),
        &file.plan.root_path,
        &file.plan.product,
        &backup_date,
        &counts,
        &run_items,
        &steps,
    )?;

    let result = execute(&file, &run_id, storage, sharding_backup_path.as_deref());
    match result {
        Ok(()) => {
            let _ = mark_plan_consumed(data_dir, &file.plan.id);
            storage.update_run_state(&run_id, "committed", None)?;
            storage.update_step(&run_id, "finish", "done", None)?;
            storage.get_run(&run_id)
        }
        Err(error) => {
            let _ = mark_plan_consumed(data_dir, &file.plan.id);
            let failures = rollback_run(storage, &run_id, true);
            let mut message = error.message.clone();
            if !failures.is_empty() {
                message.push_str(&format!("；自动回滚未完全成功：{}", failures.join("；")));
                storage.update_run_state(&run_id, "needs_attention", Some(&message))?;
                storage.update_step(&run_id, "rollback", "failed", Some(&message))?;
            } else {
                message.push_str("；已自动回滚本次改动。");
                storage.update_run_state(&run_id, "rolled_back", Some(&message))?;
                storage.update_step(&run_id, "rollback", "done", None)?;
            }
            storage.get_run(&run_id)
        }
    }
}

fn execute(
    file: &crate::models::PlanFile,
    run_id: &str,
    storage: &Storage,
    sharding_backup_path: Option<&str>,
) -> AppResult<()> {
    storage.update_step(run_id, "archive_files", "running", None)?;
    for (index, item) in file.plan.items.iter().enumerate() {
        for target in &item.targets {
            let staged = target
                .staged_path
                .as_deref()
                .ok_or_else(|| AppError::user("PLAN_INVALID", "归档计划缺少预览内容。"))?;
            let bytes =
                fs::read(staged).map_err(|error| AppError::io("读取预览内容失败", error))?;
            if sha256_bytes(&bytes) != target.sha256 {
                return Err(AppError::user("PLAN_STALE", "预览内容与计划不一致。"));
            }
            let path = Path::new(&target.path);
            if path.exists() {
                return Err(AppError::user(
                    "TARGET_EXISTS",
                    format!("归档目标已存在：{}", target.path),
                ));
            }
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| AppError::io("创建归档目录失败", error))?;
            }
            let mut handle = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|error| {
                    AppError::io(format!("创建归档文件失败：{}", target.path), error)
                })?;
            handle
                .write_all(&bytes)
                .map_err(|error| AppError::io("写入归档文件失败", error))?;
            handle
                .sync_all()
                .map_err(|error| AppError::io("刷新归档文件失败", error))?;
            let written = sha256_file(path)?;
            if written != target.sha256 {
                return Err(AppError::user(
                    "WRITE_VERIFY_FAILED",
                    "归档文件写入后校验不一致。",
                ));
            }
        }
        storage.update_run_item(run_id, index, "archived", None)?;
    }
    storage.update_step(run_id, "archive_files", "done", None)?;
    failpoint("after_archive_write")?;

    storage.update_step(run_id, "backup", "running", None)?;
    for (index, item) in file.plan.items.iter().enumerate() {
        let source = Path::new(&item.source_path);
        let bytes = fs::read(source).map_err(|error| AppError::io("读取源脚本失败", error))?;
        let expected = file
            .source_hashes
            .get(&item.source_file_name)
            .cloned()
            .unwrap_or_default();
        if !expected.is_empty() && sha256_bytes(&bytes) != expected {
            return Err(AppError::user(
                "SOURCE_CHANGED",
                "源脚本内容在预览后发生变化。",
            ));
        }
        let backup = Path::new(&item.backup_path);
        if backup.exists() {
            return Err(AppError::user(
                "BACKUP_EXISTS",
                format!("备份文件已存在：{}", item.backup_path),
            ));
        }
        if let Some(parent) = backup.parent() {
            fs::create_dir_all(parent).map_err(|error| AppError::io("创建备份目录失败", error))?;
        }
        let mut handle = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(backup)
            .map_err(|error| AppError::io("创建备份文件失败", error))?;
        handle
            .write_all(&bytes)
            .map_err(|error| AppError::io("写入备份文件失败", error))?;
        handle
            .sync_all()
            .map_err(|error| AppError::io("刷新备份文件失败", error))?;
        if sha256_file(backup)? != sha256_bytes(&bytes) {
            return Err(AppError::user(
                "BACKUP_VERIFY_FAILED",
                "备份文件写入后校验不一致。",
            ));
        }
        storage.update_run_item(run_id, index, "backed_up", None)?;
    }
    storage.update_step(run_id, "backup", "done", None)?;
    failpoint("after_backup_write")?;

    if let Some(change) = &file.plan.sharding_change {
        if change.changed {
            storage.update_step(run_id, "sharding", "running", None)?;
            let target = Path::new(&change.path);
            if target.is_file() {
                if let Some(backup) = sharding_backup_path {
                    let backup_path = Path::new(backup);
                    if let Some(parent) = backup_path.parent() {
                        fs::create_dir_all(parent)
                            .map_err(|error| AppError::io("创建分表备份目录失败", error))?;
                    }
                    fs::copy(target, backup_path)
                        .map_err(|error| AppError::io("备份分表模板失败", error))?;
                }
            }
            let staged = file
                .sharding_staged_path
                .as_deref()
                .ok_or_else(|| AppError::user("PLAN_INVALID", "缺少分表模板预览内容。"))?;
            let bytes =
                fs::read(staged).map_err(|error| AppError::io("读取分表模板失败", error))?;
            if file.sharding_after_sha.as_deref() != Some(sha256_bytes(&bytes).as_str()) {
                return Err(AppError::user(
                    "PLAN_STALE",
                    "分表模板预览内容与计划不一致，请重新预览。",
                ));
            }
            atomic_write(target, &bytes)?;
            storage.update_step(run_id, "sharding", "done", None)?;
            failpoint("after_sharding_write")?;
        }
    }

    storage.update_step(run_id, "delete_sources", "running", None)?;
    for (index, item) in file.plan.items.iter().enumerate() {
        fs::remove_file(&item.source_path).map_err(|error| {
            AppError::io(format!("移除源脚本失败：{}", item.source_path), error)
        })?;
        storage.update_run_item(run_id, index, "done", None)?;
    }
    storage.update_step(run_id, "delete_sources", "done", None)?;
    failpoint("after_source_delete")?;
    Ok(())
}

fn failpoint(name: &str) -> AppResult<()> {
    if std::env::var("SQL_ARCHIVE_FAILPOINT").ok().as_deref() == Some(name) {
        return Err(AppError::user(
            "FAILPOINT",
            format!("已触发失败注入：{name}"),
        ));
    }
    Ok(())
}

/// 回滚一次归档：删除本次创建的归档文件、恢复源文件、恢复分表模板。
/// 返回未能恢复的项。
pub fn rollback_run(storage: &Storage, run_id: &str, include_sharding: bool) -> Vec<String> {
    let Ok(run) = storage.get_run(run_id) else {
        return vec!["无法读取归档记录。".into()];
    };
    let mut failures = Vec::new();
    let mut directories = BTreeSet::new();
    for (index, item) in run.items.iter().enumerate() {
        for path in &item.archive_paths {
            let path = Path::new(path);
            if path.exists() {
                if let Err(error) = fs::remove_file(path) {
                    failures.push(format!("删除归档文件失败 {}：{error}", path.display()));
                }
            }
            if let Some(parent) = path.parent() {
                directories.insert(parent.to_path_buf());
            }
        }
        let source = Path::new(&item.source_path);
        let backup = item.backup_path.as_deref().map(Path::new);
        if !source.exists() {
            if let Some(backup) = backup.filter(|path| path.is_file()) {
                if let Some(parent) = source.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                match fs::copy(backup, source) {
                    Ok(_) => {
                        let _ = fs::remove_file(backup);
                    }
                    Err(error) => {
                        failures.push(format!("恢复源文件失败 {}：{error}", source.display()))
                    }
                }
            } else {
                failures.push(format!("源文件缺失且备份不存在：{}", source.display()));
            }
        } else if let Some(backup) = backup.filter(|path| path.is_file()) {
            let _ = fs::remove_file(backup);
        }
        let _ = storage.update_run_item(run_id, index, "rolled_back", None);
    }
    if include_sharding {
        if let Some(summary) = run.summary.as_deref() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(summary) {
                let changed = value
                    .get("shardingChanged")
                    .and_then(|item| item.as_bool())
                    .unwrap_or(false);
                if changed {
                    if let Some(path) = value.get("shardingPath").and_then(|item| item.as_str()) {
                        let target = Path::new(path);
                        let backup = value
                            .get("shardingBackupPath")
                            .and_then(|item| item.as_str())
                            .map(Path::new);
                        let existed = value
                            .get("shardingExisted")
                            .and_then(|item| item.as_bool())
                            .unwrap_or(false);
                        if let Some(backup) = backup.filter(|path| path.is_file()) {
                            match fs::copy(backup, target) {
                                Ok(_) => {}
                                Err(error) => failures.push(format!("恢复分表模板失败：{error}")),
                            }
                        } else if !existed && target.is_file() {
                            if let Err(error) = fs::remove_file(target) {
                                failures.push(format!("移除新建分表模板失败：{error}"));
                            }
                        }
                    }
                }
            }
        }
    }
    for dir in directories.iter().rev() {
        let _ = fs::remove_dir(dir);
    }
    failures
}

/// 启动时恢复上次中断的归档：尽力回滚并返回面向用户的告警。
pub fn recover_interrupted(storage: &Storage) -> Vec<String> {
    let Ok(runs) = storage.running_runs() else {
        return vec!["无法读取上次中断的归档记录。".into()];
    };
    let mut warnings = Vec::new();
    for run in runs {
        let failures = rollback_run(storage, &run.id, true);
        let state = if failures.is_empty() {
            "rolled_back"
        } else {
            "needs_attention"
        };
        let message = if failures.is_empty() {
            "检测到上次归档在完成前中断，已自动回滚本次创建的归档文件并恢复源脚本。"
        } else {
            "检测到上次归档在完成前中断，自动回滚未完全成功，请人工核实。"
        };
        let _ = storage.update_run_state(&run.id, state, Some(message));
        let _ = storage.update_step(
            &run.id,
            "recover",
            state,
            failures.first().map(String::as_str),
        );
        warnings.push(format!(
            "上次归档（{} / {}）在完成前中断：{}",
            run.product,
            run.id,
            if failures.is_empty() {
                "已自动回滚"
            } else {
                "需要人工核实"
            }
        ));
        if !failures.is_empty() {
            warnings.push(format!("未恢复项：{}", failures.join("；")));
        }
    }
    warnings
}
