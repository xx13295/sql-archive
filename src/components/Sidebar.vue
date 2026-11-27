<script setup lang="ts">
import { Box, Clock, Setting } from "@element-plus/icons-vue";
import { computed } from "vue";
import { useAppStore } from "../stores/app";

const store = useAppStore();
const version = computed(() => store.info?.version ?? "0.1.0");

const items = [
  { key: "archive", label: "归档工作台", icon: Box },
  { key: "history", label: "操作记录", icon: Clock },
  { key: "settings", label: "设置", icon: Setting }
] as const;
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-brand">
      <div class="brand-mark">档</div>
      <div>
        <strong>脚本归档</strong>
        <small>v{{ version }}</small>
      </div>
    </div>
    <nav class="main-nav" aria-label="主导航">
      <button
        v-for="item in items"
        :key="item.key"
        type="button"
        class="nav-item"
        :class="{ active: store.area === item.key }"
        @click="store.setArea(item.key)"
      >
        <el-icon><component :is="item.icon" /></el-icon>
        <span>{{ item.label }}</span>
      </button>
    </nav>
    <div class="sidebar-foot">
      <div>{{ store.info?.portable ? "免安装版" : "桌面版" }}</div>
      <div>首版范围：整理与归档</div>
    </div>
  </aside>
</template>