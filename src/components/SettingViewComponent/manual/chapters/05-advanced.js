import { code } from '../helpers.js'

export const advancedChapters = [
  {
    id: 'settings',
    group: '扩展与配置',
    number: '13',
    eyebrow: 'Configuration · All Panes',
    title: '设置',
    subtitle: '每一个开关，都在改什么',
    summary: '设置页按内容与库、外观与体验、AI 与模型、系统与服务、帮助与关于五组组织。第一次配置只需动前两组，其余按需再开。',
    sections: [
      {
        id: 'settings-map',
        title: '设置地图',
        keywords: ['设置', '分组', '菜单', '配置'],
        blocks: [
          {
            type: 'table',
            head: ['分组', '页面', '主要用途'],
            rows: [
              ['内容与库', '笔记库 / 任务管理 / 知识采集', '目录、Git、任务库、抓取存储策略'],
              ['外观与体验', '外观 / 编辑器 / 终端', '主题、代码配色、字体字号、LSP、终端'],
              ['AI 与模型', '大模型 / 语音能力 / 认知层', '在线与本地模型、TTS/ASR、大脑'],
              ['系统与服务', '服务器 / 文件传输 / 插件 / 系统维护', '本地 8888 服务、WebRTC、插件、GC'],
              ['帮助与关于', '用户手册 / 关于', '这份手册与软件信息'],
            ],
          },
          { type: 'figure', image: 'page-setting', caption: '设置页：左侧分组菜单、右侧配置内容。' },
        ],
      },
      {
        id: 'settings-repo',
        title: '笔记库与资源库',
        keywords: ['知识库根目录', '资源库', '模板库', 'Slidev', 'P2P', 'WebDAV', 'Syncthing', 'Ticket', 'Git'],
        blocks: [
          {
            type: 'list',
            items: [
              '配置知识库根目录、资源库根目录、模板库路径、Slidev PPT 源文件目录和知识库远程仓库信息。',
              '资源库同步支持不启用、P2P 设备同步、坚果云 WebDAV、Syncthing API。',
              'P2P 可配置本机名称、Ticket 有效期、自动同步资源库、同步间隔、并发传输、连接超时、块级增量、自动组网、中继模式、可信设备和配对 Ticket。',
              'WebDAV 需要 URL、用户名、密码或应用密码、远端目录；Syncthing 需要 API URL、API Key、Folder ID。',
              '知识库远程仓库使用本机 Git：填写远程地址、用户名和 Email 后即可提交推送或拉取。',
            ],
          },
          {
            type: 'callout',
            tone: 'warn',
            title: '同步先在一台设备跑通',
            text: 'P2P 依赖两端网络可达与配对；WebDAV 依赖服务端权限；Syncthing 依赖 API 与目录映射。先用「测试连接 / 刷新状态」确认，再把第二台设备接进来。',
          },
        ],
      },
      {
        id: 'settings-tasks',
        title: '任务管理设置',
        keywords: ['任务库', '初始化目录', '扫描索引', '远程仓库', 'Git'],
        blocks: [
          {
            type: 'p',
            text: '配置任务库根目录，点击「初始化目录」准备工作、生活、学习三类目录，然后「立即扫描索引」。需要版本历史时，填写远程仓库地址、Git 用户名和 Email，再提交推送或拉取。',
          },
        ],
      },
      {
        id: 'settings-feeds',
        title: '知识采集存储',
        keywords: ['知识采集', '未读保留', '已读保留', '回收站', '每日入盘', '图片', '双写', '质量门'],
        blocks: [
          {
            type: 'p',
            text: '这一页控制抓回来的内容如何落盘：未读保留天数、已读保留天数、回收站保留时长、每日入盘上限、图片本地化与单图上限、Markdown 双写和质量门。',
          },
          {
            type: 'list',
            items: [
              '「不囤积」策略会主动清理长期未打开的文章，让阅读清单保持可读。',
              '每日入盘上限按主题计算，0 表示不限；超出额度的抓取不会写入。',
              '图片本地化会把远程图片下载到本地，避免来源失效后正文变空；单图上限防止大图拖慢入库。',
              'Markdown 双写适合希望原文与 Markdown 同时保留的场景。',
            ],
          },
        ],
      },
      {
        id: 'settings-appearance',
        title: '外观、编辑器与终端',
        keywords: ['主题', '亮色', '暗色', '代码配色', '字体', '字号', '小地图', 'LSP', '格式化', '终端', 'Git Bash'],
        blocks: [
          {
            type: 'p',
            text: '外观页只有亮色 / 暗色两个主题，切换后会立即应用并写入配置文件。编辑器页控制编辑器外观、Markdown 格式化、LSP 与 Python 代码辅助。',
          },
          {
            type: 'table',
            head: ['配置', '作用'],
            rows: [
              ['代码配色主题', 'Monaco 的语法高亮主题，与界面主题独立'],
              ['字体 / 字号 / 小地图', '编辑器显示；字号默认 18，小地图默认关闭'],
              ['折行宽度 / 缩进 / 列表与表格格式化', 'Markdown 格式化行为，影响保存时的排版'],
              ['保留代码块内格式 / 清理行尾空白 / 移除多余空行', '格式化时的保护与清理规则'],
              ['启用 LSP 扩展能力', '为支持的编程语言提供补全、诊断与跳转'],
              ['启用 Python 代码生成助手', '在 Python 代码块里提供生成能力'],
              ['默认终端 / Git Bash 路径', '内置终端启动时使用的 shell'],
            ],
          },
        ],
      },
      {
        id: 'settings-llm',
        title: '大模型、本地模型与语音',
        keywords: ['大模型', 'ChatGPT', 'DeepSeek', 'Qwen', 'Kimi', '豆包', '硅基流动', 'ChatRecord', 'local', 'TTS', 'ASR'],
        blocks: [
          {
            type: 'list',
            items: [
              '支持 ChatGPT、DeepSeek、Qwen、Kimi、豆包、硅基流动和自定义模型配置；每个供应商可填 Base URL、API Key、模型名。',
              '可设置 ChatRecord 保存路径；未设置时默认使用软件同级目录下的 ChatRecord。',
              'Markdown 转换模型安装到 <code>utils/Models/surya_models</code>；转换运行环境安装到 <code>utils/MarkdownConvertor</code>。',
              'LocalDeepseek / LocalQwen 可下载或从本地文件导入，并在聊天模型切换时启动本地 llama-server 服务。',
              '语音能力可配置 TTS / ASR Provider、模型路径、tokens 路径、exe 路径，并测试文本转语音或音频转写。',
            ],
          },
          { type: 'figure', image: 'setting-llm', caption: '大模型：供应商、密钥、模型与 ChatRecord。' },
          { type: 'figure', image: 'setting-local-llm', caption: '本地模型：下载、导入与运行状态。' },
        ],
      },
      {
        id: 'settings-cognition',
        title: '认知层',
        keywords: ['认知层', '感知来源', '打扰', '预算', '工具授权', 'MCP', '画像', '评测', '维护'],
        blocks: [
          {
            type: 'p',
            text: '这一页是大脑的开关面板，分成感知来源、打扰与预算、工具授权、MCP 接入、我的画像、自调与评测闩、存储与维护七组。每个按钮都带悬浮说明，动手之前先看一眼它会改什么。',
          },
          {
            type: 'list',
            items: [
              '<b>感知来源</b>：逐个开关网页抓取、RSS、摘录、文件、剪贴板、屏幕、手动、聊天，并设定每天自动采集的条数上限。剪贴板与屏幕默认关闭。',
              '<b>打扰与预算</b>：每天允许打断几次、打断门槛、未读抓取内容保留几天。提醒不会消失，只会降级进简报。',
              '<b>工具授权</b>：读取、写入、联网、执行四档分别授权，可查看调用审计。',
              '<b>MCP 接入</b>：把大脑作为 MCP 服务暴露给外部客户端，可复制配置或读取接入信息。',
              '<b>我的画像</b>：直接读取和编辑画像文件；它是唯一的真相，表单只是它的编辑器。',
              '<b>自调与评测闩</b>：挖掘评测样本、跑一次评测、自动调参、回滚权重。调参必须先过评测。',
              '<b>存储与维护</b>：立即清理未读、立即打分、立即维护、按文件重建画像派生层。',
            ],
          },
        ],
      },
      {
        id: 'settings-about',
        title: '关于页',
        keywords: ['关于', '版本', '许可', '作者', 'GitHub', 'Gitee', '技术栈', '致谢'],
        blocks: [
          {
            type: 'p',
            text: '设置的最后一项是「关于」：这里集中说明软件是什么、当前版本与运行环境、开源许可、源码仓库、作者与开源致谢。版本号直接读取桌面应用版本，运行环境来自系统信息，不是写死的文案。',
          },
          {
            type: 'list',
            items: [
              '「打开用户手册」会直接切回当前这份手册，不需要自己在左侧菜单里找。',
              'GitHub / Gitee 按钮用系统默认浏览器打开仓库；「复制仓库地址」会把两个地址一起复制到剪贴板。',
              '许可信息为 AGPL-3.0，完整条款在仓库根目录的 LICENSE 中。',
              '作者与联系方式在正式发布前补齐；当前可以通过仓库 Issues 反馈问题。',
            ],
          },
          {
            type: 'callout',
            tone: 'tip',
            title: '关于页也是版本排错入口',
            text: '反馈问题前先看一眼这里的版本号与运行环境；本地模型、插件和同步问题通常都需要这两个信息才能定位。',
          },
        ],
      },
      {
        id: 'settings-system',
        title: '服务器、传输与维护',
        keywords: ['服务器', '8888', '自启', 'WebRTC', '信令', '插件', 'GC', '垃圾回收'],
        blocks: [
          {
            type: 'list',
            items: [
              '<b>服务器</b>：本地 8888 服务，提供插件接口、MCP、浏览器预览与上传页面；可启动、停用、重启、设为自启，并可设为每小时自动刷新。',
              '<b>文件传输</b>：配置 WebRTC 信令服务地址与可选配对码；文件传输助手与目录分享依赖它。',
              '<b>插件</b>：启用 / 禁用插件、配置参数、查看与编辑源码（详见下一章）。',
              '<b>系统维护</b>：配置 GC 周期（小时），清理缓存与临时产物。',
            ],
          },
          {
            type: 'callout',
            tone: 'tip',
            title: '8888 服务是本地服务',
            text: '它只监听本机，用于插件、MCP、浏览器预览与接收端上传。如果浏览器预览打不开，先回设置 → 服务器确认服务在运行。',
          },
        ],
      },
    ],
  },
  {
    id: 'plugins',
    group: '扩展与配置',
    number: '14',
    eyebrow: 'Extension · Plugins',
    title: '插件',
    subtitle: '把每次都手工做的小事交出去',
    summary: '插件是运行在软件内置 Python 引擎里的扩展脚本，可以读剪贴板、读写文件、访问网络，也能调用模型和 MCP 数据工具。',
    sections: [
      {
        id: 'plugins-use',
        title: '怎么用',
        keywords: ['插件', '启用', '参数', '右键', '运行插件', '剪贴板'],
        blocks: [
          {
            type: 'steps',
            items: [
              { title: '启用', text: '进入「设置 → 插件」，用开关启用；带参数的插件点「参数」先设置默认值。' },
              { title: '选中文本', text: '回到编辑器，选中文本，或把光标放在要插入的位置。' },
              { title: '右键运行', text: '右键 →「运行插件…」。带参数的插件会先弹出运行面板。' },
              { title: '填写参数', text: '文件 / 目录字段点「选择」打开系统文件选择器；勾选「记住这些值」下次沿用。' },
              { title: '查看结果', text: '插件处理完会替换选区或插入光标处；出错会在应用内提示，不会写坏文档。' },
            ],
          },
          {
            type: 'p',
            text: '有文件、目录、下拉或必填参数的插件每次都会询问，其余参数直接用设置页里保存的默认值。插件读取剪贴板走系统原生接口，不会再弹浏览器的剪贴板授权框。',
          },
        ],
      },
      {
        id: 'plugins-builtin',
        title: '内置插件',
        keywords: ['剪贴板表格', '导入表格', '链接抓正文', '存入知识库', '双链', '润色', '翻译', '示例数据', '表单填充'],
        blocks: [
          {
            type: 'list',
            items: [
              '<b>剪贴板表格 → Markdown</b>：在 Excel / 网页表格里复制一块区域，运行后在光标处生成 Markdown 表格，纯数值列自动右对齐。',
              '<b>导入表格文件</b>：运行面板里直接选 CSV / TSV / XLSX / JSON，转成 Markdown 表格；Excel 可指定工作表。',
              '<b>链接抓正文</b>：选中一个网址，抓取正文并转成 Markdown。',
              '<b>存入知识库并双链</b>：把选中内容写成当前知识库的笔记，按标签 / 标题重叠补「相关」双链。',
              '<b>模型润色 / 翻译</b>：润色、精简、扩写、翻译、摘要、纠错，走设置里的模型。',
              '<b>生成示例数据</b>：按「列名:类型」生成 Markdown / CSV / JSON 假数据。',
              '<b>表单填充助手</b>：从笔记 frontmatter、键值、两列表格或任务查询里提取字段，生成预填好的表单链接。',
              '另外还有表格合计、代码块整理、参考文献整理、数据摘要、frontmatter 体检等基础插件。',
            ],
          },
        ],
      },
      {
        id: 'plugins-dev',
        title: '编写插件',
        keywords: ['manifest', 'run', 'ctx', 'entry', 'surface', 'selection', 'report', '参数'],
        blocks: [
          {
            type: 'p',
            text: '每个插件是 <code>utils/guaxi_editor_plugins/&lt;id&gt;/</code> 下的一个目录，至少包含 <code>manifest.json</code> 和入口脚本（默认 <code>main.py</code>）。入口实现 <code>run(ctx)</code> 并返回字典。',
          },
          {
            type: 'code',
            lang: 'json',
            text: code('{', '  "id": "my-plugin",', '  "name": "我的插件",', '  "description": "一句话说明",', '  "entry": "main.py",', '  "capabilities": ["transform"],', '  "surfaces": ["editor"],', '  "scope": "selection",', '  "args": [', '    { "key": "mode", "label": "方式", "type": "select",', '      "options": ["润色", "翻译"], "default": "润色" },', '    { "key": "file", "label": "数据文件", "type": "file",', '      "accept": ["csv", "xlsx"] }', '  ],', '  "clipboard": false,', '  "allow_empty_selection": false,', '  "needs_kb": false', '}'),
          },
          {
            type: 'p',
            text: '参数类型：<code>text</code> / <code>number</code> / <code>bool</code> / <code>select</code> / <code>textarea</code> / <code>file</code> / <code>folder</code>。<code>file</code> 与 <code>folder</code> 在运行面板里带「选择」按钮，<code>accept</code> 用来限制扩展名。',
          },
          {
            type: 'table',
            head: ['作用面', '触发位置', '可用的 ctx', '返回值'],
            rows: [
              ['editor', '编辑器右键', 'selection、document、args、file_path、kb_location、kb_name、clipboard', '{"text": "..."} 替换选区或插入光标'],
              ['knowledge-base', '知识库标题栏', 'kb_name、kb_location', '{"report": "Markdown"}'],
              ['repo', '文件库工具栏', 'resources 等', '{"report": "Markdown"}'],
            ],
          },
        ],
      },
      {
        id: 'plugins-ui',
        title: '让插件自己弹界面',
        keywords: ['ui', 'values', '向导', '交互', '表单'],
        blocks: [
          {
            type: 'p',
            text: '插件不用把交互写死在参数里：返回 <code>{"ui": {...}}</code>，宿主会渲染同一个运行面板，用户提交后带着 <code>ctx.ui.values</code> 重新调用插件。最多 5 轮，足够做向导式交互。',
          },
          {
            type: 'code',
            lang: 'python',
            text: code('def run(ctx):', '    if not ctx.get("ui"):', '        return {"ui": {', '            "title": "选择数据源",', '            "description": "先选文件，再决定是否把首行当表头",', '            "fields": [', '                {"key": "path", "label": "CSV 文件", "type": "file",', '                 "accept": ["csv"], "required": True},', '                {"key": "header", "label": "首行是表头", "type": "bool",', '                 "default": True}', '            ],', '            "submit": "生成表格"', '        }}', '    path = ctx["ui"]["values"]["path"]', '    # ... 读取 path，生成 Markdown ...', '    return {"text": markdown_table}'),
          },
        ],
      },
      {
        id: 'plugins-api',
        title: '宿主提供给插件的接口',
        keywords: ['API', 'LLM', 'MCP', '8888', 'pandas', 'openpyxl', 'httpx', 'Python'],
        blocks: [
          {
            type: 'table',
            head: ['接口', '用途'],
            rows: [
              ['ctx.*', '选区、全文、参数、剪贴板、当前文件、知识库路径、ctx.ui 交互结果'],
              ['POST 127.0.0.1:8888/api/plugin/llm', '用设置里的模型做补全；密钥留在 Rust 侧。请求 {system, prompt, model_id?, temperature?, max_tokens?}，返回 {success, text, message?}'],
              ['GET 127.0.0.1:8888/api/plugin/host/context', '版本、插件目录、知识库根、资源库根、任务根与能力声明'],
              ['POST 127.0.0.1:8888/mcp', 'JSON-RPC 2.0 数据工具：guaxi_recall、guaxi_kb_semantic_search、guaxi_kb_stats、guaxi_kb_tags、guaxi_task_query、guaxi_search_text、web_search、web_fetch、brief_generate 等'],
              ['Python 运行时', '标准库 + httpx、trafilatura、bs4、pandas、polars、openpyxl、duckdb 等；可直接读写本地文件'],
            ],
          },
          {
            type: 'code',
            lang: 'python',
            text: code('import json, urllib.request', '', 'body = json.dumps({', '    "jsonrpc": "2.0", "id": 1, "method": "tools/call",', '    "params": {"name": "guaxi_task_query", "arguments": {"keyword": "周报"}}', '}).encode("utf-8")', 'req = urllib.request.Request(', '    "http://127.0.0.1:8888/mcp", data=body,', '    headers={"Content-Type": "application/json"})', 'with urllib.request.urlopen(req, timeout=30) as resp:', '    result = json.loads(resp.read().decode("utf-8"))'),
          },
        ],
      },
      {
        id: 'plugins-security',
        title: '安全边界',
        keywords: ['安全', '沙箱', '信任', '密钥', '网络', '文件'],
        blocks: [
          {
            type: 'callout',
            tone: 'warn',
            title: '插件没有沙箱',
            text: '插件就是本地 Python 代码：能读任意文件、访问网络。只启用你信任的插件。模型的 API Key 由软件自己保存并调用，不会传给插件；插件只拿到补全结果。',
          },
          {
            type: 'p',
            text: '后续计划扩展邮件、阅读器、知识源的只读数据接口，以及保存文件、应用启动、定时任务等事件钩子；插件之间也可能互相调用与共享设置。',
          },
        ],
      },
    ],
  },
]
