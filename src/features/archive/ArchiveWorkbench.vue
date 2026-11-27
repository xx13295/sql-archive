<script setup lang="ts">
import { computed, onMounted, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpened, Refresh, Select, CloseBold, View, Check } from "@element-plus/icons-vue";
import RootSetupCard from "./RootSetupCard.vue";
import ScriptTable from "./ScriptTable.vue";
import ScriptDetailPanel from "./ScriptDetailPanel.vue";
import PlanPanel from "./PlanPanel.vue";
import ConfirmArchiveModal from "./ConfirmArchiveModal.vue";
import RunProgressPanel from "./RunProgressPanel.vue";
import StatusBadge from "../../components/StatusBadge.vue";
import { errorMessage, useAppStore } from "../../stores/app";
import { useArchiveStore } from "../../stores/archive";
import { useToastStore } from "../../stores/toast";
import type { ScriptItem } from "../../contracts/models";

const app = useAppStore();
const archive = useArchiveStore();
const toast = useToastStore();

const products = computed(() => app.profile?.products ?? []);
const selectedCount = computed(() => archive.selected.length);
const scanIssues = computed(() => archive.scan?.issues ?? []);
const baseline = computed(() => archive.scan?.baselineVersion ?? "—");
const baselineLabel = computed(() => {
  const configured = archive.product
    ? app.profile?.baselineByProduct?.[archive.product]
    : undefined;
  return configured ? `配置基线 ${configured}` : `最新版本 ${baseline.value}`;
});
const versions = computed(() => archive.scan?.versions ?? []);

onMounted(async () => {
  if (app.profile) await archive.refresh();
});

watch(
  () => app.profile?.rootPath,
  async (next, previous) => {
    if (next && next !== previous) await archive.refresh();
  }
);

async function chooseRoot() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择数据库脚本仓库根目录"
    });
    if (typeof selected !== "string") return;
    await app.loadRoot(selected);
    await archive.refresh();
    toast.show("已载入工作区。", "success");
  } catch (reason) {
    toast.show(errorMessage(reason), "error");
  }
}

function onRowClick(item: ScriptItem) {
  void archive.loadDetail(item.relativePath);
}

function openPlanConfirm() {
  archive.confirmOpen = true;
}

function onOpenPath(path: string) {
  void archive.openPath(path, false);
}
</script>

<template>
  <div class="page">
    <div class="crumbbar">
      <strong>归档工作台</strong>
      <span v-if="app.profile" class="small">
        {{ app.profile.platformKind === "industry" ? "行业应用" : "运营支撑门户" }}
      </span>
      <span class="spacer" />
      <el-button :icon="FolderOpened" @click="chooseRoot">选择仓库</el-button>
    </div>

    <div class="workbench-body">
      <div class="page-inner workbench-inner">
        <RootSetupCard
          :profile="app.profile"
          :busy="archive.scanning"
          @choose="chooseRoot"
          @refresh="archive.refresh"
          @open-path="onOpenPath"
        />

        <template v-if="app.profile">
          <div class="card">
            <div class="card-body tight">
              <div class="toolbar">
                <span class="small muted">产品</span>
                <el-select
                  :model-value="archive.product ?? undefined"
                  style="width: 220px"
                  placeholder="选择产品"
                  @update:model-value="archive.setProduct"
                >
                  <el-option
                    v-for="item in products"
                    :key="item.dirName"
                    :label="`${item.displayName}（${item.dirName}）`"
                    :value="item.dirName"
                  />
                </el-select>
                <StatusBadge kind="info" :label="baselineLabel" />
                <StatusBadge v-if="versions.length" kind="muted" :label="`脚本版本 ${versions.join(' / ')}`" />
                <span class="grow" />
                <el-button :icon="Refresh" :loading="archive.scanning" @click="archive.refresh">
                  列表刷新
                </el-button>
                <el-button
                  :icon="Select"
                  :loading="archive.validating"
                  :disabled="!archive.scripts.length"
                  @click="archive.validateSelected"
                >
                  校验{{ selectedCount ? "选中" : "全部" }}
                </el-button>
                <el-tooltip
                  content="通过选中脚本；校验无阻断错误后自动生成归档预览"
                  placement="top"
                >
                  <span>
                    <el-button
                      :icon="Check"
                      :loading="archive.passing || archive.validating || archive.previewing"
                      :disabled="!selectedCount"
                      @click="archive.passAndPreview"
                    >
                      通过
                    </el-button>
                  </span>
                </el-tooltip>
                <el-button :icon="CloseBold" :disabled="!selectedCount" @click="archive.mark('returned')">
                  退回
                </el-button>
                <el-tooltip
                  :content="archive.canPreview ? '生成归档计划' : '只有已通过且无阻断错误的脚本才能生成预览'"
                  placement="top"
                >
                  <span>
                    <el-button
                      type="primary"
                      :icon="View"
                      :loading="archive.previewing"
                      :disabled="!archive.canPreview"
                      @click="archive.preview"
                    >
                      生成归档预览
                    </el-button>
                  </span>
                </el-tooltip>
              </div>
            </div>
          </div>

          <div v-if="scanIssues.length" class="alert warning">
            <div>
              <strong>提交目录检查</strong>
              <div v-for="issue in scanIssues" :key="issue" style="margin-top: 4px">{{ issue }}</div>
            </div>
          </div>

          <div class="split" style="min-height: 0">
            <div class="pane">
              <div class="table-wrap">
                <ScriptTable
                  :scripts="archive.scripts"
                  :selected="archive.selected"
                  :loading="archive.scanning"
                  :active-path="archive.detailPath"
                  @update:selected="archive.setSelected"
                  @row-click="onRowClick"
                  @row-dblclick="onRowClick"
                />
              </div>
              <div class="table-foot">
                <span class="small muted">
                  共 {{ archive.scripts.length }} 个脚本 · 已选 {{ selectedCount }} ·
                  已通过 {{ archive.passedCount }}
                </span>
                <span class="grow" />
                <span class="small muted">双击行或点击行查看内容</span>
              </div>
            </div>
            <div class="pane">
              <ScriptDetailPanel
                :item="archive.detailItem"
                :content="archive.detail"
                :loading="archive.detailBusy"
              />
            </div>
          </div>
        </template>
      </div>
    </div>

    <PlanPanel
      v-model="archive.planOpen"
      :plan="archive.plan"
      :busy="archive.confirming"
      @confirm="openPlanConfirm"
      @open-path="onOpenPath"
    />
    <ConfirmArchiveModal
      v-model="archive.confirmOpen"
      :plan="archive.plan"
      :busy="archive.confirming"
      @confirm="archive.confirm"
    />
    <RunProgressPanel
      v-model="archive.runOpen"
      :run="archive.run"
      @open-path="onOpenPath"
    />
  </div>
</template>