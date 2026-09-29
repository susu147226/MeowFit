// 使用 vitest/config 的 defineConfig，使 test 段有类型
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
// @ts-expect-error 未引入 @types/node，此处按模板方式取用 process
import process from "node:process";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react(), tailwindcss()],

  // 防止 Vite 清屏掩盖 Rust 侧的编译错误
  clearScreen: false,

  // Tauri 期望前端固定端口，端口被占用时直接失败而非静默换端口
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },

  // 构建产物目标：Windows WebView2 基于 Chromium，可直接使用现代语法
  build: {
    target: "chrome110",
    sourcemap: false,
  },

  test: {
    environment: "jsdom",
    include: ["tests/unit/**/*.test.ts"],
  },
}));
