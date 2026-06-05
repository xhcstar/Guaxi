# 探索者模式机制详解：渲染 / 同步 / 运行 / 各 Cell 的加载·保存·输出·导出

> 本文是 [`explorer-mode-notebook.md`](./explorer-mode-notebook.md) 的「机制细化版」。前者讲设计目标与原则，本文聚焦**具体数据流与关键函数**，便于排查问题与二次开发。文中 `file:line` 为定位锚点，代码演进后以函数名为准。

涉及的核心文件：

| 层 | 文件 |
| --- | --- |
| 探索者主视图 | `src/components/HomeViewComponent/GuaxiNotebookView.vue` |
| Python 编辑器（按需挂载 Monaco） | `src/components/HomeViewComponent/GuaxiNotebookPythonEditor.vue` |
| Math 编辑器（Monaco + LaTeX） | `src/components/HomeViewComponent/GuaxiNotebookMathEditor.vue` |
| Python Cell 哨兵语法 | `utils/guaxiPythonCellSyntax.js` |
| Math Cell 哨兵语法 | `utils/guaxiMathCellSyntax.js` |
| Cell 序列化/反序列化 | `utils/guaxiNotebookCells.js` |
| Python Cell 运行时快照 | `utils/guaxiPythonCellState.js` |
| Markdown 渲染 Worker | `src/workers/notebookMarkdownRenderWorker.js` |
| 后端同步/搜索 | `src-tauri/src/commands/notebook.rs` |
| 导出 | `src/components/HomeViewComponent/GuaxiEditor.vue`（export 相关） |

---

## 0. 三层模型与「sidecar」

探索者模式是「**同一份 `.md` 上的 Notebook 视图**」，采用三层结构：

1. **源 Markdown（唯一正文真相）**：磁盘上的 `.md`。Python / Math Cell 在源里只保留**轻量哨兵引用块**（带稳定 `id`），真实正文不内联。
2. **sidecar 状态层（`.cellmeta`）**：与源同目录的伴生文件 `原文件名.md.cellmeta`（JSON）。
3. **运行时快照层**：前端内存里的执行结果 / 变量 / 折叠状态，可同步进 sidecar。

### sidecar 是什么

- **文件名**：`getNotebookSidecarPath()` → `${filePath}.cellmeta`（`GuaxiNotebookView.vue:1608`）。
- **为什么要它**：源 `.md` 要保持干净可读、可被预览/导出正常渲染，不适合塞大量交互状态；所以把「重型/结构化状态」另存一处，用稳定 `id` 与源里的哨兵配对。
- **存了什么**（payload 结构，约 `buildNotebookSidecarPayload()` `:2143`）：
  ```jsonc
  {
    "version": 4,
    "filePath": "...",
    "updatedAt": "ISO8601",
    "sourceDigest": "nb-<hash>-<len>",   // 剥离输出块后的源 Markdown 摘要，用于校验 sidecar 是否仍匹配
    "cells": [ /* NotebookCell[]，见下 */ ]
  }
  ```
- **每个 cell 字段**：`type`(markdown/python/math)、`key`、`content`(markdown 正文)、`code`(python 源码)、`expression`(数学表达式)、`meta`(`id` 稳定标识、`title`、`storage:"cellmeta"`、各类折叠位)、以及 Python 输出快照（`outputs[]` / 兼容旧的 `outputText/display/dataframe`）。

> 关键点：**py/math 的正文（代码、表达式）和输出快照存在 sidecar**，源 `.md` 里只有引用哨兵。正因如此，任何涉及 cell 序列化/回填的改动，前端兜底 reconcile 与后端 `reconcile_cells_with_sidecar` 必须同步实现（见 §2.3）。

---

## 1. 渲染机制

### 1.1 两阶段渲染（Worker + 主线程）

为避免 Cell 多时主线程卡顿，Markdown 渲染拆两阶段：

1. **Worker 阶段** `renderMarkdownHtmlWithWorker()`（`:2982`）：markdown-it 解析、代码高亮、目录、Task List、预处理 → 返回「半成品 HTML」。
2. **主线程收尾** `finalizeNotebookRenderedHtml()`（`:3087`）：Mermaid 最终 SVG、数学公式节点整理（MathJax）、媒体分组等依赖 DOM 的收尾。

统一入口 `renderMarkdownHtml(content, reason)`（`:5164`）：先查缓存，未命中走 worker（不可用则主线程 `renderMarkdown` 兜底）。

- **缓存**：`buildNotebookMarkdownCacheKey(text, {theme, filePath, basePath})`（`:2799` 附近）→ `notebookMarkdownRenderCache`(Map)。主题/路径/内容任一变化即换键。
- **结果存放**：`renderedMarkdownHtmlMap`（`:977`，`{cellKey: html}`）；输出模块的 markdown 渲染存 `renderedOutputMarkdownHtmlMap`（键 `buildCellOutputModuleCacheKey`）。
- **触发**：`syncRenderedMarkdownHtml(cells, reason)`（`:5238` 附近）由 `notebookCells` 的 watch 调用（`:8770`）。
- **渲染后增强**：`renderedMarkdownHtmlMap` 的 watch 在 `nextTick` 里对落地 DOM 调用 `injectNotebookCopyButtons()`（给代码块补复制按钮）与搜索高亮。

### 1.2 三个渲染入口（模板用）

- `getRenderedMarkdownCellHtml(cell)`（`:5416`）：Markdown Cell 预览 HTML。
- `getRenderedMathCellHtml(cell)`（`:5417`）：Math Cell 预览 HTML（表达式先转预览 Markdown）。
- `getRenderedOutputMarkdownHtml(cell, outputModule)`（`:5419`）：Python Cell 的 Markdown 类输出。

### 1.3 编辑模式下的折叠（源码视图）

主编辑器（`GuaxiEditor.vue`）对源 `.md` 注册了一个 markdown `FoldingRangeProvider`：

- 哨兵块（`<!--py-->`/`<!--math-->`/`<!-- guaxi:pyout:…:start/end -->`）→ `Region` 类型，载入后由 `autoFoldNotebookSentinels()` **默认折叠**。
- ATX 标题（`#`~`######`）→ `computeMarkdownHeadingFoldRanges()` 生成可折叠区间，但**默认展开**（仅提供折叠箭头）。

---

## 2. 同步机制（源 Markdown ↔ Cells）

### 2.1 进入探索者模式：构建 cells

主路径是 **Rust 后端命令 `notebook_sync_from_markdown`**（`notebook.rs`）：

1. `normalize_markdown_text()` 规范化；
2. `strip_output_blocks()` 剥离 `<!-- guaxi:pyout:…:start/end -->` 输出块；
3. `compute_digest()` 算源摘要（用于校验 sidecar）；
4. `parse_python_cell_blocks()` / `parse_math_cell_blocks()` 扫描哨兵；
5. `split_markdown_into_notebook_cells()` 把普通 Markdown 切成 cell；
6. **`reconcile_cells_with_sidecar()`**（`notebook.rs:980` 附近）：用 sidecar 回填 py/math 引用块的正文与 meta（见 2.3）。

前端 `hydrateCellsFromMarkdown(markdown, reason)`（`:2245` 附近）调用后端；**后端失败才走前端兜底** `createNotebookCellsFromMarkdown()`（`guaxiNotebookCells.js:319`）+ `reconcileNotebookCellsWithSidecar()`（`:1876`）。最后 `decorateNotebookCells()` 补 `cellId/index` 等运行时信息。

### 2.2 py/math 在源里是「引用块」

- Python：`buildPythonCellReferenceBlock()`（`guaxiPythonCellSyntax.js:89`）→
  ```
  <!--py {"id":"py-xxx","storage":"cellmeta"}-->
  <!--/py-->
  ```
- Math：`buildMathCellReferenceBlock()`（`guaxiMathCellSyntax.js`）→ `<!--math {"id":"…","storage":"cellmeta"}--> <!--/math-->`
- 正文（`code`/`expression`）存 sidecar `cells[]`。
- **兼容**：旧文档里的内联完整块（`<!--py-->code<!--/py-->`）仍能被 `parsePythonCellBlocks` 解析，下次保存时自动迁移为引用块。
- `$$...$$` / `\[...\]` / ` ```math ` **不再**被当作 math cell，保持普通 Markdown。

### 2.3 外部改源后如何不丢内容

`reconcile_cells_with_sidecar` 的对齐优先级：

1. **稳定 `meta.id`** 精确匹配（最可靠）；
2. `meta.id` 缺失时退化到**内容 digest 兜底**（仅当候选唯一）；
3. Markdown cell 始终以**最新源文本**为准，不被 sidecar 覆盖；
4. 引用块正文为空时从 sidecar 恢复 `code/expression`，meta 以「源覆盖 sidecar」合并。

若 `sourceDigest` 不匹配（源被外部改过），会在重建后 `schedulePersistNotebookSidecar('rescan-source-markdown')` 重写 sidecar，使两者重新一致。

> ⚠️ 危险点：reconcile 没成功恢复后，紧接着的 sidecar 重持久化会用空正文覆盖 sidecar 造成永久丢失——所以**前端兜底 reconcile（`reconcileNotebookCellsWithSidecar`）与后端 `reconcile_cells_with_sidecar` 必须同步实现**，且对引用块（空正文）务必用稳定 `meta.id` 匹配，不能只靠 digest。

### 2.4 回写源 Markdown（编辑模式）

- 序列化：`serializeNotebookCells()`（`guaxiNotebookCells.js:278`）。markdown→`content`；python→引用块；math→按 `sourceType`（marker/fenced/bracket/dollar）选不同包裹。
- flush 时机：
  - **结构性操作立即同步**（插入/删除/运行写回）：`flushNotebookMarkdown(reason)`（`:3832`）；
  - **内容编辑延迟回写**（防抖 ~220ms）：`scheduleFlushNotebookMarkdown(reason)`（`:3844`），避免每次按键整篇序列化。
- flush 后 `emit('update:markdown', …)` 给上层，并 `schedulePersistNotebookSidecar(...)` 写 sidecar。

### 2.5 sidecar 持久化时机

`schedulePersistNotebookSidecar()`（`:2175`，~220ms 防抖）由这些操作触发：cell 编辑、插入/删除、折叠变化、Python 运行、导入数据、写回 Markdown。实际写盘 `persistNotebookSidecar()`（`:2156`）→ `invoke('write_file_content', { path: sidecarPath, content })`。加载 `loadNotebookSidecar()`（`:2191`）→ `invoke('read_file_content')` + 校验 `sourceDigest`。

---

## 3. 运行机制

### 3.1 Python Cell 运行

- 入口：`handleActivePythonToolbarAction('run' | 'run-writeback')`（`:6604`）→ `runPythonCell(cell, { writeBack })`（`:8542`）。
- 后端调用：`invoke('python_exec', { request: { code, persist, expect_json, request_id, vars: { _guaxi_session, _doc_path, _py_cell_id } } })`（`:8569`）。
- **会话**：`createPythonSessionId(filePath)`（`guaxiPythonCellSyntax.js:282`）= `filePath || '__guaxi_global__'`——**同一文档的所有 Python Cell 共享一个会话**，变量/导入跨 cell 延续。
- 返回结构关键字段：`ok`、`session_id`、`session_vars[]`、`result_json.__guaxi_display__`、`__guaxi_dataframe__` / `__guaxi_tables__[]`、`__guaxi_dataflow__`。
- 转输出模块：`buildPythonCellOutputModules({outputText, display, dataframe, tables, source})`（`guaxiPythonCellOutputs.js` 附近）。`kind ∈ {text, markdown, html, svg, image, audio, video, table}`。
- 取模块：`getCellOutputModule(cell, moduleId)`（`:4688`）/ `getCellOutputModules(cell)`。

### 3.2 「仅运行」vs「运行并写回」

| | writeBack:false | writeBack:true |
| --- | --- | --- |
| 行为 | 只更新内存快照 + sidecar | 额外把输出**写回源 Markdown** |
| 函数 | `runPythonCell(cell,{writeBack:false})` | 运行后 `writeCellOutputToMarkdown(cell)`（`:7389`） |

### 3.3 写回源 Markdown 的规则

- **定位/替换**：`writeMarkdownOutputBlock()`（`:6856`）用 `<!-- guaxi:pyout:<cellId>:start -->` … `:end -->` 包裹输出。再次运行时按 start/end **就地替换**已有输出（否则会重复堆叠）。**这对标记是承重锚点，不可删除**：`stripNotebookOutputBlocks()`（`:1613`）也靠它把输出从源摘要里剔除；去掉标记会导致 re-run 重复插入、sidecar digest 失配。编辑模式只是用折叠把它隐藏，并非删除。
- **各类输出转 Markdown**：`buildMarkdownOutputFromModule()`（`:7064`）：
  - 文本 → ` ```text ` 代码块；表格 → `dataframeToMarkdownTable()`；Markdown → 原样；SVG/图片/音视频 → `serializeDisplayAssetForMarkdown()` 先入库再插 URL（`![]()` / `<audio>` / `<video>`）。
  - 交互式 HTML（plotly）→ `rasterizePlotlyHtmlToPng()` 栅格化成静态 PNG（居中）+ HTML 整体入库，`buildInteractiveArtifactMarkup(pngUrl, htmlUrl)`（`:7046`）把「在浏览器中查看交互版本」入口放静态图**右下角**。
- **二进制入库**：`registerResourceFile({ bytes, resourceType, extension, ownerFilePath, cellId, source })` → 返回可访问 `url`，再插入 Markdown（不内联二进制）。

### 3.4 表格虚拟表与物化

- 后端返回 `resultHandle + rowCount + 前若干行`（虚拟表）。
- 写回/导出前 `ensureNotebookTableRowsMaterialized(table)`（`:7000`）分页 `invoke('python_runtime_get_table_rows', {...})`（每页 1000 行）补全。
- 导出表格：`python_runtime_export_table_result(...)`。

### 3.5 Math Cell 运行（与 Python 解耦）

- 编辑：`GuaxiNotebookMathEditor.vue`（Monaco + 自定义 `latex` 语言，已开括号/引号自动闭合）。
- 预览：`buildMathPreviewMarkdown(expression, mode)`（`:4065`）把表达式逐块包成 `$$…$$`，再走与 Markdown Cell 同一渲染链。
- 运行：走**独立**数学引擎 `src-tauri/scripts/guaximath_engine.py`（SymPy/NumPy/SciPy/Plotly），结果与符号表存 `mathWorkspaceStore`，承载层在右侧面板 `GuaxiNotebookMathWorkspacePanel.vue` / 独立窗口 `/math-workbench`。**不复用、不耦合** Python Cell 的会话与面板（独立 store，不碰 `rightSiderStore.pythonSessionInspector`）。详见 §7。

---

## 4. 各 Cell 的「加载 / 保存 / 渲染 / 输出 / 导出」一览

| 维度 | Markdown Cell | Python Cell | Math Cell |
| --- | --- | --- | --- |
| 源存储 | 正文直接在 `.md` | 引用块 `<!--py {id}-->`，代码存 sidecar | 引用块 `<!--math {id}-->`，表达式存 sidecar |
| 加载 | 后端切分 + reconcile | reconcile 按 `id`/digest 从 sidecar 回填 `code` | 同左，回填 `expression` |
| 编辑 | 行内 Monaco（markdown） | `GuaxiNotebookPythonEditor`（懒挂载 Monaco） | `GuaxiNotebookMathEditor`（Monaco latex） |
| 渲染 | `getRenderedMarkdownCellHtml` | 代码区 + 输出模块（`getRenderedOutputMarkdownHtml`） | `getRenderedMathCellHtml`（$$ 渲染） |
| 运行 | — | `python_exec`（共享会话） | `guaximath_engine.py`（独立工作区） |
| 输出 | — | text/md/html/svg/image/audio/video/table | LaTeX 公式 / 图像 / 数值结果（承载层） |
| 保存 | `content`→源；UI 态→sidecar | 引用块→源；代码+输出快照→sidecar | 引用块→源；表达式→sidecar |
| 写回源 | — | `writeMarkdownOutputBlock` + pyout 标记 | 默认**不**写回源（仅留 outputModule + sidecar 快照） |

---

## 5. 导出机制

入口 `handleExportSelect()`（`GuaxiEditor.vue:19105`）；支持 PDF / Word(.docx) / HTML / LaTeX(.tex) / EPUB / ODT / 纯文本。

1. **准备**：`prepareExportContext()`（`:18483` 附近）仅放行 `.md/.markdown/.tex`，取当前源 Markdown 并落盘。
2. **剥离交互工件（源级）**：`maskPythonCellBlocksForMarkdown` + `maskMathCellBlocksForMarkdown` 把哨兵块整段替换为空行（保留行号便于大纲/目录对齐），即**导出不渲染 cell 源码**。
3. **分链路**：
   - **PDF**：`exportMarkdownRenderedPdf()`（`:18653`）——同源预览渲染 HTML → `stripInteractiveExportArtifacts()`（`:18570`，移除 `.guaxi-pycell-toolbar` / `.guaxi-*-output` 等交互节点）→ Tauri `export_rendered_html_pdf`。
   - **HTML**：`exportMarkdownRenderedHtml()`（`:18640`）→ 补页面壳/CSS 写文件。
   - **Word/EPUB/ODT/LaTeX/文本**：`exportMarkdownPandoc()`（`:19069`）走后端 Pandoc。
   - **.tex→PDF**：`exportTexPdf()`（`:19094`）走 Tectonic。
4. **公式/图表前置渲染（Pandoc 链）**：
   - 块级数学 `prerenderDisplayMathForPandoc()`（`:18851`）：LaTeX→SVG→PNG（EPUB 阅读器不渲染 MathML，必须图片化）。
   - Mermaid `prerenderMermaidForPandoc()`（`:18927`）：用 `mermaid.run({nodes})` 在隐藏 DOM 布局取真实 SVG → 栅格化 PNG（`htmlLabels:false` 防丢字），替换为图片引用。
   - PDF/HTML 链则直接复用浏览器内 MathJax/Mermaid 渲染，保证「预览同源高保真」。

---

## 8. 关键约束（改动前必读）

- **同步主路径在 Rust**：`notebook_sync_from_markdown` / `reconcile_cells_with_sidecar`（`notebook.rs`）才是生效路径，前端 `reconcileNotebookCellsWithSidecar` 只是后端失败兜底。改 cell 序列化/回填，**前后端两处 reconcile 都要同步改**；引用块（空正文）必须用稳定 `meta.id` 匹配，digest 兜底对它无效。
- **pyout / py / math 外层标记不可删**：它们是 cell 定位与输出替换的锚点（§3.3、§2.3）。要让编辑模式清爽走折叠，不要剥离标记。
- **数学与 Python 解耦**：Math Cell 用独立引擎与独立 store（§7），不要把数学承载能力接到 Python Cell 的会话/面板上。

### 延伸阅读（项目内）

- [`explorer-mode-notebook.md`](./explorer-mode-notebook.md)：设计目标与原则（高层版）。
- [`notebook-math-workspace.md`](../notebook-math-workspace.md)：数学工作区使用说明。
- [`notebook-data-lab-api.md`](../notebook-data-lab-api.md)：数据实验室 API。
- 源码锚点：`GuaxiNotebookView.vue`、`notebook.rs`、`utils/guaxi*CellSyntax.js`、`utils/guaxiNotebookCells.js`、`src-tauri/scripts/guaximath_engine.py`。
