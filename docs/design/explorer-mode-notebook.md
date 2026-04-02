# 探索者模式实现说明

## 1. 目标

探索者模式的目标，是在同一个 Markdown 文档之上，提供一层接近 Notebook 的交互视图，同时不破坏原始 Markdown 作为事实来源的地位。

这套实现需要同时满足几件事：

1. Markdown 仍然是主存储格式。
2. Python Cell 可以在文档中被识别、运行、写回结果。
3. 探索者模式自己的状态可以持久化，并在下次打开时恢复。
4. 编辑模式、预览模式、探索者模式之间可以互相切换。
5. 在文档较大、Cell 较多时，性能仍然可接受。

当前实现的核心前端文件是：

- [GuaxiNotebookView.vue](/e:/Record/source/Vue/guaxi/src/components/HomeViewComponent/GuaxiNotebookView.vue)
- [GuaxiNotebookPythonEditor.vue](/e:/Record/source/Vue/guaxi/src/components/HomeViewComponent/GuaxiNotebookPythonEditor.vue)
- [guaxiNotebookCells.js](/e:/Record/source/Vue/guaxi/src/components/HomeViewComponent/utils/guaxiNotebookCells.js)
- [guaxiPythonCellSyntax.js](/e:/Record/source/Vue/guaxi/src/components/HomeViewComponent/utils/guaxiPythonCellSyntax.js)
- [guaxiPythonCellState.js](/e:/Record/source/Vue/guaxi/src/components/HomeViewComponent/utils/guaxiPythonCellState.js)
- [notebookMarkdownRenderWorker.js](/e:/Record/source/Vue/guaxi/src/workers/notebookMarkdownRenderWorker.js)

对应后端核心文件是：

- [notebook.rs](/e:/Record/source/Vue/guaxi/src-tauri/src/commands/notebook.rs)
- [fileprocess.rs](/e:/Record/source/Vue/guaxi/src-tauri/src/commands/fileprocess.rs)
- [main.rs](/e:/Record/source/Vue/guaxi/src-tauri/src/main.rs)

## 2. 整体架构

探索者模式采用“源 Markdown + sidecar 状态层 + 运行时快照”的三层结构。

### 2.1 源 Markdown

源 Markdown 是唯一的正文事实来源。Python Cell 仍然以内嵌注释块形式存储，例如：

```markdown
<!--py-->
print("hello")
<!--/py-->
```

探索者模式不会改变这一基本事实。所有 Cell 拆分、写回、重建，最终都要回到源 Markdown。

### 2.2 sidecar 状态层

探索者模式的附加状态保存在和原文件同目录的 `原文件名.cellmeta` 中。

sidecar 负责保存的内容包括：

1. 探索者模式中的 Cell 结构。
2. Python Cell 的稳定 `meta.id`。
3. 折叠状态、标题等 UI 元信息。
4. Python 输出快照。
5. 重扫源 Markdown 后用于恢复的关系描述。

这里选择 sidecar，而不是把元信息直接塞回 Markdown 注释块，原因是：

1. 不污染用户正常编辑的 Markdown 源文。
2. 不影响预览模式渲染。
3. 更适合保存结构化状态和快照。

### 2.3 运行时快照层

Python Cell 的运行结果、变量、可视化输出等，在前端运行时通过 `guaxiPythonCellState` 管理，并可同步进 sidecar。

这一层关注的是：

1. 当前会话输出。
2. 当前变量列表。
3. 输出面板折叠状态。
4. 可视化产物引用。

## 3. 源 Markdown 到 Cell 的构建流程

### 3.1 前端职责

前端的 `GuaxiNotebookView.vue` 负责：

1. 触发同步。
2. 管理本地 Cell 状态。
3. 处理历史栈、焦点、交互行为。
4. 将渲染结果、搜索结果、运行结果投影回界面。

### 3.2 后端职责

后端 `notebook_sync_from_markdown` 负责：

1. 规范化 Markdown 文本。
2. 剥离写回输出块，计算 digest。
3. 扫描 Python Cell。
4. 将普通 Markdown 切成 Markdown Cell。
5. 结合 sidecar 恢复稳定 `meta.id` 和已有元信息。

这个命令定义在 [notebook.rs](/e:/Record/source/Vue/guaxi/src-tauri/src/commands/notebook.rs) 中，并在 [main.rs](/e:/Record/source/Vue/guaxi/src-tauri/src/main.rs) 注册。

### 3.3 重扫与关系重建

当前实现不是简单地“源文一变就丢弃 sidecar”，而是：

1. 重新扫描整个源 Markdown。
2. 生成新的扫描 Cell 列表。
3. 用稳定 `meta.id` 优先对齐。
4. 必要时退化到内容摘要匹配。
5. 恢复 sidecar 中的探索者模式状态。
6. 再把最新关系写回 sidecar。

这样即使用户在编辑模式下直接改了 Markdown，探索者模式也能尽量平稳重建。

## 4. 探索者模式中的 Cell 类型

### 4.1 Markdown Cell

Markdown Cell 的特点：

1. 预览态下直接渲染 Markdown。
2. 点击后切换到编辑态。
3. 不再保留外围冗余 layout 边框，视觉上更接近“文档内容本身”。
4. 字体、字号、链接颜色尽量向主预览模式对齐。

### 4.2 Python Cell

Python Cell 的特点：

1. 顶部工具栏提供运行、运行并写回、导入表格、变量区开关、输出区开关、删除等操作。
2. 当前不再显示 `Cell n` 这类编号。
3. 输出区支持文本、Markdown、HTML、SVG、图片、音频、视频、表格。
4. 输出可以导出到其他文件，也可以写回源 Markdown 对应位置。

## 5. Python Cell 的运行与写回

### 5.1 运行模型

当前实现是“同一文档共享一个 Python 会话”。

也就是说：

1. 同一文档下多个 Python Cell 共享上下文。
2. 变量、导入包、运行结果可以跨 Cell 延续。
3. 这更接近 Notebook 的使用体验。

提示文案层面，之前显式展示了“共享当前文档的 Python 会话”，后来为了减轻视觉噪音，已从界面去掉。

### 5.2 输出类型

当前 Python Cell 输出链路支持：

1. 文本输出。
2. Markdown 输出。
3. HTML 输出。
4. SVG 输出。
5. 图片输出。
6. 音频输出。
7. 视频输出。
8. DataFrame 表格输出。

### 5.3 写回源 Markdown 的规则

“写回源 Markdown”指的不是把结果随便存到旁边文件，而是把输出转换成 Markdown 可渲染的结构，并插回该 Python Cell 在源文中的对应位置。

当前规则如下：

1. 文本输出：写成 `text` 代码块。
2. 表格输出：转成 Markdown 表格。
3. 图片输出：先落资源库，拿到资源 URL，再写入 `![alt](url)`。
4. 音频输出：先落资源库，再写入 `<audio controls src="..."></audio>`。
5. 视频输出：先落资源库，再写入 `<video controls src="..."></video>`。
6. Markdown 输出：直接写回 Markdown 内容。
7. HTML/SVG 输出：按原内容写回。

### 5.4 二进制资源处理

对图片、音频、视频等二进制内容，处理策略是：

1. 先把二进制写入资源库。
2. 资源写入后生成可访问 URL。
3. 再把 URL 插入 Markdown。

这个策略与截图插入的资源处理思路保持一致，而不是把二进制直接塞回 Markdown。

### 5.5 路径与写文件保障

为了避免 Windows 上出现目录不存在导致的写回失败，后端 `write_file_bytes_base64` 已补了自动创建父目录逻辑。

## 6. sidecar 状态持久化

### 6.1 保存时机

下列操作会触发 sidecar 更新：

1. Cell 编辑。
2. Cell 插入、删除。
3. 折叠状态变化。
4. Python 运行。
5. 导入表格。
6. 写回 Markdown。

### 6.2 sidecar 的作用

sidecar 不是源文的替代物，而是探索者模式的增强层。

它主要解决这些问题：

1. 源 Markdown 不适合直接承载过多交互状态。
2. 探索者模式需要恢复空 Cell、顺序、折叠、输出快照。
3. 文档被外部修改后，需要有一层中间状态帮助重建。

### 6.3 稳定标识

为了让 Python Cell 在重扫后仍能对回原状态，系统会尽量保证每个 Python Cell 具有稳定 `meta.id`。

恢复顺序大致是：

1. 优先按 `meta.id` 匹配。
2. 若缺失，再退化到内容 digest 匹配。

## 7. 探索者模式的搜索实现

### 7.1 初版问题

初版搜索直接在前端 DOM 上做 TreeWalker 扫描并插入高亮节点，问题有两个：

1. 大文档时开销大。
2. 每次搜索都要重新扫 DOM，和探索者模式的整体性能目标相冲突。

### 7.2 当前方案

现在搜索改成“后端增量索引 + 前端结果定位”。

后端命令：

1. `notebook_update_search_index`
2. `notebook_search`

前端负责：

1. 在 Cell 内容变化时构造搜索文档。
2. 把搜索文档同步到后端缓存。
3. 用户输入关键词时调用后端查询。
4. 将匹配结果映射为 Cell 级高亮和跳转。

### 7.3 搜索索引内容

每个 Cell 的搜索文档会尽量合并它的可搜索信息：

1. Markdown Cell：正文内容。
2. Python Cell：标题、代码、文本输出、Markdown 输出、HTML 文本摘要、表格文本、变量预览等。

### 7.4 当前界面表现

当前搜索不再对文本节点做逐字包裹高亮，而是：

1. 高亮匹配的 Cell。
2. 给当前激活结果增加更强的外描边。
3. 支持上一个、下一个跳转。

这种方案更轻，性能更稳，也减少了对渲染 DOM 的干预。

## 8. Markdown 渲染与性能优化

### 8.1 初版问题

初版探索者模式的 Markdown 渲染直接复用主线程 `renderMarkdown`，当 Cell 多、内容复杂时，容易卡住 UI。

### 8.2 当前方案

现在采用“两阶段渲染”：

1. Worker 阶段：
   - Markdown-it 渲染
   - 代码高亮
   - 目录生成
   - Task List 处理
   - Markdown 预处理
2. 主线程阶段：
   - Mermaid 最终 SVG 渲染
   - 数学公式节点整理
   - 缓存结果写回

### 8.3 为什么 Mermaid 还在主线程

Mermaid 最终渲染仍依赖 DOM，因此保留在主线程做最后一步收尾更稳妥。

### 8.4 Mermaid 主题策略

探索者模式里的 Mermaid 当前强制统一走亮色风格，而不是跟随亮暗主题变化。

这样做的原因是：

1. 保持和项目中既有 Mermaid 表现一致。
2. 避免相同文档在不同模式/主题下出现风格不一致的图表。

### 8.5 渲染缓存

当前还有一层基于内容 digest 的渲染缓存，用于减少重复渲染。

缓存对象主要包括：

1. Markdown 渲染结果缓存。
2. 媒体 URL 解析缓存。
3. 后端搜索索引缓存。

## 9. Python 编辑器按需挂载

### 9.1 目标

探索者模式里的 Python 输入区需要：

1. 具备更接近主编辑器的编辑体验。
2. 避免所有 Cell 一上来都初始化 Monaco，导致性能雪崩。

### 9.2 当前实现

当前新建了 [GuaxiNotebookPythonEditor.vue](/e:/Record/source/Vue/guaxi/src/components/HomeViewComponent/GuaxiNotebookPythonEditor.vue)。

它的策略是：

1. 默认只渲染一个轻量占位容器。
2. 通过 `IntersectionObserver` 判断是否接近视口。
3. 接近视口时再动态加载 Monaco。
4. 远离视口的 Cell 不提前初始化编辑器。
5. 如果 Monaco 加载失败，回退到普通 `textarea`。

### 9.3 当前能力边界

这版 Monaco 子编辑器已经具备：

1. Python 语法高亮。
2. Monaco 自带编辑体验。
3. 懒加载和按需挂载。

但它还没有完全接上主编辑器现有的更完整语言能力，例如：

1. 主编辑器级别的 LSP 连接复用。
2. 更强的 Python 补全。
3. IPython magic 等更深层兼容。

这部分属于下一阶段增强。

## 10. 历史栈、保存与安全交互

### 10.1 历史栈

探索者模式内部维护了一套独立历史栈，用来支持：

1. 撤销。
2. 重做。
3. 防止误删 Cell 后无法恢复。

### 10.2 快捷键

当前已和主编辑器行为对齐：

1. `Ctrl/Cmd + S` 保存当前文档。
2. `Ctrl/Cmd + Z` 撤销。
3. `Ctrl/Cmd + Y` 重做。
4. `Ctrl/Cmd + Shift + Z` 重做。
5. `Ctrl/Cmd + F` 打开探索者模式搜索。

### 10.3 删除确认

Cell 删除不再直接生效，而是先确认，避免误操作。

## 11. 当前的主要性能策略

当前探索者模式已经应用的主要性能策略包括：

1. Markdown 切分与 sidecar 对齐迁到 Rust 后端。
2. Markdown 渲染主计算迁到 Worker。
3. 搜索索引改为后端增量缓存。
4. Python 编辑器改为按需挂载 Monaco。
5. Markdown 回写改为延迟 flush，避免每次输入都整篇序列化。
6. 多处结果使用 digest 缓存复用。

## 12. 当前已知边界与后续建议

### 12.1 已知边界

当前仍然存在这些边界：

1. Notebook Python Monaco 还没有完全接入主编辑器现有 LSP 链路。
2. IPython 风格的 magic 命令、特殊执行语义还不是完整实现。
3. Mermaid 最终渲染仍在主线程，只是前面的重活已经移到 Worker。
4. 搜索当前是 Cell 级高亮，不是全文逐词高亮。

### 12.2 后续建议

如果后续继续增强，建议优先顺序如下：

1. 让 `GuaxiNotebookPythonEditor` 复用主编辑器现有 Python/LSP 能力。
2. 进一步把 Python 会话和编辑器提示系统打通。
3. 视需要为搜索增加“结果摘要面板”和“命中词片段高亮”。
4. 如果 Mermaid 成本仍高，再考虑更细粒度的懒渲染策略。

## 13. 设计原则总结

这套探索者模式实现背后的核心原则是：

1. 源 Markdown 始终是正文真相。
2. sidecar 只承载探索者模式附加状态，不篡改正文事实。
3. 性能优化优先把重计算移出主线程。
4. 交互上优先防误删、防状态丢失、防切换模式后体验断裂。
5. 样式和行为尽量与主编辑器、预览模式保持一致，但在必要时为 Notebook 场景做局部特化。
