use std::path::{Path, PathBuf};

use crate::models::JavaStatus;
use crate::platform::background_command;

fn run_version(program: &Path) -> Option<String> {
    let output = background_command(program).arg("-version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let line = text
        .lines()
        .find(|line| line.to_ascii_lowercase().contains("version"))?
        .trim()
        .to_string();
    Some(line)
}

fn resolve_program(candidate: &str) -> PathBuf {
    let path = Path::new(candidate);
    if path.is_dir() {
        path.join("bin")
            .join(if cfg!(windows) { "java.exe" } else { "java" })
    } else {
        path.to_path_buf()
    }
}

pub fn detect_java(explicit: Option<&str>) -> JavaStatus {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(value) = explicit.filter(|value| !value.trim().is_empty()) {
        candidates.push(value.trim().to_string());
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        let home = PathBuf::from(home);
        candidates.push(
            home.join("bin")
                .join(if cfg!(windows) { "java.exe" } else { "java" })
                .to_string_lossy()
                .to_string(),
        );
    }
    candidates.push("java".to_string());

    let mut last_error = None;
    for candidate in candidates {
        let program = resolve_program(&candidate);
        match run_version(&program) {
            Some(version) => {
                return JavaStatus {
                    available: true,
                    path: Some(program.to_string_lossy().to_string()),
                    version: Some(version),
                    message: None,
                }
            }
            None => {
                last_error = Some(candidate);
            }
        }
    }
    JavaStatus {
        available: false,
        path: None,
        version: None,
        message: Some(format!(
            "未找到可用的 Java 8+ 运行时（尝试过 {}）。请在设置中指定 java.exe 或配置 JAVA_HOME。",
            last_error.unwrap_or_else(|| "java".into())
        )),
    }
}
