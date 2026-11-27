<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpened, Loading, Select } from "@element-plus/icons-vue";
import StatusBadge from "../../components/StatusBadge.vue";
import { backend } from "../../services/backend";
import { errorMessage, useAppStore } from "../../stores/app";
import { useArchiveStore } from "../../stores/archive";
import { useToastStore } from "../../stores/toast";
import type { ArchiveSettings, JavaStatus } from "../../contracts/models";

const app = useAppStore();
const archive = useArchiveStore();
const toast = useToastStore();
const form = reactive<ArchiveSettings>({
  workspaceRoot: null,
  product: null,
  javaPath: null,
  engineDir: null,
  checkRulePath: null,
  shardingPath: null,
  moduleChild: "990000_product",
  generalChild: "general",
  confirmTenantWarning: true
});
const saving = ref(false);
const testing = ref(false);
const javaStatus = ref<JavaStatus | null>(null);

const profile = computed(() => app.profile);
const dataDir = computed(() => app.info?.dataDir ?? "");

onMounted(() => {
  syncFromStore();
});

function syncFromStore() {
  if (!app.settings) return;
  Object.assign(form, app.settings);
  javaStatus.value = null;
}

async function pickDirectory(field: "workspaceRoot" | "engineDir") {
  const selected = await open({ directory: true, multiple: false, title: "选择目录" });
  if (typeof selected === "string") form[field] = selected;
}

async function pickFile(field: "javaPath" | "checkRulePath" | "shardingPath", exe = false) {
  const selected = await open({
    directory: false,
    multiple: false,
    title: "选择文件",
    filters: exe ? [{ name: "可执行文件", extensions: ["exe"] }] : undefined
  });
  if (typeof selected === "string") form[field] = selected;
}

async function testJava() {
  testing.value = true;
  try {
    javaStatus.value = await backend.testJava(form.javaPath);
    toast.show(
      javaStatus.value.available
        ? `Java 可用：${javaStatus.value.version ?? ""}`
        : javaStatus.value.message ?? "Java 不可用",
      javaStatus.value.available ? "success" : "error",
      5000
    );
  } catch (reason) {
    toast.show(errorMessage(reason), "error");
  } finally {
    testing.value = false;
  }
}

async function save() {
  saving.value = true;
  try {
    const saved = await backend.saveSettings({ ...form });
    app.settings = saved;
    if (saved.workspaceRoot) {
      await app.loadRoot(saved.workspaceRoot, false);
      archive.plan = null;
      await archive.refresh();
    }
    toast.show("设置已保存。", "success");
  } catch (reason) {
    toast.show(errorMessage(reason), "error", 6000);
  } finally {
    saving.value = false;
  }
}

async function openPath(path?: string | null) {
  if (!path || !profile.value) return;
  try {
    await backend.openPath(profile.value.rootPath, path, false);
  } catch (reason) {
    toast.show(errorMessage(reason), "error");
  }
}

function baselineText() {
  if (!profile.value) return "—";
  const pairs = Object.entries(profile.value.baselineByProduct);
  return pairs.length ? pairs.map(([product, version]) => `${product}=${version}`).join(", ") : "未配置";
}
</script>

<template>
  <div class="page">
    <div class="crumbbar">
      <strong>设置</strong>
      <span class="small">路径、工具链与工作区配置</span>
      <span class="spacer" />
      <el-button :icon="Select" type="primary" :loading="saving" @click="save">保存设置</el-button>
    </div>
    <div class="page-scroll">
      <div class="page-inner setup-grid">
        <div class="card">
          <div class="card-head"><h2>路径与工具链</h2></div>
          <div class="card-body form-grid">
            <div class="form-row">
              <label>仓库根目录</label>
              <div class="path-row">
                <el-input v-model="form.workspaceRoot" placeholder="运营支撑门户 / 行业应用 目录" clearable />
                <el-button :icon="FolderOpened" @click="pickDirectory('workspaceRoot')">选择</el-button>
              </div>
            </div>
            <div class="form-row">
              <label>Java 路径</label>
              <div class="path-row">
                <el-input v-model="form.javaPath" placeholder="留空则使用 JAVA_HOME 或 PATH 中的 java" clearable />
                <el-button @click="pickFile('javaPath', true)">选择</el-button>
                <el-button :icon="Loading" :loading="testing" @click="testJava">检测</el-button>
              </div>
            </div>
            <div v-if="javaStatus" class="alert" :class="javaStatus.available ? 'info' : 'error'">
              <div>
                <div>{{ javaStatus.available ? "Java 可用" : "Java 不可用" }}</div>
                <div class="small">{{ javaStatus.path ?? "" }} {{ javaStatus.version ?? "" }}</div>
                <div v-if="javaStatus.message" class="small">{{ javaStatus.message }}</div>
              </div>
            </div>
            <div class="form-row">
              <label>引擎目录</label>
              <div class="path-row">
                <el-input v-model="form.engineDir" :placeholder="profile?.engineDir ?? '默认 <根目录>/tool/java'" clearable />
                <el-button @click="pickDirectory('engineDir')">选择</el-button>
              </div>
            </div>
            <div class="form-row">
              <label>校验规则</label>
              <div class="path-row">
                <el-input v-model="form.checkRulePath" :placeholder="profile?.checkRulePath ?? '默认 <根目录>/tool/config/check-rule.yml'" clearable />
                <el-button @click="pickFile('checkRulePath')">选择</el-button>
              </div>
            </div>
            <div class="form-row">
              <label>分表模板</label>
              <div class="path-row">
                <el-input v-model="form.shardingPath" :placeholder="profile?.shardingPath ?? '默认 <根目录>/saas-sharding.yml.vm'" clearable />
                <el-button @click="pickFile('shardingPath')">选择</el-button>
              </div>
            </div>
            <div class="form-row">
              <label>Flyway 模块</label>
              <div class="path-row">
                <el-input v-model="form.moduleChild" placeholder="990000_product" />
                <el-input v-model="form.generalChild" placeholder="general" style="width: 160px" />
              </div>
            </div>
            <div class="small muted">
              保存后会把 <code>totalPath</code> 与产品选择写回 <code>tool/config/user-config.table</code>（该文件已被
              .gitignore 忽略），便于旧工具继续使用。<code>sys-config.table</code> 保持只读，不会被本应用修改。
            </div>
          </div>
        </div>

        <div style="display: grid; gap: 14px; align-content: start">
          <div class="card">
            <div class="card-head"><h2>当前工作区</h2></div>
            <div class="card-body tight">
              <div v-if="!profile" class="empty-state" style="padding: 20px">
                <div>尚未选择有效的工作区。</div>
              </div>
              <div v-else class="meta-list">
                <div>
                  <span class="k">平台</span>
                  <span>
                    <StatusBadge
                      :kind="profile.platformKind === 'industry' ? 'info' : 'ok'"
                      :label="profile.platformKind === 'industry' ? '行业应用' : '运营支撑门户'"
                    />
                  </span>
                </div>
                <div>
                  <span class="k">Java</span>
                  <span class="small">{{ profile.java.version ?? "不可用" }}</span>
                </div>
                <div>
                  <span class="k">引擎 JAR</span>
                  <span class="small">
                    {{ profile.engine.jars.map((jar) => jar.name).join(", ") || "未找到" }}
                  </span>
                </div>
                <div>
                  <span class="k">TDSQL 目标</span>
                  <span class="small">{{ profile.tdsqlTargets.join(", ") || "未配置" }}</span>
                </div>
                <div>
                  <span class="k">ORACLE 目标</span>
                  <span class="small">{{ profile.oracleTargets.join(", ") || "未配置" }}</span>
                </div>
                <div>
                  <span class="k">版本基线</span>
                  <span class="small">{{ baselineText() }}</span>
                </div>
                <div>
                  <span class="k">分表模板</span>
                  <span class="small">
                    {{ profile.shardingPath ?? "无" }}
                    <el-button
                      v-if="profile.shardingPath"
                      link
                      type="primary"
                      size="small"
                      @click="openPath(profile.shardingPath)"
                    >
                      打开
                    </el-button>
                  </span>
                </div>
                <div>
                  <span class="k">Flyway</span>
                  <span class="small">
                    {{ profile.flywayRoot ?? "未找到" }}
                    <el-button
                      v-if="profile.flywayRoot"
                      link
                      type="primary"
                      size="small"
                      @click="openPath(profile.flywayRoot)"
                    >
                      打开
                    </el-button>
                  </span>
                </div>
              </div>
            </div>
          </div>

          <div class="card">
            <div class="card-head"><h2>应用数据</h2></div>
            <div class="card-body tight">
              <div class="meta-list">
                <div>
                  <span class="k">数据目录</span>
                  <span class="mono-path" :title="dataDir">{{ dataDir }}</span>
                </div>
                <div>
                  <span class="k">运行模式</span>
                  <span class="small">{{ app.info?.portable ? "免安装版（数据随 EXE）" : "桌面版" }}</span>
                </div>
                <div>
                  <span class="k">版本</span>
                  <span class="small">{{ app.info?.version }}</span>
                </div>
              </div>
              <div class="small muted" style="margin-top: 10px">
                操作记录、校验缓存和预览内容保存在此目录。归档计划默认 30 分钟后过期。
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>