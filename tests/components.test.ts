import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { describe, expect, it } from "vitest";
import ScriptTable from "../src/features/archive/ScriptTable.vue";
import PlanPanel from "../src/features/archive/PlanPanel.vue";
import ConfirmArchiveModal from "../src/features/archive/ConfirmArchiveModal.vue";
import type { ArchivePlan, ScriptItem } from "../src/contracts/models";

function script(overrides: Partial<ScriptItem> = {}): ScriptItem {
  return {
    relativePath: "2026-04-01_测试_01__insert_AUTH_FUNCTION_TDSQL_[4.6.2.0].groovy",
    repoRelativePath:
      "temp/25_user_extension/2026-04-01_测试_01__insert_AUTH_FUNCTION_TDSQL_[4.6.2.0].groovy",
    fileName: "2026-04-01_测试_01__insert_AUTH_FUNCTION_TDSQL_[4.6.2.0].groovy",
    sizeBytes: 4096,
    modifiedAt: "2026-04-01T10:00:00+08:00",
    sha256: "abc",
    parsed: {
      date: "2026-04-01",
      person: "测试",
      sourceSeq: "01",
      description: "insert_AUTH_FUNCTION",
      dbType: "TDSQL",
      version: "4.6.2.0",
      extension: "groovy"
    },
    parseError: null,
    mark: "passed",
    validation: null,
    groupKey: "2026-04-01_测试_01__insert_AUTH_FUNCTION_4.6.2.0",
    pairState: "single",
    ...overrides
  };
}

function plan(conflicts: string[] = []): ArchivePlan {
  return {
    id: "plan-1",
    rootPath: "D:\\repo\\运营支撑门户",
    product: "25_user_extension",
    createdAt: "2026-04-01T10:00:00Z",
    expiresAt: "2099-01-01T00:00:00Z",
    items: [
      {
        groupKey: "g1",
        sourcePath: "D:\\repo\\运营支撑门户\\temp\\25_user_extension\\a.groovy",
        sourceRepoRelativePath: "temp/25_user_extension/a.groovy",
        sourceFileName: "2026-04-01_测试_01__insert_AUTH_FUNCTION_TDSQL_[4.6.2.0].groovy",
        sourceDb: "TDSQL",
        sourceVersion: "4.6.2.0",
        sequence: 11,
        backupPath: "D:\\repo\\运营支撑门户\\backup\\25_user_extension\\20260401\\a.groovy",
        targets: [
          {
            dbDir: "mysql",
            path: "D:\\repo\\...\\mysql\\V4.6.2.0_990000_00_11__insert_AUTH_FUNCTION_TDSQL_[4.6.2.0].groovy",
            fileName: "V4.6.2.0_990000_00_11__insert_AUTH_FUNCTION_TDSQL_[4.6.2.0].groovy",
            channel: "MYSQL",
            byteSize: 4096,
            sha256: "def"
          }
        ],
        tenantCodeWarning: false,
        state: conflicts.length ? "blocked" : "ready",
        blockers: conflicts
      }
    ],
    conflicts,
    warnings: [],
    shardingChange: null,
    digest: "digest",
    totals: { sourceCount: 1, targetCount: 1, backupCount: 1, versions: ["4.6.2.0"] }
  };
}

describe("ScriptTable", () => {
  it("renders script metadata and emits selection", async () => {
    const wrapper = mount(ScriptTable, {
      props: { scripts: [script()], selected: [] }
    });
    await nextTick();
    await nextTick();
    expect(wrapper.text()).toContain("insert_AUTH_FUNCTION");
    expect(wrapper.text()).toContain("TDSQL");
    expect(wrapper.text()).toContain("4.6.2.0");
    const checkbox = wrapper.findAll('input[type="checkbox"]').at(1);
    await checkbox?.setValue(true);
    const events = wrapper.emitted("update:selected");
    expect(events?.at(-1)?.[0]).toEqual([script().relativePath]);
  });
});

describe("PlanPanel", () => {
  it("shows sequence and blocks confirm when conflicts exist", () => {
    const wrapper = mount(PlanPanel, {
      props: { modelValue: true, plan: plan(["目标文件已存在"]) },
      global: {
        stubs: {
          ElDialog: { template: '<div><slot /><slot name="footer" /></div>' }
        }
      }
    });
    expect(wrapper.text()).toContain("下一序号 11");
    expect(wrapper.text()).toContain("目标文件已存在");
    const buttons = wrapper.findAll("button");
    const confirm = buttons.find((button) => button.text().includes("确认并归档"));
    expect(confirm?.attributes("disabled")).toBeDefined();
  });
});

describe("ConfirmArchiveModal", () => {
  it("requires explicit confirmation before emitting", async () => {
    const wrapper = mount(ConfirmArchiveModal, {
      props: { modelValue: true, plan: plan() },
      global: {
        stubs: {
          ElDialog: { template: '<div><slot /><slot name="footer" /></div>' }
        }
      }
    });
    const submit = wrapper.findAll("button").find((button) => button.text().includes("执行归档"));
    expect(submit?.attributes("disabled")).toBeDefined();
    const checkboxes = wrapper.findAll('input[type="checkbox"]');
    await checkboxes.at(-1)?.setValue(true);
    const enabled = wrapper.findAll("button").find((button) => button.text().includes("执行归档"));
    expect(enabled?.attributes("disabled")).toBeUndefined();
    await enabled?.trigger("click");
    expect(wrapper.emitted("confirm")).toBeTruthy();
  });
});
describe("ConfirmArchiveModal tenant acknowledgements", () => {
  it("checks all tenant confirmations by default and supports one-click toggle", async () => {
    const tenantPlan = plan();
    tenantPlan.items[0].tenantCodeWarning = true;
    const wrapper = mount(ConfirmArchiveModal, {
      props: { modelValue: true, plan: tenantPlan },
      global: {
        stubs: {
          ElDialog: { template: '<div><slot /><slot name="footer" /></div>' }
        }
      }
    });
    const checkboxes = wrapper.findAll('input[type="checkbox"]');
    expect(checkboxes.length).toBeGreaterThanOrEqual(3);
    const [selectAll, tenant, finalConfirm] = [
      checkboxes.at(0)?.element as HTMLInputElement,
      checkboxes.at(1)?.element as HTMLInputElement,
      checkboxes.at(-1)?.element as HTMLInputElement
    ];
    expect(selectAll.checked).toBe(true);
    expect(tenant.checked).toBe(true);
    expect(finalConfirm.checked).toBe(false);
    await checkboxes.at(0)?.setValue(false);
    expect((checkboxes.at(1)?.element as HTMLInputElement).checked).toBe(false);
    await checkboxes.at(0)?.setValue(true);
    expect((checkboxes.at(1)?.element as HTMLInputElement).checked).toBe(true);
  });
});
