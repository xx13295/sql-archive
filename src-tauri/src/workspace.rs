use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config_table::ConfigTable;
use crate::error::{AppError, AppResult};
use crate::filename::{channel_for_db_dir, is_version, parse_archive_name};
use crate::java_env::detect_java;
use crate::models::{
    ArchiveSettings, EngineJar, EngineStatus, Issue, ProductOption, RootProfile, VersionInfo,
};
use crate::sequence;
use crate::util::{sha256_bytes, to_display};

#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    pub sys_path: Option<PathBuf>,
    pub user_path: Option<PathBuf>,
    pub sys: Option<ConfigTable>,
    pub user: Option<ConfigTable>,
}

pub fn inspect_root(root_path: &str, settings: &ArchiveSettings) -> AppResult<RootProfile> {
    let root = Path::new(root_path)
        .canonicalize()
        .map_err(|error| AppError::user("ROOT_NOT_FOUND", format!("根目录不可访问：{error}")))?;
    if !root.is_dir() {
        return Err(AppError::user("ROOT_NOT_DIR", "所选路径不是目录。"));
    }
    let temp_root = root.join("temp");
    let backup_root = root.join("backup");
    let config = load_workspace_config(&root);
    let mut issues = Vec::new();
    if !temp_root.is_dir() {
        issues.push(Issue::error("TEMP_MISSING", "缺少 temp 脚本提交目录。"));
    }
    if !backup_root.is_dir() {
        issues.push(Issue::warning(
            "BACKUP_MISSING",
            "缺少 backup 目录，归档时会尝试创建。",
        ));
    }
    let flyway_root = resolve_flyway_root(&root);
    if flyway_root.is_none() {
        issues.push(Issue::error(
            "FLYWAY_MISSING",
            "未找到 nontax-flyway-db/*/config/nontax/db/migration 目录。",
        ));
    }

    let sys = config.sys.as_ref();
    let products = collect_products(sys, &temp_root, &mut issues);
    let tdsql_targets = sys
        .and_then(|table| table.get("tdsqlDatabaseDirList"))
        .map(ConfigTable::split_list)
        .unwrap_or_default();
    let oracle_targets = sys
        .and_then(|table| table.get("oracleDatabaseDirList"))
        .map(ConfigTable::split_list)
        .unwrap_or_default();
    for target in tdsql_targets.iter().chain(oracle_targets.iter()) {
        if channel_for_db_dir(target).is_none() {
            issues.push(Issue::warning(
                "TARGET_UNSUPPORTED",
                format!("目标库目录 {target} 没有对应的引擎转换通道，归档时将跳过。"),
            ));
        }
    }
    let baseline_text = sys.and_then(|table| {
        table
            .get("releaseVersion")
            .or_else(|| table.get("baseLineVersion"))
    });
    let baseline_by_product = parse_baselines(baseline_text);
    let platform_kind = if oracle_targets.is_empty() {
        "portal"
    } else {
        "industry"
    };

    let default_product_dir = default_product(&config, &products, settings);
    if products.is_empty() {
        issues.push(Issue::error(
            "PRODUCTS_EMPTY",
            "未读到任何产品目录，请检查 sys-config.table 与 temp。",
        ));
    }
    if let Some(product) = &default_product_dir {
        let product_dir = temp_root.join(product);
        if !product_dir.is_dir() {
            issues.push(Issue::warning(
                "PRODUCT_DIR_MISSING",
                format!("产品目录 temp/{product} 不存在。"),
            ));
        }
    }

    let engine_dir = settings
        .engine_dir
        .as_deref()
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .or_else(|| {
            let candidate = root.join("tool").join("java");
            candidate.is_dir().then_some(candidate)
        });
    let engine = inspect_engine(engine_dir.as_deref(), &mut issues);

    let check_rule_path = settings
        .check_rule_path
        .as_deref()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| {
            let candidate = root.join("tool").join("config").join("check-rule.yml");
            candidate.is_file().then_some(candidate)
        });
    if check_rule_path.is_none() {
        issues.push(Issue::error(
            "CHECK_RULE_MISSING",
            "缺少 check-rule.yml，无法执行引擎校验。",
        ));
    }

    let sharding_path = settings
        .sharding_path
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| {
            let candidate = root.join("saas-sharding.yml.vm");
            candidate.is_file().then_some(candidate)
        });

    let java = detect_java(settings.java_path.as_deref());
    if !java.available {
        issues.push(Issue::error(
            "JAVA_UNAVAILABLE",
            java.message
                .clone()
                .unwrap_or_else(|| "未找到 Java。".into()),
        ));
    }

    if let Some(user) = config.user.as_ref() {
        if let Some(total_path) = user.get("totalPath") {
            let configured = Path::new(total_path);
            if configured.is_dir()
                && configured
                    .canonicalize()
                    .map(|path| path != root)
                    .unwrap_or(true)
            {
                issues.push(Issue::info(
                    "USER_CONFIG_MISMATCH",
                    "user-config.table 的 totalPath 与当前根目录不同，保存后会同步。",
                ));
            }
        }
    }

    let fingerprint = compute_fingerprint(
        &root,
        &config,
        &tdsql_targets,
        &oracle_targets,
        &baseline_by_product,
        &engine,
        check_rule_path.as_deref(),
    );

    Ok(RootProfile {
        root_path: to_display(&root),
        platform_kind: platform_kind.to_string(),
        flyway_root: flyway_root.as_deref().map(to_display),
        temp_root: to_display(&temp_root),
        backup_root: to_display(&backup_root),
        config_dir: config
            .sys_path
            .as_deref()
            .and_then(Path::parent)
            .map(to_display),
        engine_dir: engine_dir.as_deref().map(to_display),
        check_rule_path: check_rule_path.as_deref().map(to_display),
        sharding_path: sharding_path.as_deref().map(to_display),
        products,
        tdsql_targets,
        oracle_targets,
        baseline_by_product,
        default_product_dir,
        java,
        engine,
        issues,
        fingerprint,
    })
}

pub fn load_workspace_config(root: &Path) -> WorkspaceConfig {
    let config_dir = root.join("tool").join("config");
    let sys_path = config_dir.join("sys-config.table");
    let user_path = config_dir.join("user-config.table");
    let sys = read_config(&sys_path);
    let user = read_config(&user_path);
    WorkspaceConfig {
        sys_path: sys_path.is_file().then_some(sys_path),
        user_path: user_path.is_file().then_some(user_path),
        sys,
        user,
    }
}

fn read_config(path: &Path) -> Option<ConfigTable> {
    let bytes = fs::read(path).ok()?;
    ConfigTable::parse(&bytes).ok()
}

pub fn resolve_flyway_root(root: &Path) -> Option<PathBuf> {
    let base = root.join("nontax-flyway-db");
    let mut candidates = Vec::new();
    if let Ok(entries) = fs::read_dir(&base) {
        for entry in entries.flatten() {
            let candidate = entry
                .path()
                .join("config")
                .join("nontax")
                .join("db")
                .join("migration");
            if candidate.is_dir() {
                candidates.push(candidate);
            }
        }
    }
    candidates.sort();
    candidates.into_iter().next()
}

fn collect_products(
    sys: Option<&ConfigTable>,
    temp_root: &Path,
    issues: &mut Vec<Issue>,
) -> Vec<ProductOption> {
    let mut products = Vec::new();
    if let Some(value) = sys.and_then(|table| table.get("productList")) {
        for item in ConfigTable::split_list(value) {
            let (display_name, dir_name) = match item.rsplit_once('-') {
                Some((display, dir)) if !dir.trim().is_empty() => {
                    (display.trim().to_string(), dir.trim().to_string())
                }
                _ => (item.clone(), item.clone()),
            };
            if !temp_root.join(&dir_name).is_dir() {
                issues.push(Issue::warning(
                    "PRODUCT_TEMP_MISSING",
                    format!("产品 {display_name} 的目录 temp/{dir_name} 不存在。"),
                ));
            }
            products.push(ProductOption {
                display_name,
                dir_name,
            });
        }
    }
    if products.is_empty() {
        if let Ok(entries) = fs::read_dir(temp_root) {
            let mut dirs: Vec<String> = entries
                .flatten()
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
                .collect();
            dirs.sort();
            for dir_name in dirs {
                products.push(ProductOption {
                    display_name: dir_name.clone(),
                    dir_name,
                });
            }
            if !products.is_empty() {
                issues.push(Issue::info(
                    "PRODUCTS_INFERRED",
                    "未读到 productList，产品列表由 temp 子目录推断。",
                ));
            }
        }
    }
    products
}

fn default_product(
    config: &WorkspaceConfig,
    products: &[ProductOption],
    settings: &ArchiveSettings,
) -> Option<String> {
    if let Some(product) = settings
        .product
        .as_ref()
        .filter(|product| products.iter().any(|item| &item.dir_name == *product))
    {
        return Some(product.clone());
    }
    if let Some(index) = config
        .user
        .as_ref()
        .and_then(|table| table.get("productListSelectIndex"))
        .and_then(|value| value.parse::<usize>().ok())
    {
        if index >= 1 && index <= products.len() {
            return Some(products[index - 1].dir_name.clone());
        }
    }
    products.first().map(|item| item.dir_name.clone())
}

fn parse_baselines(value: Option<&str>) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Some(value) = value else { return map };
    for part in value.split(',') {
        if let Some((product, version)) = part.split_once('=') {
            let product = product.trim();
            let version = version.trim();
            if !product.is_empty() && is_version(version) {
                map.insert(product.to_string(), version.to_string());
            }
        }
    }
    map
}

fn inspect_engine(engine_dir: Option<&Path>, issues: &mut Vec<Issue>) -> EngineStatus {
    let Some(dir) = engine_dir else {
        issues.push(Issue::error(
            "ENGINE_MISSING",
            "未找到 Java 引擎目录（默认 <根目录>/tool/java）。",
        ));
        return EngineStatus {
            available: false,
            jar_dir: None,
            jars: Vec::new(),
            message: Some("未找到引擎目录。".into()),
        };
    };
    let mut jars = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("jar"))
                != Some(true)
            {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if let Ok(bytes) = fs::read(&path) {
                jars.push(EngineJar {
                    name,
                    sha256: sha256_bytes(&bytes),
                });
            }
        }
    }
    jars.sort_by(|left, right| left.name.cmp(&right.name));
    let has_engine = jars
        .iter()
        .any(|jar| jar.name.to_ascii_lowercase().starts_with("sql-translate"));
    if !has_engine {
        issues.push(Issue::error(
            "ENGINE_JAR_MISSING",
            "引擎目录中缺少 sql-translate*.jar。",
        ));
    }
    EngineStatus {
        available: has_engine,
        jar_dir: Some(to_display(dir)),
        jars,
        message: if has_engine {
            None
        } else {
            Some("缺少 sql-translate*.jar。".into())
        },
    }
}

fn compute_fingerprint(
    root: &Path,
    config: &WorkspaceConfig,
    tdsql_targets: &[String],
    oracle_targets: &[String],
    baselines: &BTreeMap<String, String>,
    engine: &EngineStatus,
    check_rule: Option<&Path>,
) -> String {
    let mut parts = Vec::new();
    parts.push(to_display(root));
    parts.push(format!("tdsql:{tdsql_targets:?}"));
    parts.push(format!("oracle:{oracle_targets:?}"));
    parts.push(format!("baseline:{baselines:?}"));
    if let Some(path) = &config.sys_path {
        if let Ok(bytes) = fs::read(path) {
            parts.push(format!("sys:{}", sha256_bytes(&bytes)));
        }
    }
    if let Some(path) = &config.user_path {
        if let Ok(bytes) = fs::read(path) {
            parts.push(format!("user:{}", sha256_bytes(&bytes)));
        }
    }
    for jar in &engine.jars {
        parts.push(format!("jar:{}:{}", jar.name, jar.sha256));
    }
    if let Some(path) = check_rule {
        if let Ok(bytes) = fs::read(path) {
            parts.push(format!("rule:{}", sha256_bytes(&bytes)));
        }
    }
    sha256_bytes(parts.join("\n").as_bytes())
}

pub fn temp_product_dir(root: &Path, product: &str) -> PathBuf {
    root.join("temp").join(product)
}

pub fn backup_product_dir(root: &Path, product: &str, backup_date: &str) -> PathBuf {
    root.join("backup").join(product).join(backup_date)
}

pub fn migration_product_dir(flyway_root: &Path, product: &str) -> PathBuf {
    flyway_root.join(product)
}

pub fn version_general_dir(
    flyway_root: &Path,
    product: &str,
    version: &str,
    module_child: &str,
    general_child: &str,
) -> PathBuf {
    flyway_root
        .join(product)
        .join(version)
        .join(module_child)
        .join(general_child)
}

pub fn list_versions(flyway_root: &Path, product: &str) -> AppResult<Vec<VersionInfo>> {
    let product_dir = migration_product_dir(flyway_root, product);
    let mut versions = Vec::new();
    let entries = match fs::read_dir(&product_dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(versions),
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let Some(version) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !is_version(&version) {
            continue;
        }
        let mut latest = 0_u32;
        let mut db_dirs = BTreeSet::new();
        collect_version_stats(&entry.path(), 0, &mut latest, &mut db_dirs);
        versions.push(VersionInfo {
            version,
            latest_sequence: latest,
            db_dirs: db_dirs.into_iter().collect(),
        });
    }
    versions
        .sort_by(|left, right| crate::filename::compare_versions(&right.version, &left.version));
    Ok(versions)
}

fn collect_version_stats(
    dir: &Path,
    depth: usize,
    latest: &mut u32,
    db_dirs: &mut BTreeSet<String>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < 3 {
                collect_version_stats(&path, depth + 1, latest, db_dirs);
            }
        } else if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
            if let Some(parsed) = parse_archive_name(name) {
                *latest = (*latest).max(parsed.sequence);
                if depth == 3 {
                    if let Some(db_name) = dir.file_name().and_then(|value| value.to_str()) {
                        db_dirs.insert(db_name.to_string());
                    }
                }
            }
        }
    }
}

pub fn configured_baseline(profile: &RootProfile, product: &str) -> Option<String> {
    profile.baseline_by_product.get(product).cloned()
}

pub fn current_version(profile: &RootProfile, product: &str) -> Option<String> {
    profile
        .baseline_by_product
        .get(product)
        .cloned()
        .or_else(|| {
            let flyway_root = Path::new(profile.flyway_root.as_deref()?);
            list_versions(flyway_root, product)
                .ok()
                .and_then(|versions| versions.first().map(|item| item.version.clone()))
        })
}

pub fn latest_sequence_for(
    profile: &RootProfile,
    product: &str,
    version: &str,
    module_child: &str,
    general_child: &str,
) -> AppResult<(u32, Vec<(String, u32)>)> {
    let flyway_root = profile
        .flyway_root
        .as_deref()
        .ok_or_else(|| AppError::user("FLYWAY_MISSING", "未找到 Flyway 目录。"))?;
    let base = version_general_dir(
        Path::new(flyway_root),
        product,
        version,
        module_child,
        general_child,
    );
    let dirs = sequence::scan_db_dirs(&base)?;
    let latest = sequence::latest_sequence_from_dirs(&dirs);
    Ok((latest, dirs))
}

pub fn write_user_config(
    root: &Path,
    total_path: &str,
    product_index: Option<usize>,
) -> AppResult<()> {
    let path = root.join("tool").join("config").join("user-config.table");
    if !path.is_file() {
        return Ok(());
    }
    let bytes =
        fs::read(&path).map_err(|error| AppError::io("读取 user-config.table 失败", error))?;
    let mut table = ConfigTable::parse(&bytes).map_err(|error| {
        AppError::user(
            "USER_CONFIG_INVALID",
            format!("user-config.table 无法解析：{error}"),
        )
    })?;
    table.set("totalPath", total_path);
    if let Some(index) = product_index {
        table.set("productListSelectIndex", index.to_string());
    }
    crate::util::atomic_write(&path, &table.render())
}
