<script setup lang="ts">
import type { ScriptContent } from "../contracts/models";

defineProps<{
  modelValue: boolean;
  title: string;
  path?: string | null;
  content: ScriptContent | null;
  loading?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
}>();
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    :title="title"
    width="min(1120px, 94vw)"
    top="4vh"
    :close-on-click-modal="false"
    @update:model-value="(value: boolean) => emit('update:modelValue', value)"
  >
    <div v-if="path" class="small muted mono break-all" style="margin-bottom: 8px">{{ path }}</div>
    <div v-if="loading" class="empty-state" style="padding: 32px">正在读取脚本内容…</div>
    <pre v-else-if="content" class="detail-pre" style="max-height: 70vh; overflow: auto">{{ content.content }}</pre>
    <div v-else class="empty-state" style="padding: 32px">未读取到脚本内容。</div>
    <template #footer>
      <el-button @click="emit('update:modelValue', false)">关闭</el-button>
    </template>
  </el-dialog>
</template>