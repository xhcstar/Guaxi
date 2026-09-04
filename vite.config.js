import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import path from "node:path";
import monacoEditorPlugin from "vite-plugin-monaco-editor";

const host = process.env.TAURI_DEV_HOST;
const projectRoot = path.resolve(__dirname);
const ignoredBuildWarnings = [
  /Module level directives cause errors when bundled, "use client" .* was ignored\./,
  /dynamic import will not move module into another chunk\./,
];

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  // Design note:
  // Vite 6 emits the transformed HTML through Rollup. On Windows, when the
  // current working directory and Vite's normalized HTML id disagree on drive
  // casing, Rollup can receive an absolute fileName such as E:/.../index.html.
  // Pinning the root keeps HTML output names relative and production builds stable.
  root: projectRoot,
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
      "excalidraw-dev",
      // 这两个包只被 @codingame/monaco-vscode-api 内部的**动态** import 引用
      //（textfile/common/encoding.js 里的 encodingExists / DecoderStream / guessEncodingByBuffer），
      // 而那个包在下面的 exclude 里：依赖扫描器既进不去被 exclude 的包，也看不到动态 import，
      // 所以它们永远不会被自动发现（force: true 也救不了，扫描器压根没见过它们）。
      //
      // 不预打包的后果不是"没优化"，是直接坏掉：
      // iconv-lite-umd 是个 UMD 产物、一条 export 语句都没有，被原样当原生 ESM 下发时，
      // UMD 头部的 exports/module/define 三个探测全部落空，于是走最后那条"挂全局"分支，
      // 把 encodingExists/getDecoder 等 16 个函数拷到 window 上；import() 拿回来的
      // 命名空间则是空的，`module.default ?? module` 得到空对象，于是
      // `iconv.encodingExists is not a function`——表现为打开任意需要校验非 utf8 编码的文件时，
      // createModelReference 抛错、vscodeModelBridge 报 "retain model reference failed"，
      // 该文件拿不到 VSCode 侧的 model reference（LSP 文档同步/hover/补全都挂不上）。
      // 写进 include 后 esbuild 会做 CJS→ESM interop，default 和具名导出都有，取值就正常了。
      //
      // jschardet 同理（encoding.js 里 files.autoGuessEncoding 走的那条动态 import，纯 CJS）。
      "@vscode/iconv-lite-umd",
      "jschardet"
    ],
    exclude: [
      "@codingame/monaco-vscode-api",
      "vscode",
      "vscode/localExtensionHost"
    ],
    force: true,
    esbuildOptions: {
      target: "es2022",
    },
  },
}));
