<script setup lang="ts">
import { onMounted } from "vue";
import TitleBar from "../components/TitleBar.vue";
import Sidebar from "../components/Sidebar.vue";
import ArchiveWorkbench from "../features/archive/ArchiveWorkbench.vue";
import HistoryView from "../features/archive/HistoryView.vue";
import SettingsView from "../features/settings/SettingsView.vue";
import { useAppStore } from "../stores/app";
import { useToastStore } from "../stores/toast";

const store = useAppStore();
const toast = useToastStore();

onMounted(async () => {
  await store.bootstrap();
  for (const warning of store.startupWarnings) {
    toast.show(warning, "warning", 6000);
  }
});
</script>

<template>
  <div class="app-shell">
    <TitleBar />
    <div class="app-body">
      <Sidebar />
      <main class="workspace">
        <div v-if="store.booting" class="empty-state">
          <el-icon><svg viewBox="0 0 24 24" width="1em" height="1em"><path fill="currentColor" d="M12 2a10 10 0 1 0 10 10h-2a8 8 0 1 1-8-8V2Z" /></svg></el-icon>
          <div>正在载入本地状态…</div>
        </div>
        <template v-else>
          <ArchiveWorkbench v-if="store.area === 'archive'" />
          <HistoryView v-else-if="store.area === 'history'" />
          <SettingsView v-else />
        </template>
      </main>
    </div>
    <div class="toast-layer" aria-live="polite">
      <div
        v-for="item in toast.items"
        :key="item.id"
        class="toast-item"
        :class="item.kind"
        @click="toast.dismiss(item.id)"
      >
        {{ item.message }}
      </div>
    </div>
  </div>
</template>