# Codex Agent 与 cc-switch 供应商 · 设计文档

> 适用版本：分支 `video`。本文与代码一一对应，所有命令、字段、事件名均可在源码中检索到。
> 配套使用文档见 [codex-agent-guide.md](./codex-agent-guide.md)。

## 1. 背景与目标

智能助手原本的 Agent 模式（`task = 'agent'`）链路是：

```
GuaxiAgentAssistant.vue → invoke('agent_chat_stream')
  → chatview.rs::agent_chat_stream
    → 启动 Python FastAPI（agent_server/，127.0.0.1:9876）
      → 自研 function-calling 循环（agent.py + tools/{filesystem,web,code_exec}.py）
        → SSE 回传 → agent-step / agent-tool-confirm 事件
```

两个问题：

1. **慢**：每次要确认托管 Python 就绪、比对 pip 依赖、拉起 uvicorn，启动超时上限就有 30 秒。
2. **弱**：自研工具循环只有读写文件、检索、执行命令这几件事，没有沙箱、没有 diff、没有计划管理。

同时模型侧还有一个约束：`ChatModel` 把 0~10 十一家供应商硬编码在 `chatview.rs` 里，每加一家都要改枚举、改分支、在设置页加一个 Key 输入框。

本次改动解决这两件事：

- **引入 openai/codex 作为新的 Agent 引擎**，保留 Python `agent_server` 作为回退（它还承载 `/ocr` 等其他路由，不能删）。
- **新增 cc-switch 本地代理支持**，Guaxi 只记一个入口地址，具体用哪家供应商交给 cc-switch 决定。

## 2. 关键约束与实测结论

这几条决定了下面所有设计，都是实测得出、而非推测：

| 结论 | 依据 | 影响 |
|---|---|---|
| **没有可用的官方 codex Rust 库** | crates.io 上 `codex-core` 未发布；`codex-protocol` 是第三方 fork（namastexlabs）且停在 0.63.0 | 只能以子进程方式驱动 codex，不能 `cargo add` |
| **官方 Windows x64 裸 exe 有 297MB** | GitHub release `rust-v0.149.1` 资产清单 | 不能用 Tauri `externalBin`（会整个嵌进 NSIS 安装包），改走项目已有的 `utils/` 外部分包机制 |
| **`--ignore-user-config` 不能传** | 其语义是"不加载 `$CODEX_HOME/config.toml`"，而那正是我们生成的配置 | 隔离只靠 `CODEX_HOME` 指向私有目录 |
| **`wire_api = "chat"` 已被移除** | codex 0.149.1 启动即报 `` `wire_api = "chat"` is no longer supported ``，拒绝加载配置 | codex 的 config.toml 固定写 `responses` |
| **`--json` 的 item 用 `type` 字段** | 实跑 `codex exec --json` 观测到 `{"item":{"id":"item_0","type":"error",...}}` | 解析对 `item_type` 和 `type` 都要兜住 |
| **`codex exec resume` 不支持 `-C`/`-s`/`--add-dir`** | `codex exec resume --help` | 续接时工作目录与沙箱沿用原会话，只能传通用参数 |

## 3. 代码地图

| 层 | 文件 | 职责 |
|---|---|---|
| Codex 引擎 | `src-tauri/src/commands/codex_agent.rs` | 定位 sidecar、生成私有 `config.toml`、派生 `codex exec`、解析 JSONL 事件并翻译成前端事件、取消与进程回收、`codex_agent_status` |
| cc-switch 探测 | `src-tauri/src/commands/cc_switch.rs` | 只读探测 `~/.cc-switch/settings.json` 与 `cc-switch.db`，返回代理地址与当前供应商名 |
| 引擎分流 | `src-tauri/src/commands/chatview.rs` | `agent_chat_stream` 按 `agent_engine` 分流并在失败时回退；`ChatModel::CCSWITCH`(11) 与 `cc_switch_provider_config` / `get_answer_from_cc_switch{,_stream}` |
| 配置模型 | `src-tauri/src/commands/settingview.rs` | `LlmConfig` 新增 `cc_switch_*` / `codex_*` / `agent_engine`；`cc_switch_api_key` 接入既有加解密字段链路 |
| 进程收尾 | `src-tauri/src/guaxiservices/process_shutdown.rs` | 复用 `terminate_process_tree` / `register_managed_pid`，软件退出时清理残留 codex |
| 命令注册 | `src-tauri/src/main.rs` | 注册 `codex_agent_status`、`detect_cc_switch_config`；关闭流程加 `stop_codex_agent()` |
| 分发脚本 | `tools/fetch_codex_sidecar.mjs` | 固定版本下载官方 release 归档、系统代理、校验和、解压到 `src-tauri/utils/codex/` |
| 构建钩子 | `tools/run_tauri_with_utils.mjs` | `npm run tauri *` 前置执行上面的脚本（幂等、失败不阻断） |
| 助手前端 | `src/components/HomeViewComponent/GuaxiAgentAssistant.vue` | codex item 类型的卡片标签/图标、`codex-thread` 监听、请求带 `thread_id` |
| 设置前端 | `src/components/SettingViewComponent/LLMModalSetting.vue` | cc-switch 与 Codex 配置表单、自动探测、状态面板两个新条目 |
| 会话状态 | `src/stores/rightSiderStore.js` | 按会话持久化 `codexThreadId`，新建/切换/清空会话时同步 |
| 模型清单 | `src/stores/chatStore.js` | `remoteModel` 增加「cc-switch(本地代理)」(id 11) |

## 4. 运行时架构

```
任务 = agent
  └─ invoke('agent_chat_stream')            ← 前端入口不变，engine 由后端决定
       └─ chatview.rs::agent_chat_stream
            ├─ modelid 7/8（网页模型）→ 降级为普通对话（原有逻辑）
            ├─ agent_engine != "builtin"
            │    └─ codex_agent::run_codex_agent
            │         env: CODEX_HOME=<exe_dir>/config/codex-home
            │              GUAXI_CODEX_API_KEY=***
            │         codex exec --json --skip-git-repo-check
            │                    -C <当前库> --add-dir <其他库> -s <sandbox> -m <model> -
            │         （续问：codex exec resume <thread_id> ... -）
            │         └─ 逐行解析 JSONL → 翻译成前端事件
            │              失败（二进制缺失/供应商没配/起不来）→ Err → 下面这条回退
            └─ agent_engine == "builtin" 或 codex 失败
                 └─ agent_server::ensure_agent_server_started → SSE（原有逻辑）

普通对话（task = chat 等）
  └─ ChatModel::CCSWITCH(11) → base_url = cc-switch 代理
       └─ 复用 openai_compat_sse_stream / openai_compat_chat
```

codex 的供应商与普通对话共用同一份 cc-switch 配置：生成的 `config.toml` 里
`[model_providers.guaxi].base_url` 就是代理地址。**在 cc-switch 里换供应商，两条链路同时跟随。**
cc-switch 未启用时，两者都退回 `custom_base_url` / `custom_api_key` / `custom_model`。

## 5. 与用户环境的隔离

这是本设计的硬约束：**不影响用户机器上已有的 codex，也不读写它的配置**。四条保证：

1. **二进制只认自己的**。`codex_binary_path()` 走
   `bundled_resources::ensure_relative_dir("utils/codex")`，也就是 release 的 `<install>/utils/codex/`
   或 dev 的 `src-tauri/utils/codex/`。**刻意不查 PATH、不查 `%LOCALAPPDATA%\OpenAI\Codex`、不查 npm 全局。**
2. **配置目录是私有的**。`codex_home_dir()` = `settingview::get_config_path()` 的父目录 + `codex-home`，
   即 `<exe_dir>/config/codex-home/`。子进程的 `CODEX_HOME` 指向它，于是 codex 的
   `config.toml`、`sessions/`、缓存全部落在 Guaxi 自己的目录里，`~/.codex` 完全不参与。
3. **配置每次重写**。`write_codex_config()` 在每轮 Agent 开始前幂等覆盖 `config.toml`，
   设置页改完立刻生效，也不需要做增量合并。手写 TOML 字符串（`toml_escape` 转义反斜杠与引号），
   不引入 `toml` 依赖。
4. **密钥不落盘**。配置里写 `env_key = "GUAXI_CODEX_API_KEY"`，密钥只在派生子进程时经环境变量传入，
   不写 `auth.json`。供应商没配 Key 时用占位值 `guaxi-managed`（cc-switch 代理会用上游真实密钥）。

生成的 `config.toml` 形如：

```toml
# 本文件由 Guaxi 自动生成，每次启动 Agent 都会被覆盖，请不要手动编辑。
# 它只作用于 Guaxi 内置的 codex，与用户自己的 ~/.codex 完全无关。
model = "<model>"
model_provider = "guaxi"
approval_policy = "never"
sandbox_mode = "workspace-write"
disable_response_storage = true

[model_providers.guaxi]
name = "Guaxi (cc-switch)"
base_url = "http://127.0.0.1:15721/v1"
wire_api = "responses"
env_key = "GUAXI_CODEX_API_KEY"
```

`approval_policy = "never"` + `sandbox_mode = "workspace-write"` 是刻意的组合：
`codex exec` 本身是非交互的（`-a/--ask-for-approval` 只在顶层命令存在），
越权由 codex 自身的沙箱拦住，而不是靠 Guaxi 逐次弹确认卡片。因此 **codex 引擎下不会触发
`agent-tool-confirm`**，那张确认卡片仍然服务于内置 Python 引擎。

## 6. 配置字段

`LlmConfig`（`settingview.rs`）新增字段，全部 `#[serde(default)]`，旧的 `guaxi_config.json` 可直接升级：

| 字段 | 默认值 | 作用 |
|---|---|---|
| `cc_switch_enabled` | `false` | 是否启用 cc-switch 代理作为供应商 |
| `cc_switch_base_url` | `http://127.0.0.1:15721/v1` | 代理入口地址 |
| `cc_switch_api_key` | 空 | 可选；**已接入配置加密链路** |
| `cc_switch_model` | 空 | 转发时使用的模型名 |
| `cc_switch_wire_api` | `chat` | **只影响普通对话链路**；codex 固定 `responses` |
| `codex_model` | 空 | Codex 专用模型名，留空则沿用供应商模型名 |
| `codex_sandbox` | `workspace-write` | 未知值一律收敛到 `workspace-write` |
| `agent_engine` | `codex` | `codex` 或 `builtin` |

> 注意：`set_llm_config` 是**整体替换**。设置页 `saveConfig` 必须回传全部字段，
> 漏一个就会把它重置成默认值。

## 7. 事件协议映射

codex `exec --json` 输出 JSONL，`handle_codex_event()` 把它翻译成智能助手**已有**的事件，
所以前端的工具卡片、流式正文、错误卡片逻辑都不用改。

| codex 事件 | 前端事件 | 说明 |
|---|---|---|
| `thread.started` | `codex-thread` `{thread_id}` | 新增事件。前端存到当前会话，续问时 resume 同一线程 |
| `item.updated` / `item.completed` + `agent_message` | `modle-streaming-respond-output` | **只发增量**：按 item id 记已发送字符数，切出后缀，避免重复拼接 |
| `item.started` + `command_execution` / `file_change` / `mcp_tool_call` / `web_search` / `todo_list` / `reasoning` | `agent-step` `{type:"tool_call", name, status:"running", args}` | `args` 只挑已知小字段（command/cwd/query/server/tool/name/path/changes/items），不塞命令输出 |
| 上述 item 的 `item.completed` | `agent-step` `{type:"tool_result", name, status, result}` | `exit_code != 0` 或 `status` 为 `failed`/`error` 时标红；`result` 优先取 `aggregated_output`（带 exit_code） |
| `item.completed` + `type = "error"` | `agent-step` `{type:"error"}` | item 级错误（如模型元数据缺失）也要让用户看见 |
| `turn.failed` / 顶层 `error` | `agent-step` `{type:"error", name:"agent_stream", message}` | 与内置引擎的错误卡片格式一致 |
| 顶层 `error` 且 message 以 `Reconnecting` 开头 | 仅 `warn!` 日志 | codex 每次重试都报一条（`Reconnecting... 1/5`），过滤掉避免刷满卡片 |

非 JSON 行（codex 偶发写到 stdout 的提示文本）只记 `info!` 日志。stderr 由单独的任务收集并做尾部截断
（超过 8KB 时保留最后 4KB），进程完全没有产出时把它带进错误信息，便于定位。

## 8. 多轮、取消与进程回收

**多轮**：请求带 `thread_id` 时用 `codex exec resume <thread_id> <prompt>`，由 codex 自己维护上下文。
因此 codex 分支**不需要**前端拼 `history`。`thread_id` 按会话持久化在
`rightSiderStore.assistant.codexThreadId`，随会话切换/新建/清空消息一起更新——
清空消息等于开一段新对话，线程必须丢弃，否则续问会带上用户已经看不见的上文。

**取消**：前端停止按钮只调 `cancel_chat_stream`，两个引擎共用 `chatview` 里的同一个标志位
（`stream_cancel_requested()` 是为此新暴露的 `pub(crate)` 入口）。读 stdout 的循环用
`tokio::select!` 配一个 200ms 的 ticker 轮询该标志，命中后 `terminate_process_tree` 杀进程树
再 `child.kill()`。取消视为正常结束（返回 `Ok`），**不会**触发回退到 Python 引擎。

**回收**：子进程 PID 同时存进模块级 `Mutex` 和 `process_shutdown` 的托管表，
`main.rs` 的关闭流程调用 `stop_codex_agent()`。`Command` 也设了 `kill_on_drop(true)` 兜底。

**提示词走 stdin**：参数里传 `-`，提示词写进 stdin 后立刻关闭。这样绕开 Windows 命令行长度上限
和引号转义问题。

## 9. 回退策略

`run_codex_agent` 的返回值语义是刻意区分的：

- **返回 `Err`** = 这一轮根本没跑起来（二进制缺失、供应商没配、进程起不来、进程完全无产出）。
  `agent_chat_stream` 打 `warn!` 后重置取消标志并落回 Python `agent_server`，前端无感。
- **返回 `Ok`** = 已经进入事件流（哪怕后来失败了）。错误已经通过 `agent-step` 的 error 事件
  展示在同一轮对话里，不再重跑一遍，避免用户看到两套互相矛盾的回答。

这条边界的目的是：**codex 出问题不能让 Agent 功能整体不可用**，但也不能在模型已经开始输出后又静默换引擎。

## 10. 分发与构建

`tools/fetch_codex_sidecar.mjs` 负责把 codex 弄进 `src-tauri/utils/codex/`：

- **固定版本**：`PINNED_TAG = 'rust-v0.149.1'`。`--json` 事件是 v2 schema，字段会随版本演进，
  升级必须走一次回归（见第 11 节）。
- **归档格式统一用 `.tar.gz`**（不用体积更小的 `.zst`）：系统自带的 tar 就能解，不需要额外依赖。
  归档内可执行文件名带 target triple，解压后重命名为 `codex.exe` / `codex`。
- **校验和**：已核对过的平台在 `ASSETS` 里带 `sha256`，不匹配就丢弃下载内容不解压。
  当前 win32-x64 的归档 sha256 为 `3f770d347380a1ccec08ac2b4d43f86dcf7bee1f924bf48c57241f66b6901959`。
  下载结果同时写入 `utils/codex/.codex-version.json`（该文件应提交，用于记录 pin 与跳过重复下载）。
- **系统代理优先**：GitHub release 在国内经常直连超时。优先级为
  `HTTPS_PROXY`/`ALL_PROXY`/`HTTP_PROXY` 环境变量 → Windows 注册表
  `HKCU\...\Internet Settings` 的 `ProxyEnable` + `ProxyServer`（支持 `http=host:port;https=...` 写法）。
  优先用 `curl` 下载（原生支持代理、重定向、`--speed-time` 低速看门狗），没有 curl 才退回 node fetch。
  经代理失败会再直连重试一次。
- **绝不阻断构建**：任何失败都只 `warn` 后返回；codex 缺失时 Agent 自动回退到内置引擎。
  超时上限 15 分钟、停滞 45 秒、握手 30 秒，且结尾显式 `process.exit(0)`
  （下载被中断时 undici 的 socket 句柄可能仍活着，不显式退出会把 `npm run tauri` 一起挂死）。

两个 Windows 平台坑，已在脚本里绕开，改动时不要退回去：

- **tar 的冒号**：PATH 里的 `tar` 可能是 GNU tar（Git for Windows 自带），它会把 `D:\...`
  当成 `host:path` 远程规格解析，报 `Cannot connect to D`。解决办法是把 cwd 切到目标目录、
  只传相对文件名，这样 GNU tar 和 Windows 自带的 bsdtar 都能工作。
- **进度统计不能挂在源流上**：给 `Readable.fromWeb()` 的流直接 `.on('data')` 会把它切到 flowing 模式、
  数据被监听器吃掉，pipeline 那头收不到完整内容。必须在管道里插一个 `PassThrough` 来计数。

分发路径走项目已有机制：`src-tauri/utils/` 在 release 构建时被 `prepare_utils_zip` 打成
外部 `tar.zst` 分包、发布在安装包旁边，由 NSIS 钩子解压到 `$INSTDIR\utils`。
所以 codex 虽然有 297MB，**安装包本体不会因此膨胀**。`.gitignore` 里的 `*.exe` 已经覆盖了这个二进制。

`npm run tauri`（含 dev/build/bundle）会前置执行 fetch 脚本，幂等且失败不影响后续；
也可以单独跑 `npm run codex:fetch` / `npm run codex:fetch:force`。

## 11. 已验证 / 未验证

已验证：

- `cargo check` 无警告；`vite build` 通过。
- `npm run codex:fetch` 实际下载 + 解压成功（`codex-cli 0.149.1`），重复运行走跳过分支。
- 用本设计生成的 `config.toml` 与完整参数实跑 `codex exec --json`，确认事件结构
  （`thread.started` / `item.completed` / `turn.started` / `error` / `turn.failed`）与解析代码一致。

未验证（需要真实供应商与额度）：

- Tauri 端到端跑通一轮 Agent 对话（invoke → 事件 → UI 卡片 → resume 续问 → 停止按钮）。
- cc-switch 代理在 `wire_api = chat` 下能否服务 Guaxi 的普通对话链路（见下节风险）。

## 12. 风险与遗留

- **cc-switch 代理的协议面**：它的 Codex 代理面向 codex，说的是 Responses API；
  是否同时提供 `/v1/chat/completions` 取决于其实现与上游。因此普通对话链路用
  `cc_switch_wire_api` 显式控制，设成 `responses` 时 `cc_switch_provider_config()`
  直接返回可读错误而不是发一个必然失败的请求。若实测只支持 responses，
  需要给普通对话链路补一个 `/responses` 适配分支。
- **cc-switch 内部结构会变**：本机版本已从 `config.json` 迁到 SQLite（`cc-switch.db`）。
  探测逻辑做成"失败即降级为手填"：复制数据库副本再只读查询（避免 WAL 写锁），
  任一环节失败返回 `found: false` + 提示文案。也刻意不读 `providers.settings_config`——
  里面是别家服务的密钥。
- **codex 版本升级**：改 `PINNED_TAG` 后需要重跑 `codex:fetch:force`、更新 `ASSETS` 里的 sha256，
  并回归第 7 节的事件映射（尤其 item 字段名与新增 item 类型）。
- **联网工具的 config key 未确认**：二进制里出现的是 `web_search_request`（没有 `tools.web_search`），
  当前按 `-c tools.web_search_request=true` 传。codex 对不认识的 `-c` 只是忽略，不影响启动；
  Guaxi 自己的 `search_web_duckduckgo` 仍然可用。
- **Python `agent_server` 必须保留**：它还承载 `/ocr` 等路由，不能因 codex 接入而删除。

