#![allow(clippy::result_large_err)]

mod config_table;
mod error;
mod executor;
mod filename;
#[cfg(test)]
mod industry_tests;
mod java_bridge;
mod java_env;
mod models;
#[cfg(test)]
mod pipeline_tests;

/// Java 桥接测试会修改进程级环境变量，必须串行执行。
#[cfg(test)]
pub(crate) static PIPELINE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
mod plan;
mod platform;
mod scan;
mod sequence;
mod storage;
mod util;
mod validation;
mod workspace;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use parking_lot::Mutex as ParkingMutex;
use tauri::{Manager, State};

use error::{AppError, AppResult};
use models::{
    AppInfo, ArchivePlan, ArchiveRun, ArchiveSettings, ConfirmArchiveInput, HistorySummary,
    PreviewArchiveInput, RootProfile, ScriptContent, TempScanResult, ValidateInput,
    ValidationResult,
};
use storage::Storage;

pub struct AppState {
    storage: Storage,
    data_dir: PathBuf,
    resource_dir: PathBuf,
    startup_warnings: Mutex<Vec<String>>,
    lock: ParkingMutex<()>,
}

fn settings_for(state: &AppState) -> ArchiveSettings {
    state.storage.get_settings()
}

fn inspect(root_path: &str, state: &AppState) -> AppResult<RootProfile> {
    workspace::inspect_root(root_path, &settings_for(state))
}

fn prepare_engine(
    profile: &RootProfile,
    state: &AppState,
) -> AppResult<Option<java_bridge::JavaEngine>> {
    if !profile.java.available || !profile.engine.available {
        return Ok(None);
    }
    let jar = java_bridge::write_bridge_jar(&state.resource_dir, &state.data_dir)?;
    Ok(Some(java_bridge::JavaEngine::prepare(profile, &jar)?))
}

#[tauri::command]
fn get_app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: state.data_dir.to_string_lossy().to_string(),
        portable: cfg!(feature = "portable"),
    }
}

#[tauri::command]
fn get_archive_settings(state: State<'_, AppState>) -> ArchiveSettings {
    state.storage.get_settings()
}

#[tauri::command]
fn save_archive_settings(
    settings: ArchiveSettings,
    state: State<'_, AppState>,
) -> AppResult<ArchiveSettings> {
    state.storage.save_settings(&settings)?;
    if let Some(root) = settings.workspace_root.as_deref() {
        if Path::new(root).is_dir() {
            if let Ok(profile) = workspace::inspect_root(root, &settings) {
                let index = settings
                    .product
                    .as_ref()
                    .and_then(|product| {
                        profile
                            .products
                            .iter()
                            .position(|item| &item.dir_name == product)
                    })
                    .map(|position| position + 1);
                let _ = workspace::write_user_config(Path::new(root), root, index);
            }
        }
    }
    Ok(settings)
}

#[tauri::command]
fn inspect_archive_root(root_path: String, state: State<'_, AppState>) -> AppResult<RootProfile> {
    inspect(&root_path, &state)
}

#[tauri::command(async)]
fn scan_temp_scripts(
    root_path: String,
    product: String,
    state: State<'_, AppState>,
) -> AppResult<TempScanResult> {
    let _guard = state.lock.lock();
    let profile = inspect(&root_path, &state)?;
    scan::scan_product(&profile, &state.storage, &product)
}

#[tauri::command(async)]
fn validate_temp_scripts(
    input: ValidateInput,
    state: State<'_, AppState>,
) -> AppResult<std::collections::HashMap<String, ValidationResult>> {
    let _guard = state.lock.lock();
    let profile = inspect(&input.root_path, &state)?;
    let engine = prepare_engine(&profile, &state)?;
    validation::validate_selected(
        &profile,
        &state.storage,
        engine.as_ref(),
        &input.product,
        &input.relative_paths,
    )
}

#[tauri::command]
fn set_script_mark(
    root_path: String,
    relative_path: String,
    sha256: String,
    mark: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.storage.set_mark(
        &storage::root_id(&root_path),
        &relative_path,
        &sha256,
        &mark,
    )
}

pub(crate) fn read_script_content_at(
    profile: &RootProfile,
    relative_path: &str,
) -> AppResult<ScriptContent> {
    let normalized = relative_path.replace('\\', "/");
    if !normalized.starts_with("temp/")
        || normalized
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(AppError::user(
            "PATH_INVALID",
            "脚本路径必须是当前仓库下的 temp/ 相对路径。",
        ));
    }
    let path = Path::new(&profile.root_path).join(&normalized);
    let (content, has_bom) = util::read_text_file(&path)?;
    Ok(ScriptContent {
        content,
        encoding: "UTF-8".into(),
        has_bom,
    })
}

#[tauri::command]
fn read_script_content(
    root_path: String,
    relative_path: String,
    state: State<'_, AppState>,
) -> AppResult<ScriptContent> {
    let profile = inspect(&root_path, &state)?;
    read_script_content_at(&profile, &relative_path)
}

#[tauri::command(async)]
fn preview_archive(
    input: PreviewArchiveInput,
    state: State<'_, AppState>,
) -> AppResult<ArchivePlan> {
    let _guard = state.lock.lock();
    let settings = settings_for(&state);
    let profile = inspect(&input.root_path, &state)?;
    if !profile.java.available || !profile.engine.available {
        return Err(AppError::user(
            "ENGINE_UNAVAILABLE",
            "Java 引擎不可用，无法生成归档预览。",
        ));
    }
    let engine = prepare_engine(&profile, &state)?
        .ok_or_else(|| AppError::user("ENGINE_UNAVAILABLE", "Java 引擎不可用。"))?;
    plan::preview_archive(plan::PreviewContext {
        profile: &profile,
        engine: &engine,
        storage: &state.storage,
        data_dir: &state.data_dir,
        product: &input.product,
        relative_paths: &input.relative_paths,
        module_child: input
            .module_child
            .as_deref()
            .unwrap_or(&settings.module_child),
        general_child: input
            .general_child
            .as_deref()
            .unwrap_or(&settings.general_child),
    })
}

#[tauri::command(async)]
fn confirm_archive(
    input: ConfirmArchiveInput,
    state: State<'_, AppState>,
) -> AppResult<ArchiveRun> {
    let _guard = state.lock.lock();
    executor::confirm_archive(&state.storage, &state.data_dir, input)
}

#[tauri::command]
fn get_archive_run(run_id: String, state: State<'_, AppState>) -> AppResult<ArchiveRun> {
    state.storage.get_run(&run_id)
}

#[tauri::command]
fn list_archive_runs(
    root_path: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> AppResult<Vec<HistorySummary>> {
    state
        .storage
        .list_runs(&storage::root_id(&root_path), limit.unwrap_or(50))
}

#[tauri::command]
fn take_startup_warnings(state: State<'_, AppState>) -> Vec<String> {
    let mut warnings = state.startup_warnings.lock().expect("warnings lock");
    std::mem::take(&mut *warnings)
}

#[tauri::command]
fn open_archive_path(
    root_path: String,
    path: String,
    reveal: Option<bool>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let canonical_root = Path::new(&root_path)
        .canonicalize()
        .map_err(|error| AppError::user("ROOT_NOT_FOUND", format!("根目录不可访问：{error}")))?;
    let target = PathBuf::from(&path)
        .canonicalize()
        .map_err(|error| AppError::user("PATH_NOT_FOUND", format!("路径不可访问：{error}")))?;
    let canonical_data = state
        .data_dir
        .canonicalize()
        .unwrap_or_else(|_| state.data_dir.clone());
    if !target.starts_with(&canonical_root) && !target.starts_with(&canonical_data) {
        return Err(AppError::user(
            "PATH_OUTSIDE_ROOT",
            "只能打开当前仓库或应用数据目录下的路径。",
        ));
    }
    platform::open_path(&target, reveal.unwrap_or(false))
}

#[tauri::command]
fn test_java(java_path: Option<String>) -> models::JavaStatus {
    java_env::detect_java(java_path.as_deref())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("无法解析应用数据目录：{error}"))?;
            let data_dir = platform::data_dir(app_data);
            let storage = Storage::open(&data_dir)
                .map_err(|error| format!("无法初始化本地数据库：{}", error.message))?;
            let startup_warnings = executor::recover_interrupted(&storage);
            plan::cleanup_expired_plans(&data_dir);
            if let Ok(workspace) = std::env::var("SQL_ARCHIVE_WORKSPACE") {
                let mut settings = storage.get_settings();
                if settings.workspace_root.is_none() && Path::new(&workspace).is_dir() {
                    settings.workspace_root = Some(workspace);
                    let _ = storage.save_settings(&settings);
                }
            }
            let resource_dir = app
                .path()
                .resource_dir()
                .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
            app.manage(AppState {
                storage,
                data_dir,
                resource_dir,
                startup_warnings: Mutex::new(startup_warnings),
                lock: ParkingMutex::new(()),
            });
            if let Some(window) = app.get_webview_window("main") {
                // 某些 Windows/DPI 组合下首个原生窗口会以极小尺寸创建，
                // 这里在启动时显式恢复默认尺寸，避免出现被裁剪的界面。
                let _ = window.set_size(tauri::LogicalSize::new(1280.0, 800.0));
                let _ = window.center();
                let _ = window.show();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_archive_settings,
            save_archive_settings,
            inspect_archive_root,
            scan_temp_scripts,
            validate_temp_scripts,
            set_script_mark,
            read_script_content,
            preview_archive,
            confirm_archive,
            get_archive_run,
            list_archive_runs,
            take_startup_warnings,
            open_archive_path,
            test_java
        ])
        .run(tauri::generate_context!())
        .expect("error while running sql-archive");
}
