use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_module_child() -> String {
    "990000_product".to_string()
}

fn default_general_child() -> String {
    "general".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveSettings {
    pub workspace_root: Option<String>,
    pub product: Option<String>,
    pub java_path: Option<String>,
    pub engine_dir: Option<String>,
    pub check_rule_path: Option<String>,
    pub sharding_path: Option<String>,
    #[serde(default = "default_module_child")]
    pub module_child: String,
    #[serde(default = "default_general_child")]
    pub general_child: String,
    #[serde(default = "default_true")]
    pub confirm_tenant_warning: bool,
}

impl Default for ArchiveSettings {
    fn default() -> Self {
        Self {
            workspace_root: None,
            product: None,
            java_path: None,
            engine_dir: None,
            check_rule_path: None,
            sharding_path: None,
            module_child: default_module_child(),
            general_child: default_general_child(),
            confirm_tenant_warning: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub portable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaStatus {
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineJar {
    pub name: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub available: bool,
    pub jar_dir: Option<String>,
    pub jars: Vec<EngineJar>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub severity: String,
    pub code: String,
    pub message: String,
}

impl Issue {
    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: "error".into(),
            code: code.into(),
            message: message.into(),
        }
    }
    pub fn warning(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: "warning".into(),
            code: code.into(),
            message: message.into(),
        }
    }
    pub fn info(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: "info".into(),
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductOption {
    pub display_name: String,
    pub dir_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootProfile {
    pub root_path: String,
    pub platform_kind: String,
    pub flyway_root: Option<String>,
    pub temp_root: String,
    pub backup_root: String,
    pub config_dir: Option<String>,
    pub engine_dir: Option<String>,
    pub check_rule_path: Option<String>,
    pub sharding_path: Option<String>,
    pub products: Vec<ProductOption>,
    pub tdsql_targets: Vec<String>,
    pub oracle_targets: Vec<String>,
    pub baseline_by_product: BTreeMap<String, String>,
    pub default_product_dir: Option<String>,
    pub java: JavaStatus,
    pub engine: EngineStatus,
    pub issues: Vec<Issue>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub version: String,
    pub latest_sequence: u32,
    pub db_dirs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ParsedScriptName {
    pub date: String,
    pub person: String,
    pub source_seq: String,
    pub description: String,
    pub db_type: String,
    pub version: String,
    pub extension: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub status: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub tenant_code_warning: bool,
    pub checked_at: String,
}

impl ValidationResult {
    pub fn empty_ok() -> Self {
        Self {
            status: "ok".into(),
            errors: Vec::new(),
            warnings: Vec::new(),
            tenant_code_warning: false,
            checked_at: chrono::Utc::now().to_rfc3339(),
        }
    }
    pub fn finalize(mut self) -> Self {
        self.status = if !self.errors.is_empty() {
            "error".into()
        } else if !self.warnings.is_empty() || self.tenant_code_warning {
            "warning".into()
        } else {
            "ok".into()
        };
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptItem {
    /// 相对于产品提交目录（例如 oracle/xxx.groovy）
    pub relative_path: String,
    /// 相对于当前仓库根目录（例如 temp/11_newreimburse/oracle/xxx.groovy）
    pub repo_relative_path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub modified_at: String,
    pub sha256: String,
    pub parsed: Option<ParsedScriptName>,
    pub parse_error: Option<String>,
    pub mark: String,
    pub validation: Option<ValidationResult>,
    pub group_key: Option<String>,
    pub pair_state: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptGroup {
    pub key: String,
    pub date: String,
    pub person: String,
    pub source_seq: String,
    pub description: String,
    pub version: String,
    pub db_types: Vec<String>,
    pub scripts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TempScanResult {
    pub product: String,
    pub temp_dir: String,
    pub scripts: Vec<ScriptItem>,
    pub groups: Vec<ScriptGroup>,
    pub issues: Vec<String>,
    pub baseline_version: Option<String>,
    pub versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetFile {
    pub db_dir: String,
    pub path: String,
    pub file_name: String,
    pub channel: String,
    pub byte_size: u64,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staged_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub group_key: String,
    pub source_path: String,
    #[serde(default)]
    pub source_repo_relative_path: String,
    pub source_file_name: String,
    pub source_db: String,
    pub source_version: String,
    pub sequence: u32,
    pub backup_path: String,
    pub targets: Vec<TargetFile>,
    pub tenant_code_warning: bool,
    pub state: String,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShardingChange {
    pub path: String,
    pub before_sha: Option<String>,
    pub after_sha: Option<String>,
    pub changed: bool,
    pub affected_tables: Vec<String>,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanTotals {
    pub source_count: usize,
    pub target_count: usize,
    pub backup_count: usize,
    pub versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePlan {
    pub id: String,
    pub root_path: String,
    pub product: String,
    pub created_at: String,
    pub expires_at: String,
    pub items: Vec<PlanItem>,
    pub conflicts: Vec<String>,
    pub warnings: Vec<String>,
    pub sharding_change: Option<ShardingChange>,
    pub digest: String,
    pub totals: PlanTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanFile {
    pub plan: ArchivePlan,
    pub staging_dir: String,
    pub flyway_root: String,
    pub module_child: String,
    pub general_child: String,
    pub engine_fingerprint: String,
    pub check_rule_sha256: String,
    pub sharding_before_sha: Option<String>,
    pub sharding_changed: bool,
    pub sharding_staged_path: Option<String>,
    pub sharding_after_sha: Option<String>,
    pub sequence_base: BTreeMap<String, u32>,
    pub source_hashes: BTreeMap<String, String>,
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunItem {
    pub source_path: String,
    pub source_file_name: String,
    pub source_db: String,
    pub sequence: u32,
    pub state: String,
    pub backup_path: Option<String>,
    pub archive_paths: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStep {
    pub step: String,
    pub state: String,
    pub detail: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveRun {
    pub id: String,
    pub product: String,
    pub state: String,
    pub backup_date: String,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub items: Vec<RunItem>,
    pub steps: Vec<RunStep>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySummary {
    pub id: String,
    pub product: String,
    pub state: String,
    pub backup_date: String,
    pub versions: String,
    pub source_count: i64,
    pub target_count: i64,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptContent {
    pub content: String,
    pub encoding: String,
    pub has_bom: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewArchiveInput {
    pub root_path: String,
    pub product: String,
    pub relative_paths: Vec<String>,
    pub module_child: Option<String>,
    pub general_child: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmArchiveInput {
    pub plan_id: String,
    pub confirmed: bool,
    pub tenant_ack_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateInput {
    pub root_path: String,
    pub product: String,
    pub relative_paths: Vec<String>,
}
