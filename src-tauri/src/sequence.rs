//! 归档序号扫描。
//!
//! 序号不是按单个目标库计算，而是取 `<version>/<module>/<general>/` 下
//! 所有数据库子目录的全局最大序号；同一变更的 ORACLE/TDSQL 源共享一个序号。

use std::fs;
use std::path::Path;

use crate::error::AppResult;
use crate::filename::parse_archive_name;

/// 返回 (database dir, 该目录内最大序号)，按目录名排序。
pub fn scan_db_dirs(base_dir: &Path) -> AppResult<Vec<(String, u32)>> {
    let mut result = Vec::new();
    let entries = match fs::read_dir(base_dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(result),
    };
    for entry in entries.flatten() {
        let dir_path = entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let mut max_sequence = 0_u32;
        if let Ok(files) = fs::read_dir(&dir_path) {
            for file in files.flatten() {
                let path = file.path();
                if !path.is_file() {
                    continue;
                }
                let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                    continue;
                };
                if let Some(parsed) = parse_archive_name(file_name) {
                    max_sequence = max_sequence.max(parsed.sequence);
                }
            }
        }
        result.push((name, max_sequence));
    }
    result.sort_by(|left, right| left.0.to_lowercase().cmp(&right.0.to_lowercase()));
    Ok(result)
}

/// 所有数据库目录中的全局最大序号；目录不存在时返回 0。
pub fn latest_sequence_from_dirs(dirs: &[(String, u32)]) -> u32 {
    dirs.iter()
        .map(|(_, sequence)| *sequence)
        .max()
        .unwrap_or(0)
}

pub fn latest_sequence(base_dir: &Path) -> AppResult<u32> {
    Ok(latest_sequence_from_dirs(&scan_db_dirs(base_dir)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), b"x").unwrap();
    }

    #[test]
    fn scans_max_across_all_db_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join("general");
        touch(
            &base.join("h2"),
            "V4.6.2.0_990000_00_09__insert_A_TDSQL_[4.6.2.0].groovy",
        );
        touch(
            &base.join("mysql"),
            "V4.6.2.0_990000_00_10__insert_B_TDSQL_[4.6.2.0].groovy",
        );
        touch(
            &base.join("oracle"),
            "V4.6.2.0_990000_00_12__insert_C_ORACLE_[4.6.2.0].groovy",
        );
        // 非归档文件不计入
        touch(&base.join("mysql"), "readme.txt");
        let dirs = scan_db_dirs(&base).unwrap();
        assert_eq!(dirs.len(), 3);
        assert_eq!(latest_sequence_from_dirs(&dirs), 12);
    }

    #[test]
    fn unconfigured_dirs_still_count() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join("general");
        touch(
            &base.join("oceanbase_d"),
            "V1.14.0_990000_00_137__init_X_ORACLE_[1.14.0].groovy",
        );
        assert_eq!(latest_sequence(&base).unwrap(), 137);
    }

    #[test]
    fn missing_base_is_zero() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(latest_sequence(&temp.path().join("none")).unwrap(), 0);
    }
}
