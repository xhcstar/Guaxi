import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import path from "node:path";
import monacoEditorPlugin from "vite-plugin-monaco-editor";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [
    vue(),
    monacoEditorPlugin.default({
      // 注意这里的 .default
      publicPath: "assets/monaco-editor",
      customWorkers: [
        {
          label: "json",
          entry: "monaco-editor/esm/vs/language/json/json.worker",
        },
        {
          label: "css",
          entry: "monaco-editor/esm/vs/language/css/css.worker",
        },
        {
          label: "html",
          entry: "monaco-editor/esm/vs/language/html/html.worker",
        },
        {
          label: "typescript",
          entry: "monaco-editor/esm/vs/language/typescript/ts.worker",
        },
        {
          label: "editor",
          entry: "monaco-editor/esm/vs/editor/editor.worker",
        },
      ],
    }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
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
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  optimizeDeps: {
    include: ["monaco-editor", "monaco-vim"],
  },
}));
