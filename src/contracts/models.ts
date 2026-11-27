export type PlatformKind = "portal" | "industry";
export type DbType = "ORACLE" | "TDSQL";
export type ScriptMark = "pending" | "passed" | "returned";
export type ValidationStatus = "ok" | "warning" | "error";
export type RunState = "running" | "committed" | "failed" | "rolled_back" | "needs_attention";

export interface AppInfo {
  version: string;
  dataDir: string;
  portable: boolean;
}

export interface ArchiveSettings {
  workspaceRoot: string | null;
  product: string | null;
  javaPath: string | null;
  engineDir: string | null;
  checkRulePath: string | null;
  shardingPath: string | null;
  moduleChild: string;
  generalChild: string;
  confirmTenantWarning: boolean;
}

export interface JavaStatus {
  available: boolean;
  path: string | null;
  version: string | null;
  message: string | null;
}

export interface EngineJar {
  name: string;
  sha256: string;
}

export interface EngineStatus {
  available: boolean;
  jarDir: string | null;
  jars: EngineJar[];
  message: string | null;
}

export interface Issue {
  severity: "info" | "warning" | "error";
  code: string;
  message: string;
}

export interface ProductOption {
  displayName: string;
  dirName: string;
}

export interface RootProfile {
  rootPath: string;
  platformKind: PlatformKind;
  flywayRoot: string | null;
  tempRoot: string;
  backupRoot: string;
  configDir: string | null;
  engineDir: string | null;
  checkRulePath: string | null;
  shardingPath: string | null;
  products: ProductOption[];
  tdsqlTargets: string[];
  oracleTargets: string[];
  baselineByProduct: Record<string, string>;
  defaultProductDir: string | null;
  java: JavaStatus;
  engine: EngineStatus;
  issues: Issue[];
  fingerprint: string;
}

export interface VersionInfo {
  version: string;
  latestSequence: number;
  dbDirs: string[];
}

export interface ScriptContent {
  content: string;
  encoding: string;
  hasBom: boolean;
}

export interface ParsedScriptName {
  date: string;
  person: string;
  sourceSeq: string;
  description: string;
  dbType: DbType;
  version: string;
  extension: string;
}

export interface ValidationResult {
  status: ValidationStatus;
  errors: string[];
  warnings: string[];
  tenantCodeWarning: boolean;
  checkedAt: string;
}

export interface ScriptItem {
  relativePath: string;
  repoRelativePath: string;
  fileName: string;
  sizeBytes: number;
  modifiedAt: string;
  sha256: string;
  parsed: ParsedScriptName | null;
  parseError: string | null;
  mark: ScriptMark;
  validation: ValidationResult | null;
  groupKey: string | null;
  pairState: "paired" | "missing_pair" | "unexpected_db" | "single";
}

export interface ScriptGroup {
  key: string;
  date: string;
  person: string;
  sourceSeq: string;
  description: string;
  version: string;
  dbTypes: DbType[];
  scripts: string[];
}

export interface TempScanResult {
  product: string;
  tempDir: string;
  scripts: ScriptItem[];
  groups: ScriptGroup[];
  issues: string[];
  baselineVersion: string | null;
  versions: string[];
}

export interface ArchiveTarget {
  dbDir: string;
  channel: string;
  supported: boolean;
  message: string | null;
}

export interface TargetFile {
  dbDir: string;
  path: string;
  fileName: string;
  channel: string;
  byteSize: number;
  sha256: string;
}

export interface PlanItem {
  groupKey: string;
  sourcePath: string;
  sourceRepoRelativePath: string;
  sourceFileName: string;
  sourceDb: DbType;
  sourceVersion: string;
  sequence: number;
  backupPath: string;
  targets: TargetFile[];
  tenantCodeWarning: boolean;
  state: "ready" | "blocked";
  blockers: string[];
}

export interface ShardingChange {
  path: string;
  beforeSha: string | null;
  afterSha: string | null;
  changed: boolean;
  affectedTables: string[];
  preview: string;
}

export interface ArchivePlan {
  id: string;
  rootPath: string;
  product: string;
  createdAt: string;
  expiresAt: string;
  items: PlanItem[];
  conflicts: string[];
  warnings: string[];
  shardingChange: ShardingChange | null;
  digest: string;
  totals: {
    sourceCount: number;
    targetCount: number;
    backupCount: number;
    versions: string[];
  };
}

export interface RunItem {
  sourcePath: string;
  sourceFileName: string;
  sourceDb: DbType;
  sequence: number;
  state: string;
  backupPath: string | null;
  archivePaths: string[];
  error: string | null;
}

export interface RunStep {
  step: string;
  state: string;
  detail: string | null;
  updatedAt: string;
}

export interface ArchiveRun {
  id: string;
  product: string;
  state: RunState;
  backupDate: string;
  summary: string | null;
  error: string | null;
  startedAt: string;
  finishedAt: string | null;
  items: RunItem[];
  steps: RunStep[];
}

export interface HistorySummary {
  id: string;
  product: string;
  state: RunState;
  backupDate: string;
  versions: string;
  sourceCount: number;
  targetCount: number;
  error: string | null;
  startedAt: string;
  finishedAt: string | null;
}