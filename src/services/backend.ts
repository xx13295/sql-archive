import { invoke } from "@tauri-apps/api/core";
import type { IpcCommands } from "../contracts/ipc";
import type { ConfirmArchiveInput, PreviewArchiveInput } from "../contracts/ipc";

type CommandName = keyof IpcCommands;

export function command<N extends CommandName>(
  name: N,
  args?: IpcCommands[N]["args"]
): Promise<IpcCommands[N]["result"]> {
  return invoke<IpcCommands[N]["result"]>(name, args as Record<string, unknown> | undefined);
}

export const backend = {
  info: () => command("get_app_info"),
  settings: () => command("get_archive_settings"),
  saveSettings: (settings: IpcCommands["save_archive_settings"]["args"]["settings"]) =>
    command("save_archive_settings", { settings }),
  inspectRoot: (rootPath: string) => command("inspect_archive_root", { rootPath }),
  scan: (rootPath: string, product: string) =>
    command("scan_temp_scripts", { rootPath, product }),
  validate: (rootPath: string, product: string, relativePaths: string[]) =>
    command("validate_temp_scripts", { input: { rootPath, product, relativePaths } }),
  setMark: (
    rootPath: string,
    relativePath: string,
    sha256: string,
    mark: IpcCommands["set_script_mark"]["args"]["mark"]
  ) => command("set_script_mark", { rootPath, relativePath, sha256, mark }),
  readScript: (rootPath: string, relativePath: string) =>
    command("read_script_content", { rootPath, relativePath }),
  preview: (input: PreviewArchiveInput) =>
    command("preview_archive", { input }),
  confirm: (input: ConfirmArchiveInput) =>
    command("confirm_archive", { input }),
  run: (runId: string) => command("get_archive_run", { runId }),
  history: (rootPath: string, limit = 50) =>
    command("list_archive_runs", { rootPath, limit }),
  startupWarnings: () => command("take_startup_warnings"),
  openPath: (rootPath: string, path: string, reveal = false) =>
    command("open_archive_path", { rootPath, path, reveal }),
  testJava: (javaPath?: string | null) => command("test_java", { javaPath })
};