import { defineStore } from "pinia";
import { ref } from "vue";
import { backend } from "../services/backend";
import type { AppInfo, ArchiveSettings, RootProfile } from "../contracts/models";

export type AppArea = "archive" | "history" | "settings";

export const useAppStore = defineStore("app", () => {
  const area = ref<AppArea>("archive");
  const info = ref<AppInfo | null>(null);
  const settings = ref<ArchiveSettings | null>(null);
  const profile = ref<RootProfile | null>(null);
  const booting = ref(true);
  const startupWarnings = ref<string[]>([]);

  async function bootstrap() {
    booting.value = true;
    try {
      info.value = await backend.info();
      settings.value = await backend.settings();
      if (settings.value.workspaceRoot) {
        await loadRoot(settings.value.workspaceRoot, false);
      }
      startupWarnings.value = await backend.startupWarnings();
    } catch (reason) {
      startupWarnings.value = [errorMessage(reason)];
    } finally {
      booting.value = false;
    }
  }

  async function loadRoot(rootPath: string, persist = true) {
    const inspected = await backend.inspectRoot(rootPath);
    profile.value = inspected;
    if (persist) {
      settings.value = await backend.saveSettings({
        ...(settings.value ?? defaultSettings()),
        workspaceRoot: inspected.rootPath,
        product:
          settings.value?.product && inspected.products.some((p) => p.dirName === settings.value?.product)
            ? settings.value.product
            : inspected.defaultProductDir
      });
    }
    return inspected;
  }

  function setArea(next: AppArea) {
    area.value = next;
  }

  return { area, info, settings, profile, booting, startupWarnings, bootstrap, loadRoot, setArea };
});

export function defaultSettings(): ArchiveSettings {
  return {
    workspaceRoot: null,
    product: null,
    javaPath: null,
    engineDir: null,
    checkRulePath: null,
    shardingPath: null,
    moduleChild: "990000_product",
    generalChild: "general",
    confirmTenantWarning: true
  };
}

export function errorMessage(reason: unknown): string {
  if (typeof reason === "string") return reason;
  if (reason && typeof reason === "object" && "message" in reason) {
    return String((reason as { message: unknown }).message);
  }
  return "操作未完成。";
}