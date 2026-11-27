<script setup lang="ts">
import { ref } from "vue";
import { Document, View } from "@element-plus/icons-vue";
import ScriptContentViewer from "../../components/ScriptContentViewer.vue";
import type { ScriptContent, ScriptItem } from "../../contracts/models";

const props = defineProps<{
  item: ScriptItem | null;
  content: ScriptContent | null;
  loading?: boolean;
}>();

const viewerOpen = ref(false);

function yesNo(value?: boolean) {
  return value ? "是" : "否";
}

function openViewer() {
  if (props.item && props.content) viewerOpen.value = true;
}
</script>

<template>
  <div class="card detail" style="height: 100%">
    <div class="card-head">
      <h2>脚本详情</h2>
      <span class="spacer" />
      <span v-if="item" class="small muted">{{ item.sizeBytes }} 字节</span>
      <el-button
        type="primary"
        plain
        size="small"
        :icon="View"
        :disabled="!item || !content || loading"
        @click="openViewer"
      >
        查看脚本详情
      </el-button>
    </div>
    <div v-if="!item" class="detail-empty">
      <div>
        <el-icon style="font-size: 28px; color: var(--line-strong)"><Document /></el-icon>
        <div style="margin-top: 8px">点击左侧脚本查看内容与校验明细</div>
      </div>
    </div>
    <div v-else class="detail" style="min-height: 0">
      <div class="card-body tight" style="border-bottom: 1px solid var(--line)">
        <div class="mono break-all" style="font-size: 12px; margin-bottom: 8px">{{ item.fileName }}</div>
        <div class="meta-list">
          <div><span class="k">提交人</span><span>{{ item.parsed?.person ?? "—" }} · {{ item.parsed?.date ?? "—" }} · 序号 {{ item.parsed?.sourceSeq ?? "—" }}</span></div>
          <div><span class="k">目标版本</span><span>{{ item.parsed?.version ?? "—" }}</span></div>
          <div><span class="k">整理状态</span><span>{{ item.mark === "passed" ? "通过" : item.mark === "returned" ? "退回" : "待处理" }}</span></div>
          <div><span class="k">UTF-8 BOM</span><span>{{ content ? yesNo(content.hasBom) : "—" }}</span></div>
          <div v-if="item.repoRelativePath">
            <span class="k">仓库相对路径</span>
            <span class="small mono break-all">{{ item.repoRelativePath }}</span>
          </div>
          <div v-if="item.parseError"><span class="k">命名问题</span><span class="small" style="color: var(--danger)">{{ item.parseError }}</span></div>
        </div>
        <div v-if="item.validation" style="margin-top: 10px">
          <div v-if="item.validation.errors.length" class="alert error" style="margin-bottom: 6px">
            <div>
              <div v-for="error in item.validation.errors" :key="error">{{ error }}</div>
            </div>
          </div>
          <div v-if="item.validation.warnings.length" class="alert warning" style="margin-bottom: 6px">
            <div>
              <div v-for="warning in item.validation.warnings" :key="warning">{{ warning }}</div>
            </div>
          </div>
          <div v-if="item.validation.tenantCodeWarning" class="alert warning">
            脚本包含未分表且带租户编码字段。归档前需要在确认弹窗中逐文件勾选确认。
          </div>
        </div>
      </div>
      <div v-if="loading" class="detail-empty">正在读取脚本…</div>
      <div v-else class="detail-empty" style="gap: 10px">
        <el-button type="primary" plain :icon="View" :disabled="!content" @click="openViewer">
          查看脚本内容
        </el-button>
        <span v-if="!content" class="small muted">未读取到脚本内容。</span>
      </div>
    </div>
    <ScriptContentViewer
      v-model="viewerOpen"
      :title="item?.fileName ?? '脚本内容'"
      :path="item?.repoRelativePath ?? null"
      :content="content"
      :loading="loading"
    />
  </div>
</template>