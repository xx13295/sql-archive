//! 旧版 aardio 配置表的受限解析器与写回器。
//!
//! 文件形态（UTF-8 BOM 可选）：
//! ```text
//! {
//! productList="标准产品-01_standard,...";
//! productListSelectIndex=10;
//! totalPath="D:\repo\运营支撑门户";
//! }
//! ```
//! 值可为双引号字符串或裸数字；Windows 反斜杠按字面量保留；最后一项可以没有分号。

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
    pub quoted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigTable {
    pub entries: Vec<ConfigEntry>,
    pub bom: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "第 {} 行第 {} 列：{}",
            self.line, self.column, self.message
        )
    }
}

impl std::error::Error for ConfigError {}

impl ConfigTable {
    pub fn parse(bytes: &[u8]) -> Result<Self, ConfigError> {
        let bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
        let body = if bom { &bytes[3..] } else { bytes };
        let text = std::str::from_utf8(body).map_err(|error| ConfigError {
            message: format!("不是有效的 UTF-8 文本（{}）", error),
            line: 1,
            column: 1,
        })?;
        let mut parser = Parser::new(text);
        parser.parse_table(bom)
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.key == key)
            .map(|entry| entry.value.as_str())
    }

    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        let value = value.into();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
            entry.value = value;
            entry.quoted = needs_quote(&entry.value);
            return;
        }
        self.entries.push(ConfigEntry {
            key: key.to_string(),
            quoted: needs_quote(&value),
            value,
        });
    }

    pub fn render(&self) -> Vec<u8> {
        let mut out = String::new();
        out.push('{');
        out.push_str("\r\n");
        for entry in &self.entries {
            out.push_str(&entry.key);
            out.push('=');
            if entry.quoted || needs_quote(&entry.value) {
                out.push('"');
                out.push_str(&escape(&entry.value));
                out.push('"');
            } else {
                out.push_str(&entry.value);
            }
            out.push(';');
            out.push_str("\r\n");
        }
        out.push('}');
        out.push_str("\r\n");
        let mut bytes = Vec::with_capacity(out.len() + 3);
        if self.bom {
            bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        bytes.extend_from_slice(out.as_bytes());
        bytes
    }

    /// 解析以逗号分隔的列表值，去掉空项与首尾空白。
    pub fn split_list(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    }
}

fn needs_quote(value: &str) -> bool {
    value.is_empty()
        || value.trim() != value
        || !value
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '-')
}

fn escape(value: &str) -> String {
    value.replace('"', "\\\"")
}

struct Parser<'a> {
    chars: Vec<char>,
    index: usize,
    line: usize,
    column: usize,
    source: &'a str,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().collect(),
            index: 0,
            line: 1,
            column: 1,
            source,
        }
    }

    fn error(&self, message: impl Into<String>) -> ConfigError {
        ConfigError {
            message: message.into(),
            line: self.line,
            column: self.column,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\r' | '\n')) {
            self.bump();
        }
    }

    fn parse_table(&mut self, bom: bool) -> Result<ConfigTable, ConfigError> {
        self.skip_whitespace();
        match self.bump() {
            Some('{') => {}
            _ => return Err(self.error("配置必须以 '{' 开始")),
        }
        let mut entries = Vec::new();
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some('}') => {
                    self.bump();
                    break;
                }
                None => return Err(self.error("配置缺少结束的 '}'")),
                _ => {}
            }
            let key = self.parse_key()?;
            self.skip_whitespace();
            if self.bump() != Some('=') {
                return Err(self.error(format!("键 {key} 后缺少 '='")));
            }
            self.skip_whitespace();
            let (value, quoted) = match self.peek() {
                Some('"') => self.parse_quoted()?,
                Some(_) => self.parse_bare()?,
                None => return Err(self.error(format!("键 {key} 缺少值"))),
            };
            entries.push(ConfigEntry { key, value, quoted });
            self.skip_whitespace();
            if self.peek() == Some(';') {
                self.bump();
            }
        }
        self.skip_whitespace();
        if self.index != self.chars.len() {
            return Err(self.error("'}' 之后存在多余内容"));
        }
        let _ = self.source;
        Ok(ConfigTable { entries, bom })
    }

    fn parse_key(&mut self) -> Result<String, ConfigError> {
        let start = self.index;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.bump();
            } else {
                break;
            }
        }
        if self.index == start {
            return Err(self.error("缺少配置键名"));
        }
        Ok(self.chars[start..self.index].iter().collect())
    }

    fn parse_quoted(&mut self) -> Result<(String, bool), ConfigError> {
        self.bump();
        let mut value = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok((value, true)),
                // 旧配置把 Windows 路径按字面量保存，反斜杠不是转义符；
                // 只把双引号视为字符串结束（路径末尾可能就是反斜杠）。
                Some('\\') => value.push('\\'),
                Some(ch) => value.push(ch),
                None => return Err(self.error("字符串缺少结束的双引号")),
            }
        }
    }

    fn parse_bare(&mut self) -> Result<(String, bool), ConfigError> {
        let mut value = String::new();
        loop {
            match self.peek() {
                Some(';' | '}' | '\r' | '\n') | None => break,
                Some(_) => {
                    value.push(self.bump().expect("peeked character"));
                }
            }
        }
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            return Err(self.error("配置值为空"));
        }
        Ok((trimmed, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORTAL_SYS: &str = "\u{feff}{\r\nproductList=\"标准产品-01_standard,增值-02_value_added\";\r\ntdsqlDatabaseDirList=\"mysql,h2\";\r\noracleDatabaseDirList=\"\";\r\nreleaseVersion=\"01_standard=4.5.3.0\"\r\n}";

    const INDUSTRY_SYS: &str = "{\nproductList=\"标准产品-01_standard,电子凭证-02_voucher,\n行业项目化-07_projectized\";\noracleDatabaseDirList=\"oracle,dm\";\ntdsqlDatabaseDirList=\"mysql,tdsql,drds\"\n}";

    #[test]
    fn parses_bom_semicolon_and_missing_tail() {
        let table = ConfigTable::parse(PORTAL_SYS.as_bytes()).expect("parse");
        assert!(table.bom);
        assert_eq!(table.get("tdsqlDatabaseDirList"), Some("mysql,h2"));
        assert_eq!(table.get("releaseVersion"), Some("01_standard=4.5.3.0"));
        assert_eq!(table.get("oracleDatabaseDirList"), Some(""));
        assert_eq!(
            ConfigTable::split_list(table.get("productList").unwrap()),
            vec!["标准产品-01_standard", "增值-02_value_added"]
        );
    }

    #[test]
    fn parses_multiline_value_and_bare_number() {
        let text = "{\r\nproductList=\"A-01_a,\r\nB-02_b\";\r\nproductListSelectIndex=10\r\n}";
        let mut table = ConfigTable::parse(text.as_bytes()).expect("parse");
        assert_eq!(table.get("productListSelectIndex"), Some("10"));
        assert_eq!(table.get("productList"), Some("A-01_a,\r\nB-02_b"));
        table.set("productListSelectIndex", "11");
        let rendered = String::from_utf8(table.render()).unwrap();
        assert!(rendered.contains("productListSelectIndex=11;"));
        assert!(!rendered.contains("productListSelectIndex=\"11\""));
    }

    #[test]
    fn keeps_windows_backslashes() {
        let text = "{ totalPath=\"D:\\repo\\运营支撑门户\"; }";
        let table = ConfigTable::parse(text.as_bytes()).expect("parse");
        assert_eq!(table.get("totalPath"), Some("D:\\repo\\运营支撑门户"));
    }

    #[test]
    fn rejects_unterminated_quote_with_position() {
        let error = ConfigTable::parse(b"{\nkey=\"value;\n}").expect_err("must fail");
        assert!(error.line >= 2, "{error}");
    }

    #[test]
    fn keeps_trailing_backslash_before_closing_quote() {
        let text = r#"{ backDefaultFlywayPath="D:\repo\h2\"; totalPath="D:\repo"; }"#;
        let table = ConfigTable::parse(text.as_bytes()).expect("parse");
        assert_eq!(table.get("backDefaultFlywayPath"), Some(r"D:\repo\h2\"));
        assert_eq!(table.get("totalPath"), Some(r"D:\repo"));
    }

    #[test]
    fn parses_industry_product_list() {
        let table = ConfigTable::parse(INDUSTRY_SYS.as_bytes()).expect("parse");
        let products = ConfigTable::split_list(table.get("productList").unwrap());
        assert_eq!(products.len(), 3);
        assert_eq!(products[2], "行业项目化-07_projectized");
        assert!(!table.bom);
    }

    #[test]
    fn render_roundtrip_preserves_unknown_keys() {
        let mut table = ConfigTable::parse(INDUSTRY_SYS.as_bytes()).expect("parse");
        table.set("customUnknown", "保留值");
        let rendered = table.render();
        let reparsed = ConfigTable::parse(&rendered).expect("reparse");
        assert_eq!(reparsed.get("customUnknown"), Some("保留值"));
        assert_eq!(
            reparsed.get("tdsqlDatabaseDirList"),
            Some("mysql,tdsql,drds")
        );
    }
}
