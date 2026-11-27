import { defineStore } from "pinia";
import { ref } from "vue";

export type ToastKind = "success" | "info" | "warning" | "error";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

let nextId = 1;

export const useToastStore = defineStore("toast", () => {
  const items = ref<ToastItem[]>([]);

  function show(message: string, kind: ToastKind = "info", timeout = 3600) {
    const id = nextId++;
    items.value.push({ id, kind, message });
    if (timeout > 0) {
      window.setTimeout(() => dismiss(id), timeout);
    }
    return id;
  }

  function dismiss(id: number) {
    items.value = items.value.filter((item) => item.id !== id);
  }

  return { items, show, dismiss };
});