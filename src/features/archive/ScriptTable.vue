<script setup lang="ts">
import { computed } from "vue";
import type { ScriptItem } from "../../contracts/models";

const props = defineProps<{
  scripts: ScriptItem[];
  selected: string[];
  loading?: boolean;
  activePath?: string | null;
}>();

const emit = defineEmits<{
  "update:selected": [paths: string[]];
  "row-click": [item: ScriptItem];
  "row-dblclick": [item: ScriptItem];
}>();

const allChecked = computed(
  () => props.scripts.length > 0 && props.selected.length === props.scripts.length
);
const indeterminate = computed(
  () => props.selected.length > 0 && props.selected.length < props.scripts.length
);

function toggle(path: string) {
  if (props.selected.includes(path)) {
    emit("update:selected", props.selected.filter((item) => item !== path));
  } else {
    emit("update:selected", [...props.selected, path]);
  }
}

function toggleAll(value: boolean | string | number) {
  emit("update:selected", value ? props.scripts.map((item) => item.relativePath) : []);
}

function formatSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KiB`;
}

function formatTime(value: string) {
  if (!value) return "";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString("zh-CN", { hour12: false });
}

function validationKind(item: ScriptItem) {
  const validation = item.validation;
  if (!validation) return "muted";
  if (validation.errors.length) return "err";
  if (validation.warnings.length || validation.tenantCodeWarning) return "warn";
  return "ok";
}

function validationLabel(item: ScriptItem) {
  const validation = item.validation;
  if (!validation) return "未校验";
  if (validation.errors.length) return "阻断";
  if (validation.warnings.length || validation.tenantCodeWarning) return "待确认";
  return "通过";
}

function validationTip(item: ScriptItem) {
  const validation = item.validation;
  if (!validation) return "尚未执行校验";
  const messages = [...validation.errors, ...validation.warnings];
  if (validation.tenantCodeWarning) messages.push("脚本包含未分表且带租户编码字段，需要单独确认");
  return messages.length ? messages.join("\n") : "校验通过";
}

function markKind(mark: string) {
  return mark === "passed" ? "ok" : mark === "returned" ? "err" : "muted";
}

function markLabel(mark: string) {
  return mark === "passed" ? "通过" : mark === "returned" ? "退回" : "待处理";
}

function pairLabel(item: ScriptItem) {
  switch (item.pairState) {
    case "paired":
      return "已配对";
    case "missing_pair":
      return "缺 ORACLE/TDSQL 配对";
    case "unexpected_db":
      return "门户不允许 ORACLE";
    default:
      return "单脚本";
  }
}

function pairKind(item: ScriptItem) {
  return item.pairState === "paired" || item.pairState === "single" ? "muted" : "warn";
}
</script>

<template>
  <el-table
    v-loading="loading"
    :data="scripts"
    height="100%"
    row-key="relativePath"
    highlight-current-row
    :current-row-key="activePath ?? undefined"
    @row-click="(row: ScriptItem) => emit('row-click', row)"
    @row-dblclick="(row: ScriptItem) => emit('row-dblclick', row)"
  >
    <el-table-column width="46" fixed>
      <template #header>
        <el-checkbox
          :model-value="allChecked"
          :indeterminate="indeterminate"
          aria-label="全选"
          @change="toggleAll"
        />
      </template>
      <template #default="{ row }">
        <el-checkbox
          :model-value="selected.includes(row.relativePath)"
          aria-label="选择脚本"
          @click.stop
          @change="() => toggle(row.relativePath)"
        />
      </template>
    </el-table-column>
    <el-table-column label="脚本文件" min-width="260" show-overflow-tooltip>
      <template #default="{ row }">
        <div class="script-name">
          <span class="mono clamp" :title="row.repoRelativePath">{{ row.fileName }}</span>
          <span v-if="row.parseError" class="pill err" style="flex: 0 0 auto">命名不规范</span>
        </div>
      </template>
    </el-table-column>
    <el-table-column label="版本" width="84">
      <template #default="{ row }">{{ row.parsed?.version ?? "—" }}</template>
    </el-table-column>
    <el-table-column label="数据库" width="76">
      <template #default="{ row }">
        <span class="pill" :class="row.parsed?.dbType === 'ORACLE' ? 'info' : 'ok'">
          {{ row.parsed?.dbType ?? "—" }}
        </span>
      </template>
    </el-table-column>
    <el-table-column label="整理状态" width="92">
      <template #default="{ row }">
        <span class="pill" :class="markKind(row.mark)">{{ markLabel(row.mark) }}</span>
      </template>
    </el-table-column>
    <el-table-column label="校验" width="96">
      <template #default="{ row }">
        <el-tooltip :content="validationTip(row)" placement="top">
          <span class="pill" :class="validationKind(row)">{{ validationLabel(row) }}</span>
        </el-tooltip>
      </template>
    </el-table-column>

    <el-table-column label="提交人" width="90" show-overflow-tooltip>
      <template #default="{ row }">{{ row.parsed?.person ?? "—" }}</template>
    </el-table-column>
    <el-table-column label="日期" width="104">
      <template #default="{ row }">{{ row.parsed?.date ?? "—" }}</template>
    </el-table-column>
    <el-table-column label="大小" width="82">
      <template #default="{ row }">{{ formatSize(row.sizeBytes) }}</template>
    </el-table-column>
    <el-table-column label="配对" width="150" show-overflow-tooltip>
      <template #default="{ row }">
        <span class="pill" :class="pairKind(row)">{{ pairLabel(row) }}</span>
      </template>
    </el-table-column>
    <el-table-column label="修改时间" width="152">
      <template #default="{ row }">
        <span class="small muted">{{ formatTime(row.modifiedAt) }}</span>
      </template>
    </el-table-column>
  </el-table>
</template>

<style scoped>
.script-name { display: flex; align-items: center; gap: 8px; min-width: 0; }
.script-name .mono { min-width: 0; }
</style>