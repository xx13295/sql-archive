use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

/// 在 Windows 桌面构建中创建子进程时不弹出控制台窗口。
pub fn background_command<S: AsRef<OsStr>>(program: S) -> Command {
    // 只有 Windows 分支会修改 command，其他平台的可变绑定会触发 unused_mut。
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// 数据目录：portable 构建放在 EXE 同目录；否则使用应用数据目录。
pub fn data_dir(app_data_dir: PathBuf) -> PathBuf {
    if let Ok(override_dir) = std::env::var("SQL_ARCHIVE_DATA_DIR") {
        if !override_dir.trim().is_empty() {
            return PathBuf::from(override_dir);
        }
    }
    #[cfg(feature = "portable")]
    {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                return parent.join("data").join("sql-archive-v1");
            }
        }
    }
    app_data_dir.join("sql-archive-v1")
}

pub fn open_path(path: &PathBuf, reveal: bool) -> crate::error::AppResult<()> {
    #[cfg(target_os = "windows")]
    {
        let mut command = background_command("explorer.exe");
        if reveal {
            command.arg(format!("/select,{}", path.display()));
        } else {
            command.arg(path);
        }
        command
            .spawn()
            .map_err(|error| crate::error::AppError::io("打开本地路径失败", error))?;
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = background_command("open");
        if reveal {
            command.arg("-R");
        }
        command
            .arg(path)
            .spawn()
            .map_err(|error| crate::error::AppError::io("打开本地路径失败", error))?;
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        background_command("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|error| crate::error::AppError::io("打开本地路径失败", error))?;
    }
    Ok(())
}
