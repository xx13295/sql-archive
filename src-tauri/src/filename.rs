//! 源脚本文件名与 Flyway 归档文件名的解析/生成。

use std::cmp::Ordering;
use std::sync::OnceLock;

use chrono::NaiveDate;
use regex::Regex;

use crate::models::ParsedScriptName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveName {
    pub version: String,
    pub module_code: String,
    pub category: String,
    pub sequence: u32,
    pub description: String,
    pub db_type: String,
    pub extension: String,
}

fn source_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"^(?P<date>\d{4}-\d{2}-\d{2})_(?P<person>[^_]+)_+(?P<seq>\d{2,3})__(?P<desc>.+)_(?P<db>(?i:ORACLE|TDSQL))_\[(?P<ver>\d+(?:\.\d+)*)\]\.(?P<ext>(?i:groovy|sql))$",
        )
        .expect("source filename regex")
    })
}

fn archive_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"^V(?P<ver>\d+(?:\.\d+)*)_(?P<module>\d{3,6})_(?P<category>\d{2,3})_(?P<seq>\d{1,3})__(?P<desc>.+)_(?P<db>(?i:ORACLE|TDSQL))_\[(?P<ver2>\d+(?:\.\d+)*)\]\.(?P<ext>(?i:groovy|sql))$",
        )
        .expect("archive filename regex")
    })
}

pub fn parse_source_name(file_name: &str) -> Result<ParsedScriptName, String> {
    let captures = source_regex().captures(file_name).ok_or_else(|| {
        "文件名不符合规范 yyyy-MM-dd_姓名_两位顺序号__英文备注_DB_[版本].groovy".to_string()
    })?;
    let date = captures.name("date").expect("date").as_str().to_string();
    NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| format!("文件名中的日期不合法：{date}"))?;
    let version = captures.name("ver").expect("version").as_str().to_string();
    if !is_version(&version) {
        return Err(format!("文件名中的版本号不合法：{version}"));
    }
    let db_type = captures.name("db").expect("db").as_str().to_uppercase();
    let extension = captures.name("ext").expect("ext").as_str().to_lowercase();
    Ok(ParsedScriptName {
        date,
        person: captures
            .name("person")
            .expect("person")
            .as_str()
            .to_string(),
        source_seq: captures.name("seq").expect("seq").as_str().to_string(),
        description: captures.name("desc").expect("desc").as_str().to_string(),
        db_type,
        version,
        extension,
    })
}

pub fn group_key(parsed: &ParsedScriptName) -> String {
    format!(
        "{}_{}_{}__{}_{}",
        parsed.date, parsed.person, parsed.source_seq, parsed.description, parsed.version
    )
}

pub fn module_code(module_child: &str) -> String {
    module_child
        .split('_')
        .next()
        .filter(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        .unwrap_or("990000")
        .to_string()
}

pub fn archive_file_name(
    version: &str,
    module_child: &str,
    category: &str,
    sequence: u32,
    description: &str,
    db_type: &str,
    extension: &str,
) -> String {
    format!(
        "V{version}_{}_{category}_{sequence:02}__{description}_{db}_[{version}].{ext}",
        module_code(module_child),
        db = db_type.to_uppercase(),
        ext = extension.to_lowercase()
    )
}

pub fn parse_archive_name(file_name: &str) -> Option<ArchiveName> {
    let captures = archive_regex().captures(file_name)?;
    let version = captures.name("ver")?.as_str().to_string();
    let version_tail = captures.name("ver2")?.as_str().to_string();
    if version != version_tail {
        return None;
    }
    let sequence = captures.name("seq")?.as_str().parse::<u32>().ok()?;
    Some(ArchiveName {
        version,
        module_code: captures.name("module")?.as_str().to_string(),
        category: captures.name("category")?.as_str().to_string(),
        sequence,
        description: captures.name("desc")?.as_str().to_string(),
        db_type: captures.name("db")?.as_str().to_uppercase(),
        extension: captures.name("ext")?.as_str().to_lowercase(),
    })
}

pub fn is_version(value: &str) -> bool {
    !value.is_empty()
        && value
            .split('.')
            .all(|segment| !segment.is_empty() && segment.chars().all(|ch| ch.is_ascii_digit()))
        && value.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
}

pub fn compare_versions(left: &str, right: &str) -> Ordering {
    let split = |value: &str| -> Vec<u64> {
        value
            .split('.')
            .map(|segment| segment.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let left = split(left);
    let right = split(right);
    let len = left.len().max(right.len());
    for index in 0..len {
        let a = left.get(index).copied().unwrap_or(0);
        let b = right.get(index).copied().unwrap_or(0);
        match a.cmp(&b) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

/// 将数据库目录名映射为引擎通道名。未知目录返回 None。
pub fn channel_for_db_dir(db_dir: &str) -> Option<&'static str> {
    match db_dir.to_ascii_lowercase().as_str() {
        "oracle" => Some("ORACLE"),
        "dm" => Some("DM"),
        "kingbase" => Some("KINGBASE"),
        "highgo" => Some("HIGHGO"),
        "vastbase" => Some("VASTBASE"),
        "gbase8c" => Some("GBASE8C"),
        "polardb_o" => Some("POLARDB_O"),
        "gaussdb" => Some("GAUSSDB"),
        "gaussdb_d" => Some("GAUSSDB_D"),
        "oceanbase_d" => Some("OCEANBASE_D"),
        "mysql" => Some("MYSQL"),
        "tdsql" => Some("TDSQL"),
        "drds" => Some("DRDS"),
        "h2" => Some("H2"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_portal_names() {
        let parsed =
            parse_source_name("2024-09-25_程王泽_01__insert_AUTH_FUNCTION_TDSQL_[1.2.1].groovy")
                .expect("valid");
        assert_eq!(parsed.date, "2024-09-25");
        assert_eq!(parsed.person, "程王泽");
        assert_eq!(parsed.source_seq, "01");
        assert_eq!(parsed.description, "insert_AUTH_FUNCTION");
        assert_eq!(parsed.db_type, "TDSQL");
        assert_eq!(parsed.version, "1.2.1");
        assert_eq!(parsed.extension, "groovy");
    }

    #[test]
    fn parses_three_digit_sequence_and_sql_extension() {
        let parsed = parse_source_name("2026-03-27_陈晓斌_001__alter_table_X_ORACLE_[1.0.6.4].sql")
            .expect("valid");
        assert_eq!(parsed.source_seq, "001");
        assert_eq!(parsed.extension, "sql");
        assert_eq!(parsed.db_type, "ORACLE");
    }

    #[test]
    fn rejects_single_underscore_before_description() {
        let error = parse_source_name(
            "2026-08-13_曾俊伟_01_insert_oper_config_inter_api_TDSQL_[1.5.0].groovy",
        )
        .expect_err("invalid");
        assert!(error.contains("文件名不符合规范"));
    }

    #[test]
    fn rejects_invalid_dates() {
        let error = parse_source_name("2026-02-31_张三_01__insert_X_TDSQL_[1.0.0].groovy")
            .expect_err("invalid");
        assert!(error.contains("日期不合法"));
    }

    #[test]
    fn group_key_ignores_database_type() {
        let oracle = parse_source_name("2026-05-08_赖钰__01__insert_X_ORACLE_[4.5.3.0].groovy")
            .expect("oracle");
        let tdsql = parse_source_name("2026-05-08_赖钰__01__insert_X_TDSQL_[4.5.3.0].groovy")
            .expect("tdsql");
        assert_eq!(group_key(&oracle), group_key(&tdsql));
    }

    #[test]
    fn builds_and_parses_archive_names() {
        let name = archive_file_name(
            "4.6.2.0",
            "990000_product",
            "00",
            1,
            "insert_auth_uaa_dict_type",
            "TDSQL",
            "groovy",
        );
        assert_eq!(
            name,
            "V4.6.2.0_990000_00_01__insert_auth_uaa_dict_type_TDSQL_[4.6.2.0].groovy"
        );
        let parsed = parse_archive_name(&name).expect("archive name");
        assert_eq!(parsed.sequence, 1);
        assert_eq!(parsed.version, "4.6.2.0");
        assert_eq!(parsed.module_code, "990000");
        assert_eq!(parsed.category, "00");
        assert_eq!(parsed.db_type, "TDSQL");
    }

    #[test]
    fn compares_versions_numerically() {
        assert_eq!(compare_versions("1.5.0", "1.4.9"), Ordering::Greater);
        assert_eq!(compare_versions("4.5.3", "4.5.3.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.14.0", "1.4.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0", "1.0.10"), Ordering::Less);
    }

    #[test]
    fn maps_known_channels_only() {
        assert_eq!(channel_for_db_dir("kingbase"), Some("KINGBASE"));
        assert_eq!(channel_for_db_dir("polardb_x"), None);
    }
}
