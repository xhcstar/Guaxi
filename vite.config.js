import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import fs from "node:fs";
import path from "node:path";
import monacoEditorPlugin from "vite-plugin-monaco-editor";

const host = process.env.TAURI_DEV_HOST;
const projectRoot = path.resolve(__dirname);
const ignoredBuildWarnings = [
  /Module level directives cause errors when bundled, "use client" .* was ignored\./,
  /dynamic import will not move module into another chunk\./,
];

/**
 * 独立阅读器窗口（doc-reader.html）的源码子树根。
 *
 * 这些目录/文件里的模块必须整体钉进 reader-core，见下面 manualChunks 的说明。
 */
const READER_SUBTREE_PATHS = [
  "/src/components/HomeViewComponent/ReaderPane.vue",
  "/src/components/ReaderComponent/DocReaderWindow.vue",
  "/src/components/ReaderComponent/readers/",
  "/src/components/PdfViewer/",
  "/src/components/HomeViewComponent/GuaxiSelectionTranslatePanel.vue",
  // 通用 composable / util。阅读器与主应用都在用，正是它们被 Rollup 并进 main 的：
  // reader chunk 一旦从 main 取这几个导出，整条静态边就把主应用入口拉了进来。
  "/src/composables/useReaderReadAloud.js",
  "/src/composables/useSelectionReadAloud.js",
  "/src/composables/useSelectionTranslate.js",
  "/src/composables/usePageTranslate.js",
  "/src/composables/useReadAloud.js",
  "/src/utils/readaloudEvents.js",
  "/src/utils/textOffsets.js",
  "/src/utils/logger.js",
];

/**
 * 扫描阅读器子树，收集它实际 import 的 @iconify 图标名。
 *
 * 图标是纯数据模块（`{width,height,body}`），阅读器放进了近 40 个，主应用也注册了一大批。
 * 不钉住它们，它们就会成为 reader 与 main 之间又一条共享边；整包钉
 * （`@iconify-icons/lucide/`）又会把主应用那 200 个图标一起搬进 reader-core。
 * 所以这里按实际引用精确收集：少一个的后果是那个图标退回成 main 里的共享边，
 * 多一个只是多几百字节。
 */
const collectReaderIconIds = () => {
  const icons = new Set();
  const iconRe = /@iconify-icons\/([a-z0-9-]+)\/([a-z0-9-]+)/g;
  const walk = (rel) => {
    const abs = path.join(projectRoot, rel);
    if (!fs.existsSync(abs)) return;
    const stat = fs.statSync(abs);
    if (stat.isDirectory()) {
      for (const entry of fs.readdirSync(abs)) walk(path.posix.join(rel, entry));
      return;
    }
    if (!/\.(vue|js|ts)$/.test(abs)) return;
    const text = fs.readFileSync(abs, "utf8");
    for (const m of text.matchAll(iconRe)) icons.add(`/node_modules/@iconify-icons/${m[1]}/${m[2]}.js`);
  };
  for (const rel of READER_SUBTREE_PATHS) walk(rel);
  return icons;
};

const READER_ICON_IDS = collectReaderIconIds();

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
    {
      // 阅读器入口（doc-reader.html）是「秒开」的关键路径：它只挂阅读器子树。
      // 文件名刻意不叫 reader.html：知识库的旧阅读窗口走主应用路由 `/reader`，
      // 而 Vite 的 html fallback（以及 Tauri 的资源 fallback）会把 `/reader`
      // 解析成 `reader.html`，两个入口撞名后旧窗口就再也进不了主应用。
      // Rollup 仍会给它加一条对 docs chunk（vditor/mermaid/markdown-it 等，约 8 MB）
      // 的纯副作用 import —— 入口与 docs 之间并没有任何真实依赖（doc-reader.html 的
      // 静态图只有 vue / reader-core / 自己的样式）。这条 import 会让每个阅读器
      // 窗口白下载并执行一整个无关的文档渲染大包，所以在这里从入口 chunk 里摘掉。
      // 只动 isEntry 且 name === "doc-reader" 的 chunk，主应用不受影响。
      name: "reader-entry-drop-unused-docs",
      apply: "build",
      enforce: "post",
      generateBundle(_options, bundle) {
        const entry = Object.values(bundle).find(
          (item) => item.type === "chunk" && item.isEntry && item.name === "doc-reader"
        );
        if (!entry) return;
        const dropped = entry.imports.filter((file) => /(^|\/)docs-[^/]*\.js$/.test(file));
        if (!dropped.length) return;
        entry.imports = entry.imports.filter((file) => !dropped.includes(file));
        for (const file of dropped) {
          const base = file.split("/").pop().replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
          entry.code = entry.code.replace(new RegExp(`import"\\./${base}";?`, "g"), "");
        }
        // HTML 里的预加载/样式链接由 Vite 的构建期在 chunk 依赖快照上生成，
        // 这里一并摘掉 docs 的两种 link（同样的理由：阅读器不需要它们）。
        const html = bundle["doc-reader.html"];
        if (html && html.type === "asset") {
          html.source = String(html.source).replace(
            /<link[^>]*(?:modulepreload|stylesheet)[^>]*href="\/assets\/docs-[^"]*"[^>]*>\s*/g,
            ""
          );
        }
        console.log("[reader-entry] dropped unused imports:", dropped);
      },
    },
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
      // 独立阅读器窗口走自己的入口（doc-reader.html）：它只需要阅读器子树，不能把
      // 整个应用（Monaco / LSP / 所有路由页面）的启动成本带进每一个新窗口。
      input: {
        main: path.resolve(projectRoot, "index.html"),
        "doc-reader": path.resolve(projectRoot, "doc-reader.html"),
      },
      onwarn(warning, defaultHandler) {
        const message = String(warning?.message || "");
        if (ignoredBuildWarnings.some((pattern) => pattern.test(message))) {
          return;
        }
        defaultHandler(warning);
      },
      output: {
        // 原来的对象写法把「包名 → chunk 名」一一列出；这里改成等价的函数形式，
        // 只为多钉一个虚拟模块：Vite 的动态 import 预加载 helper（vite/preload-helper）。
        // 不钉的话 Rollup 会把它塞进「第一个用到它的 chunk」，实测落进了 9.8 MB 的
        // monaco —— 阅读器入口只 import 了这一个 helper，却因此把整个 Monaco 变成静态
        // 依赖并预加载，doc-reader.html 的「秒开」收益全没了。
        manualChunks(id) {
          // —— 阅读器窗口的独立闭包（顺序敏感，必须排在所有通用分组之前）——
          //
          // 背景：阅读器窗口走独立入口 doc-reader.html，但它与主编辑器共用 ReaderPane。
          // Rollup 把「两端都会用到的模块」归进了主应用入口 chunk（main-*.js），而那个
          // chunk 顶层就执行着 main.js 的 bootstrap()。于是阅读器窗口一旦异步加载任一
          // 阅读器（DocEbookReader 等），就会把整个主应用拉起来、挂到同一个 #app 上，
          // 把阅读器已经渲染好的 DOM 清空 —— release 里表现为「阅读器新窗口是空的」。
          // dev 下 Vite 按源码模块图逐个请求，不会执行 main.js，所以只有 release 复现。
          //
          // 修法：凡是阅读器子树用到的模块，一律钉进 reader-core，让 reader-core 成为
          // 自足闭包，从根上切断 reader → main 的静态边。
          //
          // 判断用正斜杠：Rollup 传给 manualChunks 的 id 永远是 `/` 分隔，与宿主平台无关。
          if (READER_SUBTREE_PATHS.some((p) => id.includes(p))) {
            return "reader-core";
          }
          if (READER_ICON_IDS.has(id.slice(id.indexOf("/node_modules/"))) || READER_ICON_IDS.has(id)) {
            return "reader-core";
          }

          // 阅读器自己的两份 CSS 单独成 chunk。不钉的话，它们的空模块会被 Rollup
          // 并进 docs chunk，阅读器入口就会为了这点样式去静态加载 pdfjs/vditor 那个大包。
          if (id.includes("/src/styles/guaxiBase.css") || id.includes("/readers/readerPane.css")) {
            return "reader-styles";
          }
          // 阅读器共用的外壳样式同理。
          if (id.includes("/src/components/ReaderComponent/readerChrome.css")) {
            return "reader-styles";
          }
          if (id.includes('vite/preload-helper') || id.includes('vite/modulepreload')) {
            return "shared";
          }
          const groups = {
            vue: ["vue", "vue-router", "pinia"],
            naiveui: ["naive-ui", "@vicons/antd", "@vicons/fa", "@vicons/fluent", "@vicons/tabler", "@vicons/ionicons5"],
            monaco: ["monaco-editor", "monaco-vim", "monaco-editor-textmate", "monaco-languageclient", "monaco-textmate"],
            excalidraw: ["react", "react-dom", "excalidraw-dev"],
            docs: ["docx-preview", "epubjs", "markdown-it", "markdown-it-mathjax3", "mathjax-full", "mermaid", "vditor"],
            pdf: ["pdfjs-dist"],
            syntax: ["highlight.js", "prismjs"],
            graph3d: ["three", "three-stdlib", "d3-force-3d"],
          };
          for (const [name, packages] of Object.entries(groups)) {
            if (packages.some((pkg) => id.includes(`/node_modules/${pkg}/`) || id.endsWith(`/node_modules/${pkg}`))) {
              return name;
            }
          }
          return undefined;
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
