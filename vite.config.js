import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import path from "node:path";
import monacoEditorPlugin from "vite-plugin-monaco-editor";

const host = process.env.TAURI_DEV_HOST;
const ignoredBuildWarnings = [
  /Module level directives cause errors when bundled, "use client" .* was ignored\./,
  /dynamic import will not move module into another chunk\./,
];

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
          label: "editorWorkerService",
          entry: "monaco-editor/esm/vs/editor/editor.worker",
        },
      ],
    }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      path: "path-browserify",
      react: path.resolve(__dirname, "node_modules/react"),
      "react-dom": path.resolve(__dirname, "node_modules/react-dom"),
      "react/jsx-runtime": path.resolve(__dirname, "node_modules/react/jsx-runtime.js"),
      "react/jsx-dev-runtime": path.resolve(__dirname, "node_modules/react/jsx-dev-runtime.js"),
      "excalidraw-dev": path.resolve(__dirname, "node_modules/@excalidraw/excalidraw/dist/dev/index.js"),
      "excalidraw-dev-css": path.resolve(__dirname, "node_modules/@excalidraw/excalidraw/dist/dev/index.css"),
    },
    dedupe: ["react", "react-dom", "react/jsx-runtime", "react/jsx-dev-runtime"],
  },

  worker: {
    format: "es",
  },

  build: {
    target: "es2022",
    reportCompressedSize: false,
    chunkSizeWarningLimit: 20000,
    rollupOptions: {
      onwarn(warning, defaultHandler) {
        const message = String(warning?.message || "");
        if (ignoredBuildWarnings.some((pattern) => pattern.test(message))) {
          return;
        }
        defaultHandler(warning);
      },
      output: {
        manualChunks: {
          vue: ["vue", "vue-router", "pinia"],
          naiveui: ["naive-ui", "@vicons/antd", "@vicons/fa", "@vicons/fluent", "@vicons/tabler", "@vicons/ionicons5"],
          monaco: ["monaco-editor", "monaco-vim", "monaco-editor-textmate", "monaco-languageclient", "monaco-textmate"],
          excalidraw: ["react", "react-dom", "excalidraw-dev"],
          docs: ["pdfjs-dist", "docx-preview", "epubjs", "markdown-it", "markdown-it-mathjax3", "mathjax-full", "mermaid", "vditor", "prismjs", "highlight.js"],
          graph3d: ["three", "three-stdlib", "d3-force-3d"],
        },
      },
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
    include: [
      "monaco-editor", 
      "monaco-vim",
      "react",
      "react-dom",
      "excalidraw-dev"
    ],
    esbuildOptions: {
      target: "es2022",
    },
  },
}));
