#!/usr/bin/env node
// 创建用于验证的仓库沙箱副本；只复制归档需要的目录，不复制 .git。
// 用法：node scripts/make-sandbox.mjs [源根目录] [目标目录] [产品目录名...]
import { cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const defaultSource = "D:\\IDEA-Workspace\\git\\baoxiao2\\saas-database-extension\\运营支撑门户";
const source = resolve(process.argv[2] ?? defaultSource);
const target = resolve(process.argv[3] ?? join(here, "..", ".sandbox", "portal"));
const products = process.argv.slice(4);
if (products.length === 0) products.push("02_value_added", "25_user_extension");

if (!existsSync(source)) {
  console.error(`源目录不存在：${source}`);
  process.exit(1);
}
rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });

function copy(relative) {
  const from = join(source, relative);
  if (!existsSync(from)) return;
  cpSync(from, join(target, relative), { recursive: true });
}

copy("tool");
copy("temp");
for (const product of products) {
  copy(join("temp", product));
  const migration = join("nontax-flyway-db", "nontax-flyway-db-1.0.3", "config", "nontax", "db", "migration", product);
  copy(migration);
}
mkdirSync(join(target, "backup"), { recursive: true });
for (const name of ["saas-sharding.yml.vm", "saas-sharding.yml"]) copy(name);

writeFileSync(
  join(target, ".sandbox-readme.txt"),
  [
    "本目录是 sql-archive 的验证沙箱，可随时删除。",
    `来源：${source}`,
    `产品：${products.join(", ")}`,
    "任何归档操作都只会改动本目录，不会改动真实仓库。"
  ].join("\r\n"),
  "utf8"
);
console.log(`sandbox ready: ${target}`);