# 终端调试工坊 · 设计文档

> 适用版本：分支 `video`。本文与代码一一对应，命令（`#[command]`）、事件名、档案字段均可在 `src-tauri/src/commands/` 与 `src/components/ApplicationViewComponent/apps/` 下检索到。
> 配套使用文档见 [remote-terminal-workshop-usage.md](./remote-terminal-workshop-usage.md)。
>
> 实现状态标注：**[已实现]** 已在源码中并通过 `cargo check --bin guaxi` / `@vue/compiler-sfc` 校验；**[规划]** 设计已定、尚未落地；**[可选特性]** 代码存在但默认不编译（cargo feature 门控）。

## 1. 背景与目标

"终端调试工坊"（应用 id `tools-remote-terminal`，路由 `/app/tools-remote-terminal` → `RemoteTerminalApp.vue`）把远端运维/调试常用能力收进一个独立应用窗口：SSH/串口/Telnet 终端、海量日志分析、GDB/MI 交互调试、Coredump 事后分析、二进制/证书/加解密工具箱、网络报文与 TLS 探测。

设计约束（见仓库 memory）：
- 纯 Rust 栈优先、避开 openssl/NASM（Windows 构建机无 NASM）；加密走 RustCrypto + rcgen(ring 后端)。
- release 离线自足；"能自带就别依赖运行时外部工具"。
- 单次 `cargo check` ~2–4 分钟且易链接期 OOM，验证后端必须用 `--bin guaxi`（`--lib` 会跳过 commands/main.rs）。

## 2. 顶层结构

`RemoteTerminalApp.vue` 是一个 `n-tabs`，五个顶层页：日志分析 / 实时终端(`RemoteSshTerminal`) / GDB 调试(`RemoteGdbDebugger`) / 工具箱(`RemoteToolbox`) / 网络(`RemoteNetworkToolbox`)。

**[已实现] Tab 保活**：`display-directive="show:lazy"` 必须写在每个 `<n-tab-pane>` 上（它是 pane 的 prop，默认 `'if'` 会卸载非激活页，导致切 Tab 丢终端/GDB 会话/日志）。写在 `<n-tabs>` 上会被 naive-ui 静默忽略——这是早期"切 Tab 丢状态"bug 的根因。

**[已实现] 亮/暗主题**：代码/终端面统一用 `--tw-code-bg/--tw-code-fg`（定义在 `.rt-root`，暗色由 `:root[data-guaxi-theme='dark']` 覆盖），级联到 gdb 控制台、工具箱输出、coredump、网络 hex、源码面板。xterm 终端配色按 `settingsStore.appearanceTheme` 计算并 `watch` 热更新。

## 3. 实时终端 / SSH / 串口 / Telnet（`RemoteSshTerminal.vue` + `ssh_terminal.rs` / `serial_telnet.rs`）

**档案列表实时同步**：终端窗口的各个标签页是**常驻**的（`display-directive="show:lazy"`），组件只在挂载时读一次档案——在「实时终端」新建/删除连接后，GDB、工具箱、命令下发的档案下拉不重新挂载就会一直显示旧列表（"新增的会话在这里找不到，除非重开窗口"）。做法：`ssh_save_profile` / `ssh_delete_profile` 写盘后广播 `ssh-profiles-changed`（后端是唯一写档案的地方，覆盖面最全，且事件是全局广播、跨窗口也生效）；前端统一用 `composables/useSshProfiles.js` —— 挂载时加载一次 + 监听该事件重载，卸载时解绑（`listen` 是异步的，注册期间若已卸载会立刻 `off()`）。`RemoteSshTerminal` / `RemoteGdbDebugger` / `RemoteToolbox` / `CommandDispatchPanel` 四处共用它；`SftpBrowser` 通过 prop 拿列表，随父组件一起更新。

**面板选择持久化**（`utils/uiState.js`）：GDB 与命令下发的选择/布局记在 localStorage，下次开窗接着用——与仓库既有做法一致（`GuaxiDiffPanel` 记分栏比例、`ResumeBuilderApp` 记模板等 30+ 处），各窗口同源共享一份，无需新增后端命令。
- GDB：目标档案、启动/附加模式、提权方式、可执行文件路径、字号、左右侧栏宽度、源码来源/规则/源码根/编译机档案/构建根前缀、源码面板高度。
- 命令下发：目标档案、会话类型、退出码开关、图形/文本编辑模式、左栏宽度、上次用的策略。
- **不存任何凭据**（enable 口令在系统 keychain，策略库保存时也会清空 `global`）。
- 读取带默认值与校验（枚举用 `oneOf`、尺寸用 `within`），脏数据/禁用存储一律退回默认；**档案或策略被删后，列表到位时自动清掉失效的选中**，避免下拉挂着一个不存在的项。命令下发还会在开窗时自动载入上次用的策略（此时任务表本就是空的，不会覆盖）。

**[已实现] 连接档案（统一 `SshProfile`）**：`ssh_terminal.rs::SshProfile` 字段 `id,name,host,port,user,auth,key_path` + `kind("ssh"|"serial"|"telnet")`、`group`(文件夹)、`serial_path`、`serial_baud`（均 `#[serde(default)]`，旧档案兼容）。CRUD：`ssh_list_profiles / ssh_save_profile / ssh_delete_profile`；密码存系统 keychain：`ssh_set_secret / ssh_delete_secret / ssh_has_secret`。

**[已实现] 会话**：SSH 走 russh 自带栈 `ssh_create_session(cols,rows)`→`ssh-session-output`/`ssh-session-exit`，输入/尺寸 `ssh_write_session`/`ssh_resize_session`，关闭 `ssh_close_session`；串口 `serial_open(path,baud)`、Telnet `telnet_open(host,port)`。前端按 `profile.kind` 分派连接，串口/Telnet 也落档入"连接会话"。

**[已实现] 左栏文件夹 + 右栏录制**：连接列表按 `group` 分可折叠文件夹 + 垂直滚动（Xshell/MobaXterm 风格）；"会话录制"移到右侧栏（可折叠 + 拖拽改宽）；各分区可折叠/缩放。

**[已实现] SFTP（`sftp_fs.rs`）**：`sftp_connect/list/canonicalize/download/upload/mkdir/remove/rename/close`。SSH 连上后 `SftpBrowser` 按当前激活会话的 `profile_id` **自动连同主机 SFTP**。上传修复：`sftp_upload` 用 `open_with_flags(CREATE|TRUNCATE|WRITE)` + `write_all` + `shutdown()`（原先 `session.write()` 只带 `WRITE`，目标不存在即 `No such file`）。

**[已实现] 录制回放**：会话输出原样落盘（`ssh_list_recordings`），点开喂进"日志分析"管道。

## 4. GDB/MI 交互调试（`RemoteGdbDebugger.vue` + `gdb_mi.rs`）

通过复用 SSH 连接开 exec 通道跑 `gdb -q -nx -i=mi`，自写 MI 解析器把结果/异步/流记录转 JSON，事件 `gdb-mi-output`（流）/`gdb-mi-async`（`*stopped` 等）/`gdb-mi-exit`。命令：`gdb_mi_start / gdb_mi_command / gdb_mi_raw / gdb_mi_close`，宏 `gdb_save_macros / gdb_load_macros`。

**[已实现] 三种启动模式**：
- 启动程序：`gdb … --args <program> <args…>`。
- 附加进程：`gdb_mi_start(..., attach_pid, use_sudo)`。命令行**不带 `-p`**（那样 attach 发生在前端订阅前、`*stopped` 丢失、失败只打印控制台）；改由前端在会话建立后发 MI 命令 `-target-attach <PID>`，拿结构化结果与停住事件。
- 仅连 gdb（coredump/手动）。

**[已实现] attach 健壮性**（修"启动闪退/无法加断点"）：
- 提权方式 `none/sudo/su`：`sudo` 用 `sudo -n`（免密），`su` 用 `su -c '<cmd>'` 并在 PTY 上喂 root 密码。两者都需要 PTY（`requiretty` / su 读 `/dev/tty`），故申请 PTY。
- **PTY 回显必须关掉**：PTY 默认把输入回显一遍，我们写进去的每条 MI 命令都会被 tty 回显混进 gdb 的 MI 输出流——既污染解析，又可能与 gdb 输出**交错截断**记录（`*stopped` 被吃掉 → 表现为"卡在断点/命令超时"）。做法是命令链前置 `stty -echo 2>/dev/null; exec`，并在解析器里用 `is_echo_line()` 兜底过滤（MI 记录的 token 后必为 `^ * + = ~ @ &`，否则就是回显）。
- MI 解析器把**非 MI 文本（sudo/gdb 的 stderr：ptrace/tty/password 等）转发到控制台**，不再静默吞掉。
- gdb 退出后保留并在空状态区展示控制台输出；前端区分"会话已退出/ptrace 受限/sudo 需免密/PID 不存在"并给出对应提示（如 `echo 0 > /proc/sys/kernel/yama/ptrace_scope`、配置 NOPASSWD、同用户）。

**运行中不下发栈/变量查询**：目标运行中时 gdb 会把 `-stack-*` / `-var-*` / `-data-evaluate-expression` 之类的命令**排队到停下才回复**，前端只会干等超时——这是"命令超时/取不到结果"的主因。前端维护 `isRunning`（由 `*running` / `*stopped` 驱动，发出 resume 命令时乐观置位），`blockedWhileRunning()` 在这些查询入口直接拦截并提示"运行中，需停下后才可查"；`refreshAll` 与停住后的 `refreshOnStop` 都改为**逐个 await**（不再并发，避免多条命令在 gdb 侧排队互相拖累）。

### 交互架构：提交 / 等待分离（对慢对端健壮）

原设计是"发一条命令 → 后端固定 30s 超时 → 抛错给用户"，对**慢对端**（加载大符号表、链路卡顿、su 认证慢）不成立：不是对端坏了，只是慢；而超时后应答也被丢掉。现在拆成两步，等待时长完全由前端掌握：

| 命令 | 作用 |
|---|---|
| `gdb_mi_submit(session_id, command) -> token` | 登记并下发，**立即返回 token**，不等应答 |
| `gdb_mi_await(session_id, token, wait_ms) -> Option<Value>` | 取应答：`Some` 拿到；`None` 还在路上（可继续等）；`Err` 会话已结束 |
| `gdb_mi_state(session_id) -> {alive, ready, pending_results}` | 会话状态快照 |
| `gdb_mi_command(...)` | 一次性包装（submit + 等满），保留给单次调用场景 |

要点：
- **应答不再丢**：会话任务把每条带 token 的 Result 存进共享 `done: HashMap<token, Value>` 并 `notify_waiters()`；前端不管等多久、中途刷新几次，都能取到。旧实现用 oneshot，超时即丢，且**迟到的 `^done` 会被误当成异步通知推给前端**（会把界面搞乱）——现在带 token 的结果一律只进 `done`，绝不当异步事件。
- **会话结束即时感知**：`alive` 置 false + 唤醒等待者，在途 `await` 立刻返回"会话已结束"，而不是干等满自己的上限。
- **就绪超时**：su 认证 + gdb 启动若在 `READY_TIMEOUT_SECS`(45s) 内没完成，后端把**排队中的命令全部以明确错误回掉**并推 `notready` 事件（提示 su 密码错/远端无 gdb/登录脚本卡住），前端不会无限等。
- **前端等待可见**：等待中的命令进入 `inflight`，顶栏显示"正在建立（su 认证 / gdb 启动）N s"或"等待对端响应 N s"，超过 6s 追加"较慢：对端可能在加载符号或链路卡顿，可继续等或点中断"。等待期间每 ~4s 用 `gdb_mi_state` 同步一次状态（既区分两种等待，也兜住 `ready` 事件漏收）。
- **等待有上限但不报错**：单条命令上限 10 分钟，到点只"停止等待"并说明，不再抛 error。
- **区分"提示"与"错误"**：会话结束、等待超时这类可自愈的情况走 `notice`（琥珀色、自动消失），只有真正的失败才进 `error`（红色）。
- `done` 设上限 `DONE_CAP`(512) 防无界增长；组件卸载/结束会话时清理计时器与在途列表。

**[已实现] 三栏布局 + 固定标题的分块面板**：中间列固定为「源码对照 + 控制台 + 命令输入」；左右两侧各有一条**可拖宽**的侧栏，面板分块停放——
- 左栏：断点、调用栈（可点选帧 `-stack-select-frame`）、局部变量（`-stack-list-variables`）、变量树（`-var-create`/`-var-list-children`，支持 natural/hex/dec/bin 切换 `-var-set-format`）；
- 右栏：线程、Watch、寄存器（`-data-list-register-names` + `-data-list-register-values x`）、内存（`-data-read-memory-bytes` 格式化为 hex+ASCII）、宏。

每个面板 = **固定标题条 + 独立滚动的内容区 + 底部拖拽手柄**：标题条（`gdb-panel-h`）不随内容滚动，也**可点击折叠**；内容区（`gdb-scroll`）自己滚动；手柄（`gdb-panel-rz`）上下拖拽改块高（`panelH`）。标题条右侧是**图标按钮**（`gdb-ib`，全部 `n-tooltip` 带中文说明），例如刷新 / 导出到主界面编辑器 / 删除 / 选目录 / 对照当前行 / 读内存。顶部命令板同样改为图标按钮（运行/继续/下一步/步入/步出/中断 + 全量刷新 + 结束），鼠标悬停给出快捷键与语义说明。

居中列顶部**源码对照面板**：停住时按当前帧 file:line 定位源码、高亮并滚动；可一键发送到主界面编辑器。字体与终端一致（`--gdb-mono`）。

**源码定位（`source_locate.rs`）——用户只给一个"源码工程根"，路径映射由后端按规则解**：

现实里调试信息里的路径几乎从不等于源码在你机器上的位置：原地/out-of-tree 构建还算对得上，但 **chroot / docker 构建**里工程被挂在容器内的 `/build`（本仓库正是这种：调试信息写 `/build/usr/click/lib/libssl_cli/ssl_cli.c`，源码在工程根的 `usr/click/lib/libssl_cli/ssl_cli.c`）；**源码还可能在另一台 Linux 编译机上**。所以提供规则，默认 `auto` 依次试：

| 规则 | 做法 |
|---|---|
| 原样 | `debug_path` 直接用 |
| **后缀匹配**（auto 里最有用） | 从左侧逐段砍掉调试路径的前缀，看剩下的是否存在于源码根下 —— 不需要知道构建根叫 `/build` 还是 `/src` |
| 前缀替换 | 把常见构建根（`/build` `/builddir` `/src` `/source` `/work` `/workspace` `/code` `/app` `/root` `/opt`）换成源码根 |
| 按文件名搜索 | 在源码根下按 basename 递归找（限深、排除 `node_modules/.git/target/out/.cache`；多个匹配时**不猜**，直接告诉用户缩小源码根） |

- `cmd_source_locate`：只做路径解析与**存在性探测**，不下发任何命令；返回命中路径、用了哪条规则、**尝试过的候选**与下一步建议（找不到时把候选列出来便于诊断）。
- `cmd_source_read`：读正文——本机 `std::fs`，远端走 `ssh_read_text_file`。
- **远端（编译机）源码**：面板可切「编译机源码」，选 SSH 档案后用 `ssh_list_dirs`（`ls -1p` 只列目录）浏览远端目录选源码根，之后所有定位/读取都经 SSH 在该机器上完成。探测用一次 exec 把所有候选批量 `test -f`，省往返。
- 规则为「前缀替换」时，顺手对 gdb 下 `set substitute-path`，让它内部的 `list`/断点定位也一致。

## 5. 工具箱（`RemoteToolbox.vue`）

顶层子页：计算器 / 证书·密钥 / 二进制分析 / Coredump / 远程执行 / 多机对齐。

**[已实现] 计算器 IP 转换（IPv4 + IPv6）**：支持 IPv4 点分/十进制/0x 十六进制/CIDR（给出网段、掩码、网络与广播地址、可用范围、主机数，并附 `IPv4-mapped IPv6` 形式）；以及 **IPv6** 压缩写法 / 完整展开 / 128 位十六进制与十进制（BigInt）/ 二进制分组 / 内嵌 IPv4（`::ffff:a.b.c.d`）与地址类型归类（环回、链路本地 fe80::/10、唯一本地 fc00::/7、组播、全球单播、6to4、Teredo、文档示例 2001:db8::/32 等）；IPv6 CIDR 给出前缀掩码、网络地址、末地址、可用范围、地址总数与可用主机数。压缩遵循 RFC 5952（最长零段用 `::`，单段零不压缩）。

**[已实现] Coredump（`remote_terminal_tools.rs::gdb_coredump_analyze`）**：远端 `gdb -batch` 抽全线程栈+寄存器，输出区**占满高度 + 字号可调**。
**[已实现] 远程执行/定时采集**：`ssh_exec_capture`、`ssh_start_periodic`/`ssh_stop_periodic`，输出落盘可回喂分析。
**[已实现] 二进制分析**：`binary_analyze`（goblin，段熵）。

### 5.1 证书 / 密钥 / 加解密工作台（`crypto_lab.rs`）

纯 Rust：rcgen(ring)+RustCrypto(rsa/ecdsa/p256/p384/ed25519-dalek/pkcs8/pkcs1/sec1/spki)+x509-parser+p12。子页：

UI 约定（全页统一）：
- **顶部子标签栏固定**，只有内容区滚动（`.cx-pane` 不滚、`.cx-tabs` 的 pane 自己滚），不随垂直滚动消失。
- **按钮一律用图标 + `n-tooltip` 中文说明**（`.tb-ib`，含 primary/danger/lg 变体），需要解释的字段旁挂 `?` 帮助图标（`.tb-help`）。
- **填写项都给默认值与说明**：主题字段默认 `C=CN / ST=Shannxi / L=xi'an / O=XA / OU=Dev`，有效期默认 3650 天；并支持把 openssl 的 `-subj` 串（`/C=CN/ST=…/CN=…` 或逗号形式）**粘贴后一键解析填入**。
- **算法/参数全量列出**：RSA 1024/2048/3072/4096/8192、ECDSA P-256/P-384、Ed25519（P-521 需 rcgen 的 aws_lc 后端，本机无 NASM 故不提供）；签名方案含 RSA PKCS#1 与 PSS 的 SHA-256/384/512、ECDSA、Ed25519；散列 MD5/SHA-1/256/384/512。

- **[已实现] 解析/导入**：`crypto_load_bytes`（识别 PEM/DER/PFX/key/csr/cert）；`crypto_inspect`（x509 摘要 + ASN.1 dump）；PFX 拆包 `crypto_pfx_parse(password)`。**`crypto_pem_details(input)`**：解析证书/私钥/公钥/CSR/CRL，返回可读字段（主题/颁发者/序列号/有效期/签名与公钥算法/是否 CA/是否自签）、SAN 列表、SHA-256 指纹、归一化 PEM、DER(base64)，驱动"查看"弹窗。
- **[已实现] 证书链/签发（CA 库驱动）**：后台 CA 库（`crypto_lab/ca_store.json`）记录名称/类别/主题/算法/cert/key/是否根/父 id/序列号计数/CRL 号/吊销表。命令：`crypto_ca_list / crypto_ca_create(name,category,subject,rsa_bits,curve,days,issuer_ca_id) / crypto_ca_import / crypto_ca_delete / crypto_ca_export`；用库内 CA 直签终端证书 `crypto_issue_from_ca(ca_id, subject, profile=server|client, sans, days)`；吊销 `crypto_ca_revoke`、CRL `crypto_ca_gen_crl`；终端证书库 `crypto_leaf_list/delete/export`（`export` 顺 `parent_id` 拼完整**证书链**）。
- **[已实现] 查看/管理**：CA 行、终端证书行、资产行都带**查看图标**——弹窗分「字段明细 / PEM 原文 / DER(base64) / ASN.1」四页，均可一键复制；证书与其配套私钥在同一行并列可见（CA 与叶证书都是证书+私钥成对入库）。
- **[已实现] PFX**：`crypto_pfx_build(key_pem, certs_pem[], password)` 打包；`crypto_pfx_parse(data_b64, password)` 拆包。**PFX 页同时提供打包与解包**：解包可直接选外部 .pfx/.p12、填密码，列出包内全部私钥与证书链，逐个查看/复制/保存（"解析/导入"页也支持选 PFX 文件自动拆包）。
- **[已实现] 加解密（单页，分 对称/非对称/散列）**：
  - 对称 `crypto_sym(algo, mode, key_hex, iv_hex, data_b64, encrypt, aad_hex, padding)`——**分组密码** AES-128/192/256、SM4、Camellia-128/192/256、Twofish、Serpent（128 位分组）与 DES、3DES/TDEA-3key、Blowfish、CAST5、RC2、IDEA（64 位分组），模式支持 **ECB / CBC / CFB / OFB / CTR**（ECB 无需 IV；ECB/CBC 可 PKCS7 填充，否则要求块对齐）；**流密码** ChaCha20、Salsa20；**AEAD** AES-128/256-GCM、ChaCha20-Poly1305（可填 AAD，密文带 16 字节认证标签）。key/IV 可一键安全随机生成，长度随所选算法提示。
  - 非对称 `crypto_rsa_crypt`：PKCS#1 v1.5 与 **OAEP + SHA-1/256/384/512**。
  - 散列 `crypto_hash`：MD5/SHA-1/256/384/512。
  - 签名 `crypto_sign/verify`：RSA PKCS#1 与 PSS 的 SHA-256/384/512、ECDSA P-256/P-384、Ed25519。
- **[已实现] 受管资产库**：`crypto_asset_list/save/get/delete`（`asset_store.json`）统一管理私钥/CSR/CRL，可导入/查看/导出/删除。
- 文件写盘统一走后端 `fileprocess::write_file_content / write_file_bytes_base64`（不受前端 `allow-write-file` 作用域限制——修"导出 forbidden path"）。

### 5.2 [规划] 受管 PKI 资产库（key/CSR/CRL + 四类 CA + 终端证书库）

目标：把密钥、CSR、CRL、证书都做成**持久化受管库**，逐项查看/导入/导出，并体系化四类 CA。

- **四类 CA 库**：CA 条目增 `category: rsa|ecc|sm2|pqc`、`serial_counter`、`crl_number`、`revoked:[{serial,reason,at}]`。RSA/ECC 本地 rcgen 全功能（建/切换当前根/重生成/中间链/签发/吊销/CRL）；SM2/PQC 的签发转 §6 引擎。
- **吊销 + CRL**：`crypto_ca_revoke(ca_id,serial,reason)`、`crypto_ca_gen_crl(ca_id)`（rcgen `CertificateRevocationListParams.signed_by().pem()`）。签发时 CA `serial_counter` 递增作为证书序列号，供吊销定位。
- **终端证书库**（`leaf_store.json`）：`crypto_leaf_list/issue/delete/export`，条目记 `kind,ca_id,subject_cn,serial,cert_pem,key_pem`，可顺 `parent_id` 回溯拼完整**证书链**并标注"由哪个 CA 签发"。
- **key/CSR/CRL 库**：各自 JSON store + 查看/导入/导出；证书页统一按资产类型分区管理。

### 5.3 [可选特性] SM2 / TLCP / PQC —— Tongsuo 集成（cargo feature `tlcp`，默认关闭）

无成熟纯 Rust 的 TLCP 栈，故按用户方案接 **Tongsuo 8.5+**（C，含 SM2/SM3/SM4 + NTLS/TLCP + ML-KEM/ML-DSA）。为不破坏默认（纯 Rust、可编译、无 openssl-sys）构建，整套集成放在 **cargo feature `tlcp`** 后：

- 依赖（`optional=true`，仅 `tlcp` 启用）：`openssl = "0.10"`、`openssl-sys = "0.9"`（**不开 vendored**，经 `OPENSSL_DIR` 指向 Tongsuo，`OPENSSL_STATIC=0` 动态链接）。
- `src-tauri/src/commands/crypto_engine/ntls.rs`（`#[cfg(feature="tlcp")]`）：`extern "C"` 垫片——`NTLS_client_method/NTLS_server_method/SSL_CTX_enable_ntls/SSL_CTX_use_sign_certificate_file/..._enc_certificate_file/..._sign_PrivateKey_file/..._enc_PrivateKey_file`（签名须与所链 Tongsuo 头文件对齐，存在跨版本改名）。
- SM2 证书/链（零额外 FFI）：`EcKey::generate(Nid::SM2)` + `X509::builder().sign(&key, MessageDigest::sm3())`，接入 §5.2 的 `category=sm2` 签发路径。
- TLCP 握手：`SslContextBuilder` → `enable_ntls` → 加载双证书（签名证书在前、加密证书在后；两套密钥独立）→ `set_cipher_list("ECC-SM2-SM4-CBC-SM3"...)`。网络页加"TLCP(国密)"探测模式。
- PQC 混合交换（零额外 FFI）：`set_groups_list("SM2MLKEM768:X25519MLKEM768:P256MLKEM768:SM2:X25519")`，混合+降级回退。
- 异步：fork `tokio-openssl`（<200 行）把 `openssl::` 指到本 crate。
- 风险（见用户方案）：`openssl-sys` 符号与现有 rustls/reqwest 冲突（`cargo tree -i openssl-sys` 保证唯一）；Tongsuo 跨小版本 NTLS 函数漂移；双证书链拼接顺序；Windows 编译需 Strawberry Perl+VS Build Tools+NASM。**合规定位：SM2 为主、ML-KEM 为叠加增强。**
- 验证边界：该路径的链接/握手需在装好 Tongsuo 的机器上 `cargo build --features tlcp` + `OPENSSL_DIR=…` 真机验证；默认构建不含它。

### 5.4 命令下发（`CommandDispatchPanel.vue` + `cmd_dispatch.rs` / `cmd_catalog.rs`）

把「按配置在设备上批量下发 CLI」做进工坊：配置驱动、支持多条命令聚合成高级流程、交互式设备 CLI 全自动走通。

**为什么不是"exec 一条命令"**：这类 CLI 是**交互式**的。所以核心是一个**提示符状态机 + 常驻 shell 会话**（`DeviceCli`），并且要区分两种目标：

- **设备 CLI**（手册 Chapter1 的四种提示符）：
  | 级别 | 提示符 | 说明 |
  |---|---|---|
  | User | `Demo>` | 只能跑 ping/traceroute 之类 |
  | Enable | `Demo#` | `enable` + 口令进入（口令可空）。真实设备会把模式也显示出来，如 `[2Zsd4cWhQH:Standalone Mode]#` |
  | Config | `Demo(config)#` | `config terminal`（老脚本用 `config terminal force`，两个都试） |
  | 调试（Linux shell） | `[test@Demo ~]$` | 带 `@` 且以 `$` 结尾 |

  导入证书/私钥时设备进入"粘贴 PEM"状态（提示 `use "..." on a single line`，以单独一行 `...` 结束）；还有 yes/no 确认与 `--more--` 分页。
- **Linux shell**（编译机调试级别）：**没有** enable/config 这一套，命令就是 shell 命令。若任务带 PEM 且命令里有 heredoc（`<<EOF`），就把 PEM 当作 heredoc 内容喂进去。

会话类型可选 `自动探测 / 设备 CLI / Linux shell`。**shell 的判据只看 `@` 与 `$`**：`$` 结尾算 shell；`#` 结尾只有带 `@`（`[root@host ~]#`）才算 shell —— 曾经用"`[...]` 开头的 `#` 也算 shell"去兼容 `[root@host ~]#`，结果把设备自己的 `[主机:模式]#` 误判成 shell、跳过 `config terminal` 直接报"无法进入配置模式"，现在有单测守着这条。

- **自动应答**：设备模式下 `type yes to continue` / `continue:` / `yes/no` / `are you sure` → 发 `YES`；两种模式都处理 `--more--`／`:` 分页。**shell 模式不代答 yes/no**（`rm -i`、`apt` 之类的确认不该被代替），但 sudo 口令提示会喂一次 enable 口令。
- **结果判定**：设备 CLI 用硬失败标志（`invalid input`/`syntax error`/`access denied`/`failed`/`unknown command`…）+ 幂等成功标志（`already exists`/`not configured`…）+ **"交互未完成"守卫**；把"脱字符指示"收窄为**整行只有 `^` `~`** 才算失败，避免命令里出现 `^`（正则/路径）被误判。
  **shell 模式改用真实退出码**：给命令追加 `; echo __DISPATCH_RC=$?`，以退出码为准（`cp` 失败不会打印任何关键词，关键词启发式对 shell 不可靠）。展示输出里会把探针行剥掉。可用「退出码」开关关掉（命令是交互式时）。

- **提示符状态机**：按最后一行非空内容判定 `>`（用户）/ `#`（特权）/ `(config*)#`（配置）/ 密码提示 / PEM 等待。`ensure_config_mode()` 先空行探测，必要时 `config terminal force`，再必要时 `enable`(+enable 口令) 后重试。
- **自动应答**：`type yes to continue` / `continue:` / `yes/no` / `are you sure` → 发 `YES`；`--more--` → 发空格；PEM 等待 → 发 PEM 正文 + 单独一行 `...`。
- **结果判定**：硬失败标志（`invalid input`/`syntax error`/`access denied`/`failed`/`unknown command`…）+ 幂等成功标志（`already exists`/`not configured`…）+ **"交互未完成"守卫**（命令报成功但仍在等输入时判失败）。相比参考工具，把"脱字符指示"收窄为**整行只有 `^` `~`** 才算失败，避免命令里出现 `^`（正则/路径）被误判。**判定依据（命中了哪个关键词）与设备原始输出一起返回**，而不是只给一个 bool。
- **配置**：`serde_yaml` 解析，**完全兼容**参考工具 config.yaml（`global`/`targets{hostnames,domains}`/`tasks[{id,cmd,pem_file,index,depends_on,only_if_domain,only_if_no_domain}]`）；`index: 1` 与 `"1"` 都接受。`{hostname}/{domain}/{index}` 占位符手写替换（不用 `format!`，命令里可能含 `{`）。
- **展开**：照搬参考规则 —— `only_if_domain` 只取非空域名、`only_if_no_domain` 强制 `domain=""`、含 `{domain}` 遍历全部域名、否则只跑 host 级。
- **DAG**：Kahn 排序 + **作用域化依赖解析**，并在原工具基础上**优先匹配同 index**（解决同名任务被 index 1/2/3 拆成多条时 `exact[0]` 可能选错的问题）；歧义写入 `warnings`，缺失依赖报"哪个任务的哪个依赖找不到"，成环**列出环上节点**。
- **执行**：后台逐条执行，推事件 `cmd-dispatch-task`（status: running/ok/ok-idem/fail/skipped + reason + output）/ `cmd-dispatch-done`；`cmd_dispatch_cancel` 可中断；日志 JSONL 落盘 `app_data/cmd_dispatch/runs/<run_id>.jsonl`（字段对齐参考工具）。**失败即停**（`continue_on_error` 可关）。
- **先看清再执行**：`cmd_dispatch_plan` 只展开+排序不下发，UI 用它做"预览将要逐条执行的命令"与配置校验。
- **YAML ⇄ 任务表双向同步**：YAML 只由后端解析/生成（`cmd_dispatch_parse` / `cmd_dispatch_to_yaml`），前端不引 YAML 库，任务表的改动回写成 YAML 再送去预览/执行。
- **凭证**：优先复用库内 SSH 档案；enable 口令另存 keychain（`guaxi-ssh-enable`）。配置里带 `global` 段时也可直接用它直连（兼容原工具用法）。
- **编辑方式**：「图形」（任务表：命令列给足宽度、多行输入，长命令也能看全）与「文本」（**Monaco 编辑器**编辑整块 YAML，批量改效率高）两种模式切换，YAML 与任务表双向同步。
  - Monaco **懒加载**：只有切到文本模式才 `import('monaco-editor')` + YAML 语法贡献（`basic-languages/yaml/yaml.contribution`，Monarch 分词，无需额外 worker），不打开这个模式就不付这份体积。
  - **生命周期**：加载是异步的，用 `disposed` 闸门保证"组件已卸载 / 已切回图形模式"时不再去 `create`；离开文本模式立刻 `dispose()` editor 与 model（顺序：先 editor 后 model）；`onDidChangeModelContent` 的 disposable 一并释放；外部改写 `yamlText` 时经 `setValue` 同步进 model（`pushingToModel` 标志防成环）。
  - **主题**：`getGuaxiMonacoTheme(appearanceTheme)` → `guaxi-light`/`guaxi-dark`，并 `watch` 应用外观变化调 `editor.setTheme()`（与仓库其它 monaco 用法一致）。
- **用外部编辑器打开 / 在文件管理器中显示**（`cmd_open_in_os` / `cmd_reveal_in_os`，**走后端**）：配置 YAML、每个任务的 PEM 文件、本次运行的 JSONL 日志都能一键"用系统默认程序打开"或"在文件管理器中定位"。
  - 配置没有对应文件时先落到受管固定路径 `cmd_dispatch/config.yaml`（重复打开复用同一文件，外部编辑器里 reload 即可）；导入过 config.yaml 则直接开用户那份。
  - 相对 PEM 路径按「导入 config.yaml 的目录」解析（与执行时一致），解析不出绝对路径时明确提示而不是静默失败。
  - **为什么不用前端 `plugin-opener`**：本仓库能力文件把 `opener:allow-open-path` 限定在 `$APPDATA/resume-builder/**`，前端 openPath 会被权限拦下；后端走系统 `start`/`open`/`xdg-open` 不受该作用域限制（复用 resource_hunter 里已有的 `open_in_os`/`reveal_in_os`，已提升为 `pub(crate)`）。
- **策略库**（`cmd_strategy_*`，落 `app_data/cmd_dispatch/strategies.json`）：把一组 targets+tasks 存成命名策略，下次一键载入复用。**凭据不入库**（保存时清空 `global`），所以策略可安全分享。
- **文件选择**（`cmd_dispatch_stage_pem`）：任务的 `pem_file` 可点文件图标从**证书页的 CA 库 / 终端证书库**直接取（证书/私钥/整链），或选任意本地文件；取出的内容落到受管目录 `app_data/cmd_dispatch/pem/` 并回填绝对路径（内容相同复用同一文件）。

**命令目录（`cmd_catalog_list`）**：把手册里的命令做成结构化条目 `{group,name,syntax,summary,cmd_template,pem,params[{name,required,doc,example}]}`，覆盖 **SSL**（Ch13/14：`ssl host`、`ssl sni`、`ssl import rootca|interca|crlca|key|certificate`、`ssl activate certificate`、`ssl backup|restore`、`ssl csr`、`ssl sm2 import signkey|signcertificate|enckey|enccertificate` 及 SNI 变体）与 **SLB**（Ch8：`slb real <type>`、`slb group method|member`、`slb virtual https|http`、`slb policy static|hashurl`、`show slb real|virtual`）。UI 据此渲染"命令浏览器 + 参数表单 + 插入任务"。

**场景模板（`cmd_scenario_list` / `cmd_scenario_render`）**：一键生成整套任务（结构由 serde_yaml 生成，保证 YAML 合法）——
- `ssl_chain`：clear → 建虚拟主机 →（SNI）→ 导入 rootca/interca/crlca → 导入服务端私钥与证书 → 激活（等价于参考 config.yaml 的产物）。
- `slb_https_ssl`：定义后台服务 → 服务组 + 加成员 → 定义 HTTPS 虚拟服务 → 静态策略关联虚拟服务与后台服务 → 建 SSL 虚拟主机并绑定该虚拟服务 → SNI → 导入证书链与私钥 → 激活。

## 6. 网络（`RemoteNetworkToolbox.vue` + `net_conn.rs` / `net_toolbox.rs` / `net_capture.rs`）

**[已实现]** 报文构造/解析（etherparse）、TCP/TLS 探测（`net_tls_inspect`，rustls+ring 报协议/套件/证书链）、抓包（运行时加载 Npcap）。**[规划/可选]** TLCP(国密)探测随 §5.3 `tlcp` 特性落地。

## 7. 关键命令/事件索引（后端）
- gdb_mi：`gdb_mi_start(profileId,program,args,attachPid,useSudo)` `gdb_mi_command` `gdb_mi_raw` `gdb_mi_close` `gdb_save_macros` `gdb_load_macros`；事件 `gdb-mi-output/async/exit`。
- ssh_terminal/serial_telnet/sftp_fs：见 §3。
- remote_terminal_tools：`ssh_exec_capture` `ssh_read_text_file` `gdb_coredump_analyze` `ssh_start_periodic/stop` `binary_analyze` `crypto_inspect`。
- crypto_lab：`crypto_load_bytes` `crypto_pem_details` `crypto_gen_key` `crypto_gen_csr` `crypto_issue_cert` `crypto_sign_csr` `crypto_verify_cert` `crypto_pfx_parse/build` `crypto_sym` `crypto_rsa_crypt` `crypto_sign/verify` `crypto_hash` `crypto_ca_list/create/import/delete/export/revoke/gen_crl` `crypto_issue_from_ca` `crypto_leaf_list/delete/export` `crypto_asset_list/save/get/delete`；[可选特性 tlcp] `crypto_engine::{ca_create,issue_leaf,gen_crl}`、`crypto_engine::ntls`。
- cmd_dispatch / cmd_catalog：`cmd_dispatch_plan`（只展开+排序，不下发）`cmd_dispatch_parse` / `cmd_dispatch_to_yaml`（YAML ⇄ 结构，供任务表双向同步）`cmd_dispatch_run` `cmd_dispatch_cancel` `cmd_dispatch_stage_pem`（PEM 落受管目录）`cmd_dispatch_stage_config`（YAML 落受管固定路径）`cmd_open_in_os` / `cmd_reveal_in_os`（用系统程序打开 / 在文件管理器中定位）`cmd_strategy_list/save/load/delete`（策略库）`cmd_catalog_list` `cmd_scenario_list` `cmd_scenario_render`；事件 `cmd-dispatch-task` / `cmd-dispatch-done`。ssh_terminal 另加 `ssh_set_enable_secret` / `ssh_delete_enable_secret` / `ssh_has_enable_secret`（enable 口令进 keychain）。
- source_locate：`cmd_source_locate`（按规则把调试路径映射到本机/远端源码，返回命中路径 + 规则 + 尝试过的候选 + 建议）`cmd_source_read`（本机 fs / 远端 SSH 读正文）`ssh_list_dirs`（列远端子目录，选编译机源码根用）。


