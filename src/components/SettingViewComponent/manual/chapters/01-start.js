export const startChapters = [
  {
    id: 'start',
    group: '入门',
    number: '00',
    eyebrow: 'Orientation',
    title: '开始使用',
    subtitle: '先认识 Guaxi，再把库和模型配好',
    summary: '用五分钟建立正确的心智模型：它不是一个聊天框，而是一张把资料、笔记、任务和工具串起来的工作台。',
    sections: [
      {
        id: 'start-what',
        title: '它是什么',
        keywords: ['简介', '定位', '工作流', '知识库', '资料库', 'AI', '本地'],
        blocks: [
          {
            type: 'lead',
            text: 'Guaxi 是一款面向个人知识工作流的桌面软件。它把本地文件、Markdown 编辑、资料阅读、AI 对话、知识图谱、网络知识源、任务记录和工具应用放在同一套界面里。',
          },
          {
            type: 'p',
            text: '它适合完成一条长期循环：<b>导入资料 → 阅读摘录 → AI 辅助理解 → 写作整理 → 知识库沉淀 → 统计复盘</b>。每一段都能在同一个窗口里完成，不需要在十几个应用之间搬来搬去。',
          },
          { type: 'figure', image: 'overview', caption: 'Guaxi 主界面：左侧导航、中间工作区、右侧辅助面板。' },
          {
            type: 'callout',
            tone: 'note',
            title: '先建立三个概念',
            text: '知识库（KnowledgeBases）放你自己的 Markdown 笔记；资源库（Libraries）放图片、图表数据和阅读器产生的资产；任务库放工作 / 生活 / 学习三类任务。三者可以分别设目录、分别同步。',
          },
        ],
      },
      {
        id: 'start-map',
        title: '九个主页面，一眼找到入口',
        keywords: ['功能地图', '导航', '页面', '入口', '快捷键'],
        blocks: [
          {
            type: 'p',
            text: '左侧图标列是全局导航，按 <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>1</kbd> ~ <kbd>9</kbd> 可以直接切换。每个页面都只解决一类问题，遇到具体任务时先判断它属于哪一类。',
          },
          {
            type: 'cards',
            items: [
              { index: '01', title: '知识编辑', text: '写 Markdown、管理知识库文件树、搜索、分屏预览。日常待得最久的地方。', go: 'home' },
              { index: '02', title: '智能助手', text: '给任务而不是给问题：读文件、跑命令、查资料、操作电脑。', go: 'agent' },
              { index: '03', title: '仓库', text: '管理导入的资料，按表格或书架浏览，打开阅读器深读。', go: 'repo' },
              { index: '04', title: '知识图谱', text: '把知识库构建成可浏览、可筛选、可语义检索的 3D 网络。', go: 'graph' },
              { index: '05', title: '搜索', text: '订阅、抓取、阅读外部内容，收藏后插入笔记。', go: 'sources' },
              { index: '06', title: '应用', text: '邮件、相册、音乐、翻译、传输、OCR、简历等独立工具。', go: 'apps' },
              { index: '07', title: '统计', text: '看知识量趋势、Git 活跃热力图、阅读进度与资料标签。', go: 'stats' },
              { index: '08', title: '白板', text: '用 Excalidraw 画结构图、流程图和临时构思，可嵌入笔记。', go: 'whiteboard' },
              { index: '09', title: '设置', text: '目录、模型、同步、终端、语音、插件与认知层配置。', go: 'settings' },
            ],
          },
          {
            type: 'callout',
            tone: 'tip',
            title: '大脑不在导航列里',
            text: '「大脑（认知层）」与知识图谱共用同一个页面，用顶栏的「大脑 / 图谱」开关切换。图谱是你主动去翻的地图，大脑是它读完之后主动递给你的结论。',
          },
        ],
      },
      {
        id: 'start-workspace',
        title: '界面的骨架',
        keywords: ['布局', '三栏', '侧栏', '工具栏', '状态'],
        blocks: [
          {
            type: 'defs',
            items: [
              { term: '左：导航与文件树', text: '图标列切换主页面；文件树在知识库、任务目录、普通目录和打开历史之间工作。' },
              { term: '中：主工作区', text: '编辑器、阅读器、图谱、应用等内容都渲染在这里。多数页面支持分屏。' },
              { term: '右：辅助面板', text: '大纲、智能助手、搜索结果、数学工作区、节点信息等，可随时收起。' },
              { term: '顶：工具栏', text: '全局搜索、新建知识库、模板、资源管理、任务、语言学习、截图、分屏与引导。' },
            ],
          },
          {
            type: 'p',
            text: '顶栏两端的侧栏图标用来展开或收起左右侧栏（<kbd>Alt</kbd>+<kbd>[</kbd> / <kbd>Alt</kbd>+<kbd>]</kbd>）；侧栏收起时图标会变淡。所有带快捷键的图标都使用两键组合（<kbd>Alt</kbd>+字母），鼠标悬停就能看到属于它的那一个。',
          },
        ],
      },
      {
        id: 'start-quick',
        title: '五分钟快速开始',
        keywords: ['快速开始', '配置', '目录', '模型', '首次使用'],
        blocks: [
          {
            type: 'steps',
            items: [
              { title: '配置目录', text: '进入「设置 → 笔记库」，设置知识库根目录、资源库根目录、模板库路径和 Slidev 源文件目录。' },
              { title: '初始化任务库', text: '进入「设置 → 任务管理」，选择任务库根目录并点击「初始化目录」，系统会准备工作、生活、学习三类目录。' },
              { title: '接入模型', text: '进入「设置 → 大模型」，按需填写 ChatGPT、DeepSeek、Qwen、Kimi、豆包、硅基流动或自定义模型的 Base URL、API Key 与模型名。' },
              { title: '开始第一份笔记', text: '回到「知识编辑」，在左栏新建文件，用 Monaco 编辑器写 Markdown；<kbd>Ctrl</kbd>+<kbd>S</kbd> 保存。' },
              { title: '把资料读起来', text: '进入「仓库」导入一个 PDF 或 Word，打开阅读器，选中文本即可翻译、问 AI、做摘录。' },
            ],
          },
          { type: 'figure', image: 'setting-noterepo', caption: '设置 → 笔记库：目录、模板、Slidev 与远程仓库都集中在这里。' },
        ],
      },
      {
        id: 'start-check',
        title: '第一次使用检查清单',
        keywords: ['检查清单', '排错', '路径', '权限', '同步'],
        blocks: [
          {
            type: 'list',
            items: [
              '知识库、资源库目录存在且可读写；截图、图片和摘录会依赖资源库或图床目录。',
              '如果使用 PDF / 文档转换，确认安装位置：<code>utils/Models/surya_models</code> 与 <code>utils/MarkdownConvertor</code>。',
              '如果要离线推理，先在「设置 → 大模型」下载或导入 LocalDeepseek / LocalQwen。',
              '如果要同步，先在一台设备上跑通，再接第二台；P2P、WebDAV、Syncthing 三选一即可。',
              '如果知识图谱或大脑提示“语义服务未就绪”，先去图谱视图给知识库生成一次向量。',
            ],
          },
          {
            type: 'callout',
            tone: 'tip',
            title: '不要一次打开所有能力',
            text: '第一周只配目录、模型和一个同步方式就够了。插件、认知层采集、MCP 这类能力等有具体需求时再开，排错会轻松很多。',
          },
        ],
      },
    ],
  },
  {
    id: 'setup',
    group: '入门',
    number: '01',
    eyebrow: 'Configuration',
    title: '基本配置',
    subtitle: '目录、模型、同步：把地基打稳',
    summary: '配置只做一次，但配错会一直影响后面。这一章按“必须配 / 建议配 / 按需配”三档说明每一项的作用。',
    sections: [
      {
        id: 'setup-dirs',
        title: '目录与库',
        keywords: ['目录', '知识库根目录', '资源库', '模板', 'Slidev', '路径'],
        blocks: [
          {
            type: 'table',
            head: ['配置项', '位置', '作用', '建议'],
            rows: [
              ['知识库根目录', '设置 → 笔记库', '所有 Markdown 知识库的父目录', '放在固定磁盘，并纳入 Git'],
              ['资源库根目录', '设置 → 笔记库', '图片、图表数据、阅读器资产', '与知识库分开，便于单独同步'],
              ['模板库路径', '设置 → 笔记库', 'Markdown 模板的来源', '先建目录再导入模板'],
              ['Slidev 源文件目录', '设置 → 笔记库', 'PPT 制作与管理的工程目录', '需要时再配'],
              ['任务库根目录', '设置 → 任务管理', '工作 / 生活 / 学习三类任务的根目录', '初始化后不要频繁移动'],
            ],
          },
          {
            type: 'p',
            text: '知识库可以有很多个，每个知识库就是一个独立目录；资源库是跨知识库共享的资产池。图表数据推荐固化进资源库，这样笔记换一台机器打开也不会丢图。',
          },
        ],
      },
      {
        id: 'setup-models',
        title: '模型与语音',
        keywords: ['大模型', 'API Key', 'Base URL', 'ChatRecord', '本地模型', 'TTS', 'ASR', '语音'],
        blocks: [
          {
            type: 'list',
            items: [
              '<b>在线模型</b>：在「设置 → 大模型」中分别填写 ChatGPT、DeepSeek、Qwen、Kimi、豆包、硅基流动或自定义模型的 Base URL、API Key 和模型名。',
              '<b>会话记录</b>：设置 ChatRecord 保存路径；未设置时默认使用软件同级目录下的 ChatRecord。',
              '<b>本地模型</b>：LocalDeepseek / LocalQwen 会在选择模型时检查并启动本地 llama-server，首次加载需要等待模型文件与服务初始化。',
              '<b>语音能力</b>：在「设置 → 语音能力」配置 TTS / ASR Provider、模型路径与可执行文件路径，并可直接测试文本转语音或音频转写。',
            ],
          },
          {
            type: 'callout',
            tone: 'warn',
            title: '密钥只在本地使用',
            text: 'API Key 由软件自己保存和调用，不会传给插件。插件调用模型时只拿到补全结果，密钥留在 Rust 侧。',
          },
        ],
      },
      {
        id: 'setup-sync',
        title: '同步与版本管理',
        keywords: ['同步', 'P2P', 'WebDAV', '坚果云', 'Syncthing', 'Git', '远程仓库'],
        blocks: [
          {
            type: 'p',
            text: '资源库同步支持不启用、P2P 设备同步、坚果云 WebDAV、Syncthing API 四种方式。知识库与任务库则通过本机 Git 远程仓库同步。',
          },
          {
            type: 'table',
            head: ['方式', '适合场景', '注意'],
            rows: [
              ['P2P 设备同步', '两台自己的电脑，局域网直连', '可配置中继、可信设备与配对 Ticket'],
              ['坚果云 WebDAV', '跨网络、无需公网 IP', '注意流量与单文件大小限制'],
              ['Syncthing API', '已有 Syncthing 集群', '需要填写 API 地址与密钥'],
              ['Git 远程仓库', '需要版本历史与回滚', '知识库、任务库分别配置'],
            ],
          },
          {
            type: 'p',
            text: '如果 Git 推送或拉取失败，先在系统终端里确认对应目录能正常 <code>git pull</code> / <code>git push</code>，再回到 Guaxi 操作。软件调用的是本机 git，不内置凭据管理。',
          },
        ],
      },
      {
        id: 'setup-first-import',
        title: '第一次导入与第一次对话',
        keywords: ['导入', '转换', 'Markdown', '向量', '第一次', '问答'],
        blocks: [
          {
            type: 'steps',
            items: [
              { title: '原样导入还是转换', text: '知识库、文件编辑、任务编辑三个模式的「导入文件」都提供两项：保留原格式，或转换成 Markdown 后导入。转换不会改动原文件。' },
              { title: '先读再问', text: '在「仓库」打开资料，选中一段文字，从浮动工具条里翻译、查询或问 AI；结果可以写入学习记录或摘录笔记。' },
              { title: '建立图谱', text: '进入「知识图谱」，对知识库执行构建索引；需要语义能力时再生成向量。' },
              { title: '打开大脑', text: '在图谱页顶栏切到「大脑」，先点一次「立即审议」，之后它会自己在后台运行。' },
            ],
          },
          {
            type: 'callout',
            tone: 'note',
            title: '转换引擎怎么选',
            text: 'docx / pptx / xlsx / epub / html / ipynb / org / rst / latex / fb2 / csv 走 pandoc 或极速引擎；旧版 Office（doc / ppt / ods / odp）只有极速引擎能读；扫描版 PDF 需要 OCR，改用仓库导入的 Marker 引擎。',
          },
        ],
      },
    ],
  },
]
