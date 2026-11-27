import type {
  AppInfo,
  ArchivePlan,
  ArchiveRun,
  ArchiveSettings,
  HistorySummary,
  RootProfile,
  ScriptMark,
  TempScanResult,
  ValidationResult
} from "./models";

export interface PreviewArchiveInput {
  rootPath: string;
  product: string;
  relativePaths: string[];
  moduleChild?: string | null;
  generalChild?: string | null;
}

export interface ConfirmArchiveInput {
  planId: string;
  confirmed: boolean;
  tenantAckPaths: string[];
}

export interface ValidateInput {
  rootPath: string;
  product: string;
  relativePaths: string[];
}

export interface IpcCommands {
  get_app_info: { args: undefined; result: AppInfo };
  get_archive_settings: { args: undefined; result: ArchiveSettings };
  save_archive_settings: { args: { settings: ArchiveSettings }; result: ArchiveSettings };
  inspect_archive_root: { args: { rootPath: string }; result: RootProfile };
  scan_temp_scripts: { args: { rootPath: string; product: string }; result: TempScanResult };
  validate_temp_scripts: { args: { input: ValidateInput }; result: Record<string, ValidationResult> };
  set_script_mark: {
    args: { rootPath: string; relativePath: string; sha256: string; mark: ScriptMark };
    result: void;
  };
  read_script_content: {
    args: { rootPath: string; relativePath: string };
    result: { content: string; encoding: string; hasBom: boolean };
  };
  preview_archive: { args: { input: PreviewArchiveInput }; result: ArchivePlan };
  confirm_archive: { args: { input: ConfirmArchiveInput }; result: ArchiveRun };
  get_archive_run: { args: { runId: string }; result: ArchiveRun };
  list_archive_runs: {
    args: { rootPath: string; limit?: number };
    result: HistorySummary[];
  };
  take_startup_warnings: { args: undefined; result: string[] };
  open_archive_path: {
    args: { rootPath: string; path: string; reveal?: boolean };
    result: void;
  };
  test_java: { args: { javaPath?: string | null }; result: import("./models").JavaStatus };
}