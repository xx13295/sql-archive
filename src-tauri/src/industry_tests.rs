use std::fs;
use std::path::{Path, PathBuf};

use crate::java_bridge::JavaEngine;
use crate::models::{ArchiveSettings, ConfirmArchiveInput};
use crate::plan::{preview_archive, PreviewContext};
use crate::storage::{root_id, Storage};

const INDUSTRY_ORACLE: &str = include_str!("../fixtures/industry_oracle.groovy");
const INDUSTRY_TDSQL: &str = include_str!("../fixtures/industry_tdsql.groovy");
const ORACLE_NAME: &str =
    "2026-05-08_赖钰__01__insert_table_saas_base_tsbdm_mult_dict_def_ORACLE_[4.5.3.0].groovy";
const TDSQL_NAME: &str =
    "2026-05-08_赖钰__01__insert_table_saas_base_tsbdm_mult_dict_def_TDSQL_[4.5.3.0].groovy";

fn engine_dir() -> Option<PathBuf> {
    let candidate = std::env::var("SQL_ARCHIVE_TEST_INDUSTRY_ENGINE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(
                r"D:\IDEA-Workspace\git\baoxiao2\saas-database-extension\行业应用\tool\java",
            )
        });
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
    let check_rule = engine.parent()?.join("config").join("check-rule.yml");
    if !check_rule.is_file() {
        return None;
    }
    let root = std::env::temp_dir().join(format!(
        "sql-archive-industry-{}",
        uuid::Uuid::new_v4().simple()
    ));
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
    fs::create_dir_all(root.join("tool").join("config")).unwrap();
    fs::copy(
        &check_rule,
        root.join("tool").join("config").join("check-rule.yml"),
    )
    .unwrap();
    let root_text = root.to_string_lossy().to_string();
    write(
        &root
            .join("tool")
            .join("config")
            .join("sys-config.table"),
        "{\r\nproductList=\"标准产品-01_standard\";\r\noracleDatabaseDirList=\"oracle,dm,kingbase,highgo,vastbase,gbase8c,polardb_o,gaussdb,gaussdb_d\";\r\ntdsqlDatabaseDirList=\"mysql,tdsql,drds\";\r\nbaseLineVersion=\"01_standard=1.14.0\"\r\n}",
    );
    write(
        &root.join("tool").join("config").join("user-config.table"),
        &format!("{{\r\nproductListSelectIndex=1;\r\ntotalPath=\"{root_text}\";\r\n}}"),
    );
    write(
        &root.join("saas-sharding.yml.vm"),
        "#set ( $d = \"$\")\nspring: null\n",
    );
    let temp = root.join("temp").join("01_standard");
    fs::create_dir_all(&temp).unwrap();
    fs::create_dir_all(root.join("backup").join("01_standard")).unwrap();
    let general = root
        .join("nontax-flyway-db")
        .join("nontax-flyway-db-1.0.3")
        .join("config")
        .join("nontax")
        .join("db")
        .join("migration")
        .join("01_standard")
        .join("4.5.3.0")
        .join("990000_product")
        .join("general");
    let oracle_dirs = [
        "oracle",
        "dm",
        "kingbase",
        "highgo",
        "vastbase",
        "gbase8c",
        "polardb_o",
        "gaussdb",
        "gaussdb_d",
    ];
    let tdsql_dirs = ["mysql", "tdsql", "drds"];
    for db in oracle_dirs.iter().chain(tdsql_dirs.iter()) {
        let dir = general.join(db);
        fs::create_dir_all(&dir).unwrap();
        if *db == "oracle" || *db == "mysql" {
            let db_token = if *db == "oracle" { "ORACLE" } else { "TDSQL" };
            fs::write(
                dir.join(format!(
                    "V4.5.3.0_990000_00_10__seed_{db_token}_[4.5.3.0].groovy"
                )),
                b"// seed",
            )
            .unwrap();
        }
    }
    // 税局行业平台使用 temp/<产品>/oracle|tdsql 子目录，验证递归扫描。
    write(&temp.join("oracle").join(ORACLE_NAME), INDUSTRY_ORACLE);
    write(&temp.join("tdsql").join(TDSQL_NAME), INDUSTRY_TDSQL);
    Some(root)
}

#[test]
fn industry_pipeline_translates_all_dialects() {
    let _guard = crate::PIPELINE_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let Some(root) = setup_sandbox() else {
        eprintln!("skip: industry Java engine unavailable");
        return;
    };
    let Some(bridge) = bridge_jar() else {
        eprintln!("skip: bridge jar unavailable");
        return;
    };
    std::env::set_var("SQL_ARCHIVE_BRIDGE_DIR", root.join("bridge"));
    let data_dir = root.join("data");
    let storage = Storage::open(&data_dir).unwrap();
    let settings = ArchiveSettings {
        workspace_root: Some(root.to_string_lossy().to_string()),
        product: Some("01_standard".into()),
        ..ArchiveSettings::default()
    };
    let profile = crate::workspace::inspect_root(&root.to_string_lossy(), &settings).unwrap();
    assert!(profile.java.available);
    assert!(profile.engine.available);
    let scan = crate::scan::scan_product(&profile, &storage, "01_standard").unwrap();
    assert_eq!(scan.scripts.len(), 2, "issues: {:?}", scan.issues);
    assert!(scan
        .scripts
        .iter()
        .all(|script| script.pair_state == "paired"));
    for name in [ORACLE_NAME, TDSQL_NAME] {
        let item = scan
            .scripts
            .iter()
            .find(|script| script.file_name == name)
            .unwrap_or_else(|| panic!("missing {name}"));
        storage
            .set_mark(
                &root_id(&profile.root_path),
                &item.relative_path,
                &item.sha256,
                "passed",
            )
            .unwrap();
    }
    let oracle_rel = format!("oracle/{ORACLE_NAME}");
    let tdsql_rel = format!("tdsql/{TDSQL_NAME}");
    let oracle_content =
        crate::read_script_content_at(&profile, &format!("temp/01_standard/{oracle_rel}"))
            .expect("read nested script content");
    assert!(oracle_content
        .content
        .contains("SAAS_BASE_TSBDM_MULT_DICT_DEF"));
    let engine = JavaEngine::prepare(&profile, &bridge).unwrap();
    let plan = preview_archive(PreviewContext {
        profile: &profile,
        engine: &engine,
        storage: &storage,
        data_dir: &data_dir,
        product: "01_standard",
        relative_paths: &[oracle_rel.clone(), tdsql_rel.clone()],
        module_child: "990000_product",
        general_child: "general",
    })
    .unwrap();
    assert!(plan.conflicts.is_empty(), "conflicts: {:?}", plan.conflicts);
    assert_eq!(plan.items.len(), 2);
    let oracle_item = plan
        .items
        .iter()
        .find(|item| item.source_db == "ORACLE")
        .unwrap();
    let tdsql_item = plan
        .items
        .iter()
        .find(|item| item.source_db == "TDSQL")
        .unwrap();
    assert_eq!(oracle_item.sequence, 11);
    assert_eq!(tdsql_item.sequence, 11);
    assert_eq!(oracle_item.targets.len(), 9, "oracle dialects");
    assert_eq!(tdsql_item.targets.len(), 3, "tdsql dialects");
    let ack = plan
        .items
        .iter()
        .filter(|item| item.tenant_code_warning)
        .map(|item| item.source_file_name.clone())
        .collect::<Vec<_>>();
    let run = crate::executor::confirm_archive(
        &storage,
        &data_dir,
        ConfirmArchiveInput {
            plan_id: plan.id,
            confirmed: true,
            tenant_ack_paths: ack,
        },
    )
    .unwrap();
    assert_eq!(run.state, "committed", "run error: {:?}", run.error);
    let general = root
        .join("nontax-flyway-db")
        .join("nontax-flyway-db-1.0.3")
        .join("config")
        .join("nontax")
        .join("db")
        .join("migration")
        .join("01_standard")
        .join("4.5.3.0")
        .join("990000_product")
        .join("general");
    let target_count = general
        .read_dir()
        .unwrap()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            fs::read_dir(entry.path())
                .unwrap()
                .flatten()
                .filter(|file| file.file_name().to_string_lossy().contains("_00_11__"))
                .count()
        })
        .sum::<usize>();
    assert_eq!(target_count, 12, "expected 9 oracle + 3 tdsql targets");
    assert!(!root
        .join("temp")
        .join("01_standard")
        .join(&oracle_rel)
        .exists());
    assert!(!root
        .join("temp")
        .join("01_standard")
        .join(&tdsql_rel)
        .exists());
    let _ = fs::remove_dir_all(&root);
}
