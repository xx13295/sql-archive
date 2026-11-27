use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppResult;
use crate::filename::{group_key, parse_source_name};
use crate::models::{RootProfile, ScriptGroup, ScriptItem, TempScanResult};
use crate::storage::{root_id, Storage};
use crate::util::sha256_bytes;
use crate::workspace;

const MAX_SCAN_DEPTH: usize = 3;

pub fn scan_product(
    profile: &RootProfile,
    storage: &Storage,
    product: &str,
) -> AppResult<TempScanResult> {
    let root = Path::new(&profile.root_path);
    let temp_dir = workspace::temp_product_dir(root, product);
    let marks = storage.marks_for_root(&root_id(&profile.root_path))?;
    let mut scripts = Vec::new();
    let mut issues = Vec::new();
    // key -> (分组元信息, [(相对路径, 文件名)])
    let mut groups: BTreeMap<String, (ParsedGroup, Vec<(String, String)>)> = BTreeMap::new();

    if temp_dir.is_dir() {
        let mut paths = Vec::new();
        collect_script_paths(&temp_dir, &mut paths, 0);
        paths.sort();
        for path in paths {
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let relative_path = path
                .strip_prefix(&temp_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    issues.push(format!("{relative_path} 无法读取：{error}"));
                    continue;
                }
            };
            let sha256 = sha256_bytes(&bytes);
            let metadata = fs::metadata(&path).ok();
            let size_bytes = metadata.as_ref().map(|value| value.len()).unwrap_or(0);
            let modified_at = metadata
                .as_ref()
                .and_then(|value| value.modified().ok())
                .map(|value| chrono::DateTime::<chrono::Local>::from(value).to_rfc3339())
                .unwrap_or_default();
            let (parsed, parse_error) = match parse_source_name(file_name) {
                Ok(parsed) => (Some(parsed), None),
                Err(message) => {
                    issues.push(format!("{relative_path}：{message}"));
                    (None, Some(message))
                }
            };
            let mark = marks
                .get(&relative_path)
                .filter(|(stored_sha, _)| stored_sha == &sha256)
                .map(|(_, status)| status.clone())
                .unwrap_or_else(|| "pending".to_string());
            let key = parsed.as_ref().map(group_key);
            if let (Some(parsed), Some(key)) = (parsed.as_ref(), key.as_ref()) {
                let entry = groups.entry(key.clone()).or_insert_with(|| {
                    (
                        ParsedGroup {
                            date: parsed.date.clone(),
                            person: parsed.person.clone(),
                            source_seq: parsed.source_seq.clone(),
                            description: parsed.description.clone(),
                            version: parsed.version.clone(),
                        },
                        Vec::new(),
                    )
                });
                entry.1.push((relative_path.clone(), file_name.to_string()));
            }
            let repo_relative_path = format!("temp/{product}/{relative_path}");
            scripts.push(ScriptItem {
                relative_path,
                repo_relative_path,
                file_name: file_name.to_string(),
                size_bytes,
                modified_at,
                sha256,
                parsed,
                parse_error,
                mark,
                validation: None,
                group_key: key,
                pair_state: "single".into(),
            });
        }
    } else {
        issues.push(format!("产品目录 temp/{product} 不存在。"));
    }

    scripts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let industry = !profile.oracle_targets.is_empty();
    let mut group_models = Vec::new();
    for (key, (parsed, files)) in &groups {
        let mut db_types = BTreeSet::new();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for (_relative_path, file_name) in files {
            if let Ok(source) = parse_source_name(file_name) {
                db_types.insert(source.db_type.clone());
                *counts.entry(source.db_type).or_default() += 1;
            }
        }
        for (db, count) in &counts {
            if *count > 1 {
                issues.push(format!(
                    "同一变更组 {key} 中 {db} 脚本出现 {count} 次，顺序号必须唯一。"
                ));
            }
        }
        if industry && db_types.len() < 2 {
            issues.push(format!("行业应用的变更组 {key} 缺少 ORACLE / TDSQL 配对。"));
        }
        group_models.push(ScriptGroup {
            key: key.clone(),
            date: parsed.date.clone(),
            person: parsed.person.clone(),
            source_seq: parsed.source_seq.clone(),
            description: parsed.description.clone(),
            version: parsed.version.clone(),
            db_types: db_types.into_iter().collect(),
            scripts: files
                .iter()
                .map(|(relative_path, _)| relative_path.clone())
                .collect(),
        });
    }
    group_models.sort_by(|left, right| left.key.cmp(&right.key));

    for script in &mut scripts {
        let Some(key) = &script.group_key else {
            continue;
        };
        let Some(group) = group_models.iter().find(|group| &group.key == key) else {
            continue;
        };
        let db = script
            .parsed
            .as_ref()
            .map(|parsed| parsed.db_type.as_str())
            .unwrap_or("");
        script.pair_state = if industry {
            if group.db_types.len() >= 2 {
                "paired"
            } else {
                "missing_pair"
            }
        } else if db == "ORACLE" {
            "unexpected_db"
        } else {
            "single"
        }
        .to_string();
    }

    let mut versions: Vec<String> = group_models
        .iter()
        .map(|group| group.version.clone())
        .collect();
    versions.sort_by(|left, right| crate::filename::compare_versions(right, left));
    versions.dedup();
    let baseline_version = workspace::current_version(profile, product);

    Ok(TempScanResult {
        product: product.to_string(),
        temp_dir: temp_dir.to_string_lossy().to_string(),
        scripts,
        groups: group_models,
        issues,
        baseline_version,
        versions,
    })
}

/// temp/<产品>/ 既可能是平铺脚本，也可能是 oracle/tdsql/mysql 子目录。
fn collect_script_paths(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_script_paths(&path, out, depth + 1);
            continue;
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if extension == "groovy" || extension == "sql" {
            out.push(path);
        }
    }
}

struct ParsedGroup {
    date: String,
    person: String,
    source_seq: String,
    description: String,
    version: String,
}
