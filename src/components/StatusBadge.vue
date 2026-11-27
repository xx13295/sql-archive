<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  kind?: "ok" | "warn" | "err" | "info" | "muted";
  label: string;
  dot?: boolean;
}>();

const cls = computed(() => `pill ${props.kind ?? "info"}`);
</script>

<template>
  <span :class="cls"><span v-if="dot" class="dot" />{{ label }}</span>
</template>