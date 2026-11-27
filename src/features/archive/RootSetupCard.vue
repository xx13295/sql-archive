<script setup lang="ts">
import { FolderOpened, Refresh, SwitchButton } from "@element-plus/icons-vue";
import { computed } from "vue";
import StatusBadge from "../../components/StatusBadge.vue";
import type { RootProfile } from "../../contracts/models";

const props = defineProps<{
  profile: RootProfile | null;
  busy?: boolean;
}>();

const emit = defineEmits<{
  choose: [];
  refresh: [];
  openPath: [path: string];
}>();

const platform = computed(() =>
  props.profile?.platformKind === "industry" ? "行业应用" : "运营支撑门户"
);
const javaVersion = computed(() => {
  const raw = props.profile?.java.version ?? "";
  return raw.match(/"([^"]+)"/)?.[1] ?? raw;
});
const errorCount = computed(
  () => props.profile?.issues.filter((issue) => issue.severity === "error").length ?? 0
);
const warningCount = computed(
  () => props.profile?.issues.filter((issue) => issue.severity === "warning").length ?? 0
);
const targetSummary = computed(() => {
  if (!props.profile) return "";
  const parts = [];
  if (props.profile.tdsqlTargets.length) parts.push(`TDSQL → ${props.profile.tdsqlTargets.join(", ")}`);
  if (props.profile.oracleTargets.length) parts.push(`ORACLE → ${props.profile.oracleTargets.join(", ")}`);
  return parts.join("　");
});
</script>

<template>
  <div v-if="!profile" class="card">
    <div class="card-body">
      <div class="empty-state">
        <el-icon><FolderOpened /></el-icon>
        <h2>选择数据库脚本仓库根目录</h2>
        <p style="max-width: 560px">
          请选择 <code class="mono">运营支撑门户</code> 或 <code class="mono">行业应用</code> 目录。
          工具会读取其中的 <code>temp/</code>、<code>backup/</code>、
          <code>nontax-flyway-db/</code> 与 <code>tool/</code> 配置。
        </p>
        <el-button type="primary" size="large" :icon="FolderOpened" @click="emit('choose')">
          选择根目录
        </el-button>
      </div>
    </div>
  </div>
  <div v-else class="card">
    <div class="card-head">
      <h2>工作区</h2>
      <StatusBadge :kind="profile.platformKind === 'industry' ? 'info' : 'ok'" :label="platform" dot />
      <StatusBadge
        :kind="profile.java.available ? 'ok' : 'err'"
        :label="profile.java.available ? `Java ${javaVersion || '可用'}` : 'Java 不可用'"
        dot
      />
      <StatusBadge
        :kind="profile.engine.available ? 'ok' : 'err'"
        :label="profile.engine.available ? `引擎 ${profile.engine.jars.length} 个 JAR` : '引擎不可用'"
        dot
      />
      <StatusBadge
        v-if="errorCount"
        kind="err"
        :label="`${errorCount} 个阻断问题`"
        dot
      />
      <StatusBadge
        v-else-if="warningCount"
        kind="warn"
        :label="`${warningCount} 个提醒`"
        dot
      />
      <span class="spacer" />
      <el-button :icon="Refresh" :loading="busy" @click="emit('refresh')">重新检查</el-button>
      <el-button :icon="SwitchButton" @click="emit('choose')">切换目录</el-button>
    </div>
    <div class="card-body tight">
      <div class="meta-list">
        <div>
          <span class="k">根目录</span>
          <span class="path-row">
            <span class="mono-path" :title="profile.rootPath">{{ profile.rootPath }}</span>
            <el-button link type="primary" size="small" @click="emit('openPath', profile.rootPath)">
              打开
            </el-button>
          </span>
        </div>
        <div>
          <span class="k">Flyway</span>
          <span class="path-row">
            <span class="mono-path" :title="profile.flywayRoot ?? ''">{{ profile.flywayRoot ?? "未找到" }}</span>
            <el-button
              v-if="profile.flywayRoot"
              link
              type="primary"
              size="small"
              @click="emit('openPath', profile.flywayRoot)"
            >
              打开
            </el-button>
          </span>
        </div>
        <div>
          <span class="k">目标库</span>
          <span class="small">{{ targetSummary || "未配置" }}</span>
        </div>
        <div v-if="profile.shardingPath">
          <span class="k">分表模板</span>
          <span class="mono-path">{{ profile.shardingPath }}</span>
        </div>
      </div>
      <ul v-if="profile.issues.length" class="issue-list" style="margin-top: 12px">
        <li v-for="issue in profile.issues" :key="issue.code + issue.message" class="issue" :class="issue.severity">
          <span>{{ issue.message }}</span>
        </li>
      </ul>
    </div>
  </div>
</template>