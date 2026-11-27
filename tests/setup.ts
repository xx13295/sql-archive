import { config } from "@vue/test-utils";
import ElementPlus from "element-plus";
import { createPinia } from "pinia";
import { vi } from "vitest";

config.global.plugins = [ElementPlus, createPinia()];
config.global.stubs = {
  teleport: true
};

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

if (!("ResizeObserver" in globalThis)) {
  Object.defineProperty(globalThis, "ResizeObserver", {
    value: ResizeObserverStub,
    writable: true
  });
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn()
  })
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn()
}));