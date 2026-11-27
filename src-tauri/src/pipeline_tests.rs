use std::fs;
use std::path::{Path, PathBuf};

use crate::java_bridge::JavaEngine;
use crate::models::{ArchiveSettings, ConfirmArchiveInput};
use crate::plan::{preview_archive, PreviewContext};
use crate::storage::{root_id, Storage};

const SAMPLE: &str = include_str!("../fixtures/sample_tdsql.groovy");

fn engine_dir() -> Option<PathBuf> {
    let candidate = std::env::var("SQL_ARCHIVE_TEST_ENGINE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(r"D:\ai-work\database-work\tool\java"));
    if !candidate.is_dir() {
        return None;
    }
    let has_engine = fs::read_dir(&candidate).ok()?.flatten().any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .to_ascii_lowercase()
            .starts_with("sql-translate")
    });
    has_engine.then_some(candidate)
}

fn bridge_jar() -> Option<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("java-bridge")
        .join("dist")
        .join("sql-archive-bridge.jar");
    path.is_file().then_some(path)
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn setup_sandbox() -> Option<PathBuf> {
    let engine = engine_dir()?;
    let check_rule = engine_dir()?
        .parent()?
        .join("config")
        .join("check-rule.yml");
    let root = std::env::temp_dir().join(format!(
        "sql-archive-test-{}",
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(root.join("tool").join("java")).unwrap();
    for entry in fs::read_dir(&engine).unwrap().flatten() {
        if entry.path().extension().and_then(|ext| ext.to_str()) == Some("jar") {
            fs::copy(
                entry.path(),
                root.join("tool").join("java").join(entry.file_name()),
            )
            .unwrap();
        }
    }
    if check_rule.is_file() {
        fs::create_dir_all(root.join("tool").join("config")).unwrap();
        fs::copy(
            &check_rule,
            root.join("tool").join("config").join("check-rule.yml"),
        )
        .unwrap();
    } else {
        return None;
    }
    let root_text = root.to_string_lossy().to_string();
    write(
        &root.join("tool").join("config").join("sys-config.table"),
        "{\r\nproductList=\"用户扩展服务-25_user_extension\";\r\ntdsqlDatabaseDirList=\"mysql,h2\";\r\noracleDatabaseDirList=\"\";\r\nreleaseVersion=\"25_user_extension=4.6.2.0\"\r\n}",
    );
    write(
        &root.join("tool").join("config").join("user-config.table"),
        &format!("{{\r\nproductListSelectIndex=1;\r\ntotalPath=\"{root_text}\";\r\n}}"),
    );
    let temp = root.join("temp").join("25_user_extension");
    fs::create_dir_all(&temp).unwrap();
    fs::create_dir_all(root.join("backup").join("25_user_extension")).unwrap();
    let general = root
        .join("nontax-flyway-db")
        .join("nontax-flyway-db-1.0.3")
        .join("config")
        .join("nontax")
        .join("db")
        .join("migration")
        .join("25_user_extension")
        .join("4.6.2.0")
        .join("990000_product")
        .join("general");
    for db in ["mysql", "h2"] {
        let dir = general.join(db);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("V4.6.2.0_990000_00_10__seed_TDSQL_[4.6.2.0].groovy"),
            b"// seed",
        )
        .unwrap();
    }
    let name = "2026-04-01_测试_01__insert_AUTH_FUNCTION_TDSQL_[4.6.2.0].groovy";
    write(&temp.join(name), SAMPLE);
    Some(root)
}

#[test]
fn archive_pipeline_success_and_rollback() {
    let _guard = crate::PIPELINE_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let Some(root) = setup_sandbox() else {
        eprintln!("skip: Java engine or bridge jar unavailable");
        return;
    };
    let Some(bridge) = bridge_jar() else {
        eprintln!("skip: bridge jar unavailable");
        return;
    };
    let bridge_dir = root.join("bridge");
    std::env::set_var("SQL_ARCHIVE_BRIDGE_DIR", &bridge_dir);
    let data_dir = root.join("data");
    let storage = Storage::open(&data_dir).unwrap();
    let settings = ArchiveSettings {
        workspace_root: Some(root.to_string_lossy().to_string()),
        product: Some("25_user_extension".into()),
        ..ArchiveSettings::default()
    };
    let profile = crate::workspace::inspect_root(&root.to_string_lossy(), &settings).unwrap();
    assert!(
        profile.java.available,
        "java unavailable: {:?}",
        profile.java.message
    );
    assert!(
        profile.engine.available,
        "engine unavailable: {:?}",
        profile.engine.message
    );
    let scan = crate::scan::scan_product(&profile, &storage, "25_user_extension").unwrap();
    let name = "2026-04-01_测试_01__insert_AUTH_FUNCTION_TDSQL_[4.6.2.0].groovy";
    let item = scan
        .scripts
        .iter()
        .find(|script| script.file_name == name)
        .expect("scanned script");
    storage
        .set_mark(&root_id(&profile.root_path), name, &item.sha256, "passed")
        .unwrap();
    let engine = JavaEngine::prepare(&profile, &bridge).unwrap();
    let plan = preview_archive(PreviewContext {
        profile: &profile,
        engine: &engine,
        storage: &storage,
        data_dir: &data_dir,
        product: "25_user_extension",
        relative_paths: &[name.to_string()],
        module_child: "990000_product",
        general_child: "general",
    })
    .unwrap();
    assert!(plan.conflicts.is_empty(), "conflicts: {:?}", plan.conflicts);
    assert_eq!(plan.items.len(), 1);
    let plan_item = &plan.items[0];
    assert_eq!(plan_item.sequence, 11);
    assert_eq!(plan_item.targets.len(), 2, "expected mysql+h2 targets");
    let ack = if plan_item.tenant_code_warning {
        vec![plan_item.source_file_name.clone()]
    } else {
        Vec::new()
    };
    let run = crate::executor::confirm_archive(
        &storage,
        &data_dir,
        ConfirmArchiveInput {
            plan_id: plan.id.clone(),
            confirmed: true,
            tenant_ack_paths: ack,
        },
    )
    .unwrap();
    assert_eq!(run.state, "committed", "run error: {:?}", run.error);
    let temp_dir = root.join("temp").join("25_user_extension");
    assert!(!temp_dir.join(name).exists(), "source should be moved");
    let backup = root
        .join("backup")
        .join("25_user_extension")
        .join(&run.backup_date)
        .join(name);
    assert!(backup.is_file(), "backup missing: {}", backup.display());
    for db in ["mysql", "h2"] {
        let target = root
            .join("nontax-flyway-db")
            .join("nontax-flyway-db-1.0.3")
            .join("config")
            .join("nontax")
            .join("db")
            .join("migration")
            .join("25_user_extension")
            .join("4.6.2.0")
            .join("990000_product")
            .join("general")
            .join(db)
            .join("V4.6.2.0_990000_00_11__insert_AUTH_FUNCTION_TDSQL_[4.6.2.0].groovy");
        assert!(target.is_file(), "target missing: {}", target.display());
    }

    // 第二个脚本：触发删除源文件后的失败注入，验证自动回滚。
    let second = "2026-04-01_测试_02__insert_AUTH_FUNCTION_TDSQL_[4.6.2.0].groovy";
    write(&temp_dir.join(second), SAMPLE);
    let scan = crate::scan::scan_product(&profile, &storage, "25_user_extension").unwrap();
    let item = scan
        .scripts
        .iter()
        .find(|script| script.file_name == second)
        .unwrap();
    storage
        .set_mark(&root_id(&profile.root_path), second, &item.sha256, "passed")
        .unwrap();
    let plan = preview_archive(PreviewContext {
        profile: &profile,
        engine: &engine,
        storage: &storage,
        data_dir: &data_dir,
        product: "25_user_extension",
        relative_paths: &[second.to_string()],
        module_child: "990000_product",
        general_child: "general",
    })
    .unwrap();
    assert!(plan.conflicts.is_empty(), "conflicts: {:?}", plan.conflicts);
    std::env::set_var("SQL_ARCHIVE_FAILPOINT", "after_source_delete");
    let run = crate::executor::confirm_archive(
        &storage,
        &data_dir,
        ConfirmArchiveInput {
            plan_id: plan.id,
            confirmed: true,
            tenant_ack_paths: Vec::new(),
        },
    )
    .unwrap();
    std::env::remove_var("SQL_ARCHIVE_FAILPOINT");
    assert_eq!(run.state, "rolled_back", "run error: {:?}", run.error);
    assert!(temp_dir.join(second).is_file(), "source should be restored");
    let failed_target = root
        .join("nontax-flyway-db")
        .join("nontax-flyway-db-1.0.3")
        .join("config")
        .join("nontax")
        .join("db")
        .join("migration")
        .join("25_user_extension")
        .join("4.6.2.0")
        .join("990000_product")
        .join("general")
        .join("mysql")
        .join("V4.6.2.0_990000_00_12__insert_AUTH_FUNCTION_TDSQL_[4.6.2.0].groovy");
    assert!(
        !failed_target.exists(),
        "rolled back target must be removed"
    );
    let _ = fs::remove_dir_all(&root);
}
