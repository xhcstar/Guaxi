# Codex Agent 与 cc-switch 使用指南

这份文档教你：**怎么让智能助手用上 Codex**、**怎么用 cc-switch 统一管供应商**、
以及**出问题时怎么判断卡在哪一步**。

设计与实现细节见 [codex-agent-design.md](./codex-agent-design.md)。

---

## 一、这次多了两个东西

**1. Codex Agent 引擎**

智能助手底部任务下拉里的 `Agent` 模式，现在默认由 [openai/codex](https://github.com/openai/codex)
驱动，比原来的自研 Python Agent 更快、工具更全（会自己执行命令、改文件、管任务清单）。

codex 是**随 Guaxi 一起分发**的，和你机器上可能已经装过的 codex 完全无关：

- 只用 Guaxi 目录下的 `utils/codex/codex.exe`，不会去找 PATH 或 `%LOCALAPPDATA%\OpenAI\Codex`。
- 配置放在 **`<Guaxi 安装目录>/config/codex-home/`**，你自己的 `~/.codex`（配置、登录凭据、会话记录）
  全程不读不写。
- 如果 codex 不可用，Agent 会自动回退到原来的内置 Python 引擎，功能不会整体失效。

**2. cc-switch 供应商**

模型下拉里新增了 **「cc-switch(本地代理)」**。选它以后，Guaxi 只记住一个代理地址，
具体用哪家模型供应商由 [cc-switch](https://github.com/farion1231/cc-switch) 决定。

好处很直接：**在 cc-switch 里点一下换供应商，Guaxi 的普通对话和 Codex Agent 同时跟随**，
不用回到 Guaxi 设置页改任何东西，也不用再为每家供应商单独存一个 API Key。

---

## 二、前置：先在 cc-switch 里打开本地代理

Guaxi 走的是 cc-switch 的**本地代理**，不是直接读它的配置文件。所以必须先在 cc-switch 里：

1. 打开设置，启用**本地代理**（总开关）。
2. 为 **Codex** 这个应用启用代理，记下监听地址与端口（出厂默认 `127.0.0.1:15721`）。
3. 选好你要用的供应商。

没打开代理的话，Guaxi 设置页里点「自动探测」会明确告诉你缺哪一步。

---

## 三、Guaxi 设置页怎么填

**设置 → 大模型**，往下滚到「自定义模型名」下面的新区块。

### cc-switch 部分

| 项 | 怎么填 |
|---|---|
| 启用 cc-switch 代理 | 打开 |
| cc-switch 代理地址 | 点旁边的**自动探测**自动回填；也可手填，如 `http://127.0.0.1:15721/v1` |
| cc-switch 模型名 | 代理转发时用的模型名，跟着你在 cc-switch 里选的供应商填 |
| cc-switch API Key | **一般留空**。代理会用上游供应商自己的密钥 |
| cc-switch 协议 | 保持 `chat`。这一项**只影响"对话"模式**；Codex 固定用 Responses 协议 |

「自动探测」做的事：只读地看一眼 `~/.cc-switch/settings.json` 和 `cc-switch.db`，
把代理端口回填进来，并在右边提示当前激活的供应商名字。**它不会修改 cc-switch 的任何文件。**
探测失败也不影响使用，手填地址就行。

### Codex 部分

| 项 | 怎么填 |
|---|---|
| Agent 引擎 | `Codex`（默认）。选 `内置 Python Agent` 可以强制用回老引擎 |
| Codex 模型名 | 留空则沿用上面的 cc-switch 模型名；想让 Agent 用更强的模型就单独填 |
| Codex 沙箱 | 默认 `workspace-write`，见下面第五节 |
| Codex 配置目录 | 只读展示，就是那个与 `~/.codex` 隔离的私有目录 |

**填完必须点「保存」。** 保存是整体替换，页面上的所有字段会一起写回。

---

## 四、看状态面板判断有没有配好

页面底部「当前状态」新增两条，鼠标悬停能看到详细路径与提示：

- **Codex 引擎**
  - `已内置` + 版本号（如 `codex-cli 0.149.1`）：二进制在位、供应商也配好了。
  - `未内置`：Guaxi 目录里没有 codex，Agent 会走内置 Python 引擎。开发环境下执行
    `npm run codex:fetch` 获取；安装版遇到这个情况说明打包时漏了 utils 分包。
- **cc-switch 代理**
  - `已启用`：地址与模型名都填了。
  - `未启用` / `配置不全`：悬停看提示，通常是没勾启用、没填模型名，或 cc-switch 那边代理没开。

---

## 五、开始用

1. 打开右侧智能助手。
2. 底部任务下拉选 **`Agent`**。
3. 模型下拉选 **`cc-switch(本地代理)`**。
4. 正常提问。想让它读写某个文件，可以用 `@` 引用知识库文件。

过程中你会看到：

- **工具卡片**：`执行命令` / `修改文件` / `MCP 工具` / `网络搜索` / `任务清单` / `思考`，
  点标题可展开看参数和结果（命令输出会带 `exit_code`）。运行中是转圈，完成变对勾，失败标红。
- **正文流式输出**在卡片下方逐步出现。
- **追问会自动接上上一轮**：codex 自己维护会话上下文，Guaxi 按会话记住线程 id。
  换会话、新建会话、清空消息都会重新开一条线程。
- **停止按钮**会真的把 codex 进程树杀掉，不会留后台进程。

---

## 六、沙箱：codex 能改什么、不能改什么

Codex 引擎下**不会**弹出原来那种"AI 请求执行写操作，请确认"的卡片。
原因是 `codex exec` 本身是非交互的，越权由 codex 自己的沙箱拦住，而不是逐次问你。
那张确认卡片仍然服务于内置 Python 引擎。

三档沙箱：

| 沙箱 | 含义 | 适用 |
|---|---|---|
| `read-only` | 只能读，不能改任何文件 | 只想让它做分析、检索、解释 |
| `workspace-write`（默认） | 可以改**当前知识库**及其他已配置知识库里的文件，不能碰别处 | 日常使用 |
| `danger-full-access` | 不设限 | 除非你清楚自己在做什么，否则别用 |

工作目录是当前活动知识库，其余已配置的知识库通过 `--add-dir` 一并加入可写范围，
所以跨库引用也能正常读写。没有打开任何知识库时，codex 会在
`config/codex-home/workspace/` 这个临时工作区里跑，不会把 Guaxi 的配置目录当工作区。

> 默认档位意味着**模型可以自主修改你知识库里的文件**。这是换用 codex 换来的速度与能力的代价。
> 如果不接受，把沙箱改成 `read-only`，写操作仍可通过内置引擎的确认卡片走。
> 重要内容建议先纳入 Git 管理，改坏了可以回滚。

---

## 七、常见问题

**Q：状态面板显示「Codex 引擎 未内置」**
开发环境：在项目根执行 `npm run codex:fetch`。`npm run tauri -- dev` / `npm run tauri -- build` 也会自动前置执行它。
下载约 100MB，成功后会在 `src-tauri/utils/codex/` 生成 `codex.exe` 和 `.codex-version.json`。

**Q：下载一直失败或超时**
脚本会**优先使用系统代理**：先看 `HTTPS_PROXY` / `ALL_PROXY` / `HTTP_PROXY` 环境变量，
Windows 上再读注册表里的系统代理（`Internet Settings` 的 `ProxyEnable` / `ProxyServer`）。
所以只要你本机有可用代理并已开启，通常直接就能下下来（日志里会打印用了哪个代理、来源是哪里）。
经代理失败会再直连重试一次。仍然失败也不影响构建，只是 Agent 会走内置引擎，稍后重跑即可。

**Q：报 `` `wire_api = "chat"` is no longer supported ``**
codex 0.149 起不再支持 chat 协议。Guaxi 生成给 codex 的配置固定用 `responses`，
不会出现这个问题。设置页那个「cc-switch 协议」下拉只作用于"对话"模式。

**Q：「对话」模式选 cc-switch 报错说协议不对**
把「cc-switch 协议」设为 `chat`。如果你的 cc-switch 代理只提供 Responses 接口，
那么"对话"模式暂时用不了它——改用「自定义(OpenAI兼容)」直连一个支持
`/v1/chat/completions` 的地址，Agent 那边继续用 cc-switch。

**Q：Agent 表现和以前一样，怀疑没走 codex**
看日志（`[codex_agent]` 前缀）。回退时会有一条
`agent_chat_stream: codex 引擎不可用，回退到内置 Python Agent。原因: ...`，
原因里会写明是二进制缺失、供应商没配还是进程起不来。

**Q：想彻底回到老引擎**
设置页把「Agent 引擎」改成 `内置 Python Agent` 并保存。

**Q：关掉软件后还有 codex 进程残留吗**
不会。子进程 PID 会登记进统一的进程收尾表，退出时按进程树清理；点停止按钮也是同样的处理。

---

## 八、给开发者

```bash
npm run codex:fetch          # 幂等获取（已是固定版本则跳过）
npm run codex:fetch:force    # 强制重新下载
npm run tauri -- dev         # 会自动前置执行 codex:fetch
```

升级 codex 版本的步骤：

1. 改 `tools/fetch_codex_sidecar.mjs` 里的 `PINNED_TAG`。
2. 跑 `npm run codex:fetch:force`，把日志里打印的 `sha256` 填回 `ASSETS` 对应平台。
3. 回归验证：`--json` 的事件类型与 item 字段名有没有变
   （见设计文档第 7 节的映射表），以及 `codex exec resume` 的参数支持范围。

排查用的关键位置：

- Rust 侧日志前缀：`[codex_agent]`、`[cc_switch]`。
- 私有配置目录：`<exe_dir>/config/codex-home/`（`config.toml` 每轮重写，可直接看生成结果）。
- 版本 pin 记录：`src-tauri/utils/codex/.codex-version.json`。

