<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Close, FullScreen, Minus } from "@element-plus/icons-vue";

const appWindow = getCurrentWindow();
</script>

<template>
  <div class="titlebar" data-tauri-drag-region>
    <div class="titlebar-brand" data-tauri-drag-region>
      <img src="/app-icon.png" alt="" data-tauri-drag-region />
      <strong data-tauri-drag-region>数据库脚本归档工具</strong>
      <span data-tauri-drag-region>整理 · 校验 · 归档</span>
    </div>
    <div class="window-actions">
      <button title="最小化" @click="appWindow.minimize()"><el-icon><Minus /></el-icon></button>
      <button title="最大化" @click="appWindow.toggleMaximize()"><el-icon><FullScreen /></el-icon></button>
      <button title="关闭" @click="appWindow.close()"><el-icon><Close /></el-icon></button>
    </div>
  </div>
</template>