import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { backend } from "../services/backend";
import type {
  ArchivePlan,
  ArchiveRun,
  HistorySummary,
  ScriptContent,
  ScriptItem,
  TempScanResult,
  ValidationResult
} from "../contracts/models";
import { errorMessage, useAppStore } from "./app";
import { useToastStore } from "./toast";

export const useArchiveStore = defineStore("archive", () => {
  const app = useAppStore();
  const toast = useToastStore();

  const product = ref<string | null>(null);
  const scan = ref<TempScanResult | null>(null);
  const selected = ref<string[]>([]);
  const detailPath = ref<string | null>(null);
  const detail = ref<ScriptContent | null>(null);
  const detailBusy = ref(false);
  const scanning = ref(false);
  const validating = ref(false);
  const previewing = ref(false);
  const passing = ref(false);
  const confirming = ref(false);
  const plan = ref<ArchivePlan | null>(null);
  const run = ref<ArchiveRun | null>(null);
  const planOpen = ref(false);
  const confirmOpen = ref(false);
  const runOpen = ref(false);
  const history = ref<HistorySummary[]>([]);
  const historyBusy = ref(false);

  const profile = computed(() => app.profile);
  const scripts = computed<ScriptItem[]>(() => scan.value?.scripts ?? []);
  const selectedItems = computed(() =>
    scripts.value.filter((item) => selected.value.includes(item.relativePath))
  );
  const passedCount = computed(
    () => scripts.value.filter((item) => item.mark === "passed").length
  );
  const detailItem = computed(
    () => scripts.value.find((item) => item.relativePath === detailPath.value) ?? null
  );
  const canPreview = computed(
    () =>
      selectedItems.value.length > 0 &&
      selectedItems.value.every(
        (item) => item.mark === "passed" && (!item.validation || item.validation.errors.length === 0)
      )
  );

  async function refresh() {
    const root = app.profile?.rootPath;
    if (!root) return;
    const nextProduct =
      product.value && app.profile?.products.some((item) => item.dirName === product.value)
        ? product.value
        : app.settings?.product ?? app.profile?.defaultProductDir ?? null;
    if (!nextProduct) return;
    if (product.value !== nextProduct) {
      product.value = nextProduct;
      selected.value = [];
      detailPath.value = null;
      detail.value = null;
    }
    scanning.value = true;
    try {
      const result = await backend.scan(root, nextProduct);
      scan.value = result;
      const existing = new Set(result.scripts.map((item) => item.relativePath));
      selected.value = selected.value.filter((path) => existing.has(path));
      if (detailPath.value && !existing.has(detailPath.value)) {
        detailPath.value = null;
        detail.value = null;
      } else if (detailPath.value) {
        await loadDetail(detailPath.value);
      }
    } catch (reason) {
      toast.show(errorMessage(reason), "error", 6000);
    } finally {
      scanning.value = false;
    }
  }

  async function setProduct(next: string) {
    product.value = next;
    selected.value = [];
    plan.value = null;
    if (app.settings) {
      try {
        app.settings = await backend.saveSettings({ ...app.settings, product: next });
      } catch (reason) {
        toast.show(errorMessage(reason), "warning");
      }
    }
    await refresh();
  }

  async function loadDetail(path: string) {
    const root = app.profile?.rootPath;
    if (!root) return;
    const item = scripts.value.find((script) => script.relativePath === path);
    if (!item) return;
    detailPath.value = path;
    detailBusy.value = true;
    try {
      detail.value = await backend.readScript(root, item.repoRelativePath);
    } catch (reason) {
      detail.value = null;
      toast.show(errorMessage(reason), "error");
    } finally {
      detailBusy.value = false;
    }
  }

  async function setSelected(paths: string[]) {
    selected.value = paths;
  }

  async function mark(status: "passed" | "returned" | "pending") {
    const root = app.profile?.rootPath;
    if (!root || selectedItems.value.length === 0) {
      toast.show("请先选择脚本。", "warning");
      return;
    }
    try {
      for (const item of selectedItems.value) {
        await backend.setMark(root, item.relativePath, item.sha256, status);
        item.mark = status;
      }
      toast.show(
        status === "passed"
          ? `已将通过状态写入 ${selectedItems.value.length} 个脚本。`
          : status === "returned"
            ? `已退回 ${selectedItems.value.length} 个脚本。`
            : "已重置为待处理。",
        "success"
      );
    } catch (reason) {
      toast.show(errorMessage(reason), "error");
    }
  }

  async function validateSelected() {
    const root = app.profile?.rootPath;
    if (!root || !product.value) return;
    const targets = selected.value.length > 0 ? selected.value : scripts.value.map((item) => item.relativePath);
    if (targets.length === 0) {
      toast.show("当前产品没有可校验的脚本。", "warning");
      return;
    }
    validating.value = true;
    try {
      const results = await backend.validate(root, product.value, targets);
      applyValidation(results);
      const errors = Object.values(results).filter((item) => item.errors.length > 0).length;
      const warnings = Object.values(results).filter(
        (item) => item.errors.length === 0 && (item.warnings.length > 0 || item.tenantCodeWarning)
      ).length;
      toast.show(
        errors > 0
          ? `校验完成：${errors} 个脚本存在阻断问题。`
          : warnings > 0
            ? `校验通过：${warnings} 个脚本有需要确认的警告。`
            : `校验通过：${targets.length} 个脚本。`,
        errors > 0 ? "error" : warnings > 0 ? "warning" : "success",
        5000
      );
    } catch (reason) {
      toast.show(errorMessage(reason), "error", 8000);
    } finally {
      validating.value = false;
    }
  }

  function applyValidation(results: Record<string, ValidationResult>) {
    if (!scan.value) return;
    for (const item of scan.value.scripts) {
      const result = results[item.relativePath];
      if (result) item.validation = result;
    }
  }

  async function passAndPreview() {
    const root = app.profile?.rootPath;
    if (!root || !product.value) return;
    const items = selectedItems.value;
    if (items.length === 0) {
      toast.show("请先选择脚本。", "warning");
      return;
    }
    passing.value = true;
    try {
      const results = await backend.validate(
        root,
        product.value,
        items.map((item) => item.relativePath)
      );
      applyValidation(results);
      const blocking = items.filter(
        (item) => (results[item.relativePath]?.errors.length ?? 0) > 0
      );
      if (blocking.length > 0) {
        toast.show(
          `校验未通过：${blocking.length} 个脚本存在阻断问题，未生成归档预览。`,
          "error",
          7000
        );
        return;
      }
      for (const item of items) {
        if (item.mark !== "passed") {
          await backend.setMark(root, item.relativePath, item.sha256, "passed");
          item.mark = "passed";
        }
      }
      await preview();
    } catch (reason) {
      toast.show(errorMessage(reason), "error", 8000);
    } finally {
      passing.value = false;
    }
  }

  async function preview() {
    const root = app.profile?.rootPath;
    if (!root || !product.value) return;
    if (!canPreview.value) {
      toast.show("只有已标记为通过且无阻断错误的脚本才能生成归档预览。", "warning", 5000);
      return;
    }
    previewing.value = true;
    try {
      plan.value = await backend.preview({
        rootPath: root,
        product: product.value,
        relativePaths: selectedItems.value.map((item) => item.relativePath)
      });
      planOpen.value = true;
    } catch (reason) {
      toast.show(errorMessage(reason), "error", 8000);
    } finally {
      previewing.value = false;
    }
  }

  async function confirm(tenantAckPaths: string[]) {
    if (!plan.value) return;
    confirming.value = true;
    try {
      run.value = await backend.confirm({
        planId: plan.value.id,
        confirmed: true,
        tenantAckPaths
      });
      confirmOpen.value = false;
      planOpen.value = false;
      runOpen.value = true;
      await refresh();
      await loadHistory();
      if (run.value.state === "committed") {
        toast.show("归档完成，源脚本已移动到备份目录。", "success", 6000);
      } else if (run.value.state === "rolled_back") {
        toast.show(run.value.error ?? "归档失败，已自动回滚。", "error", 8000);
      } else {
        toast.show(run.value.error ?? "归档需要人工核实。", "error", 10000);
      }
    } catch (reason) {
      toast.show(errorMessage(reason), "error", 8000);
    } finally {
      confirming.value = false;
    }
  }

  async function loadHistory() {
    const root = app.profile?.rootPath;
    if (!root) return;
    historyBusy.value = true;
    try {
      history.value = await backend.history(root, 100);
    } catch (reason) {
      toast.show(errorMessage(reason), "error");
    } finally {
      historyBusy.value = false;
    }
  }

  async function openPath(path: string, reveal = false) {
    const root = app.profile?.rootPath;
    if (!root) return;
    try {
      await backend.openPath(root, path, reveal);
    } catch (reason) {
      toast.show(errorMessage(reason), "error");
    }
  }

  return {
    product,
    scan,
    scripts,
    selected,
    selectedItems,
    passedCount,
    detailPath,
    detail,
    detailBusy,
    detailItem,
    scanning,
    validating,
    passing,
    previewing,
    confirming,
    plan,
    run,
    planOpen,
    confirmOpen,
    runOpen,
    history,
    historyBusy,
    profile,
    canPreview,
    refresh,
    setProduct,
    loadDetail,
    setSelected,
    mark,
    validateSelected,
    passAndPreview,
    preview,
    confirm,
    loadHistory,
    openPath
  };
});