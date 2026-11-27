# 数据库脚本归档工具（sql-archive）

用于运营支撑门户 / 行业应用数据库脚本的**整理与归档**桌面工具。替代旧版 aardio「数据库脚本工具.exe」的整理归档主链路，并把过程做成可校验、可预览、失败可回滚的流程。

- 技术栈：Tauri 2 + Rust + Vue 3 + Element Plus + Pinia + SQLite
- 产品标识：`cn.bosicloud.sqlarchive`
- 当前版本：`0.1.3`（首版范围：脚本整理与归档）

## 更新记录

### 0.1.3

- 确认归档弹窗的租户编码确认默认全选，并支持一键勾选/取消
- 确认归档弹窗每个脚本提供「详情」弹窗，可查看脚本全文
- 移除冗余的「一键确认候选」按钮
- 脚本详情面板移除内嵌长文本，改为「查看脚本详情」弹窗
- 读取脚本内容统一使用当前仓库根的 `temp/...` 相对路径
- 点击「通过」时先校验选中脚本；无阻断错误则自动标记通过并直接生成归档预览

### 0.1.2

- 支持 `temp/<产品>/oracle|tdsql|mysql` 等子目录（税局行业平台结构）
- 点击脚本即可读取并预览当前脚本内容（按当前仓库根的 `temp/...` 相对路径解析，兼容 oracle/tdsql 子目录）
- 确认归档弹窗支持「一键勾选租户确认」并默认全选；每个脚本提供「详情」弹窗查看内容
- 脚本详情面板移除内嵌长文本，改为「查看脚本详情」弹窗
- 修复确认归档弹窗告警文字溢出

## 首版能力

1. 读取仓库根目录（`运营支撑门户` 或 `行业应用`）的旧配置：
   - `tool/config/sys-config.table`
   - `tool/config/user-config.table`
   - `tool/config/check-rule.yml`
2. 扫描 `temp/<产品>`（支持平铺文件，也支持 `oracle/`、`tdsql/`、`mysql/` 等子目录），校验文件名、UTF-8、64 KiB 大小、版本基线与 ORACLE/TDSQL 配对。
3. 调用仓库自带的 Java 引擎（`tool/java/sql-translate*.jar`）执行 SQL 校验与多方言转换。
4. 点击表格行可在右侧查看脚本信息，并点「查看脚本详情」弹出脚本全文。
4. 生成归档预览：变更组共享序号、各目标库完整路径、备份路径、分表模板差异、冲突与警告。
5. 确认后执行归档：
   - 新增 Flyway 归档文件（绝不覆盖）
   - 源脚本复制到 `backup/<产品>/<yyyyMMdd>/<原文件名>`（与旧工具一致，不保留子目录）并从 `temp` 移走
   - 按需更新 `saas-sharding.yml.vm`
   - 任一步失败自动回滚本次改动
6. 操作记录（SQLite）与启动时中断恢复。

**不包含**（后续迭代）：脚本回退、批量脚本迁移、版本号变更、Git pull/commit/push。

## 环境要求

- Windows 10/11，WebView2 运行时
- **Java 8 或更高版本**（可用 `JAVA_HOME` 或 PATH，也可在设置中指定 `java.exe`）
- 仓库根目录下必须存在 `tool/java` 引擎目录与 `tool/config/check-rule.yml`

> 引擎 JAR 不从本应用打包，而是每次从当前仓库的 `tool/java` 读取并复制到 ASCII 工作区。
> 门户与行业的引擎 JAR 不同，不能混用。

## 使用流程

1. 打开应用，选择 `运营支撑门户` 或 `行业应用` 根目录。
2. 选择产品，点击「列表刷新」。
3. 点击「校验」：文件名 / 版本 / 配对 / 引擎 SQL 校验会显示在表格的「校验」列。
4. 勾选脚本后点击「通过」：工具会先校验，选中脚本无阻断错误时自动标记通过并直接打开归档预览；有问题时停留在校验结果。点击「退回」则退回选中脚本。
5. 点击「生成归档预览」：
   - 检查变更组序号、目标文件、备份路径、分表模板差异
   - 有冲突时不能执行
6. 点击「确认并归档」，阅读确认项并勾选；含租户编码警告的脚本需要逐文件确认。
7. 执行结果显示每一步状态；失败会自动回滚并说明结果。

## 归档规则

### 源文件命名

```
yyyy-MM-dd_姓名_两位顺序号__英文备注_DB_[版本].groovy
```

- `DB` 为 `ORACLE` 或 `TDSQL`；支持 `.groovy` / `.sql`
- 顺序号与备注之间必须是双下划线 `__`
- 文件必须为 UTF-8，单文件不超过 64 KiB
- 行业应用同一变更必须同时存在 ORACLE 与 TDSQL 两个脚本

### 归档文件命名

```
V<版本>_990000_00_<序号>__<备注>_<DB>_[<版本>].groovy
```

- 写入 `<根目录>/nontax-flyway-db/*/config/nontax/db/migration/<产品>/<版本>/990000_product/general/<目标库>/`
- 序号取该版本、该模块下**所有数据库子目录**（含未在配置中列出的目录）的最大序号 + 1
- 同一变更组的 ORACLE/TDSQL 以及所有目标库共享同一个序号
- `DB` 取源脚本类型，不随目标库变化

### 目标库

- 来源 TDSQL → `tdsqlDatabaseDirList`（门户：mysql,h2；行业：mysql,tdsql,drds）
- 来源 ORACLE → `oracleDatabaseDirList`（行业：oracle,dm,kingbase,highgo,vastbase,gbase8c,polardb_o,gaussdb,gaussdb_d）
- 没有对应引擎通道的目录会明确提示并跳过，不会静默生成错误脚本
- 引擎报错的目标库会阻断该脚本归档

### 版本守卫

- `sys-config.table` 中 `releaseVersion`（门户）或 `baseLineVersion`（行业）配置了产品基线时：脚本版本低于基线阻断，等于/高于基线允许
- 未配置基线时不做版本下限校验，避免误伤仍在归档的旧版本脚本

### 分表模板

- 只有检测到分表建表语句（`CREATE TABLE`）时才更新 `saas-sharding.yml.vm`
- 写入前会备份旧文件到应用数据目录；失败回滚时恢复
- 引擎对非分表脚本产生的 YAML 归一化不会触发写入

## 安全设计

- 预览阶段把翻译结果写入应用数据目录的 staging，并计算 SHA-256；确认执行时不再调用 Java
- 执行前重新校验：源文件 hash、配置 hash、引擎 fingerprint、分表模板 hash、目录内最大序号、目标文件是否已存在
- 归档文件与备份文件使用 `create_new` 语义，绝不覆盖
- 任一失败按逆序回滚：删除新建归档文件、从备份恢复源脚本、恢复分表模板
- 回滚失败标记 `needs_attention` 并在界面提示人工核实，绝不伪装成功
- 启动时检测上次中断的运行并尝试恢复

## 配置兼容

- `sys-config.table`、`check-rule.yml` 只读
- `user-config.table` 在保存设置 / 切换产品时写回 `totalPath` 与 `productListSelectIndex`（该文件已被 `.gitignore` 忽略）
- 解析器支持 UTF-8 BOM、CRLF、多行值、裸数字、末尾反斜杠路径与未知键

## 开发

环境：Node.js 22+、pnpm 11+、Rust 1.90+、JDK 8+

```powershell
pnpm install
pnpm test
pnpm check
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

开发启动：

```powershell
pnpm tauri dev
```

沙箱验证时可用 `SQL_ARCHIVE_WORKSPACE` 指定启动时载入的仓库根目录，用
`SQL_ARCHIVE_DATA_DIR` 覆盖应用数据目录，避免污染真实仓库与日常数据。

创建验证沙箱（只读复制真实仓库的一段，可随时删除）：

```powershell
node scripts/make-sandbox.mjs
```

构建免安装版：

```powershell
pnpm package:release
```

产物位于 `release/数据库脚本归档工具-免安装版-0.1.3-x64.exe`，并生成 `SHA256SUMS.txt`、`build-info.txt`。
免安装版的数据目录固定在 EXE 同目录 `data/sql-archive-v1`；开发/安装版使用系统应用数据目录。

### macOS 构建

`.github/workflows/build-macos.yml` 在 GitHub Actions 上完成 macOS 构建，矩阵覆盖两种架构：

- `macos-15`：Apple Silicon，`aarch64-apple-darwin`
- `macos-15-intel`：Intel，`x86_64-apple-darwin`

每个架构产出 `.dmg` 与 `.app`，并附带 `SHA256SUMS.txt`。未配置 Apple 开发者证书时使用 ad-hoc 签名（`APPLE_SIGNING_IDENTITY="-"`），产物可直接安装验证。

本地在 macOS 上构建（需 Xcode Command Line Tools）：

```bash
pnpm install
pnpm tauri build --bundles app,dmg
```

产物位于 `src-tauri/target/release/bundle/`。`scripts/package-release.mjs` 只用于 Windows 免安装版，macOS 不在其处理范围内。

### Java 桥接

`java-bridge/` 包含 `ArchiveBridge` 源码与构建脚本。它读取 UTF-8 请求文件，调用仓库自带的 `sql-translate` 引擎，输出翻译结果与分表配置：

```powershell
cd java-bridge
./build.ps1
```

编译产物 `dist/sql-archive-bridge.jar` 会随应用打包；如果仓库引擎 JAR 更新，应用会自动重新复制到 ASCII 工作区并更新 fingerprint。

## 工程结构

```
src/                     Vue 3 界面、Pinia 状态、Tauri invoke 封装
src-tauri/src/           Rust：配置解析、扫描校验、计划/执行、Java 桥接、SQLite
src-tauri/fixtures/      端到端测试脚本样例
java-bridge/             Java 桥接源码与 jar
scripts/                 沙箱与发布脚本
tests/                   前端 vitest
```

## 已知限制

- 首版只覆盖整理归档，不提供脚本回退 / 迁移 / 版本变更入口
- 依赖本机 Java 8+；未安装 Java 时校验与归档会被阻断，但文件名/版本等本地检查仍可用
- 分表模板只在检测到 `CREATE TABLE` 时更新；分表 ALTER 脚本不会改写模板
- 归档计划默认 30 分钟过期，期间源文件/目标目录/配置变化都会强制重新预览