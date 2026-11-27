#!/usr/bin/env node
// 构建 Windows 免安装版：一次 tauri build --no-bundle --features portable，
// 产物放到 release/ 并生成 SHA256 与构建信息。
// 注意：release/data 是免安装版的应用数据，打包时绝不能删除。
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const version = pkg.version;
const pnpm = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
const built = spawnSync(
  pnpm,
  ["tauri", "build", "--no-bundle", "--features", "portable"],
  { cwd: root, stdio: "inherit", shell: process.platform === "win32" }
);
if (built.status !== 0) {
  console.error("tauri build failed");
  process.exit(built.status ?? 1);
}
const sourceExe = join(root, "src-tauri", "target", "release", "sql-archive.exe");
if (!existsSync(sourceExe)) {
  console.error(`未找到构建产物：${sourceExe}`);
  process.exit(1);
}

const releaseDir = join(root, "release");
mkdirSync(releaseDir, { recursive: true });
const targetName = `数据库脚本归档工具-免安装版-${version}-x64.exe`;
const targetExe = join(releaseDir, targetName);

// 只清理旧的同系列 exe，不删除 release/data（应用数据）。
// 旧版本可能仍在运行导致文件被占用；占用时保留旧文件，新版本使用新文件名。
for (const entry of readdirSync(releaseDir)) {
  if (!entry.startsWith("数据库脚本归档工具-免安装版-") || !entry.endsWith(".exe")) continue;
  if (entry === targetName) continue;
  try {
    rmSync(join(releaseDir, entry), { force: true });
  } catch {
    // 旧版本仍在运行，保留。
  }
}

copyFileSync(sourceExe, targetExe);
const bytes = readFileSync(targetExe);
const sha256 = createHash("sha256").update(bytes).digest("hex");

const sums = readdirSync(releaseDir)
  .filter((entry) => entry.startsWith("数据库脚本归档工具-免安装版-") && entry.endsWith(".exe"))
  .sort()
  .map((entry) => {
    const hash = createHash("sha256").update(readFileSync(join(releaseDir, entry))).digest("hex");
    return `${hash} *${entry}`;
  });
writeFileSync(join(releaseDir, "SHA256SUMS.txt"), sums.join("\n") + "\n");

const info = [
  `name=sql-archive`,
  `version=${version}`,
  `artifact=${targetName}`,
  `sha256=${sha256}`,
  `size=${bytes.length}`,
  `builtAt=${new Date().toISOString()}`,
  `mode=portable`,
  `note=需要本机 Java 8+ 与所选仓库 tool/java 引擎`
].join("\n") + "\n";
writeFileSync(join(releaseDir, "build-info.txt"), info);
writeFileSync(join(releaseDir, `build-info-${version}.txt`), info);

console.log(`release ready: ${targetExe}`);
console.log(`sha256: ${sha256}`);