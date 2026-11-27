use std::fs;
use std::io::Write;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn sha256_file(path: &Path) -> AppResult<String> {
    let bytes = fs::read(path).map_err(|error| AppError::io("读取文件失败", error))?;
    Ok(sha256_bytes(&bytes))
}

pub fn read_text_file(path: &Path) -> AppResult<(String, bool)> {
    let bytes = fs::read(path).map_err(|error| AppError::io("读取文件失败", error))?;
    let has_bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
    let body = if has_bom { &bytes[3..] } else { &bytes[..] };
    let text = std::str::from_utf8(body).map_err(|error| {
        AppError::user("INVALID_UTF8", format!("文件不是有效的 UTF-8：{error}"))
    })?;
    Ok((text.to_string(), has_bom))
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::internal("INVALID_PATH", "目标路径缺少父目录。"))?;
    fs::create_dir_all(parent).map_err(|error| AppError::io("创建目录失败", error))?;
    let temp = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension().and_then(|ext| ext.to_str()).unwrap_or(""),
        uuid::Uuid::new_v4()
    ));
    {
        let mut file =
            fs::File::create(&temp).map_err(|error| AppError::io("创建临时文件失败", error))?;
        file.write_all(bytes)
            .map_err(|error| AppError::io("写入临时文件失败", error))?;
        file.sync_all()
            .map_err(|error| AppError::io("刷新临时文件失败", error))?;
    }
    if path.exists() {
        fs::remove_file(path).map_err(|error| AppError::io("替换目标文件失败", error))?;
    }
    fs::rename(&temp, path).map_err(|error| AppError::io("提交目标文件失败", error))
}

pub fn to_display(path: &Path) -> String {
    user_facing_path(&path.to_string_lossy())
}

/// 去掉 Windows canonicalize 产生的 `\\?\` / `\\?\UNC\` 前缀。
pub fn user_facing_path(value: &str) -> String {
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    value.strip_prefix(r"\\?\").unwrap_or(value).to_string()
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn timestamp_for_backup() -> String {
    chrono::Local::now().format("%Y%m%d").to_string()
}
