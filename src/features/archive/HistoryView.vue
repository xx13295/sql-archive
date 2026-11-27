<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Refresh } from "@element-plus/icons-vue";
import StatusBadge from "../../components/StatusBadge.vue";
import RunProgressPanel from "./RunProgressPanel.vue";
import { backend } from "../../services/backend";
import { useAppStore, errorMessage } from "../../stores/app";
import { useArchiveStore } from "../../stores/archive";
import { useToastStore } from "../../stores/toast";
import type { ArchiveRun, HistorySummary } from "../../contracts/models";

const app = useAppStore();
const archive = useArchiveStore();
const toast = useToastStore();
const run = ref<ArchiveRun | null>(null);
const runOpen = ref(false);
const loading = ref(false);

onMounted(() => {
  void load();
});

async function load() {
  loading.value = true;
  await archive.loadHistory();
  loading.value = false;
}

async function openRun(row: HistorySummary) {
  try {
    run.value = await backend.run(row.id);
    runOpen.value = true;
  } catch (reason) {
    toast.show(errorMessage(reason), "error");
  }
}

function stateKind(state: string) {
  return state === "committed" ? "ok" : state === "rolled_back" ? "warn" : state === "running" ? "info" : "err";
}

function stateLabel(state: string) {
  switch (state) {
    case "committed":
      return "完成";
    case "rolled_back":
      return "已回滚";
    case "needs_attention":
      return "需核实";
    case "running":
      return "执行中";
    default:
      return "失败";
  }
}

function formatTime(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString("zh-CN", { hour12: false });
}
</script>

<template>
  <div class="page">
    <div class="crumbbar">
      <strong>操作记录</strong>
      <span class="small">只读 · 不提供归档撤销</span>
      <span class="spacer" />
      <el-button :icon="Refresh" :loading="loading" @click="load">刷新</el-button>
    </div>
    <div class="page-scroll">
      <div class="page-inner">
        <div v-if="!app.profile" class="empty-state">
          <div>请先在归档工作台选择仓库根目录。</div>
        </div>
        <div v-else class="card">
          <div class="card-head">
            <h2>归档运行记录</h2>
            <span class="spacer" />
            <span class="small muted">当前根目录：{{ app.profile.rootPath }}</span>
          </div>
          <div class="card-body tight">
            <el-table
              :data="archive.history"
              v-loading="loading"
              empty-text="还没有归档记录"
              row-key="id"
              @row-click="openRun"
              style="width: 100%"
            >
              <el-table-column label="开始时间" width="170">
                <template #default="{ row }">{{ formatTime(row.startedAt) }}</template>
              </el-table-column>
              <el-table-column label="产品" width="180" show-overflow-tooltip>
                <template #default="{ row }">{{ row.product }}</template>
              </el-table-column>
              <el-table-column label="版本" min-width="160" show-overflow-tooltip>
                <template #default="{ row }">{{ row.versions || "—" }}</template>
              </el-table-column>
              <el-table-column label="源脚本" width="80">
                <template #default="{ row }">{{ row.sourceCount }}</template>
              </el-table-column>
              <el-table-column label="目标文件" width="90">
                <template #default="{ row }">{{ row.targetCount }}</template>
              </el-table-column>
              <el-table-column label="状态" width="100">
                <template #default="{ row }">
                  <StatusBadge :kind="stateKind(row.state)" :label="stateLabel(row.state)" dot />
                </template>
              </el-table-column>
              <el-table-column label="错误" min-width="200" show-overflow-tooltip>
                <template #default="{ row }">
                  <span class="small" style="color: var(--danger)">{{ row.error ?? "" }}</span>
                </template>
              </el-table-column>
            </el-table>
          </div>
        </div>
      </div>
    </div>
    <RunProgressPanel v-model="runOpen" :run="run" @open-path="(path) => archive.openPath(path)" />
  </div>
</template>