<script setup lang="ts">
import { computed } from "vue";
import StatusBadge from "../../components/StatusBadge.vue";
import type { ArchivePlan } from "../../contracts/models";

const props = defineProps<{
  modelValue: boolean;
  plan: ArchivePlan | null;
  busy?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  confirm: [];
  openPath: [path: string];
}>();

const blocked = computed(() => (props.plan?.conflicts.length ?? 0) > 0);
const sequences = computed(() => {
  if (!props.plan) return [];
  const map = new Map<string, number[]>();
  for (const item of props.plan.items) {
    const list = map.get(item.sourceVersion) ?? [];
    if (!list.includes(item.sequence)) list.push(item.sequence);
    map.set(item.sourceVersion, list);
  }
  return [...map.entries()].map(([version, list]) => ({ version, list: list.sort((a, b) => a - b) }));
});
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    title="归档预览"
    width="min(1120px, 94vw)"
    top="4vh"
    :close-on-click-modal="false"
    @update:model-value="(value: boolean) => emit('update:modelValue', value)"
  >
    <div v-if="plan" style="max-height: 68vh; overflow: auto; padding-right: 4px">
      <div class="status-strip" style="margin-bottom: 12px">
        <StatusBadge kind="info" :label="`${plan.totals.sourceCount} 个源脚本`" dot />
        <StatusBadge kind="info" :label="`${plan.totals.targetCount} 个归档文件`" dot />
        <StatusBadge kind="info" :label="`${plan.totals.backupCount} 个备份`" dot />
        <StatusBadge
          v-for="entry in sequences"
          :key="entry.version"
          kind="ok"
          :label="`版本 ${entry.version}：下一序号 ${entry.list.join(', ')}`"
        />
        <StatusBadge v-if="blocked" kind="err" :label="`${plan.conflicts.length} 个阻断项`" dot />
        <StatusBadge v-else kind="ok" label="可以执行" dot />
      </div>

      <div v-if="plan.conflicts.length" class="alert error" style="margin-bottom: 10px">
        <div>
          <strong>需要先处理以下阻断项</strong>
          <div v-for="conflict in plan.conflicts" :key="conflict" style="margin-top: 4px">{{ conflict }}</div>
        </div>
      </div>
      <div v-if="plan.warnings.length" class="alert warning" style="margin-bottom: 10px">
        <div>
          <strong>请确认</strong>
          <div v-for="warning in plan.warnings" :key="warning" style="margin-top: 4px">{{ warning }}</div>
        </div>
      </div>

      <div v-if="plan.shardingChange" class="card" style="margin-bottom: 12px">
        <div class="card-head">
          <h2>分表模板变更</h2>
          <StatusBadge kind="warn" label="将写入 saas-sharding.yml.vm" dot />
          <span class="spacer" />
          <el-button link type="primary" size="small" @click="emit('openPath', plan.shardingChange.path)">
            打开文件
          </el-button>
        </div>
        <div class="card-body tight">
          <div class="small" style="margin-bottom: 8px">
            影响表：{{ plan.shardingChange.affectedTables.join(", ") || "未知" }}
            · 写入前会备份到应用数据目录
          </div>
          <el-collapse>
            <el-collapse-item title="查看新的分表配置（截取前 20000 字符）" name="sharding">
              <pre class="detail-pre" style="max-height: 260px">{{ plan.shardingChange.preview }}</pre>
            </el-collapse-item>
          </el-collapse>
        </div>
      </div>

      <div class="plan-grid">
        <div
          v-for="item in plan.items"
          :key="item.sourcePath"
          class="plan-item"
          :style="item.blockers.length ? 'border-color: #f3c9c7' : ''"
        >
          <div class="plan-item-head">
            <StatusBadge kind="ok" :label="`序号 ${item.sequence}`" dot />
            <StatusBadge :kind="item.sourceDb === 'ORACLE' ? 'info' : 'ok'" :label="item.sourceDb" />
            <strong class="mono" style="font-size: 12.5px">{{ item.sourceFileName }}</strong>
            <span class="spacer" />
            <StatusBadge v-if="item.tenantCodeWarning" kind="warn" label="租户编码确认" dot />
            <StatusBadge v-if="item.blockers.length" kind="err" label="阻断" dot />
            <el-button link type="primary" size="small" @click="emit('openPath', item.sourcePath)">
              源文件
            </el-button>
          </div>
          <div class="plan-item-body">
            <div v-for="blocker in item.blockers" :key="blocker" class="small" style="color: var(--danger)">
              {{ blocker }}
            </div>
            <div class="map-line">
              <span>备份</span>
              <span class="target-file">{{ item.backupPath }}</span>
            </div>
            <div class="map-line">
              <span>归档文件</span>
              <div class="target-list">
                <div v-for="target in item.targets" :key="target.path">
                  <div class="target-file">
                    <span class="pill" style="margin-right: 6px">{{ target.dbDir }}</span>
                    {{ target.fileName }}
                  </div>
                  <div class="small muted break-all" style="margin: 2px 0 4px 2px">{{ target.path }}</div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
    <template #footer>
      <div class="toolbar">
        <span class="small muted">预览内容已保存；执行时不会重新调用 Java 引擎。</span>
        <span class="grow" />
        <el-button @click="emit('update:modelValue', false)">取消</el-button>
        <el-button type="primary" :disabled="blocked || busy" :loading="busy" @click="emit('confirm')">
          确认并归档
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>