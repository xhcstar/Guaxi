# 认知层（cognition）· 设计文档

> 适用版本：分支 `video`。本文与代码一一对应，所有命令、表、字段、事件名均可在源码中检索到。

## 1. 背景与目标

改造前的实测结论（不是推测）：

| 事实 | 依据 | 后果 |
|---|---|---|
| 应用注册了 **529** 个 Tauri 命令，Agent 只能碰到 **8** 个 | `main.rs::generate_handler!` vs `agent_server/tools/{filesystem,web,code_exec}.py` | 大脑接不到身体上：Agent 不能查图谱、不能语义检索、不能建索引、不能建笔记 |
| 对话链路**完全没有检索** | `chatview.rs::get_chat_answer_stream` 只是 provider 分发器，输入即 question + history | 整库向量已经算好，却在最该用的地方没用 |
| 没有跨会话记忆 | 会话落文件，codex `thread_id` 只在单线程内连续 | 没有沉淀"我了解这个用户什么""我上次得出过什么结论"的位置 |
| "主动"只有拉取没有投递 | `flipboard::start_feed_scheduler_loop` 只把文章存进缓存；无 tray、无 notification 插件 | 判断"这件事现在值得打断你"的环节根本不存在 |
| 轨迹即用即丢 | `agent-step` 事件只用于画 UI | 没有任何可学习的标签，"自我进化"无从谈起 |

认知层解决这五件事。它**不是又一个功能纵向**，而是横切在已有能力之上的一层：

```
感知 percept ──► 记忆 episode / belief / profile ──► 召回 recall ──► 主动 nudge / brief
                          ▲                                            │
                          └──────────── 进化 evolve（评测闩） ◄─────────┘
```

## 2. 三条贯穿全层的硬约束

1. **不可信内容永不越权。** percept 派生文本只能当资料读；由它驱动的调用无论授权如何都只放行 Read 级工具。
2. **本地优先。** 感知打分、新颖度、亲和度全部走本地 embedding；远端模型只用于每天真正触达用户的少数几件事，且受每日 token 预算硬约束。
3. **可解释可回滚。** 所有自调产物都是带版本的行，附评测分数与一键回滚。

## 3. 代码地图

| 层 | 文件 | 职责 |
|---|---|---|
| 模块根 | `src-tauri/src/cognition.rs` | 模块清单与对外再导出（`hook_*` / `start_background_loops` / `shutdown`） |
| 持久层 | `cognition/store.rs` | `repos/cognition.db` 的 schema、迁移、token 账本、工具审计、向量编解码 |
| 感知 | `cognition/percept.rs` | 统一入库、指令中和、近似去重、新颖度/显著性打分、percept 向量检索 |
| 记忆·情节 | `cognition/episode.rs` | append-only 轨迹、引证落库与命中回填、情节召回、裁剪 |
| 记忆·画像 | `cognition/profile.rs` | `profile.md` 解析/写回、兴趣衰减与强化、亲和度、免打扰时段 |
| 记忆·信念 | `cognition/belief.rs` | 章节 → 可反驳主张的抽取；感知 ↔ 信念的冲突判决 |
| 召回 | `cognition/recall.rs` | 四路证据 RRF 融合、引证装配、接地入口、评测用通路快照 |
| 主动 | `cognition/nudge.rs` | 六类提醒产出、治理器（预算/免打扰/自适应阈值）、策略训练与回滚、SRS |
| 简报 | `cognition/brief.rs` | 确定性 markdown 简报、落盘与落库、到点判定 |
| 进化 | `cognition/evolve.rs` | 评测集挖掘、评测运行、坐标上升调参 + 门禁、回滚 |
| 工具面 | `cognition/tools.rs` | 32 个分级工具的注册与分发、等级强制、路径闸门 |
| MCP | `cognition/mcp.rs` | JSON-RPC 2.0 服务端，挂在既有 `127.0.0.1:8888/mcp` |
| stdio 桥 | `src-tauri/src/bin/guaxi_mcp.rs` | stdio ⇄ HTTP 转发，供只会说 stdio 的客户端（codex）使用 |
| 运行时 | `cognition/runtime.rs` | 全局 AppHandle、四条后台循环、总开关 |
| 接线点 | `cognition/hooks.rs` | 与既有链路的钩子；**永不改变宿主行为** |
| 命令面 | `cognition/commands.rs` | 46 个 `cognition_*` Tauri 命令 |

改动到的既有文件：

| 文件 | 改动 |
|---|---|
| `main.rs` | `mod cognition`、注入 AppHandle、启动后台循环、注册命令、退出时 `cognition::shutdown()` |
| `guaxiservices/server.rs` | 在 8888 上 `configure(cognition::mcp::config)` |
| `commands/chatview.rs` | `get_chat_answer_stream` 接地 + 记轨迹；`agent_chat_stream` 记轨迹 |
| `commands/codex_agent.rs` | 生成的 `config.toml` 追加 `[mcp_servers.guaxi]`（桥接缺失则不写） |
| `commands/kgraph.rs` | 新增 `kg_vec_topk` / `kg_node_of` / `kg_node_text` 三个 pub helper |
| `commands/flipboard.rs` | `save_articles_to_cache` / `persist_hydrated_article` 出口挂 percept 钩子 |
| `guaxiservices/chrome_extension_service.rs` | capture 导入成功后挂 percept 钩子 |

前端：`stores/cognitionStore.js`、`utils/cognitionEvents.js`、
`components/BrainViewComponent/GuaxiCognitionInbox.vue`、
`components/SettingViewComponent/CognitionSetting.vue`，
并改 `GuaxiCenterBrainView.vue`（大脑/图谱切换）、`GuaxiSettingContent.vue` + `SidebarList.vue`（设置入口）。

## 4. 数据模型（`repos/cognition.db`）

与 `kg_index.db` 同目录、同 PRAGMA 风格（WAL + NORMAL）。`store.rs::init_schema` 建表幂等，
`migrate` 按 `cog_settings.schema_version` 推进（当前 v3）。**新增表**只靠 `CREATE TABLE IF NOT EXISTS`
即可覆盖新旧库；**新增列**必须在 `migrate` 里补一段 `add_column`。

| 表 | 作用 | 关键点 |
|---|---|---|
| `percepts` | 感知条目 | `text_hash` 唯一索引做近似去重；`novelty=-1` 表示未打分；`vec` 为 f32 小端 BLOB |
| `episodes` / `episode_steps` | 行为轨迹 | append-only；`vec` 供情节召回 |
| `citations` | 每轮召回给出的候选 | `used`（答案里出现编号）与 `clicked`（用户点开）是学习标签 |
| `beliefs` / `belief_evidence` | 可反驳主张与证据 | `(kb_name, statement_hash)` 唯一；`stance` ∈ supports/contradicts/unrelated |
| `nudges` / `nudge_feedback` | 提醒与裁决 | `dedup_key` 唯一；`features` 存打分时的特征快照 |
| `policy` | 自调产物 | `nudge_gate`（逻辑回归权重+bias）与 `recall_weights`（四路权重）各一行，带 `version`/`metric` |
| `delivery_log` | 打断令牌桶账本 | 只记 notify/interrupt，board/brief 不计入 |
| `token_ledger` | 每日 token 账本 | 按 `purpose` 分类，预算护栏读它 |
| `tool_audit` | 工具调用审计 | 每次经 MCP/Agent/UI 的调用都落一行，args 截断 2000 字 |
| `review_state` | SRS 排期 | SM-2 简化版：`ease` / `interval_days` / `due_date` / `lapses` |
| `interests` | 画像派生层 | `source` ∈ manual（来自 profile.md，不衰减）/ derived |
| `briefs` | 简报成品 | `(day, kind)` 唯一；正文同时落 markdown 文件 |
| `eval_items` / `eval_runs` | 评测闩 | `frozen=1` 的样本不允许被调参当天的数据改写 |

**真相与派生的分界**（沿用 kgraph 的原则）：`profile.md`、简报 markdown、以及知识库里的
`.md` 是真相；本库全部是派生索引、状态机与账本。删库不丢用户内容，
可由 `cognition_maintenance{rederiveProfile}` 与后台循环重建大部分内容。

## 5. 感知层

### 5.1 入口与去重

`percept::ingest` 是唯一写入点，同步、O(1)、绝不触网。打分交给后台，所以采集链路不会被拖慢。

去重用**近似键**而不是原文哈希：`dedup_hash` 取「标题+正文」里的字母数字与 CJK、小写、前 3000 字后
SHA256。理由是同一篇文章经 RSS 摘要、整站抓取、浏览器插件三条路进来时，广告位与时间戳几乎必然不同，
原文哈希去不掉重。

命中去重时**不动已有行**——否则用户已归档的条目会被重新拉回收件箱。

### 5.2 指令中和（防投毒）

`neutralize_untrusted` 是纵深防御的第一层。规则表在 `INJECTION_PATTERNS`（20 条正则，中英双语），
覆盖：越权句式（ignore previous instructions / 忽略以上指令）、角色改写（you are now / 从现在开始你是）、
对话模板标记（`<|im_start|>`、行首 `system:`）、外传诱导（curl http、send it to https://）、
以及 `"tool_calls":` 这类工具诱导。另外单独处理零宽字符与双向控制符（直接删）、HTML 注释（替空格）、
`javascript:`/`data:` 伪协议（替 `blocked-scheme:`）。

命中片段被替换成 `〔已中和的指令样文本〕`——**中和而非删除**：正文语义仍可读，用户能看出
"这里原本有一句想指挥 AI 的话"，也便于事后审计误伤。中和次数写进 `meta.neutralized`，
在收件箱卡片、简报、设置页三处可见。

第二层是 `untrusted_block()`：任何 percept 文本进入 prompt 时都包一层
`<untrusted source="...">` 并显式声明"只能当资料读"。第三层是工具分级（见 §9）。

### 5.3 打分

`score_pending(limit)` 每 20 秒跑一批（默认 24 条），全程本地 embedding：

- **新颖度** = `1 - max cosine(percept, 知识库所有节点)`。复用 `kg_vec_topk`，零新增基础设施。
- **锚点** = 取到最大相似度的那个节点（连同它所属的库），显著的感知会据此强化画像。
- **显著性** = `0.42·novelty_fit + 0.32·affinity + 0.16·trust + 0.10·length_fit`。

`novelty_fit` 是关键设计：它不是新颖度本身，而是一条在 `NOVELTY_SWEET=0.45` 处取峰值的三角函数。
太低=已知（冗余），太高=与我的知识毫无接壤（多半是噪声），中间才是"值得看"。

`trust` 按来源给默认值：manual/chat 1.0、file 0.9、capture 0.75、clipboard 0.6、rss 0.55、crawl 0.5。

### 5.4 来源与授权

`hooks::PERCEPT_SOURCES` 列出 7 路可配置来源。默认策略刻意保守：
`crawl / rss / capture / file / manual` **默认开**（本来就是用户主动配置的采集链路）；
`clipboard / screen` **默认关**（涉及用户在别处的活动，必须显式同意）。
开关落 `cog_settings.percept_source_<kind>`，`hooks::hook_percept` 在入口处检查。

已接线的钩子：

| 来源 | 位置 |
|---|---|
| `crawl` | `flipboard.rs::save_articles_to_cache`（正文 ≥200 字才入）与 `persist_hydrated_article`（深解析后补入） |
| `capture` | `chrome_extension_service.rs::handle_capture_import` 写盘成功后 |
| `manual` / `clipboard` | 前端 `cognition_capture` 命令 |
| `chat` | `hooks::hook_chat_start` 记情节（对话本身也是感知） |

## 6. 召回（`recall.rs`）

四路证据，RRF 融合（`RRF_K=60`）：

| 通路 | 来源 | 默认权重 | 它补的是什么 |
|---|---|---|---|
| `semantic` | 每库 `kg_vec_topk(k*3)`，类型限 section/paragraph/file/concept | 1.0 | 语义相近 |
| `graph` | 语义前 3 名的 n 跳邻居，按 PageRank 排序 | 0.6 | **措辞不同但结构相邻**：前置知识、被引用的定义、兄弟段落 |
| `percept` | `percept_topk(salience≥0.2)` | 0.5 | 还没入库的新材料 |
| `episodic` | `episode_topk` 相似历史问答 | 0.4 | 对话连续性 |

一次调用只做**一次** embedding，四路共用。融合公式只有一份实现（`recall::rrf_fuse`），
召回与评测共用它——否则会出现"调参调的是一套公式、生效的是另一套"。

预算切分：`per = max_chars / k`，前 3 名各拿 1.5×。percept 通路的片段经 `untrusted_block` 包裹，
知识库与情节直出。上下文块要求模型引用时写编号 `[n]`，这是 `citations.used` 的判据。

失败**不返回 Err**：embedding 不可用时返回 `degraded=true` 的空结果，调用方原样走原链路。
召回不可用绝不能让对话失败。

`ground_question` 是对话链路的一步到位入口：召回 → 落引证 → 返回可前置到问题上的接地块；
返回空串即表示这一轮没有可用证据。`should_ground` 过滤掉太短与纯寒暄的输入，省掉一次无谓召回。

## 7. 信念与矛盾检测（`belief.rs`）

两段式，钱花在该花的地方：

1. **候选生成靠向量**（便宜、可跑全量）。embedding 相似只说明"在谈同一件事"，不说明"观点一致"，
   所以它只负责挑出可能冲突的对子。闸门 `JUDGE_SIM_GATE=0.55`。
2. **判决靠 LLM**（贵、严格限量）。单轮上限 `MAX_JUDGE_PER_TICK=6`，且判为 `unrelated`
   也会落一条证据，避免下一 tick 重复烧钱。

抽取只扫 **section 节点**：段落太碎（一句话抽不出自足主张），文件太粗（一份文件混着十几个主题，
抽出来的主张互相污染）。章节是主张的天然粒度。按 PageRank 降序取样，已抽过的章节不重复。

判决提示词的判定标准刻意保守，且只有 `verdict=contradicts && confidence≥0.6` 才上报。
理由写在代码注释里：**一条误报的"你的结论被推翻了"会让用户永久不再信任这类提醒。**
冲突成立时把 `beliefs.confidence` 乘 0.7，但**不自动改状态**——那是用户的判断。

## 8. 主动层（`nudge.rs`）

### 8.1 六类提醒

| kind | 判据 | 基础效用 | 说明 |
|---|---|---|---|
| `contradiction` | belief ↔ percept 冲突判决成立 | 0.9×conf | 最高价值：不处理知识库会一直错下去 |
| `gap` | section/concept 的 PageRank > 全库均值 2 倍且 degree ≤ 2 | 0.55 | 跳过画像里的专长主题 |
| `due` | org 任务 2 天内到期，附一次召回给出的 3 份材料 | 0.6/0.8 | 增量价值在"把材料一起端上来" |
| `review` | `review_state.due_date ≤ 今天`，一次打包 3 条 | 0.45 | SM-2 简化版；种子=写下超 7 天的章节 |
| `emerging` | 近 7 天高新颖度感知贪心聚类（阈值 0.70）≥3 条 | 0.45~0.75 | 知识库里还不存在的主题 |
| `stale` | 画像里「在做的项目」对应文件 14 天未改且留有问句/TODO | 0.5 | 只针对在做的项目，否则纯噪声 |

三个纯结构产出器（gap/review/stale）**共享一次图快照**（`take_graph_snapshots`）。
若各自去查 `kg_query_graph(max_depth=99, limit=100k)`，十万节点的库上就是每 5 分钟克隆整张图三遍。

### 8.2 治理器

这是决定产品"聪明"还是"垃圾邮件"的地方。三道闸，顺序固定：

1. **阈值**：`effective = policy_score×0.7 + utility×0.3 − cost×0.15`，低于 `delivery_threshold()` 降级到 brief。
2. **免打扰**：`profile::in_quiet_hours()` 命中降级到 brief。
3. **令牌桶**：`max_interrupts_per_day`（默认 3）/ `max_interrupts_per_hour`（默认 1）用尽降级到 brief。

**没有任何提醒会被丢掉。** 够不上打断的一律降级到 board/brief，用户随时能在看板里看到全部。
治理器只决定"要不要现在戳你一下"。只有 `contradiction` 且明显超阈值（+0.15）才允许 `interrupt`。

`delivery_threshold()` 按近期采纳率**反向**自适应：`base + (0.5 − accept)×0.4`，夹在 [0.35, 0.85]。
你越少采纳，门槛越高。夹紧区间是为了防止自我强化到极端。

### 8.3 打断策略（逻辑回归）

特征全部是可解释的标量：`utility`、`cost`、`topic_weight`、`ref_count`、`hour_norm`、
`is_workhour`、`kind_*`（6 个 one-hot）、`recent_accept`。约 14 维。

训练：批量梯度下降 400 轮、lr 0.15、L2 0.01，80/20 时序切分。
**门禁**：样本 ≥20 时，新版留出准确率不得比现版低 2 个百分点以上，否则放弃并保留旧策略。
样本 <20 时只做引导训练（此时任何度量都不可信，不设卡）。样本 <8 不训练。

`reset_policy()` 一键回到基线（阈值退回 base + `policy_score = utility` 直通），反馈样本保留。
自调系统必须有这个开关：用户一旦觉得提醒变差了，能立刻把学习成果清掉，而不是只能等它自己收敛回来。

## 9. 工具面与 MCP

### 9.1 分级

| 等级 | 默认 | 内容 |
|---|---|---|
| `read` | **恒开**（不可关） | 召回、图谱查询、收件箱、信念、提醒、任务、画像、全文检索 |
| `write` | 关 | 建笔记、加标签、建关联、改状态、改画像、复习反馈 |
| `network` | 关 | 搜索、加入抓取队列 |
| `exec` | 关 | 重建索引、生成向量、抽取信念、生成简报 |

共 32 个工具。等级授权在 `tools::dispatch` 里**强制执行**，不是提示词约束。

另有一条独立的闸：`dispatch(..., untrusted=true)` 无论授权如何都只放行 Read 级。
这是防止"爬回来的网页指挥 Agent 改你的文件或把内容发出去"的最后一道防线。

`ensure_path_allowed` 是路径闸门：只允许知识库根与认知数据目录之内的路径，
用规范化后的绝对路径前缀比对（`normalize` 消掉 `..`，不依赖文件是否存在）。

每次调用都落 `tool_audit`，设置页可查。

### 9.2 MCP 传输

服务端挂在**既有的** `127.0.0.1:8888/mcp`：不新开端口、不扩大暴露面。
`tools/list` 只列已授权的工具——让模型看见它调不动的东西，只会换来一串失败重试。

codex 只会说 stdio，所以有 `guaxi_mcp` 薄桥接（stdio JSONL ⇄ HTTP POST）。
`codex_agent.rs::mcp_server_toml` 在生成 `config.toml` 时追加 `[mcp_servers.guaxi]`，
**桥接可执行文件不存在就不写这一段**——写了 codex 每轮都会尝试拉起不存在的进程并报错。

外部客户端（Claude Code / Cursor）用同一个桥接即可接入，配置片段由
`cognition_mcp_info` 直接给出可复制的 JSON。

## 10. 进化闩（`evolve.rs`）

没有这一层，"自我进化"和"静默退化"在外部看来完全一样。三件东西：

1. **评测集自动挖掘**：每个 section 由模型生成一个只能靠该章节回答的问题，章节本身是标准出处。
   按 PageRank 取样，落 `eval_items` 且 `frozen=1`。**不允许**用调参当天的数据反过来改评测集——
   那就是自己给自己出题、自己打分。
2. **评测运行**：hit@1 / hit@5 / MRR。关键工程点是 `collect_cases` 与 `score` 分离——
   候选生成一次（每条一次 embedding），融合上百次。否则一轮坐标上升就要几百次 embedding，跑不动。
3. **坐标上升 + 门禁**：4 个维度 × 7 个候选倍率 × 2 轮。最后只有 MRR 提升 ≥0.005 才写
   `policy.recall_weights`；提升不足就什么都不改，并记一条 `accepted=0` 的评测记录，
   让用户看到"系统试过，没找到更好的"。

样本 <20 条直接拒绝调参并给出可读原因。`rollback_recall_weights()` 回到内置缺省，评测历史保留。

明确**不做**：本地 LoRA 微调。相对检索/提示词/策略的收益太小，且会让回归无法归因。

## 11. 后台循环（`runtime.rs`）

四条，节奏刻意错开，避免同时抢 llama-server 与磁盘：

| 循环 | 间隔 | 首次延迟 | 内容 |
|---|---|---|---|
| 打分 | 20s | 25s | percept 打分 + episode/belief/interest 向量补齐（各 16 条一步） |
| 审议 | 300s（可调） | 90s | 六类产出 + 治理器投递 |
| 简报 | 300s 检查 | 150s | `brief::due_now()` 为真才生成 |
| 维护 | 6h 检查 | 240s | 每天一次：兴趣衰减、轨迹裁剪、策略重训 |

首次延迟是为了让主界面先起来，不与首屏抢资源。全部受 `cognition_enabled` 总开关控制——
关闭后采集入口仍然写库（percept 照常入，只是不打分不审议），重新打开时不丢数据。

审议里的预算优先级：token 预算耗尽时**先砍要花钱的**（矛盾检测），
纯结构信号的产出器始终运行，因为它们不花 token。

## 12. 接线纪律（`hooks.rs`）

所有钩子遵守同一条：**永不改变宿主行为**。认知层不可用、未启用、或出错时，钩子静默退化成空操作，
调用方拿到的结果与改造前完全一致。这是把一整层横切能力塞进一个已经在跑的应用里唯一可接受的方式。

具体到对话链路：`get_chat_answer_stream` 把 `match model` 的结果收进 `dispatch_result`，
Err 时收尾情节，然后**原样返回**。接地失败（新库/embedding 未就绪/问题太短）时前置块为空串，
问题原样传给 provider。

`agent_chat_stream` **不改写问题**——Agent 有 `guaxi_recall` 可以自己取更精确的东西，
前置一大段召回反而挤占它的工具预算。只落引证 + 广播给前端。

答案文本由前端在流结束后经 `cognition_chat_finish` 回报。为什么不在后端收：
流式回复逐段 emit，后端要拿全文就得在十来处 provider 流循环里插累积逻辑，
代价与风险都远高于让前端回报一次；而全文的价值很实在——它决定 `citations.used`，
也就是检索权重学习的标签。

## 13. 事件协议

| 事件 | payload | 收端 |
|---|---|---|
| `cognition_nudge` | `{id, kind, channel, push}` | 看板重拉；`push=true` 立刻透传成消息 |
| `cognition_brief` | `{day, kind, path}` | 简报页刷新 |
| `cognition_grounded` | `{episodeId, citations, channels, elapsedMs, mode?}` | 聊天视图渲染引证条 |

前端订阅封装在 `utils/cognitionEvents.js`：`cognition_nudge` 做 600ms 合并防抖
（一次审议常连发多条看板类），但 `push=true` 必须立刻透传，否则用户会觉得提醒"迟到"。

## 14. 命令接口（46 个）

按模块分组，完整清单见 `cognition/commands.rs`。常用的：

| 命令 | 说明 |
|---|---|
| `cognition_status` | 一次拿全：总开关、四层统计、召回权重、工具授权、各可调项、embedding 就绪 |
| `cognition_recall` | 手动召回（带引证），前端"搜我的大脑"用 |
| `cognition_percept_list` / `_get` / `_set_state` / `cognition_capture` | 收件箱 |
| `cognition_nudge_list` / `_decide` / `_get` / `cognition_deliberate_now` | 看板与手动审议 |
| `cognition_brief_get` / `_generate` / `_list` | 简报 |
| `cognition_profile_get` / `_save` | 画像（markdown 直读直写） |
| `cognition_belief_*` / `cognition_detect_contradictions` | 信念与冲突 |
| `cognition_tools_list` / `cognition_tool_set_tier` / `cognition_tool_call` / `cognition_tool_audit` | 工具面 |
| `cognition_mcp_info` | 外部客户端接入信息 |
| `cognition_eval_mine` / `_run` / `_tune` / `_runs` / `cognition_recall_weights_rollback` | 进化闩 |
| `cognition_channel_stats` | 各召回通路的历史命中率——调参前该先看的东西 |
| `cognition_chat_finish` / `cognition_citation_clicked` | 学习标签回报 |

`cognition_set_setting` 是**白名单化**的：只接受 7 个可调项，不做任意键透传。

## 15. 前端

- **大脑页**（`GuaxiCenterBrainView.vue`）改成「大脑 / 图谱」两视图切换，默认落在大脑。
  用 `KeepAlive` 是刻意的：3D 图谱组件已实现 `onDeactivated` 时停 RAF，
  所以既保留相机与布局状态，又不在后台空耗 GPU。
- **收件箱**（`GuaxiCognitionInbox.vue`）三个 tab：提醒 / 收件箱 / 简报。
  顶栏把"今日已打断 / 剩余额度 / 阈值 / 采纳率"放在最显眼处——用户要能一眼看出系统在多大程度上
  打扰了自己，以及为什么。每条提醒都渲染证据链（`why`），没有它提醒就只是断言。
  外部内容统一用左侧橙色竖线标记，全文用弹窗展示并带不可信横幅，不直接塞进编辑器。
- **设置页**（`CognitionSetting.vue`）七节：感知来源、打扰与预算、工具授权、MCP 接入、
  我的画像、自调与评测闩、存储与维护。

## 16. 已验证 / 未验证

已验证：

- `cargo check --bins` 无错误、认知层零警告。
- `npm run build` 通过（23209 modules，3m40s）。
- 新增 bin `guaxi_mcp` 一并编译通过。

未验证（需要真实运行环境与额度）：

- 端到端跑一轮：采集 → 打分 → 审议 → 投递 → 简报。
- codex 经 `[mcp_servers.guaxi]` 实际调用 `guaxi_recall`（需 codex sidecar 与供应商额度）。
- 矛盾检测的误报率——它依赖判决模型质量，需要真实知识库上跑一段时间才能定阈值。
- 评测闩的调参收益：需要先积累 ≥20 条评测样本。

## 17. 首次启用步骤

1. 设置 → 认知层，确认总开关已开（默认开）。
2. 若「语义服务未就绪」提示出现：到知识图谱页对至少一个库跑一次「生成向量」
   （`kg_embed_kb`，首次会自动下载 embedding 模型）。召回与打分都依赖它。
3. 编辑「我的画像」：在「关注」里写下你真正关心的主题，在「在做的项目」里写下项目名。
   这两节直接决定什么会被判为显著、什么会被提醒。
4. 需要 Agent 能改知识库时，在「工具授权」里开 write（以及按需 network / exec）。
5. 想让系统主动找矛盾：在设置里对某个库跑一次「抽取信念」（Exec 级工具或
   `cognition_belief_extract`），之后审议循环会自动把新材料与这些主张对撞。
6. 想启用自调：先「挖掘评测样本」（建议 ≥40 条），再「跑一次评测」拿基线，最后「自动调参」。

## 18. 风险与遗留

- **矛盾检测的误报是最大风险。** 当前用两道保守闸（相似度 0.55 + 置信 0.6）压制，
  但真实误报率必须在实际知识库上观测。若偏高，优先调 `deliberate_modelid` 换更强的模型，
  而不是放宽阈值。
- **首次索引成本。** 大库的 embedding 生成 + 信念抽取都是重活。已做的缓解：全部分批、
  可多次调用推进、受 token 预算约束。但用户第一次用会等。
- **`clipboard` / `screen` 只做了开关与入口，没有采集器。** 后端 `hook_percept` 与
  `cognition_capture` 已就绪，前端定时抓剪贴板 / 屏幕 OCR 的实现留待下一步——
  这两路是隐私最敏感的，值得单独设计交互与可见性，不适合和这次改造一起上。
- **没有系统托盘与 OS 通知。** `channel=notify` 目前落在应用内消息与看板。
  接 `tauri-plugin-notification` + tray 是独立的一小步，需要改 `Cargo.toml` 与
  `capabilities/`，本次刻意没动打包面。
- **邮件 / 日历 / arXiv 类型化适配器**未做。`percept` 的 schema 已经为它们留好位置
  （`source_kind` + `meta`），加一路只需实现一个采集器 + 调 `hook_percept`。
- **codex 的 MCP 兼容性未实跑。** `[mcp_servers.guaxi]` 的字段名按 codex 0.149.1 的
  config 语义写，桥接协议按 MCP 2024-11-05。若 codex 版本升级，参照
  `codex-agent-design.md` 第 11 节做一次回归。

## 关键文件索引

| 关注点 | 文件 |
|---|---|
| 建表 / 迁移 / 账本 / 审计 | `src-tauri/src/cognition/store.rs` |
| 防投毒（中和 + 包裹） | `src-tauri/src/cognition/percept.rs` |
| 四路融合与引证 | `src-tauri/src/cognition/recall.rs` |
| 治理器与策略学习 | `src-tauri/src/cognition/nudge.rs` |
| 评测闩 | `src-tauri/src/cognition/evolve.rs` |
| 工具分级与强制 | `src-tauri/src/cognition/tools.rs` |
| MCP 服务端 / stdio 桥 | `src-tauri/src/cognition/mcp.rs`、`src-tauri/src/bin/guaxi_mcp.rs` |
| 接线纪律 | `src-tauri/src/cognition/hooks.rs` |
| 前端看板 / 设置 | `src/components/BrainViewComponent/GuaxiCognitionInbox.vue`、`src/components/SettingViewComponent/CognitionSetting.vue` |
