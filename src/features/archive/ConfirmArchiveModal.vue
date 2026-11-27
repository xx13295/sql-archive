<script setup lang="ts">
import { computed, ref, watch } from "vue";
import ScriptContentViewer from "../../components/ScriptContentViewer.vue";
import StatusBadge from "../../components/StatusBadge.vue";
import { backend } from "../../services/backend";
import { errorMessage } from "../../stores/app";
import { useToastStore } from "../../stores/toast";
import type { ArchivePlan, PlanItem, ScriptContent } from "../../contracts/models";

const props = defineProps<{
  modelValue: boolean;
  plan: ArchivePlan | null;
  busy?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  confirm: [ackPaths: string[]];
}>();

const toast = useToastStore();
const acknowledged = ref<string[]>([]);
const confirmed = ref(false);
const viewerOpen = ref(false);
const viewerTitle = ref("");
const viewerPath = ref<string | null>(null);
const viewerContent = ref<ScriptContent | null>(null);
const viewerLoading = ref(false);

const tenantItems = computed(
  () => props.plan?.items.filter((item) => item.tenantCodeWarning) ?? []
);
const allTenantAcked = computed(
  () =>
    tenantItems.value.length > 0 &&
    tenantItems.value.every((item) => acknowledged.value.includes(item.sourceFileName))
);
const someTenantAcked = computed(
  () =>
    tenantItems.value.some((item) => acknowledged.value.includes(item.sourceFileName)) &&
    !allTenantAcked.value
);
const selectAllTenant = computed<boolean>({
  get: () => allTenantAcked.value,
  set: (value) => {
    acknowledged.value = value
      ? tenantItems.value.map((item) => item.sourceFileName)
      : [];
  }
});
const canConfirm = computed(
  () =>
    !props.busy &&
    confirmed.value &&
    (tenantItems.value.length === 0 || allTenantAcked.value) &&
    (props.plan?.conflicts.length ?? 1) === 0
);

function resetAcknowledgements() {
  // 默认全勾选；用户仍可逐项取消，全部勾选后才允许执行。
  acknowledged.value = tenantItems.value.map((item) => item.sourceFileName);
}

watch(
  () => [props.modelValue, props.plan?.id] as const,
  ([open]) => {
    if (open) {
      resetAcknowledgements();
      confirmed.value = false;
    }
  },
  { immediate: true }
);

function toggleAck(name: string) {
  acknowledged.value = acknowledged.value.includes(name)
    ? acknowledged.value.filter((item) => item !== name)
    : [...acknowledged.value, name];
}

async function viewScript(item: PlanItem) {
  if (!props.plan) return;
  viewerTitle.value = item.sourceFileName;
  viewerPath.value = item.sourceRepoRelativePath || null;
  viewerContent.value = null;
  viewerOpen.value = true;
  viewerLoading.value = true;
  try {
    if (!item.sourceRepoRelativePath) {
      throw new Error("该归档计划缺少仓库相对路径，请重新生成预览。");
    }
    viewerContent.value = await backend.readScript(props.plan.rootPath, item.sourceRepoRelativePath);
  } catch (reason) {
    toast.show(errorMessage(reason), "error", 6000);
  } finally {
    viewerLoading.value = false;
  }
}

function submit() {
  if (!canConfirm.value) return;
  emit("confirm", acknowledged.value);
}
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    title="确认执行归档"
    width="min(860px, 94vw)"
    top="6vh"
    :close-on-click-modal="false"
    @update:model-value="(value: boolean) => emit('update:modelValue', value)"
  >
    <div v-if="plan" class="form-grid">
      <div class="alert warning">
        <div>
          本次将执行以下不可逆操作：移动 <strong>{{ plan.totals.sourceCount }}</strong> 个源脚本到 backup、
          新增 <strong>{{ plan.totals.targetCount }}</strong> 个归档文件、
          写入 <strong>{{ plan.totals.backupCount }}</strong> 份备份
          <span v-if="plan.shardingChange?.changed">，并更新分表模板</span>。
          任一失败会自动回滚本次改动。
        </div>
      </div>

      <div class="card">
        <div class="card-head">
          <h2>待归档脚本</h2>
          <span class="small muted">共 {{ plan.items.length }} 个</span>
          <span class="spacer" />
          <el-checkbox
            v-if="tenantItems.length"
            v-model="selectAllTenant"
            :indeterminate="someTenantAcked"
          >
            一键勾选租户确认
          </el-checkbox>
        </div>
        <div class="card-body tight">
          <div class="confirm-script-list">
            <div v-for="item in plan.items" :key="item.sourcePath" class="confirm-script-row">
              <el-checkbox
                v-if="item.tenantCodeWarning"
                :model-value="acknowledged.includes(item.sourceFileName)"
                :aria-label="`确认 ${item.sourceFileName} 的租户编码警告`"
                @change="() => toggleAck(item.sourceFileName)"
              />
              <span v-else class="confirm-script-placeholder" />
              <StatusBadge
                v-if="item.tenantCodeWarning"
                kind="warn"
                label="租户编码"
              />
              <span class="mono clamp" :title="item.sourceFileName">{{ item.sourceFileName }}</span>
              <StatusBadge
                :kind="item.sourceDb === 'ORACLE' ? 'info' : 'ok'"
                :label="`${item.sourceDb} ${item.sourceVersion}`"
              />
              <el-button link type="primary" size="small" @click="viewScript(item)">
                详情
              </el-button>
            </div>
          </div>
          <div v-if="tenantItems.length" class="small muted" style="margin-top: 8px">
            默认已勾选全部租户编码确认；取消勾选后将无法执行归档。
          </div>
        </div>
      </div>

      <div class="meta-list">
        <div><span class="k">产品</span><span>{{ plan.product }}</span></div>
        <div><span class="k">版本</span><span>{{ plan.totals.versions.join(", ") }}</span></div>
        <div><span class="k">分表模板</span><span>{{ plan.shardingChange ? "将更新" : "无变化" }}</span></div>
        <div><span class="k">计划有效期</span><span class="small">{{ new Date(plan.expiresAt).toLocaleString("zh-CN", { hour12: false }) }}</span></div>
      </div>

      <el-checkbox v-model="confirmed">
        我已核对目标文件、备份路径与序号，确认执行归档
      </el-checkbox>
    </div>
    <template #footer>
      <el-button @click="emit('update:modelValue', false)">取消</el-button>
      <el-button type="primary" :disabled="!canConfirm" :loading="busy" @click="submit">
        执行归档
      </el-button>
    </template>
  </el-dialog>
  <ScriptContentViewer
    v-model="viewerOpen"
    :title="viewerTitle || '脚本内容'"
    :path="viewerPath"
    :content="viewerContent"
    :loading="viewerLoading"
  />
</template>