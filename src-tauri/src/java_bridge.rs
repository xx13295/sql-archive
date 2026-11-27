//! Rust 与 sql-translate Java 引擎之间的桥接。
//!
//! Java 8 在 Windows 上会按 ANSI 代码页解码命令行参数，因此所有传入 Java 的
//! 路径都必须是纯 ASCII：本模块把引擎 jar 和 bridge jar 复制到 ASCII 工作区，
//! 每次运行在 ASCII 临时目录中准备脚本、文件名与分表配置。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};
use crate::models::RootProfile;
use crate::platform::background_command;
use crate::util::{sha256_bytes, sha256_file};

const DEFAULT_SHARDING: &str = "spring:";
const BRIDGE_MAIN: &str = "cn.bosicloud.sqlarchive.ArchiveBridge";
const RUN_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone)]
pub enum BridgeRequest {
    Check {
        index: usize,
        source_db: String,
    },
    Translate {
        index: usize,
        source_db: String,
        target_db: String,
    },
}

#[derive(Debug, Clone)]
pub struct BridgeScript {
    pub index: usize,
    pub file_name: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct BridgeJobResult {
    pub ok: bool,
    pub flag: bool,
    pub message: String,
    pub sql: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BridgeOutput {
    pub check: BTreeMap<usize, BridgeJobResult>,
    pub translate: BTreeMap<(usize, String), BridgeJobResult>,
    pub sharding: String,
}

pub struct JavaEngine {
    pub java_path: String,
    pub bridge_jar: PathBuf,
    pub ascii_engine_dir: PathBuf,
    pub fingerprint: String,
    pub check_rule_path: PathBuf,
    pub check_rule_sha: String,
    pub sharding: String,
    pub sharding_sha: String,
}

impl JavaEngine {
    pub fn prepare(profile: &RootProfile, bridge_jar: &Path) -> AppResult<Self> {
        if !profile.java.available {
            return Err(AppError::user(
                "JAVA_UNAVAILABLE",
                profile
                    .java
                    .message
                    .clone()
                    .unwrap_or_else(|| "未找到 Java 运行时。".into()),
            ));
        }
        if !profile.engine.available {
            return Err(AppError::user(
                "ENGINE_UNAVAILABLE",
                profile
                    .engine
                    .message
                    .clone()
                    .unwrap_or_else(|| "Java 引擎不可用。".into()),
            ));
        }
        let engine_dir = PathBuf::from(
            profile
                .engine_dir
                .as_deref()
                .ok_or_else(|| AppError::user("ENGINE_UNAVAILABLE", "未找到引擎目录。"))?,
        );
        let check_rule_path = PathBuf::from(
            profile
                .check_rule_path
                .as_deref()
                .ok_or_else(|| AppError::user("CHECK_RULE_MISSING", "缺少 check-rule.yml。"))?,
        );
        let check_rule_sha = sha256_file(&check_rule_path)?;
        let (_sharding_path, sharding, sharding_sha) =
            read_sharding(profile.sharding_path.as_deref())?;
        let bridge_sha = sha256_file(bridge_jar)?;
        // fingerprint 只覆盖引擎相关内容，避免切换产品/写回 user-config 时重复复制 JAR。
        let mut fingerprint_parts = vec![
            format!("bridge:{bridge_sha}"),
            format!("rule:{check_rule_sha}"),
            format!("sharding:{sharding_sha}"),
            format!("java:{}", profile.java.version.clone().unwrap_or_default()),
        ];
        for jar in &profile.engine.jars {
            fingerprint_parts.push(format!("jar:{}:{}", jar.name, jar.sha256));
        }
        fingerprint_parts.sort();
        let fingerprint = sha256_bytes(fingerprint_parts.join("\n").as_bytes());

        let ascii_base = ascii_workspace_base()?;
        let ascii_engine_dir = ascii_base.join("engine").join(&fingerprint);
        fs::create_dir_all(&ascii_engine_dir)
            .map_err(|error| AppError::io("创建 Java 工作区失败", error))?;
        for entry in fs::read_dir(&engine_dir)
            .map_err(|error| AppError::io("读取引擎目录失败", error))?
            .flatten()
        {
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("jar"))
                != Some(true)
            {
                continue;
            }
            let name = entry.file_name();
            let target = ascii_engine_dir.join(&name);
            copy_if_changed(&path, &target)?;
        }
        let ascii_bridge = ascii_engine_dir.join("sql-archive-bridge.jar");
        copy_if_changed(bridge_jar, &ascii_bridge)?;
        cleanup_engine_dirs(&ascii_base.join("engine"), &fingerprint);

        Ok(Self {
            java_path: profile
                .java
                .path
                .clone()
                .ok_or_else(|| AppError::user("JAVA_UNAVAILABLE", "未找到 java.exe。"))?,
            bridge_jar: ascii_bridge,
            ascii_engine_dir,
            fingerprint,
            check_rule_path,
            check_rule_sha,
            sharding,
            sharding_sha,
        })
    }

    pub fn check_batch(
        &self,
        scripts: &[BridgeScript],
        source_db: &str,
    ) -> AppResult<BTreeMap<usize, BridgeJobResult>> {
        let requests = scripts
            .iter()
            .map(|script| BridgeRequest::Check {
                index: script.index,
                source_db: source_db.to_string(),
            })
            .collect::<Vec<_>>();
        let output = self.run(&requests, scripts)?;
        Ok(output.check)
    }

    /// 用指定的分表配置作为输入进行翻译；用于在一个归档批次内顺序累积分表配置。
    pub fn translate_batch_with_sharding(
        &self,
        scripts: &[BridgeScript],
        source_db: &str,
        target_db: &str,
        sharding: &str,
    ) -> AppResult<BridgeOutput> {
        let requests = scripts
            .iter()
            .map(|script| BridgeRequest::Translate {
                index: script.index,
                source_db: source_db.to_string(),
                target_db: target_db.to_string(),
            })
            .collect::<Vec<_>>();
        self.run_with_sharding(&requests, scripts, sharding)
    }

    fn run(&self, requests: &[BridgeRequest], scripts: &[BridgeScript]) -> AppResult<BridgeOutput> {
        self.run_with_sharding(requests, scripts, &self.sharding)
    }

    fn run_with_sharding(
        &self,
        requests: &[BridgeRequest],
        scripts: &[BridgeScript],
        sharding: &str,
    ) -> AppResult<BridgeOutput> {
        let run_dir = self
            .ascii_engine_dir
            .join("runs")
            .join(uuid::Uuid::new_v4().simple().to_string());
        fs::create_dir_all(run_dir.join("scripts"))
            .map_err(|error| AppError::io("创建桥接临时目录失败", error))?;
        fs::create_dir_all(run_dir.join("names"))
            .map_err(|error| AppError::io("创建桥接临时目录失败", error))?;
        fs::create_dir_all(run_dir.join("out"))
            .map_err(|error| AppError::io("创建桥接临时目录失败", error))?;
        for script in scripts {
            fs::write(
                run_dir
                    .join("scripts")
                    .join(format!("{}.groovy", script.index)),
                &script.content,
            )
            .map_err(|error| AppError::io("写入桥接脚本失败", error))?;
            fs::write(
                run_dir.join("names").join(format!("{}.txt", script.index)),
                script.file_name.as_bytes(),
            )
            .map_err(|error| AppError::io("写入桥接文件名失败", error))?;
        }
        fs::copy(&self.check_rule_path, run_dir.join("check-rule.yml"))
            .map_err(|error| AppError::io("复制校验规则失败", error))?;
        fs::write(run_dir.join("sharding.in"), sharding.as_bytes())
            .map_err(|error| AppError::io("写入分表配置失败", error))?;
        let mut request_text = String::new();
        for request in requests {
            match request {
                BridgeRequest::Check { index, source_db } => {
                    request_text.push_str(&format!("CHECK\t{index}\t{source_db}\n"));
                }
                BridgeRequest::Translate {
                    index,
                    source_db,
                    target_db,
                } => {
                    request_text
                        .push_str(&format!("TRANSLATE\t{index}\t{source_db}\t{target_db}\n"));
                }
            }
        }
        request_text.push_str("END\n");
        fs::write(run_dir.join("request.tsv"), request_text.as_bytes())
            .map_err(|error| AppError::io("写入桥接请求失败", error))?;

        let result_path = run_dir.join("results.tsv");
        let stdout_path = run_dir.join("stdout.txt");
        let stderr_path = run_dir.join("stderr.txt");
        let stdout = fs::File::create(&stdout_path)
            .map_err(|error| AppError::io("创建桥接日志失败", error))?;
        let stderr = fs::File::create(&stderr_path)
            .map_err(|error| AppError::io("创建桥接日志失败", error))?;
        let separator = if cfg!(windows) { ";" } else { ":" };
        let classpath = format!(
            "{}*{separator}{}",
            with_trailing_separator(&self.ascii_engine_dir),
            self.bridge_jar.display()
        );
        let mut command: Command = background_command(&self.java_path);
        command
            .arg("-Dfile.encoding=UTF-8")
            .arg("-Dsun.jnu.encoding=UTF-8")
            .arg("-cp")
            .arg(classpath)
            .arg(BRIDGE_MAIN)
            .arg(&run_dir)
            .arg(&result_path)
            .current_dir(&run_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        let mut child = command
            .spawn()
            .map_err(|error| AppError::io("启动 Java 引擎失败", error))?;
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if start.elapsed() > RUN_TIMEOUT {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(AppError::user(
                            "ENGINE_TIMEOUT",
                            "Java 引擎执行超时，请检查脚本或引擎配置。",
                        ));
                    }
                    thread::sleep(Duration::from_millis(80));
                }
                Err(error) => {
                    return Err(AppError::io("等待 Java 引擎失败", error));
                }
            }
        }
        if !result_path.is_file() {
            let stderr_text = fs::read_to_string(&stderr_path).unwrap_or_default();
            let stdout_text = fs::read_to_string(&stdout_path).unwrap_or_default();
            return Err(AppError::user(
                "ENGINE_NO_RESULT",
                format!(
                    "Java 引擎未返回结果。{} {}",
                    stderr_text.lines().next().unwrap_or_default(),
                    stdout_text.lines().next().unwrap_or_default()
                ),
            ));
        }
        let output = self.parse_output(&run_dir, &result_path)?;
        let _ = fs::remove_dir_all(&run_dir);
        Ok(output)
    }

    fn parse_output(&self, run_dir: &Path, result_path: &Path) -> AppResult<BridgeOutput> {
        let text = fs::read_to_string(result_path)
            .map_err(|error| AppError::io("读取桥接结果失败", error))?;
        let mut check = BTreeMap::new();
        let mut translate = BTreeMap::new();
        let mut fatal = None;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 4 {
                continue;
            }
            let status = fields[0];
            let index = fields[1].parse::<usize>().unwrap_or(0);
            let kind = fields[2];
            if status == "FATAL" {
                fatal = Some(decode(fields.get(4).copied().unwrap_or_default()));
                continue;
            }
            if status == "DONE" {
                continue;
            }
            let flag = fields[3] == "1";
            let message = decode(fields.get(4).copied().unwrap_or_default());
            if kind == "CHECK" {
                check.insert(
                    index,
                    BridgeJobResult {
                        ok: status == "OK",
                        flag,
                        message,
                        sql: None,
                    },
                );
            } else if kind == "TRANSLATE" {
                let target = fields
                    .get(5)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_string())
                    .or_else(|| target_from_out_file(run_dir, index))
                    .unwrap_or_default();
                let sql = read_out_file(run_dir, index, &target);
                translate.insert(
                    (index, target),
                    BridgeJobResult {
                        ok: status == "OK",
                        flag,
                        message,
                        sql,
                    },
                );
            }
        }
        if let Some(message) = fatal {
            return Err(AppError::user("ENGINE_FATAL", message));
        }
        let sharding = fs::read_to_string(run_dir.join("sharding.out"))
            .unwrap_or_else(|_| self.sharding.clone());
        Ok(BridgeOutput {
            check,
            translate,
            sharding,
        })
    }
}

fn read_sharding(path: Option<&str>) -> AppResult<(Option<PathBuf>, String, String)> {
    let Some(path) = path else {
        return Ok((
            None,
            DEFAULT_SHARDING.to_string(),
            sha256_bytes(DEFAULT_SHARDING.as_bytes()),
        ));
    };
    let path = PathBuf::from(path);
    let content = if path.is_file() {
        fs::read_to_string(&path).map_err(|error| AppError::io("读取分表模板失败", error))?
    } else {
        DEFAULT_SHARDING.to_string()
    };
    let content = if content.trim().is_empty() {
        DEFAULT_SHARDING.to_string()
    } else {
        content
    };
    let sha = sha256_bytes(content.as_bytes());
    Ok((Some(path), content, sha))
}

fn ascii_workspace_base() -> AppResult<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(value) = std::env::var("SQL_ARCHIVE_BRIDGE_DIR") {
        if !value.trim().is_empty() {
            candidates.push(PathBuf::from(value));
        }
    }
    if let Some(public) = std::env::var_os("PUBLIC") {
        candidates.push(PathBuf::from(public));
    }
    candidates.push(PathBuf::from(r"C:\ProgramData"));
    candidates.push(std::env::temp_dir());
    for candidate in candidates {
        let candidate = candidate.join("sql-archive-bridge");
        if !is_ascii_path(&candidate) {
            continue;
        }
        if fs::create_dir_all(&candidate).is_ok() {
            return Ok(candidate);
        }
    }
    Err(AppError::user(
        "BRIDGE_ASCII_WORKSPACE_UNAVAILABLE",
        "无法创建纯 ASCII 的 Java 工作区，请设置 SQL_ARCHIVE_BRIDGE_DIR 指向 ASCII 路径。",
    ))
}

fn is_ascii_path(path: &Path) -> bool {
    path.to_string_lossy().is_ascii()
}

fn copy_if_changed(source: &Path, target: &Path) -> AppResult<()> {
    let same = target.is_file()
        && sha256_file(source).ok().is_some()
        && sha256_file(source).ok() == sha256_file(target).ok();
    if same {
        return Ok(());
    }
    fs::copy(source, target)
        .map(|_| ())
        .map_err(|error| AppError::io("复制引擎文件失败", error))
}

fn cleanup_engine_dirs(engine_root: &Path, current: &str) {
    let Ok(entries) = fs::read_dir(engine_root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if entry.file_name().to_string_lossy() != current {
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn with_trailing_separator(dir: &Path) -> String {
    let mut value = dir.display().to_string();
    if !value.ends_with('\\') && !value.ends_with('/') {
        value.push(std::path::MAIN_SEPARATOR);
    }
    value
}

fn decode(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    STANDARD
        .decode(value)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| value.to_string())
}

fn target_from_out_file(run_dir: &Path, index: usize) -> Option<String> {
    let out_dir = run_dir.join("out");
    let prefix = format!("{index}__");
    let entries = fs::read_dir(out_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(rest) = name.strip_prefix(&prefix) {
            return Some(rest.trim_end_matches(".sql").to_string());
        }
    }
    None
}

fn read_out_file(run_dir: &Path, index: usize, target: &str) -> Option<String> {
    fs::read_to_string(run_dir.join("out").join(format!("{index}__{target}.sql"))).ok()
}

const EMBEDDED_BRIDGE: &[u8] = include_bytes!("../../java-bridge/dist/sql-archive-bridge.jar");

pub fn write_bridge_jar(resource_dir: &Path, data_dir: &Path) -> AppResult<PathBuf> {
    let bundled = resource_dir
        .join("java-bridge")
        .join("sql-archive-bridge.jar");
    if bundled.is_file() {
        return Ok(bundled);
    }
    #[cfg(debug_assertions)]
    {
        // 仅开发构建回退到源码树；发布版必须走随包资源或嵌入字节，
        // 否则在构建机上会意外命中源码路径，导致产物离开构建机后失效。
        let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("java-bridge")
            .join("dist")
            .join("sql-archive-bridge.jar");
        if dev.is_file() {
            return Ok(dev);
        }
    }
    // 免安装构建可能没有随包资源目录；此时把编译期嵌入的桥接包落到应用数据目录。
    let target = data_dir.join("java-bridge").join("sql-archive-bridge.jar");
    if !target.is_file() || sha256_file(&target).ok() != Some(sha256_bytes(EMBEDDED_BRIDGE)) {
        crate::util::atomic_write(&target, EMBEDDED_BRIDGE)?;
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_bridge_is_materialized_when_resources_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let resource_dir = dir.path().join("missing-resources");
        let data_dir = dir.path().join("data");
        fs::create_dir_all(&data_dir).expect("data dir");
        let path = write_bridge_jar(&resource_dir, &data_dir).expect("bridge jar");
        assert!(path.is_file());
        assert_eq!(fs::read(&path).expect("read"), EMBEDDED_BRIDGE);
    }
}
