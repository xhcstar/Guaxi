import { code } from '../helpers.js'

export const pageChaptersA = [
  {
    id: 'home',
    group: '页面详解',
    number: '02',
    eyebrow: 'Page · Knowledge Editor',
    title: '知识编辑',
    subtitle: '写、找、读、导入，都从这一页开始',
    summary: '首页是主工作台：左边管文件，中间写内容，右边放辅助信息。搜索、导入、图表、分屏都在这里完成。',
    sections: [
      {
        id: 'home-layout',
        title: '页面结构',
        keywords: ['首页', '布局', '侧栏', '编辑器', '大纲', '搜索结果'],
        blocks: [
          {
            type: 'p',
            text: '首页是主要工作台。左侧栏可在知识库树、任务目录、普通目录和打开历史之间工作；中间是编辑器；右侧栏承载大纲、智能助手、搜索结果和数学工作区等辅助面板。',
          },
          { type: 'figure', image: 'home-editor', caption: '知识编辑：顶栏工具、左栏文件树、Markdown 编辑器与右侧缩略图。' },
          { type: 'figure', image: 'home-split', caption: '分屏预览：左侧渲染、右侧源码，适合边写边看排版。' },
          { type: 'figure', image: 'home-outline', caption: '右侧大纲：按标题层级快速跳转当前文档。' },
          {
            type: 'defs',
            items: [
              { term: '知识库树', text: '新建文件 / 文件夹、导入、重命名、删除、移动、在文件浏览器打开、Neovim 打开、浏览器预览。' },
              { term: '任务目录', text: '直接显示工作 / 生活 / 学习三类目录，可快速创建文件或导入任务。' },
              { term: '普通目录', text: '把任意本地目录当作临时工作区打开，并保留打开历史。' },
              { term: '右侧面板', text: '大纲、智能助手、搜索结果、数学工作区；可用 <kbd>Alt</kbd>+<kbd>]</kbd> 收起。' },
            ],
          },
        ],
      },
      {
        id: 'home-toolbar',
        title: '顶栏图标速查',
        keywords: ['顶栏', '工具栏', '图标', '新建知识库', '模板', '分屏', '截图', '新手引导'],
        blocks: [
          {
            type: 'p',
            text: '顶栏图标一律使用<b>两键</b>组合（<kbd>Alt</kbd>+字母），字母取自动作英文名的首字母；首字母被占用时换一个更有辨识度的字母。鼠标悬停图标就能看到属于它的快捷键。',
          },
          {
            type: 'table',
            head: ['图标', '作用', '快捷键'],
            rows: [
              ['书 +', '新建知识库', 'Alt+B'],
              ['树状列表', '打开当前文档大纲', 'Alt+D'],
              ['机器人', '打开或收起智能助手', 'Alt+A'],
              ['包裹', '模板管理', 'Alt+E'],
              ['投影幕', 'PPT 制作与管理', '无'],
              ['文件夹', '知识库资源管理', 'Alt+R'],
              ['清单', '任务管理面板', '无'],
              ['对比箭头', '差异对比（任意文件 / 目录）', '无'],
              ['语言', '语言学习窗口', '无'],
              ['相机', '截图并插入（可选是否隐藏窗口）', 'Alt+S'],
              ['左右栏', '纵向分屏', 'Alt+V'],
              ['上下栏', '横向分屏', 'Alt+H'],
              ['问号', '重新观看新手引导', '无'],
            ],
          },
          {
            type: 'p',
            text: '左右两端还有侧栏开关：<kbd>Alt</kbd>+<kbd>[</kbd> 展开 / 收起左侧栏，<kbd>Alt</kbd>+<kbd>]</kbd> 展开 / 收起右侧栏。主窗口最小化时，<kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>A</kbd> 是系统级唤醒键，会把窗口拉到前台并打开助手。',
          },
          { type: 'figure', image: 'window-new-kb', caption: '新建知识库：名称、标签与描述在同一个窗口内完成。' },
          { type: 'figure', image: 'window-diff', caption: '差异对比：选择左右文件或目录后开始比较。' },
        ],
      },
      {
        id: 'home-filetree',
        title: '文件树与目录',
        keywords: ['文件树', '新建文件', '文件夹', '导入', 'Git', 'Neovim', '浏览器查看'],
        blocks: [
          {
            type: 'list',
            items: [
              '知识库树支持新建文件 / 文件夹、导入 Markdown、重命名、删除、移动、在文件浏览器中打开、用 Neovim 打开、在浏览器中查看。',
              '在浏览器中查看只对 <code>.md</code> / <code>.markdown</code> 文件出现，用本地 8888 服务渲染成静态网页，便于对照与分享。',
              'Git 面板可从文件树打开当前工作区 Git 窗口，执行状态查看、暂存 / 取消暂存、提交、推送、拉取、分支切换、差异查看和 hunk 级操作。',
              '文件树三个快捷操作沿用三键组合：<kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>N</kbd> 新建文件、<kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>N</kbd> 新建文件夹、<kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>I</kbd> 导入文件。',
            ],
          },
          {
            type: 'callout',
            tone: 'tip',
            title: '工具栏与面包屑可以藏起来',
            text: '格式工具栏和面包屑会占两行高度。打开 PDF、电子书、图片这类只读内容时，点 Guaxi 标志右侧的收起图标或按 Alt+G，把高度让给正文；再点一次恢复，状态按窗口记住。',
          },
          { type: 'figure', image: 'window-git', caption: 'Git 面板：更改、暂存区、提交与提交历史。' },
        ],
      },
      {
        id: 'home-editor',
        title: '编辑器能力',
        keywords: ['Monaco', '编辑', '预览', '分屏', '表格', '格式化', '右键', 'Python'],
        blocks: [
          {
            type: 'list',
            items: [
              '支持 Markdown 编辑、预览、分屏预览和实时预览；<kbd>Ctrl</kbd>+<kbd>S</kbd> 保存，<kbd>Ctrl</kbd>+<kbd>F</kbd> 搜索，<kbd>Ctrl</kbd>+<kbd>1</kbd>/<kbd>2</kbd>/<kbd>3</kbd>/<kbd>4</kbd> 切换模式。',
              '选中文本后出现浮动工具条：加粗、斜体、删除线、链接、行内代码、引用。',
              '光标在 Markdown 表格内时出现表格工具条：增删行列、设置对齐、格式化。',
              '右键菜单包含剪切 / 复制 / 粘贴、文件内搜索、Markdown 全文或选区格式化、编辑位置跳转、插入模板和命令面板。',
              'Python 代码块、数据表格和数学内容可配合数据实验室、数学工作区和终端面板完成运行、分析、绘图与报告写回。',
            ],
          },
          {
            type: 'shortcuts',
            groups: [
              {
                title: '编辑器',
                rows: [
                  ['保存', 'Ctrl+S'],
                  ['文件内搜索', 'Ctrl+F'],
                  ['编辑 / 预览 / 分屏 / 实时预览', 'Ctrl+1 / 2 / 3 / 4'],
                  ['工具栏与面包屑显隐', 'Alt+G'],
                ],
              },
              {
                title: '文件树',
                rows: [
                  ['新建文件', 'Ctrl+Alt+N'],
                  ['新建文件夹', 'Ctrl+Alt+Shift+N'],
                  ['导入文件', 'Ctrl+Alt+I'],
                ],
              },
            ],
          },
          { type: 'figure', image: 'home-modes', caption: '知识编辑：编辑 / 分屏 / 大纲 / 助手状态切换（动图）。' },
          { type: 'figure', image: 'home-assistant', caption: '右侧智能助手：会话列表与输入区。' },
        ],
      },
      {
        id: 'home-open',
        title: '打开文件：按真实类型分流',
        keywords: ['打开文件', 'PDF', 'Word', 'EPUB', 'Excel', 'PPT', '图片', '音视频', 'HTML', '二进制'],
        blocks: [
          {
            type: 'p',
            text: '左栏文件树打开文件时先看扩展名，扩展名不认识时再读文件头魔数，所以把 PNG 改名成 <code>.txt</code> 也不会被当成文本硬读。文本类进入编辑器，其余类型自动切换成只读阅读器。',
          },
          {
            type: 'table',
            head: ['类型', '行为'],
            rows: [
              ['文本类（md / txt / 代码 / 配置 / csv / svg …）', '在主显示区用编辑器打开，可编辑可保存'],
              ['PDF', '内嵌 PDF 阅读器：连续滚动、搜索、分页、目录、文字高亮批注'],
              ['Word', '.docx 版式还原预览；.doc / .rtf / .odt 转为只读 Markdown'],
              ['电子书', 'epub / mobi / azw3 / azw / kfx / fb2 / lit / cbz，带目录与上一节 / 下一节'],
              ['表格', 'xlsx / xls / ods，按工作表分页，首行可切换表头，大表只渲染可视区域'],
              ['幻灯片', '.pptx 按原版式分页预览；.ppt / .odp 显示为转出的 Markdown 大纲'],
              ['图片', 'png / jpg / gif / webp / bmp / ico / tif / avif / heic，支持缩放、平移、旋转、适应窗口'],
              ['音视频', 'mp3 / wav / flac / ogg / m4a / mp4 / mkv / webm / mov 等，编码不支持时可「系统打开」'],
              ['HTML', '隔离沙箱渲染，默认禁止脚本，需要时可勾选「允许脚本」'],
              ['其它二进制', '显示大小与三个出口：文件浏览器、转换为 Markdown、编辑器打开'],
            ],
          },
          {
            type: 'callout',
            tone: 'warn',
            title: 'Office 系列是只读预览',
            text: '网页版 Office 会把文档上传，离线重新生成又会丢批注、修订与复杂域。本版只保证“看得准”，不提供“改得动”；需要修改请在 Word / PowerPoint 里编辑后重新打开。',
          },
        ],
      },
      {
        id: 'home-import',
        title: '导入文件：两种模式',
        keywords: ['导入', '原样导入', '转换', 'Markdown', 'pandoc', 'OCR', '资产目录'],
        blocks: [
          {
            type: 'p',
            text: '知识库、文件编辑、任务编辑三个模式里，「导入文件」图标与右键菜单都提供两项选择。点击图标只展开菜单，不会影响文件树的展开 / 折叠状态。',
          },
          {
            type: 'cards',
            items: [
              { index: 'A', title: '原样导入', text: '把文件原封不动拷进当前目录。与旧版行为一致；Markdown 直接入库。' },
              { index: 'B', title: '转换为 Markdown', text: '按格式自动选引擎，生成同名 .md，原始文件不会被改动。' },
            ],
          },
          {
            type: 'p',
            text: '转换能力覆盖 docx / pptx / xlsx / odt / rtf / epub / html / ipynb / org / rst / latex / fb2 / csv；旧版 Office 自动路由到极速引擎；有文字层的 PDF 抽取正文与图片；图片走离线 OCR；电子书按章节转成带标题的 Markdown。',
          },
          {
            type: 'p',
            text: '转换产物里的图片会统一放到与 Markdown 同级的 <code>&lt;文件名&gt;_assets</code> 目录，用相对路径引用，随笔记一起被知识库和 Git 管理。',
          },
        ],
      },
      {
        id: 'home-search',
        title: '搜索与查询语法',
        keywords: ['搜索', '全局搜索', '查询语法', 'ext', 'path', 'tag', 'after', '正则', '全词', '中文分词', '命令面板'],
        blocks: [
          {
            type: 'p',
            text: '首页顶部的搜索框是本地检索总入口，一个输入框同时写“找什么”和“在哪找”，结果落在右侧面板的三个标签里。<kbd>Ctrl</kbd>+<kbd>P</kbd> 聚焦搜索框。搜索根目录跟随左栏文件树当前激活的根；只有摘录笔记例外，它固定在资源库目录下找。',
          },
          {
            type: 'cards',
            items: [
              { index: '01', title: '搜索文件', text: '按文件名和相对路径匹配，components/header 这种“目录 + 文件名”写法也能中。' },
              { index: '02', title: '搜索关键字', text: '搜文件正文，命中按文件分组、一行一条列出，点某一行直接跳到那一行。' },
              { index: '03', title: '搜索摘录笔记', text: '搜阅读器里存下来的摘录和笔记。' },
            ],
          },
          {
            type: 'h4',
            title: '在搜索词里直接写筛选条件',
          },
          { type: 'code', lang: 'text', text: code('配置 加载 ext:md,txt after:7d path:src tag:待办') },
          {
            type: 'list',
            items: [
              '<code>ext:</code>（也可写 <code>type:</code>）扩展名，逗号带多项；<code>.MD</code> 和 <code>md</code> 是一个意思。',
              '<code>tag:</code> / <code>tags:</code> 标签。',
              '<code>path:</code>（也可写 <code>in:</code> / <code>dir:</code>）路径包含某一段，是子串匹配而不是前缀匹配。',
              '<code>after:</code> / <code>since:</code> 与 <code>before:</code> / <code>until:</code> 修改时间，取值可以是 2026-01-31、2026/01、today、yesterday，也可以是 7d / 2w / 3m / 1y。',
              '值里有空格就加引号：<code>path:"我的 笔记"</code>。',
              '记号会从搜索词里剥掉，不会被当成字面量去搜。不认识的键（title:、C:、http://）原样留下当搜索词。',
              '时间写错了不会被静默丢掉，那一段会退回当普通搜索词处理。',
            ],
          },
          {
            type: 'h4',
            title: '命令面板：输入 > 开头',
          },
          {
            type: 'p',
            text: '在搜索框第一个字符打 <code>&gt;</code> 就进命令模式，下拉里只列命令，可以接着敲字过滤（中英文和拼音首字母都能搜）。开关类命令右侧会显示当前是“已开启”还是“已关闭”。常用命令包括重建文件名索引、重建正文全文索引、清除全部筛选，以及切换正则 / 全词 / 大小写。',
          },
          {
            type: 'h4',
            title: '三个匹配开关',
          },
          {
            type: 'p',
            text: '搜索框右侧的 <code>Aa</code>、<code>ab|</code>、<code>.*</code> 分别是区分大小写、全词匹配和正则。默认三个都关，此时按字面量搜索、走智能大小写；开关状态记在本机。',
          },
          {
            type: 'callout',
            tone: 'note',
            title: '中文的全词匹配靠分词器',
            text: '正则的 \\b 在 Unicode 语义下把汉字也算成单词字符，所以“参数”和“高效”之间不存在词边界，\\b参数\\b 在“参数高效微调”里永远匹配不上。这里把词边界交给 jieba：所以“参数”能以整词命中，“数高”不会；“使用LoRA的”里的 LoRA 也能被找到。',
          },
          {
            type: 'h4',
            title: '结果与键盘操作',
          },
          {
            type: 'list',
            items: [
              '内容标签把命中按文件收成组；组头是文件名、目录和命中条数，行首是行号，行尾是原文。点组头折叠或展开，点命中才跳转。',
              '一行放不下时会平移到命中附近，两端用 <b>…</b> 标出截掉的部分；悬停能看到完整整行与行号。',
              '搜索框：<kbd>Ctrl</kbd>+<kbd>P</kbd> 聚焦，<kbd>↑</kbd><kbd>↓</kbd> 选行，<kbd>Enter</kbd> 打开或执行，<kbd>Esc</kbd> 收起下拉。',
              '右侧结果面板：<kbd>Alt</kbd>+<kbd>1</kbd> / <kbd>Alt</kbd>+<kbd>2</kbd> / <kbd>Alt</kbd>+<kbd>3</kbd> 切到文件 / 内容 / 摘录，<kbd>←</kbd><kbd>→</kbd> 顺次换标签，<kbd>Enter</kbd> 打开。',
            ],
          },
          { type: 'figure', image: 'search-results', caption: '全局搜索：输入 Markdown 后同时列出文件、内容与摘录结果。' },
        ],
      },
      {
        id: 'home-charts',
        title: '笔记内嵌图表与动画',
        keywords: ['mermaid', 'chart', 'ECharts', 'excalidraw', '动画', 'manim', 'plotly', '导出', 'CSV'],
        blocks: [
          {
            type: 'p',
            text: '笔记里可以用围栏代码块直接嵌图表，不需要离开编辑器。所有内嵌块在编辑器预览、只读 Markdown 阅读器和导出 HTML/PDF 三条链路上表现一致。',
          },
          {
            type: 'h4',
            title: '1. 结构图：mermaid',
          },
          { type: 'code', lang: 'markdown', text: code('```mermaid', 'flowchart LR', '  A[收集资料] --> B[阅读摘录]', '  B --> C[整理成笔记]', '  C --> A', '```') },
          {
            type: 'h4',
            title: '2. 数据图表：chart',
          },
          {
            type: 'p',
            text: '用 ECharts 渲染。数据可以内联，也可以指向任意位置的 CSV / TSV / JSON 文件——改数据文件后刷新预览就会更新。',
          },
          { type: 'code', lang: 'yaml', text: code('```chart', 'spec:', '  type: bar', '  x: month', '  y: [revenue, cost]', '  title: 季度营收对比', 'data: ./data/sales.csv', '```') },
          {
            type: 'p',
            text: '`data` 支持四种写法：<code>resource:</code> 指向资源库内的副本（推荐）、相对当前笔记的路径、系统绝对路径、<code>~/</code> 用户目录。支持 .csv / .tsv / .json；CSV 首行是表头，单元格可用双引号包裹。',
          },
          {
            type: 'callout',
            tone: 'tip',
            title: '推荐把数据固化进资源库',
            text: '编辑器工具栏点「图表」按钮选择数据文件，文件会被复制到资源库 charts/年/月/ 下，并插入一段引用 resource: 路径的围栏。笔记换机器打开时图表仍能渲染，资源库本身也可以通过同步带走。',
          },
          {
            type: 'p',
            text: '支持 26 种图表：line / area、bar / waterfall / pictorialbar、pie / doughnut / ring、scatter / effectscatter / boxplot / heatmap、radar / parallel、sankey / funnel / gauge、tree / treemap / sunburst、graph / map / lines、candlestick / themeriver、custom（甘特 / 子弹图）、liquidfill / wordcloud。规格字段包括 type、x、y、title、smooth、stack、orient、fields、path、source/target/value、variant，以及 seriesStyle / xAxisStyle / yAxisStyle。',
          },
          {
            type: 'h4',
            title: '3. 自由布局白板：excalidraw',
          },
          { type: 'code', lang: 'markdown', text: code('```excalidraw', './assets/architecture.excalidraw', '```') },
          {
            type: 'p',
            text: '围栏里写场景文件路径（相对当前笔记），渲染为只读 SVG。画好后从白板页保存为 .excalidraw 即可嵌入；嵌入块有「在其他窗口打开」按钮，导出与打印都不会变形。',
          },
          {
            type: 'h4',
            title: '4. 动画与导出',
          },
          {
            type: 'p',
            text: '动画不用新语法，用 Python Cell：manim 输出 mp4，matplotlib + FuncAnimation 做数据动图，plotly 在隔离 iframe 里交互。写一个 python 围栏，点「运行」或「运行并写回」，输出会挂在 Cell 下方。',
          },
          {
            type: 'list',
            items: [
              '图表右上角悬停出现三个按钮：查看（放大预览）、SVG（矢量导出）、PNG（位图导出）。',
              'PNG 导出会自动铺白底，导出的图固定为浅色，方便贴进文档与打印。',
              '导出 HTML / PDF 走与预览同源的渲染器，mermaid、MathJax、图表、白板都会一起导出。',
              '图表数据文件不会被打包进导出的 HTML；分享时把数据文件或用 resource: 入库后的资源库一起带上更稳妥。',
            ],
          },
        ],
      },
    ],
  },
  {
    id: 'agent',
    group: '页面详解',
    number: '03',
    eyebrow: 'Page · Agent Workspace',
    title: '智能助手',
    subtitle: '给任务，而不是给问题',
    summary: '它自己读文件、跑命令、查资料、看屏幕，把每一步摆在你面前；需要你拍板的时候才来打断你。',
    sections: [
      {
        id: 'agent-layout',
        title: '三栏工作台',
        keywords: ['智能助手', 'Agent', '会话', '转录', '账本', '仪表', '任务栏'],
        blocks: [
          {
            type: 'p',
            text: '智能助手页不是聊天框，而是一个工作台。它由会话栏、转录区、右侧仪表三部分组成；每个任务都有独立的运行目录和沙箱策略。',
          },
          { type: 'figure', image: 'page-chat', caption: 'Agent 工作台：左任务、中转录与运行账本、右仪表与计划。' },
          {
            type: 'defs',
            items: [
              { term: '左：任务栏', text: '一条任务 = 一个独立工作目录 + 一套沙箱策略。点 ＋ 新建，点条目切换，右键删除；圆点表示状态（琥珀色在跑，红色上次失败）。' },
              { term: '中：转录与账本', text: '上方是对话；一轮结束后下方出现「本次运行」账本，逐行列出跑过的命令、改过的文件，点开可看命令原文、退出码和输出。' },
              { term: '右：仪表', text: '雷荷波圆环显示运转状态：外圈随输出速率起伏，短刻度每调用一次工具点亮，圈外亮点代表有几件事在等你。下面是计划、改动、用量、模式与沙箱。' },
            ],
          },
        ],
      },
      {
        id: 'agent-modes',
        title: '四种任务模式',
        keywords: ['探索', '编码', '研究', '操作电脑', '模式', '只读', '联网', '截屏'],
        blocks: [
          {
            type: 'p',
            text: '输入框上沿的模式切换的是<b>它能做什么</b>，不是提示词措辞。模式在创建任务时决定，之后跟着任务走。',
          },
          {
            type: 'cards',
            items: [
              { index: '01', title: '探索', text: '只读。能检索、读文件、画关系，不能改任何东西。适合“这个库里有几处提到 X”这类问题。' },
              { index: '02', title: '编码', text: '可改工作区内的文件、跑测试、看 diff。适合改代码或批量整理笔记。' },
              { index: '03', title: '研究', text: '只读但允许联网。先查知识库，再上网补充，并要求分开写清两类出处。' },
              { index: '04', title: '操作电脑', text: '截屏、点按窗口、打字。需要先在设置里开启「操控本机」授权，且模型要支持图像输入。' },
            ],
          },
        ],
      },
      {
        id: 'agent-approval',
        title: '审批：它动手之前会问你',
        keywords: ['审批', '确认', '拒绝', '本会话允许', '超时', 'diff', '权限'],
        blocks: [
          {
            type: 'p',
            text: '只要动作有副作用（执行命令、改文件、申请额外权限），界面会弹出一张确认卡，把<b>要做什么</b>写在最上面：完整命令、工作目录、涉及哪些文件、每个文件增删多少行。',
          },
          {
            type: 'list',
            items: [
              '<b>拒绝</b>永远在左边，点错也不会造成后果；拒绝后它会换一种做法，而不是硬来。',
              '命令类审批多一个<b>本会话允许</b>：同类命令在这一轮里不再逐条问。',
              '没有「全部允许」。同时有多条待批时逐个处理，仪表右上角显示还有几条。',
              '超过 5 分钟没有回应会按拒绝处理，不会让任务一直挂在那里。',
            ],
          },
        ],
      },
      {
        id: 'agent-steer',
        title: '运行中改指令与常用操作',
        keywords: ['转向', '停止', 'Esc', '工作区', '模型', 'diff', '快捷键'],
        blocks: [
          {
            type: 'p',
            text: '它跑着的时候输入框没有锁死。直接输入新要求按 Enter，这条指令会<b>插进当前这一轮</b>（转录里标为「转向」），不用停下来重说一遍。想彻底中止就按 <kbd>Esc</kbd> 或点「停止」。',
          },
          {
            type: 'list',
            items: [
              '<b>换工作目录</b>：点输入区上沿的「工作区」。换目录等于换任务，当前转录会清空。',
              '<b>看改动</b>：仪表「改动」区列出本次改过的文件，点「查看完整 diff」在独立差异窗口逐行对照。',
              '<b>换模型</b>：点「模型」。清单由引擎直接读出，标注了哪些模型能看图像。',
              '<b>快捷键</b>：Enter 发送（运行中为转向），Shift+Enter 换行，Esc 停止。',
            ],
          },
          {
            type: 'callout',
            tone: 'note',
            title: '旧版对话页仍然保留',
            text: '原来的多模型聊天页在 /chat-legacy，保留网页版模型（ChatGPT / 元宝）、翻译模式和纯闲聊。工作台覆盖“让 AI 干活”，但闲聊与网页模型还没搬过去，两者并存一段时间。',
          },
        ],
      },
    ],
  },
  {
    id: 'repo',
    group: '页面详解',
    number: '04',
    eyebrow: 'Page · Repository',
    title: '仓库',
    subtitle: '资料进来，深读出去',
    summary: '仓库管理导入的资料：表格与书架两种视图、按字段搜索、重复资源检测、分享发布，以及所有阅读器的入口。',
    sections: [
      {
        id: 'repo-views',
        title: '资料库与两种视图',
        keywords: ['仓库', '资料库', '表格视图', '书架视图', '搜索导入文件', '重复资源'],
        blocks: [
          {
            type: 'p',
            text: '仓库用于管理导入资料。顶部搜索框可以按文件名、描述、标签、所属文件库、知识库或文件类型筛选；右侧可在表格视图和书架视图之间切换。',
          },
          { type: 'figure', image: 'page-repo-search', caption: '仓库：书架视图与知识库筛选。' },
          {
            type: 'list',
            items: [
              '<b>表格视图</b>：文件名、描述、所属文件库、入库时间、文件类型、大小、标签；支持排序、清除筛选、多选与分页。',
              '<b>书架视图</b>：把资料像书一样陈列，适合按主题浏览；空库会提示导入。',
              '<b>重复资源</b>：按文件内容检测完全相同的资源，删除前会明确提示不可恢复。',
              '<b>分享与发布</b>：标题栏的分享按钮打开「资料库 · 分享与发布」，可生成站点或对外分享。',
            ],
          },
        ],
      },
      {
        id: 'repo-reader',
        title: '阅读器',
        keywords: ['阅读器', 'PDF', 'EPUB', '翻译', '摘录', '高亮', '笔记', '书签', '目录'],
        blocks: [
          {
            type: 'p',
            text: '资料库页面用于管理导入资料，阅读器用于深度阅读。当前链路覆盖 PDF、转换后的 PDF / Markdown、EPUB、HTML、Markdown、文本、Word 等类型。',
          },
          {
            type: 'list',
            items: [
              '支持上一页 / 下一页、缩放、适合宽度、布局切换、书签、页内搜索、目录 / 缩略图和阅读状态记录。',
              '选中文本后可翻译、查询、问 AI、用 AI 翻译或插入笔记；相关结果会写入学习记录或摘录笔记。',
              'PDF 阅读器支持高亮、标注、删除高亮、保存回原 PDF；右侧「我的笔记」可新增、编辑、删除卡片。',
              '阅读记录会被统计页读取，用于显示阅读进度、完成状态和总耗时。',
              '独立阅读窗口支持逐句朗读与整篇对照翻译；朗读能力在设置 → 语音能力里配置。',
            ],
          },
          { type: 'figure', image: 'reader-pdf', caption: 'PDF 阅读器：分页、缩放、页内搜索与右侧译文 / 笔记。' },
          { type: 'figure', image: 'reader-markdown', caption: 'Markdown 阅读器：左侧原文、右侧渲染，共用同一套阅读器外壳。' },
        ],
      },
      {
        id: 'repo-import',
        title: '导入与资源管理',
        keywords: ['导入', '文件库', '资源库', '知识库资源管理', '标签', '转换'],
        blocks: [
          {
            type: 'p',
            text: '仓库的「导入文件」会把资料登记到当前文件库，并按原格式保存；如果要在知识库里直接编辑，用首页的「转换为 Markdown 后导入」。导入后可以补描述与标签，它们是仓库搜索的主要依据。',
          },
          {
            type: 'callout',
            tone: 'tip',
            title: '资料库与知识库不是一回事',
            text: '资料库保存原始资料（PDF、Word、电子书等），知识库保存你自己写的 Markdown。阅读器负责把前者变成摘录，摘录再进入后者——这条链路不要反过来用。',
          },
        ],
      },
    ],
  },
]
