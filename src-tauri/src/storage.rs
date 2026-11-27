use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{AppError, AppResult};
use crate::models::{
    ArchiveRun, ArchiveSettings, HistorySummary, RunItem, RunStep, ValidationResult,
};
use crate::util::{now_rfc3339, sha256_bytes};

const DATABASE_NAME: &str = "sql-archive.sqlite3";
const SCHEMA: &str = include_str!("schema.sql");

pub struct Storage {
    pub(crate) connection: Mutex<Connection>,
}

#[derive(Debug, Clone, Default)]
pub struct RunCounts {
    pub versions: Vec<String>,
    pub source_count: i64,
    pub target_count: i64,
    pub sharding_path: Option<String>,
    pub sharding_backup_path: Option<String>,
    pub sharding_before_sha: Option<String>,
    pub sharding_changed: bool,
    pub sharding_existed: bool,
}

impl Storage {
    pub fn open(data_dir: &Path) -> AppResult<Self> {
        fs::create_dir_all(data_dir)
            .map_err(|error| AppError::io("创建应用数据目录失败", error))?;
        let connection = Connection::open(data_dir.join(DATABASE_NAME))
            .map_err(|error| AppError::io("打开应用数据库失败", error))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| AppError::io("配置数据库等待失败", error))?;
        connection
            .execute_batch("PRAGMA journal_mode = WAL;")
            .map_err(|error| AppError::io("初始化数据库日志模式失败", error))?;
        connection
            .execute_batch(SCHEMA)
            .map_err(|error| AppError::io("初始化数据库结构失败", error))?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn get_settings(&self) -> ArchiveSettings {
        let connection = self.connection.lock();
        let value: Option<String> = connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'archive_settings'",
                [],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten();
        value
            .and_then(|value| serde_json::from_str::<ArchiveSettings>(&value).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &ArchiveSettings) -> AppResult<()> {
        let value = serde_json::to_string(settings)
            .map_err(|error| AppError::internal("SETTINGS_SERIALIZE", error.to_string()))?;
        let connection = self.connection.lock();
        connection
            .execute(
                "INSERT INTO settings(key, value) VALUES('archive_settings', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![value],
            )
            .map_err(|error| AppError::io("保存设置失败", error))?;
        Ok(())
    }

    pub fn marks_for_root(&self, root_id: &str) -> AppResult<HashMap<String, (String, String)>> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare(
                "SELECT relative_path, file_sha256, status FROM script_marks WHERE root_id = ?1",
            )
            .map_err(|error| AppError::io("读取脚本标记失败", error))?;
        let rows = statement
            .query_map(params![root_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    (row.get::<_, String>(1)?, row.get::<_, String>(2)?),
                ))
            })
            .map_err(|error| AppError::io("读取脚本标记失败", error))?;
        let mut marks = HashMap::new();
        for row in rows.flatten() {
            marks.insert(row.0, row.1);
        }
        Ok(marks)
    }

    pub fn set_mark(
        &self,
        root_id: &str,
        relative_path: &str,
        sha256: &str,
        status: &str,
    ) -> AppResult<()> {
        if !matches!(status, "pending" | "passed" | "returned") {
            return Err(AppError::user("MARK_INVALID", "未知的脚本标记状态。"));
        }
        let connection = self.connection.lock();
        connection
            .execute(
                "INSERT INTO script_marks(root_id, relative_path, file_sha256, status, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(root_id, relative_path) DO UPDATE SET
                   file_sha256 = excluded.file_sha256,
                   status = excluded.status,
                   updated_at = excluded.updated_at",
                params![root_id, relative_path, sha256, status, now_rfc3339()],
            )
            .map_err(|error| AppError::io("保存脚本标记失败", error))?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_validation(
        &self,
        root_id: &str,
        file_sha256: &str,
        source_db: &str,
        engine_fingerprint: &str,
        check_rule_sha: &str,
        sharding_sha: &str,
    ) -> AppResult<Option<ValidationResult>> {
        let connection = self.connection.lock();
        let row: Option<(String, String, String, i64, String)> = connection
            .query_row(
                "SELECT status, errors_json, warnings_json, tenant_code, checked_at
                 FROM validation_cache
                 WHERE root_id = ?1 AND file_sha256 = ?2 AND source_db = ?3
                   AND engine_fingerprint = ?4 AND check_rule_sha256 = ?5 AND sharding_sha256 = ?6",
                params![
                    root_id,
                    file_sha256,
                    source_db,
                    engine_fingerprint,
                    check_rule_sha,
                    sharding_sha
                ],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| AppError::io("读取校验缓存失败", error))?;
        Ok(row.map(
            |(status, errors, warnings, tenant, checked_at)| ValidationResult {
                status,
                errors: serde_json::from_str(&errors).unwrap_or_default(),
                warnings: serde_json::from_str(&warnings).unwrap_or_default(),
                tenant_code_warning: tenant != 0,
                checked_at,
            },
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn put_validation(
        &self,
        root_id: &str,
        file_sha256: &str,
        source_db: &str,
        engine_fingerprint: &str,
        check_rule_sha: &str,
        sharding_sha: &str,
        result: &ValidationResult,
    ) -> AppResult<()> {
        let connection = self.connection.lock();
        connection
            .execute(
                "INSERT INTO validation_cache(
                   root_id, file_sha256, source_db, engine_fingerprint,
                   check_rule_sha256, sharding_sha256, status, errors_json,
                   warnings_json, tenant_code, checked_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(root_id, file_sha256, source_db, engine_fingerprint, check_rule_sha256, sharding_sha256)
                 DO UPDATE SET status = excluded.status, errors_json = excluded.errors_json,
                   warnings_json = excluded.warnings_json, tenant_code = excluded.tenant_code,
                   checked_at = excluded.checked_at",
                params![
                    root_id,
                    file_sha256,
                    source_db,
                    engine_fingerprint,
                    check_rule_sha,
                    sharding_sha,
                    result.status,
                    serde_json::to_string(&result.errors).unwrap_or_else(|_| "[]".into()),
                    serde_json::to_string(&result.warnings).unwrap_or_else(|_| "[]".into()),
                    if result.tenant_code_warning { 1 } else { 0 },
                    result.checked_at
                ],
            )
            .map_err(|error| AppError::io("写入校验缓存失败", error))?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_run(
        &self,
        run_id: &str,
        root_id: &str,
        root_path: &str,
        product: &str,
        backup_date: &str,
        counts: &RunCounts,
        items: &[RunItem],
        steps: &[RunStep],
    ) -> AppResult<()> {
        let connection = self.connection.lock();
        let summary = serde_json::json!({
            "versions": counts.versions,
            "sourceCount": counts.source_count,
            "targetCount": counts.target_count,
            "shardingPath": counts.sharding_path,
            "shardingBackupPath": counts.sharding_backup_path,
            "shardingBeforeSha": counts.sharding_before_sha,
            "shardingChanged": counts.sharding_changed,
            "shardingExisted": counts.sharding_existed,
        });
        connection
            .execute(
                "INSERT INTO archive_runs(
                   id, root_id, root_path, product, state, backup_date, summary_json, started_at
                 ) VALUES(?1, ?2, ?3, ?4, 'running', ?5, ?6, ?7)",
                params![
                    run_id,
                    root_id,
                    root_path,
                    product,
                    backup_date,
                    summary.to_string(),
                    now_rfc3339()
                ],
            )
            .map_err(|error| AppError::io("创建归档记录失败", error))?;
        for (index, item) in items.iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO archive_run_items(
                       run_id, group_key, sort_order, source_path, source_file_name,
                       source_sha256, source_db, sequence, backup_path, archive_paths_json, state
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending')",
                    params![
                        run_id,
                        format!("{index}"),
                        index as i64,
                        item.source_path,
                        item.source_file_name,
                        crate::util::sha256_file(Path::new(&item.source_path))
                            .unwrap_or_else(|_| sha256_bytes(item.source_path.as_bytes())),
                        item.source_db,
                        item.sequence as i64,
                        item.backup_path,
                        serde_json::to_string(&item.archive_paths).unwrap_or_else(|_| "[]".into()),
                    ],
                )
                .map_err(|error| AppError::io("写入归档明细失败", error))?;
        }
        for step in steps {
            connection
                .execute(
                    "INSERT INTO archive_run_steps(run_id, step, state, updated_at)
                     VALUES(?1, ?2, ?3, ?4)",
                    params![run_id, step.step, step.state, now_rfc3339()],
                )
                .map_err(|error| AppError::io("写入归档步骤失败", error))?;
        }
        Ok(())
    }

    pub fn update_run_item(
        &self,
        run_id: &str,
        index: usize,
        state: &str,
        error: Option<&str>,
    ) -> AppResult<()> {
        let connection = self.connection.lock();
        connection
            .execute(
                "UPDATE archive_run_items SET state = ?1, error = ?2 WHERE run_id = ?3 AND sort_order = ?4",
                params![state, error, run_id, index as i64],
            )
            .map_err(|error| AppError::io("更新归档明细失败", error))?;
        Ok(())
    }

    pub fn update_run_state(
        &self,
        run_id: &str,
        state: &str,
        error: Option<&str>,
    ) -> AppResult<()> {
        let connection = self.connection.lock();
        let finished = matches!(state, "committed" | "failed" | "rolled_back").then(now_rfc3339);
        connection
            .execute(
                "UPDATE archive_runs SET state = ?1, error = ?2, finished_at = COALESCE(?3, finished_at)
                 WHERE id = ?4",
                params![state, error, finished, run_id],
            )
            .map_err(|error| AppError::io("更新归档状态失败", error))?;
        Ok(())
    }

    pub fn update_step(
        &self,
        run_id: &str,
        step: &str,
        state: &str,
        detail: Option<&str>,
    ) -> AppResult<()> {
        let connection = self.connection.lock();
        connection
            .execute(
                "INSERT INTO archive_run_steps(run_id, step, state, detail_json, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(run_id, step) DO UPDATE SET
                   state = excluded.state, detail_json = excluded.detail_json,
                   updated_at = excluded.updated_at",
                params![run_id, step, state, detail, now_rfc3339()],
            )
            .map_err(|error| AppError::io("更新归档步骤失败", error))?;
        Ok(())
    }

    pub fn get_run(&self, run_id: &str) -> AppResult<ArchiveRun> {
        let connection = self.connection.lock();
        let run = connection
            .query_row(
                "SELECT id, product, state, backup_date, summary_json, error, started_at, finished_at
                 FROM archive_runs WHERE id = ?1",
                params![run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<String>>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| AppError::io("读取归档记录失败", error))?
            .ok_or_else(|| AppError::user("RUN_NOT_FOUND", "归档记录不存在。"))?;

        let mut statement = connection
            .prepare(
                "SELECT source_path, source_file_name, source_db, sequence, state,
                        backup_path, archive_paths_json, error
                 FROM archive_run_items WHERE run_id = ?1 ORDER BY sort_order",
            )
            .map_err(|error| AppError::io("读取归档明细失败", error))?;
        let items = statement
            .query_map(params![run_id], |row| {
                let archive_paths: String = row.get(6)?;
                Ok(RunItem {
                    source_path: row.get(0)?,
                    source_file_name: row.get(1)?,
                    source_db: row.get(2)?,
                    sequence: row.get::<_, i64>(3)? as u32,
                    state: row.get(4)?,
                    backup_path: row.get(5)?,
                    archive_paths: serde_json::from_str(&archive_paths).unwrap_or_default(),
                    error: row.get(7)?,
                })
            })
            .map_err(|error| AppError::io("读取归档明细失败", error))?
            .flatten()
            .collect();

        let mut step_statement = connection
            .prepare(
                "SELECT step, state, detail_json, updated_at
                 FROM archive_run_steps WHERE run_id = ?1 ORDER BY updated_at",
            )
            .map_err(|error| AppError::io("读取归档步骤失败", error))?;
        let steps = step_statement
            .query_map(params![run_id], |row| {
                Ok(RunStep {
                    step: row.get(0)?,
                    state: row.get(1)?,
                    detail: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })
            .map_err(|error| AppError::io("读取归档步骤失败", error))?
            .flatten()
            .collect();

        Ok(ArchiveRun {
            id: run.0,
            product: run.1,
            state: run.2,
            backup_date: run.3,
            summary: run.4,
            error: run.5,
            started_at: run.6,
            finished_at: run.7,
            items,
            steps,
        })
    }

    pub fn list_runs(&self, root_id: &str, limit: usize) -> AppResult<Vec<HistorySummary>> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare(
                "SELECT id, product, state, backup_date, summary_json, error, started_at, finished_at
                 FROM archive_runs WHERE root_id = ?1 ORDER BY started_at DESC LIMIT ?2",
            )
            .map_err(|error| AppError::io("读取历史记录失败", error))?;
        let rows = statement
            .query_map(params![root_id, limit as i64], |row| {
                let summary: Option<String> = row.get(4)?;
                let summary: serde_json::Value = summary
                    .and_then(|value| serde_json::from_str(&value).ok())
                    .unwrap_or(serde_json::Value::Null);
                let versions = summary
                    .get("versions")
                    .and_then(|value| value.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                Ok(HistorySummary {
                    id: row.get(0)?,
                    product: row.get(1)?,
                    state: row.get(2)?,
                    backup_date: row.get(3)?,
                    versions,
                    source_count: summary
                        .get("sourceCount")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    target_count: summary
                        .get("targetCount")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    error: row.get(5)?,
                    started_at: row.get(6)?,
                    finished_at: row.get(7)?,
                })
            })
            .map_err(|error| AppError::io("读取历史记录失败", error))?;
        Ok(rows.flatten().collect())
    }

    pub fn running_runs(&self) -> AppResult<Vec<ArchiveRun>> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare("SELECT id FROM archive_runs WHERE state = 'running' ORDER BY started_at")
            .map_err(|error| AppError::io("读取中断记录失败", error))?;
        let ids: Vec<String> = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| AppError::io("读取中断记录失败", error))?
            .flatten()
            .collect();
        drop(statement);
        drop(connection);
        ids.into_iter().map(|id| self.get_run(&id)).collect()
    }
}

pub fn root_id(root_path: &str) -> String {
    let digest = sha256_bytes(root_path.as_bytes());
    digest[..16].to_string()
}
