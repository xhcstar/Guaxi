# Guaxi Agent Workspace · 方案

> 目标：把 `/chat`（`GuaxiChatPage`）从「多模型聊天页」改造成 Harness 形态的桌面 Agent 工作台：
> 能探索、研究、编码，也能操控本机；保留西部世界雷荷波（Rehoboam）动态背景，并让它承担真实信息。
>
> 本文只描述方案，不含实现。所有结论都对照过当前分支 `video` 的代码与本机 `codex.exe`（rust-v0.149.1）。
> 标注「已实测」的条目是在本机真实跑过协议得到的，不是文档推断。

## 0. 已实测的协议事实

在本机用 `codex.exe app-server --stdio` 跑了一轮握手，得到四条结论，它们直接决定方案可行性：

| 实测项 | 结果 |
|---|---|
| `initialize` → `model/list` | 正常返回。`model/list` **不需要供应商凭证**，直接列出模型（`gpt-5.6-sol` / `gpt-5.6-terra` …），含 `supportedReasoningEfforts`、`inputModalities`（含 image）、`serviceTiers`。设置页的模型下拉可以直接吃这份数据 |
| `thread/start` | 成功，返回 `thread.id`、`cwd`、`runtimeWorkspaceRoots`、`approvalPolicy`、`sandbox: {type, networkAccess}`，以及 rollout 文件的绝对路径。**codex 自己持久化会话**，我们的 sqlite 只需存索引 |
| `dynamicTools` | 默认被拒：`thread/start.dynamicTools requires experimentalApi capability`。在 `initialize` 里加 `capabilities: { experimentalApi: true }` 后，`thread/start` 带 `dynamicTools` **成功创建线程**。界面层的实验性接口因此可用 |
| `CODEX_HOME` | 目录不存在时 app-server 直接退出。启动前必须先 `create_dir_all` |

另外两点来自 schema 与代码：`sandbox` 对象带独立的 `networkAccess` 开关（「研究」模式可以 read-only + 联网，
不必为了联网放开写权限）；`thread/start` 支持 `baseInstructions` / `developerInstructions`，模式预设不必靠提示词硬凑。

> 一条实现约束：`experimentalApi` 是全局能力开关，开启后能用实验字段，也意味着这些字段更易随版本变动。
> 因此 §2.3 的「协议适配层单独成文件」不是洁癖，是必需的。

---

## 1. 现状：不缺引擎，缺工作台

先把已经存在的东西列清楚，这决定了本方案不该做什么。

| 已有能力 | 位置 | 状态 |
|---|---|---|
| 内置 codex 引擎（OpenAI 开源 harness） | `src-tauri/src/commands/codex_agent.rs`，`src-tauri/utils/codex/codex.exe` | 已打包、已跑通，走 `codex exec --json` 单轮子进程 |
| 认知层 MCP 工具面（33 个工具：18 read / 9 write / 2 network / 4 exec，四级授权 + 不可信闸 + 路径白名单 + 审计） | `src-tauri/src/cognition/tools.rs`、`mcp.rs`、`src-tauri/src/bin/guaxi_mcp.rs` | 已挂给 codex（`[mcp_servers.guaxi]`），服务端强制授权 |
| Python 兜底 Agent（ReAct + 工具 + 写操作确认卡片） | `agent_server/`（127.0.0.1:9876） | 可用，codex 失败时自动回退 |
| 右侧智能助手 | `src/components/HomeViewComponent/GuaxiAgentAssistant.vue`（3935 行） | 已接 codex，有工具卡片 / 确认卡 / codex 线程续接 |
| 截图 + OCR | `src-tauri/src/commands/guaxiscreencap.rs`、`commands/ocr.rs` | 已有，未接给 Agent |
| 雷荷波背景 | `src/components/ChatViewComponent/GuaxiRehoboamBackdrop.vue` | 纯装饰 canvas，只吃一个 `active` 布尔 |
| 旧聊天页 | `src/views/GuaxiChatPage.vue` + `ChatViewComponent/*` | 多模型 topic 列表 + `get_chat_answer_stream`，与 Agent 链路无关 |

**结论**：引擎（开源 Codex）、工具面（MCP）、权限分级（tier）、审计、兜底链都已就位。
短板有两处：

1. **驱动方式太弱**。现在每轮起一个 `codex exec` 进程，只能「发一句 → 等事件 → 进程退出」。
   没有中途改指令（steer）、没有逐动作审批、没有计划面板、没有 turn 级 diff、没有 token 预算——
   Harness 体验的核心恰恰是这些。
2. **界面形态是「聊天」不是「工作台」**。消息气泡 + 头像 + 模型下拉，
   表达不了「一个跑着的任务」：当前在哪个目录、改了哪几个文件、花了多少、卡在哪个审批。

---

## 2. 复用策略：换协议，不换引擎

用户的诉求是「直接复用已有的开源 Claude 等 agent 实现」。逐项核对后，结论是
**继续用已内置的 openai/codex，但把集成方式从 `exec --json` 升级到 `codex app-server`**。

### 2.1 三个候选

| 方案 | 可行性 | 判断 |
|---|---|---|
| **A. codex `app-server`（推荐）** | 本机 `codex.exe app-server --help` 确认存在；`generate-json-schema` 导出 252 个 v2 schema；握手、`model/list`、`thread/start`、`dynamicTools` 均已实测通过（§0） | 同一个二进制、同一份配置、同一套 MCP 挂载；零新增运行时 |
| B. Claude Agent SDK | 需要 Node 运行时并捆绑 Claude Code CLI（数百 MB），且要求 Anthropic 凭证 | 与「release 包离线自足」冲突；且本项目模型栈是 cc-switch / OpenAI 兼容 Responses，接不上 |
| C. opencode 等第三方 harness | 自带 TS server，需要再分发一个运行时 | 等于引入第三套引擎，维护成本翻倍 |

选 A 还有一个实际收益：`model/list` 直接给出模型能力（是否支持 image 输入、有哪些 reasoning effort），
「操作电脑」模式可以据此判断模型能不能看截图——不需要我们自己维护一张模型能力表。

### 2.2 为什么 app-server 就是「复用开源 agent 实现」

`codex app-server` 是 Codex 官方给 IDE 扩展/桌面端用的 JSON-RPC 协议，不是 Guaxi 自造的壳。
它把 agent 的完整生命周期暴露出来（均来自 `app-server generate-json-schema` 的实测结果）：

**客户端 → 服务端**
`thread/start`、`thread/resume`、`thread/fork`、`thread/rollback`、`thread/compact/start`、
`thread/list`、`thread/read`、`thread/name/set`、`thread/archive`、`thread/delete`、
`turn/start`、`turn/steer`、`turn/interrupt`、`review/start`、
`model/list`、`skills/list`、`mcpServerStatus/list`、`config/read`、`config/value/write`。

**服务端 → 客户端（通知）**
`item/started`、`item/completed`、`item/agentMessage/delta`、
`item/commandExecution/outputDelta`、`item/fileChange/patchUpdated`、
`item/reasoning/summaryTextDelta`、`item/plan/delta`、
`turn/plan/updated`、`turn/diff/updated`、`turn/completed`、
`thread/tokenUsage/updated`、`thread/status/changed`、`error`。

**服务端 → 客户端（请求，即审批）**
`item/commandExecution/requestApproval`、`item/fileChange/requestApproval`、
`item/permissions/requestApproval`、`item/tool/requestUserInput`、
`mcpServer/elicitation/request`、`item/tool/call`（客户端执行工具）、
`execCommandApproval`、`applyPatchApproval`。

这里组接口直接对应 Harness 的全部手感：计划、审批、中断、转向、diff、用量。
`item/tool/call` 还提供了「把工具执行留在客户端」的通道——这是后面接电脑操控的备选路径，
且已实测需要 `experimentalApi` 能力（见 §0）。

一项目前没被利用、但价值很高的能力：`codex app-server` 的 `thread/list` / `thread/read` / `thread/archive` /
`thread/section/*` 是**完整的历史管理**。工作台的会话栏可以直接以 codex 自己的 rollout 为数据源，
Guaxi 只在旁边挂一层「标注 + 检索索引」，不必自建一套会话模型。

### 2.3 集成形态

新增 `src-tauri/src/commands/codex_app_server.rs`，与现有 `codex_agent.rs` 并列，不替换：

```
前端 AgentWorkspace
  └─ invoke('agent_start_turn' / 'agent_steer' / 'agent_interrupt' / 'agent_decide')
       └─ codex_app_server.rs
            ├─ 常驻子进程：codex app-server --listen stdio://
            │     env: CODEX_HOME=<exe_dir>/config/codex-home
            │          GUAXI_CODEX_API_KEY=***
            │     stdin/stdout 走 JSON-RPC 2.0 行协议
            ├─ 通知 → Tauri 事件 agent://notification（原样透传 + 轻量归一）
            ├─ 服务端请求 → Tauri 事件 agent://approval（等前端回 decision）
            └─ 崩溃/退出 → 自动重启一次；仍失败则回退 codex_agent.rs（exec）
```

三条设计要点：

1. **协议适配层单独成文件**，上游 `[experimental]` 的字段变动只改这一层。
   前端只认 `agent://*` 事件，不认 codex 原始字段名。
2. **配置沿用现有生成器**。`write_codex_config()` 继续负责 provider / MCP / sandbox；
   只把 `approval_policy` 从写死的 `never` 改成按模式下发（见 §4）。
3. **旧链路保留**。`agent_engine` 增加第三档 `codex-app-server`，默认开；失败自动降级到
   `codex exec`，再失败降级到 Python Agent。三步都有日志前缀，便于定位。

---

## 3. 信息架构：从「聊天」到「工作台」

### 3.1 结构

```
/chat  (GuaxiChatPage 改写为壳)
└─ AgentWorkspace
   ├─ AgentSessionRail        左：会话（按工作区分组）+ 状态点 + 新建
   ├─ AgentConversation       中：转录 + 运行账本（ledger）+ 输入区
   │   ├─ AgentRunLedger      一次 run 内的工具/命令/改动，逐行
   │   ├─ AgentMessage        正文（Markdown，复用现有渲染管线）
   │   └─ AgentComposer       模式 + 工作区 + 模型 + @ 引用 + 发送/停止
   ├─ AgentInstrument         右：雷荷波仪表（数据绑定）+ 计划 + 改动 + 用量
   │   ├─ AgentPlanPanel      计划步骤（turn/plan/updated）
   │   ├─ AgentDiffPanel      turn 级 diff（turn/diff/updated，复用 GuaxiDiffPanel）
   │   └─ AgentUsage          上下文用量、耗时、工具计数
   └─ AgentApprovalSheet      浮层：审批（命令 / 改动 / 权限 / 用户输入）
```

左栏不再是「模型 → topic」，而是会话：一条会话 = 一个 codex thread = 一个工作目录 + 一份沙箱策略。
模型变成输入区里的一个可切换项（`model/list` 提供），因为「用哪个模型」在 Agent 语境下是轮次级参数，
由 `turn/start` 的 `model` / `effort` 覆盖，不是会话身份。

### 3.2 四种任务模式

模式是一组预设，落到 `thread/start` + `turn/start` 的参数上，不是提示词魔法。

| 模式 | 沙箱 | 联网 | 审批 | 说明 | 典型动作 |
|---|---|---|---|---|---|
| 探索 | `read-only` | 关 | `on-request` | 只读，产出结论 | 检索、读文件、画关系、写摘要到知识库 |
| 编码 | `workspace-write` | 关 | `on-request` | 在独立 git worktree 里改 | 读→改→跑测试→看 diff→review/start |
| 研究 | `read-only` | **开**（`sandbox.networkAccess`） | `on-request` | 联网 + 认知层召回为主 | `guaxi_recall` → 抓网页 → 归纳 → 落笔记 |
| 操作电脑 | `workspace-write` | 关 | `on-request` + GUI 逐动作确认 | 截屏优先、坐标网格、可中断 | 看屏、点按、输入、切窗口 |

「研究」模式的关键在 `read-only` 搭配独立的 `networkAccess`：**联网不需要放开写权限**。
如果沿用旧的三档沙箱（`--sandbox read-only` 连网一起关掉），研究类任务就只能退到 `workspace-write`，
等于为了查资料把整个知识库交给模型改。app-server 的 `sandbox: { type, networkAccess }` 对象避免了这个取舍。

「编码」模式建议在 `config/codex-home/workspaces/<session>` 下自动 `git worktree add`（仓库已用 `git2`），
改完用现有 diff 面板审核，确认后再合回主工作区。避免默认档位直接改用户的活文件。

### 3.3 会话与持久化

现有 `rightSiderStore` 用 localStorage 存助手会话，够用但不适合 Agent（thread 可能很长，还要能按目录检索）。
新增 `agent_sessions` / `agent_turns` 两张表进现有 sqlite（`cognition/store.rs` 已提供连接）：

```
agent_sessions(id, thread_id, title, work_dir, mode, sandbox, created_at, updated_at, last_status)
agent_turns(id, session_id, turn_id, status, input, items_json, diff, tokens_json, started_at, ended_at)
```

恢复会话优先 `thread/resume`（codex 自己维护上下文），
`thread/read` 只在需要完整历史渲染时调用；本地表只存「索引 + 摘要 + 我们自己的标注」。
即时打开（不转圈）靠本地表，细节按需补齐。

---

## 4. 电脑操控

### 4.1 现在有什么、缺什么

子代理对后端做了一次完整盘点，结论比预想的更明确：

**已有（可直接复用）**

- 截图：`ScreenShotTool.exe`，但**只能交互式框选**，没有静默全屏/指定窗口抓取。
- OCR：`screenshot_ocr` → agent_server `/ocr`（rapidocr，离线），返回文本；无带坐标的文本块。
- 打开应用/文件：`cmd_open_in_os`、`reveal_in_os`、`neovim_open_path`、`open_in_craft_app`、opener 插件。
- 浏览器：chromiumoxide 无头 Chrome（仅抓取，有反检测脚本，**无 click/type/screenshot 驱动**）；
  webview 驱动 ChatGPT/元宝（硬编码两个站点）。
- 终端：`terminal.rs` 的 PTY（portable-pty），SSH / 串口 / Telnet 执行通道。
- 剪贴板：插件级读写。

**全缺**

- 鼠标 / 键盘注入：仓库内零实现，没有任何输入注入依赖。
- 窗口枚举 / 激活窗口识别。
- 静默周期截屏、屏幕语义理解（OCR 只到纯文本，无坐标）。
- 剪贴板监听（只有手动读写）。
- 通用浏览器自动化（CDP click/type）。

### 4.2 设计

新增 `src-tauri/src/computer/mod.rs`，能力登记进 `cognition/tools.rs` 的既有注册表（不另起一套工具系统）：

| 工具 | 等级 | 说明 |
|---|---|---|
| `guaxi_screen_capture` | `Gui` | 截全屏/指定窗口/区域，返回图片路径 + 逻辑/物理尺寸 + DPI |
| `guaxi_screen_ocr` | `Gui` | 截图后 OCR，返回**带坐标**的文本块 |
| `guaxi_window_list` / `guaxi_window_focus` | `Gui` | 枚举/激活窗口（标题、进程、矩形） |
| `guaxi_mouse` | `Gui` | move / click / drag / scroll（逻辑坐标） |
| `guaxi_keyboard` | `Gui` | type / key / hotkey |
| `guaxi_clipboard` | `Gui` | 读/写文本与图片 |

实现依赖：输入注入用 `enigo`（跨平台、无加密依赖，符合 Windows 无 NASM 的构建约束），
窗口枚举用 `windows` crate 的最小 feature 集。**引入前必须 `cargo update --dry-run` 验证解析，
并确认不拉进 ring 之外的加密栈。**

### 4.3 安全约束

盘点过程中发现两处现有缺口，本方案要顺带堵上，否则 GUI 能力会把它们放大：

1. **`agent_server` 的 `/tools/exec` 绕过了确认闸**（`agent_server/main.py:120`）。
   它直接调 `tool_dispatch`，不经过 `needs_confirm`；本机任何进程都能无确认执行
   `write_file` / `exec_shell`。现在只靠 127.0.0.1 绑定缓解。
   建议在 P4 之前就把该路由限制为「仅 agent 循环内部可用的回环 token」或直接下线。
2. **`exec_python` 的 `timeout_secs` 被读取但未强制**（`agent_server/tools/code_exec.py:31`），
   且受限 builtins 白名单里含 `open` / `Path`，实际文件可达。
3. **新等级 `Gui`，默认关闭**。与现有 `read/write/network/exec` 同一套服务端强制授权，
   开启时在设置页明确说明「AI 可以移动你的鼠标并打字」。
4. **逐动作确认**。GUI 工具默认 `require_confirm`：调用时阻塞等待前端审批卡，
   卡片展示动作摘要 + 截图缩略 + 「本次允许 / 本会话允许 / 拒绝」。与现有
   `agent-tool-confirm` 事件复用同一套交互；不可信来源（网页/剪贴板）发起的 GUI 调用直接拒绝，不弹卡。
5. **会话级熔断**。工作台顶部常驻「AI 正在控制电脑」横幅，移动鼠标前 300ms 高亮；
   `Esc` 或全局热键立即 `turn/interrupt` 并撤销 `Gui` 授权；所有动作走现有 `audit_tool`。

### 4.4 看屏的正确姿势

不要只丢一张原图。给模型的应是一张**带编号网格的截图**（如 12×8，编号 + 像素坐标标注），
外加一次**带坐标的 OCR 文本块表**——这是 §4.2 里 `guaxi_screen_ocr` 要返回坐标的原因，
现有 `/ocr` 只回纯文本，做不了定位。

模型输出「点 7 号格中心」比输出裸坐标稳定得多，也便于在审批卡里向人解释。网格合成在 Rust 侧完成
（`image` 已在依赖内，`imageproc` 需评估），不额外引库。

还有一条限制要说清：能看截图的前提是模型支持图像输入。`model/list` 的 `inputModalities`
直接标了这一项，「操作电脑」模式启动时据此判断，不支持就禁用该模式并说明原因，而不是发出去一张图等报错。

---

## 5. 视觉方案

雷荷波是这条产品线的身份，不能当成一张贴在聊天背后的壁纸。设计上把它变成**仪表**：
它表示的是一台机器正在运转，而不是装饰。

### 5.1 设计原则

- **一个主角**：右侧仪表。其余全部安静——细线、无卡片堆叠、无阴影。
- **结构即信息**：环形上的每一段都必须对应真实数据；没有数据就不画。
- **正文优先**：转录列左对齐、限宽 68ch；仪表再好看也不许抢正文的宽度。
- **动作型动效**：只有三种运动——扫描线随模型活动推进、待审批时琥珀色脉冲一次、账本行展开。不做入场动画。

### 5.2 色彩（沿用现有主题变量，不另立一套）

| 角色 | 亮色 | 暗色 |
|---|---|---|
| 底色 | `#e7e7e4` | `#17191d` |
| 面板 | `#f1f1ef` | `#1c1f24` |
| 正文 | `#1f2328` | `#d8e0eb` |
| 次要 | `#57606a` | `#9da5b4` |
| 细线 | `rgba(18,18,18,0.14)` | `rgba(241,241,239,0.12)` |
| 强调（仅「需要你」） | `#b47d1a` | `#e0b050` |
| 错误 | `#b42318` | `#e06c75` |

刻意不用绿色表示成功：成功就是「安静地变成已完成」，只有异常和等待才着色。
这样琥珀色在这个界面里永远只意味一件事：**轮到你了**。

### 5.3 字体

- 正文：`"Segoe UI Variable", "Microsoft YaHei UI", system-ui`，15px / 1.75。
- 仪表与代码：`Maple Mono NF CN`（已随包分发，`src-tauri/utils/fonts`）用于路径、命令、token 数、时间。
  等宽只出现在「机器的语言」里，正文保持可读。
- 不做全大写标签，不加眉标（eyebrow），不加 `→` 后缀。

### 5.4 布局

```
┌──────────────┬──────────────────────────────────────────┬────────────────────┐
│ 会话          │  转录列 ≤68ch                              │  仪表               │
│              │                                            │    ╭──────╮        │
│ ● 编码 · 运行 │  you   把这章的图表重画成横向对比           │   │  ◯   │        │
│ ◐ 探索 · 待批 │                                            │    ╰──────╯        │
│ ○ 研究 · 完成 │  run 04 · 命令 12 · 改动 3 文件 · 2m14s     │  计划  3/7          │
│              │  ┌                                          │  改动  +42 −17      │
│ 工作区        │  │ 读 src/…/ChartPanel.vue        0.8s ✓   │  待批  1            │
│ ▾ guaxi      │  │ 改 src/…/ChartPanel.vue      +38 −12    │  token 18.2k/200k   │
│ ▾ notes      │  │ 跑 npm run test             12s  ✓       │                    │
│              │  └                                          │  沙箱 workspace-write│
│              │                                            │  模型 sonnet-5      │
│              │  正文 …                                    │                    │
├──────────────┴────────────────────────────────────────────┴────────────────────┤
│ ◇ 编码   工作区 guaxi-wt   模型 ▾    @ 引用           [ 发送 / 停止 ]            │
└─────────────────────────────────────────────────────────────────────────────────┘
```

转录列与仪表之间不设分隔线，用一条 1px 的细线标出仪表边界即可。
运行账本用左侧一条竖引导线，行与行之间不加边框——这是「一次执行」，不是一堆卡片。

### 5.5 雷荷波如何数据绑定

`GuaxiRehoboamBackdrop.vue` 新增可选 `telemetry` prop（旧 `active` 保持兼容）：

| 视觉元素 | 数据来源 | 映射规则 |
|---|---|---|
| 外圈波动幅度 | token 输出速率 | `thread/tokenUsage/updated` 相邻两帧差分，取最近 3 秒均值；0 → 静止基线，>200 tok/s → 满幅 |
| 环上分段数 | 工具调用计数 | `item/started` 中 commandExecution / fileChange / mcpToolCall 累加；每段代表 1 次，上限 24 段后不再增加（避免糊成一圈） |
| 事件锚点亮度 | 待处理审批数 | 0 → 全部压到 0.12；≥1 → 逐个点亮，第 3 个之后统一满亮 |
| 扫描线角度 | 当前 item 序号 | 每开始一个新 item 前进固定角度；无 item 时以 ~0.26 rad/s 缓慢自转（保持现有实现） |
| 环心文字（可选） | 模式 + 工作区名 | 只在宽度 > 720px 时绘制，等宽小字，`rgba(...,0.5)` |

这是纯视觉的只读映射，不改协议、不影响性能。三条实现约束：

- **不引入额外渲染负担**。现有实现已是单 canvas + `requestAnimationFrame`，telemetry 只改绘制参数，
  不新增图层；每帧最多读 4 个数字，不做字符串拼接。
- **`prefers-reduced-motion` 下退化为静态图**，但保留一帧「当前状态」的定格（环上仍能看出有几个待批）。
- **数据不可用时不能撒谎**。没拿到 `tokenUsage` 就画基线，而不是随机波动——仪表一旦会「假装有数据」，
  它就从仪表退化回壁纸了。

---

## 6. 分阶段实施

| 阶段 | 内容 | 产出 | 估算 |
|---|---|---|---|
| **P0 协议冒烟** | ~~探针~~ 已完成（§0）。剩下把探针固化成 `tools/codex_app_server_probe.mjs`，供每次升级 codex 后回归 | 可重复的协议回归脚本 | 0.5 天 |
| **P1 引擎层** | `codex_app_server.rs` + 5 个 Tauri 命令 + `agent://*` 事件契约；`agent_engine` 增加第三档，失败逐级降级 | 后端可独立驱动一轮带审批的 turn | 2–3 天 |
| **P2 工作台壳** | 路由切到 `AgentWorkspace`；会话栏、转录、账本、输入区、仪表（先接 token 与工具计数）；旧页保留在 `/chat-legacy` 直到对齐 | 可日常使用的 Harness 界面 | 3–4 天 |
| **P3 审批 / 计划 / diff** | 审批浮层；计划面板；turn diff 复用现有 diff 组件；`review/start` 入口 | 完整 Harness 闭环 | 2 天 |
| **P4 电脑操控** | `computer/mod.rs`、`Gui` 等级、网格截图、逐动作审批、熔断横幅、审计 | 能看屏并操作本机 | 3–4 天 |
| **P5 会话与细节** | sqlite 会话表、恢复、搜索、导出、快捷键 | 可长期使用 | 2 天 |
| **P6 回归与打包** | 三档引擎降级回归、离线打包、codex 版本升级流程、协议适配层单测 | 可发布 | 2 天 |

合计约 15–19 人日。P1 结束即可用最小界面验证，不必等 P2 全做完。

---

## 7. 风险与对策

| 风险 | 影响 | 对策 |
|---|---|---|
| `app-server` 标记 `[experimental]`，字段可能变 | 升级 codex 后解析失败 | 版本钉死在 `tools/fetch_codex_sidecar.mjs`；所有字段访问集中在适配层；升级时跑 P0 探针 |
| `experimentalApi` 是全局开关 | 开了才能用 `dynamicTools`，字段稳定性更低 | 默认只开 `dynamicTools` 必需的部分；客户端工具主路线仍走 MCP（`guaxi_mcp` 已验证），`item/tool/call` 作为补充 |
| 审批流与现有 `agent-tool-confirm` 语义重叠 | 两套交互 | 新引擎用自己的审批事件；旧 Python 引擎的确认卡保留，二者在 UI 层统一成同一个 `AgentApprovalSheet` |
| 新增 `enigo` / `windows` 依赖触发构建 OOM | 打包失败 | 引库前 `cargo update --dry-run`；`windows` 只开必需 feature；在现有 4 分钟构建预算内验证 |
| GUI 控制被滥用 | 安全事故 | 默认关闭 + 逐动作确认 + 不可信来源强制拒绝 + 熔断 + 审计；并先堵上 §4.3 的两处既有缺口 |
| 转录很长时渲染卡顿 | 体验退化 | 虚拟滚动；账本行折叠为单行；正文按 turn 分块懒渲染 |
| 三套引擎（app-server / exec / Python）并存 | 维护与回归成本 | 明确各档定位：app-server 主用、exec 仅降级、Python 只保 `/ocr` 等旁路路由；每次升级只回归前两档 |

---

## 8. 验收标准

- 一次「编码」任务：选工作区 → 模型读文件 → 改文件 → 跑测试 → 打开 diff → 中途 steer →
  拒绝一次命令审批 → 任务完成；全程不出现旧的模型/topic 列表。
- 一次「操作电脑」任务：截屏 → 网格编号 → 点开某窗口 → 输入文本 → 中途 Esc 中断，鼠标立刻交还。
- 三档引擎降级：删掉 `codex.exe` 后仍可用（Python 兜底），日志能说明降级原因。
- 断网、断代理、cc-switch 未启动时，界面给出可读的失败原因，而不是空白。
- 雷荷波在任务空闲、运行、待审批三种状态下视觉上可区分，且 `prefers-reduced-motion` 下静止。

---

## 9. 与现有代码的关系（改动清单）

**新增**
`src-tauri/src/commands/codex_app_server.rs`、`src-tauri/src/computer/mod.rs`、
`src/stores/agentWorkspaceStore.js`、`src/components/AgentWorkspace/*.vue`、
`tools/codex_app_server_probe.mjs`。

**修改**
`src-tauri/src/main.rs`（命令注册）、`commands.rs`（模块声明）、
`cognition/tools.rs`（`Tier::Gui` + GUI 工具注册）、`cognition/commands.rs`（tier 状态）、
`commands/settingview.rs`（`agent_engine` 第三档、`tool_tier_gui`）、
`GuaxiRehoboamBackdrop.vue`（telemetry 绑定）、`router/index.js`（`/chat` → 工作台）、
`GuideAgentAssistant.vue`（抽出共享的引擎客户端，避免两套逻辑）。

**退役（延后）**
`ChatViewComponent/ChatList.vue`、`GuaxiSidebarChatView.vue` 的 topic 列表；
`chatStore` 的 `SelectModelchatSessions` 结构在新页面里不再使用。
建议先并存一个版本，确认新页覆盖了「普通对话」场景后再删。
