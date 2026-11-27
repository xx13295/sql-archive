<script setup lang="ts">
import { computed } from "vue";
import StatusBadge from "../../components/StatusBadge.vue";
import type { ArchiveRun } from "../../contracts/models";

const props = defineProps<{
  modelValue: boolean;
  run: ArchiveRun | null;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  openPath: [path: string];
}>();

const stateLabel = computed(() => {
  switch (props.run?.state) {
    case "committed":
      return "归档完成";
    case "rolled_back":
      return "已回滚";
    case "needs_attention":
      return "需要人工核实";
    case "failed":
      return "失败";
    default:
      return "执行中";
  }
});
const stateKind = computed(() => {
  switch (props.run?.state) {
    case "committed":
      return "ok";
    case "rolled_back":
      return "warn";
    case "needs_attention":
    case "failed":
      return "err";
    default:
      return "info";
  }
});
const stepLabels: Record<string, string> = {
  prepare: "准备",
  archive_files: "写入归档文件",
  backup: "备份源脚本",
  sharding: "更新分表模板",
  delete_sources: "移动源脚本",
  finish: "完成",
  rollback: "回滚",
  recover: "中断恢复",
  skipped: "跳过"
};
const stepOrder: Record<string, number> = {
  prepare: 0,
  archive_files: 1,
  backup: 2,
  sharding: 3,
  delete_sources: 4,
  finish: 5,
  rollback: 6,
  recover: 7
};
const orderedSteps = computed(() =>
  [...(props.run?.steps ?? [])].sort(
    (left, right) => (stepOrder[left.step] ?? 99) - (stepOrder[right.step] ?? 99)
  )
);
const itemLabels: Record<string, string> = {
  pending: "待处理",
  archived: "已写入",
  backed_up: "已备份",
  done: "已完成",
  rolled_back: "已回滚",
  failed: "失败"
};
const stateLabels: Record<string, string> = {
  pending: "待执行",
  running: "执行中",
  done: "完成",
  failed: "失败",
  rolled_back: "已回滚",
  skipped: "跳过",
  needs_attention: "需核实"
};
function stepKind(state: string) {
  return state === "done"
    ? "ok"
    : state === "failed"
      ? "err"
      : state === "running"
        ? "info"
        : "muted";
}
function itemKind(state: string) {
  return state === "done" ? "ok" : state === "failed" ? "err" : state === "rolled_back" ? "warn" : "info";
}
function backupDirectory(run: ArchiveRun) {
  const first = run.items.find((item) => item.backupPath)?.backupPath;
  if (!first) return null;
  return first.replace(/[\\/][^\\/]+$/, "");
}
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    title="归档执行结果"
    width="min(900px, 94vw)"
    top="8vh"
    :close-on-click-modal="false"
    @update:model-value="(value: boolean) => emit('update:modelValue', value)"
  >
    <div v-if="run" style="max-height: 66vh; overflow: auto; display: grid; gap: 12px">
      <div class="status-strip">
        <StatusBadge :kind="stateKind" :label="stateLabel" dot />
        <StatusBadge kind="info" :label="`产品 ${run.product}`" />
        <StatusBadge kind="info" :label="`${run.items.length} 个源脚本`" />
        <span class="spacer" />
        <el-button
          v-if="backupDirectory(run)"
          link
          type="primary"
          size="small"
          @click="emit('openPath', backupDirectory(run)!)"
        >
          打开备份目录
        </el-button>
      </div>

      <div v-if="run.error" class="alert" :class="run.state === 'committed' ? 'info' : 'error'">
        {{ run.error }}
      </div>

      <div class="card">
        <div class="card-head"><h2>执行步骤</h2></div>
        <div class="card-body tight">
          <div class="step-list">
            <div v-for="step in orderedSteps" :key="step.step" class="step">
              <span>{{ stepLabels[step.step] ?? step.step }}</span>
              <StatusBadge :kind="stepKind(step.state)" :label="stateLabels[step.state] ?? step.state" />
              <span class="small muted">{{ step.detail ?? "" }}</span>
            </div>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-head"><h2>源脚本</h2></div>
        <div class="card-body tight">
          <div class="run-list">
            <div v-for="item in run.items" :key="item.sourcePath" class="run-row">
              <StatusBadge :kind="itemKind(item.state)" :label="itemLabels[item.state] ?? item.state" />
              <span class="mono clamp" :title="item.sourceFileName">{{ item.sourceFileName }}</span>
              <span class="small muted">序号 {{ item.sequence }}</span>
              <span class="small muted">{{ item.archivePaths.length }} 个目标文件</span>
            </div>
          </div>
        </div>
      </div>

      <div v-if="run.state !== 'committed'" class="alert warning">
        <div>
          本次归档没有成功完成。失败时工具会按逆序删除本次新建的归档文件、恢复源脚本与分表模板；
          如果状态为“需要人工核实”，请打开备份目录和 Flyway 目录逐项检查。
        </div>
      </div>
    </div>
    <template #footer>
      <el-button type="primary" @click="emit('update:modelValue', false)">关闭</el-button>
    </template>
  </el-dialog>
</template>