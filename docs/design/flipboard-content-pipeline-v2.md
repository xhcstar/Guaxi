# 内容采集管线 v2：大规模抓取 → 高质量 Markdown

> 目标：把现在的 "RSS 摘要聚合器" 改造成 "大规模、可重放、以 Markdown 为真源的高质量内容采集系统"，
> 同时服务于两个下游：**人阅读** 与 **LLM 训练语料导出**。
>
> 本文只描述目标架构与迁移路径，不包含实现。确认后再按第 10 节分期落地。

---

## 1. 现状诊断

### 1.1 实测数据（`target/debug/repos/flipboard_cache.sqlite3`，120 行）

| 指标 | 实测 | 说明 |
|---|---|---|
| `clean_content_markdown` 为空 | **101 / 120（84%）** | 从未深解析过，只有 RSS 摘要 |
| 有效正文（md ≥ 500 字符） | 19 / 120（16%） | 真实可读的正文 |
| `relevance_score = -1`（未评分） | **120 / 120** | 相关性打分整条链路是死的 |
| Wired 源里的促销页 | **18 / 50** | `Promo Code` / `Coupon` / `Deal` |
| 爱范儿 `#欢迎关注…公众号` 模板尾巴 | **20 / 20** | 全部命中，没人剥 |
| 聚合索引贴（派早报/早报｜/本周看什么/一图流） | ≥ 9 | 与深度文章同权占位 |
| `description` 里残留 HTML 标签 | 21 / 120 | 直接渲染到卡片上 |
| `content == description` | 60 / 120 | 正文就是摘要的重复 |
| DB 文件大小 / 有效行 | **128 MB / 120 行** | `freelist_count=30902`，98% 是回收页，从未 VACUUM |

### 1.2 结构性问题（按严重度）

**S1｜过滤发生在 `LIMIT` 之后，选择阶段无门槛。**
`load_cached_articles`（`flipboard.rs:783`）与 `fetch_knowledge_from_feeds`（`:3196`）都是
先 `ORDER BY updated_at DESC LIMIT n`，再在内存里 `retain(is_globally_muted)`。
选中进去的是"最近更新的 N 条"，不是"最好的 N 条"。
`topic` 过滤同样在 `LIMIT` 之后（`:815`），**专题检索会漏掉较早的高质量文章**。

**S2｜抓取与抽取耦合，正文永远停在"摘要"态。**
`fetch_rss_feed_internal`（`:3347`）把 `item.content()`/`description()` 原样塞进 `content`；
真正抽取只在三处触发：用户点开（`get_article_detail`）、
`preheat_thin_articles`（每源每 tick 2 篇）、`preheat_display_articles`（一次 10 篇）。
162 个源 × 60 分钟一次，靠"打开时再抓"补，永远补不完。

**S3｜抽取并发实际上是坏的。**
抽取走单一常驻 Python worker（固定 key `__guaxi_flipboard_extract__`）。
`with_python_worker_for_key`（`code_runtime.rs:3644`）用 `try_lock()`，
一旦 `WouldBlock` 就 **移除并重建 worker**（日志："replace cached worker to avoid hanging requests"）。
而 `preheat_display_articles` 是 4 并发 → 互相踩，反复冷启动 `import trafilatura`。
另外 worker 的响应同时返回 `html + markdown + text`，一份内容传三份。

**S4｜清洗只有一层硬编码关键词。**
`is_noise`（`flipboard_extract.rs:354`）判 `contains("广告")` 这类；
`is_globally_muted`（`flipboard.rs:2749`）只看 `title + description`。
所以：
- Wired 的 `Uber Eats Promo Codes: $15 Off` 一词不命中；
- 正文里的公众号尾巴、导航、相关阅读完全绕过；
- 无法区分"文章 / 列表索引 / 登录墙 / 商品页"。

**S5｜没有原文留存，规则升级无法重放。**
只存抽取结果，不存 raw HTML。换了抽取器或调了阈值，只能重抓网络。

**S6｜源管理混乱。**
- `FeedSource` 把"源"和"抓取方式"揉在一个扁平结构；162 个源全 enabled、全 60 分钟。
- `ensure_category_default_feeds`（`:1667`）会自动补源，**用户删掉的内置源自愈复活**。
- 前端 `FeedSourceConfigDialog` 收 `interestProfile / relevanceThreshold / llmEnhance` 三个字段，
  但后端 RSS 路径**完全没有接 LLM**（`flipboard_llm::enhance` 只在 `process_crawl_item` 调用），
  阈值因为没有打分而永不生效 —— 三个 UI 控件都是装饰。
- `save_articles_to_cache("all", ...)`（`:5519`）用假 feed_id 写缓存，污染来源维度。

**S7｜mock 数据能进生产路径。**
`refresh_articles_from_feeds`（`:5476`）与 `refresh_topic_knowledge_from_feeds`（`:5564`）
在无可用源时 `generate_mock_articles()` 并写入全局 `ARTICLES`。空源会变成假新闻。

**S8｜正文格式无保证，前端被迫兜底。**
`FlipboardDetailView.contentSource`（`:362`）要判断"clean_html 是不是只有图没正文"再退回 `content`；
`ArticleCard.cardSummary`（`:89`）直接拿 `description` 渲染 → HTML 标签上屏。
根因是数据侧没有契约：同一字段可能是完整 HTML、纯文本、或 RSS 摘要。

---

## 2. 目标与验收指标

三个硬目标，全部可量化：

| 维度 | 目标 | 当前 |
|---|---|---|
| **完整** | `ready` 态文章 ≥ 90% 正文 ≥ 800 字符 | 16% |
| **干净** | 抽检 200 篇，噪声（促销/导航/登录墙/索引页）≤ 2% | 促销 36%、模板尾巴 100% |
| **可重放** | 任何清洗规则变更后，无需重抓网络即可全量重算 | 不可能 |

附加：

- **吞吐**：单机稳态 ≥ 300 篇/小时完成全流程（发现→取页→抽取→评分），峰值 ≥ 1200 篇/小时。
- **语料可用**：导出的 JSONL 中，`quality ≥ 80` 且 `lang` 已标注的比例 ≥ 60%。
- **不打扰**：任何源失败、被墙、0 产出，都必须**看得见**（看板 + 状态），不能静默降级成摘要或 mock。

---

## 3. 总体架构

```
                    ┌──────────────── Source Registry（源登记，版本化）────────────────┐
                    │  rss / atom / html_index / site_crawl / api_json / rsshub       │
                    └───────────────────────────┬────────────────────────────────────┘
                                                │
   ┌────────────────────────────────────────────▼─────────────────────────────────────┐
   │ A 发现 Discovery  →  只拿条目元数据（url/title/published/source/summary）         │
   │                     写 item 行 state=discovered；不做任何网络二次请求            │
   ├──────────────────────────────────────────────────────────────────────────────────┤
   │ B 取页 Fetch/Render  →  HTTP 优先，按源策略升级 Chrome；落 raw_html（压缩）       │
   │                      state=fetched；失败记 reason，可重试                        │
   ├──────────────────────────────────────────────────────────────────────────────────┤
   │ C 识别 Classify      →  page_kind: article|index|login_wall|product|forum|video   │
   │                      非 article 走分支或丢弃，reason 落库                         │
   ├──────────────────────────────────────────────────────────────────────────────────┤
   │ D 抽取 Extract       →  trafilatura（+站点适配器）→ 块级清洗 → Markdown 规范化    │
   │                      state=extracted                                            │
   ├──────────────────────────────────────────────────────────────────────────────────┤
   │ E 增强 Enrich        →  质量评分 / 语言 / 去重合并 / LLM 摘要标签 / 分类         │
   │                      state=ready                                                │
   └────────────────────────────────────────────┬─────────────────────────────────────┘
                                                │ 双写
                        ┌───────────────────────┴────────────────────────┐
                        ▼                                                ▼
        Markdown 文件（真源，可移植）                        SQLite（索引 + 状态机 + 队列）
        Feeds/<collection>/<src>/<yyyy-MM>/<slug>.md         items(id, url, state, quality, path…)
                        │
                        ▼
        知识的统一出口：全文检索 / 知识图谱 / 编辑器插入 / 认知层 / 训练语料导出
```

**三条设计原则**

1. **每个阶段独立可重试、可重放。** 状态机推进，改规则只重跑对应阶段。
2. **Markdown 是唯一正文真源。** SQLite 里只放索引、状态与检索字段，不放正文。
3. **判定先于抽取。** 用页面类型决定怎么处理，而不是抽完再猜"像不像文章"。

---

## 4. 信息源：组织与管理

### 4.1 两层模型（关键改动）

现在 `category` 既是"抓取范围"又是"展示分组"，导致"取消分类 = 停止抓取"的隐式行为。
拆成两个正交概念：

**Source（抓取单元）** —— 一个可抓取的入口。

```rust
pub struct Source {
    pub id: String,
    pub kind: SourceKind,        // Rss | Atom | HtmlIndex | SiteCrawl | ApiJson | Rsshub
    pub url: String,
    pub name: String,
    pub enabled: bool,

    // 抓取策略
    pub interval_secs: i64,      // 每源独立
    pub render_policy: RenderPolicy, // Never | Auto | Always
    pub max_items_per_run: u32,
    pub budget_weight: u8,       // 全局预算分配权重

    // 收录判定（替代现在的 include/exclude 两个字符串）
    pub policy: AdmitPolicy,     // 见 6.2

    // 实测信誉（由管线自动更新，用户可覆盖）
    pub quality_ema: f32,        // 最近 N 篇质量分指数均值
    pub yield_ema: f32,          // 每次抓取产出条数
    pub success_ema: f32,        // 成功率

    pub origin: SourceOrigin,    // Builtin | User
    pub tombstone: bool,         // Builtin 被用户删除 → 永久不再自动补回
    pub remark: Option<String>,
}
```

**Collection（阅读单元）** —— 用户视角的主题，绑定 N 个 Source + 一组筛选。

```rust
pub struct Collection {
    pub id: String,
    pub name: String,            // "AI 前沿" / "系统编程" / "医学证据"
    pub sources: Vec<String>,    // Source id 列表
    pub filter: CollectionFilter, // 语言 / 最低质量 / 关键词 / 时间窗 / 允许的 page_kind
    pub sort: SortPolicy,
    pub pinned: bool,
}
```

`category` 降级为 Collection 上的一个标签，不再参与抓取。

### 4.2 版本化源清单，内置源不复活

现在 `ensure_category_default_feeds` 会自动补回用户删掉的源。改为：

- 内置源清单编译进二进制（`curated_feeds()`），带 `manifest_version`。
- 用户对内置源的操作（禁用/删除/改参数）写入 **overlay**：`{id → {enabled, tombstone, overrides}}`。
- 合并规则：`manifest ⊕ overlay`。`tombstone=true` 的永不出现；新版本清单新增的源才会自动出现，且**不自动启用**（用户确认）。
- 这解决了"删了又回来"和"升级后突然多出 30 个源在后台跑"。

### 4.3 源健康看板（替代现在的 `FeedHealth` 简表）

每个源一行，直接回答"这个源在不在产垃圾"：

| 列 | 含义 |
|---|---|
| 最近 N 次 | 成功率、平均条目数 |
| 平均正文长度 / 质量分 | 低于阈值标红 |
| 页面类型分布 | article / index / product / login_wall 占比 |
| 噪声率 | 被 Classify 或质量门拒掉的比例 |
| 最近失败原因 | 取页失败 / 抽取空 / 登录墙 / robots 拒 |
| 自动降频状态 | 连续 N 次低产 → 自动拉长间隔（**不自动删**） |

新增诊断视图：点开某个源，看**最近 10 条的处置轨迹**（页面类型判定、质量分、丢弃理由）。这是让用户能自己判断"该不该留这个源"的唯一办法。

### 4.4 内置源清单的清理

`curated_feeds()` 现在 162 条，其中一些必然产噪声，应分级：

- **Tier A（高信噪，默认启用）**：arXiv / 期刊 / 个人技术博客 / 高质量刊物（少数派、Ars、Lil'Log…）。
- **Tier B（有噪声，默认启用但带更严 policy）**：Wired、TechCrunch 这类会混促销页的站点 → 必须带 `deny_url_patterns`。
- **Tier C（登录墙平台）**：知乎 / 微博 / 小红书 / Twitter —— **默认启用**，但走"匿名优先"策略（见 4.5）。
- **移除**：`site_crawl` 指向 `zhuanlan.zhihu.com/p/198861...` 这种"单篇文章当整站抓"的配置（现有配置里就有两个）。

### 4.5 登录墙平台：匿名优先，登录是加成而不是前提（决策 3）

用户的要求是"这些站点往往有最新最高质量的内容，最好开了，或者不登录就能抓"。
我实测了本机 release 版 RSSHub sidecar（`D:\guaxi\utils\SlidevEnv\node.exe`，端口 1200）与直连抓取，结论如下：

**实测（2026-11-04，无任何 cookie）**

| 端点 | 结果 | 说明 |
|---|---|---|
| `https://www.zhihu.com/api/v3/feed/topstory/...`（现代码用的） | **401** | 现实现就是走的这个 |
| `https://api.zhihu.com/topstory/hot-lists/total` | **200 / 61KB / 30 条带摘要** | **换域名即可匿名** |
| `https://www.zhihu.com/question/{id}` 直连 | 403 | 需登录 |
| `rsshub:/zhihu/hot` | **200 / 42KB** | 匿名可用（仅标题，无摘要） |
| `rsshub:/zhihu/daily` | **200 / 443KB / 30 条 × ~5KB 完整正文** | **匿名可用，正文级** |
| `rsshub:/sspai/index` | 200 / 269KB | 匿名可用 |
| `rsshub:/36kr/newsflashes` | 200 / 13KB | 匿名可用 |
| `rsshub:/zhihu/question/{id}` | 503 | 需登录热榜之外的内容 |
| `rsshub:/weibo/search/hot` | 503 | 需 cookie |
| `rsshub:/bilibili/hot-search` | 503 | 需 Playwright 浏览器（本机未装） |
| `rsshub:/twitter/user/*` | 503 | `ConfigNotFoundError: Twitter API is not configured` |

**三条可落地的结论**

1. **现在有两个现成的 bug 级机会，且都是免费的。**
   - 知乎热榜：把 `www.zhihu.com/api/v3/...` 改成 `api.zhihu.com/topstory/hot-lists/total`，**匿名立刻可用**，还带摘要。
   - 知乎日报：`rsshub:/zhihu/daily` 匿名返回**每篇约 5KB 的完整 HTML 正文** —— 这是今天唯一"不登录就拿到全文"的高价值来源，而当前配置里根本没有它。

2. **RSSHub 已经完整打包在安装目录里（26MB dist + node 运行时），是现成的匿名抓取通道。**
   现在代码只用它做 Radar 探测，没当数据源。应该把 `rsshub://` 提升为一等 `SourceKind`：
   路由列表可枚举、可订阅、可配置 cookie。

3. **登录是"加成"，不是"前提"。** 因此策略定为 **AnonymousFirst + 登录加成**：

```
对每个登录墙源：
  ① 先走匿名路径（公开 API 域名 / rsshub 免登录路由 / 移动端 API）
  ② 命中 login_wall 且有登录态 → 走已登录路径
  ③ 命中 login_wall 且无登录态 → 入 needs_login 队列，UI 提示"登录可提升该源质量"，但不阻塞其他源
```

**关键修正：不再有"Tier C 默认关闭"。** 改为：

| 平台 | 默认 | 匿名能力 | 登录后的增量 |
|---|---|---|---|
| 知乎 | **启用** | 热榜（摘要级）+ 日报（全文级） | 问题回答、专栏全文、个人动态 |
| 微博 | **启用** | 热门话题（受限） | 完整信息流 |
| 小红书 | **启用** | 公开笔记（受限，302 跳登录） | 关注流、搜索完整结果 |
| Quora | 启用 | 公开问答页 | — |
| 微信公众号 | 启用 | 搜狗微信入口（实测 200） | 更多账号 |
| Twitter/X | 启用但标注"需 API" | 无（需官方 API key） | 取决于配置 |

**实现要点**

- `SourceKind::Rsshub`：`url` 存路由路径（如 `/zhihu/daily`），抓取时拼接 `rsshub_base_url()`。
  RSSHub 的生命周期由现有 `rsshub_server::ensure_started` 管理。
- **匿名 cookie 池**：不注入用户 cookie 时，RSSHub 自身有"访客 cookie"机制（实测微博报了
  `Cooling down before new visitor Cookies`），需要给 sidecar 预留冷却时间；抓取侧对此建 cooling 退避。
- **登录加成检测**：登录后调 `rsshub_refresh_cookies()` 重启 sidecar（已有接口），
  然后对 `needs_login` 队列批量重试。
- **Playwright 缺失**：bilibili 等路由依赖 RSSHub 自带的 Playwright 浏览器，但本机没装。
  两条路：要么在 `prepare_rsshub_env` 里把 chromium 一起装上（体积大），要么禁用这类路由并标注原因。
  建议**先标注禁用**，不为此增加安装包体积；用户想用可手动开。
- **合规**：匿名抓取只走站点公开接口/公开页面；登录态仅用于用户自己有权访问的内容；不破解验证码。

### 4.6 源的分级改为"配置模板"而非硬编码

`Tier A/B/C` 只作为**初始默认值**，落到 `Source` 的字段上（`enabled` / `render_policy` / `admit_policy`）。
用户可以在配置页整体调整，也可以单源覆盖。这样"分级"不是一个用户看不见的枚举，
而是一组可读可改的初始设置。


---

## 5. 抓取

### 5.1 阶段 A：发现

只做一件事：拿到条目列表的元数据。**不取正文、不渲染、不抽取。**

- RSS/Atom：解析 `<item>` → `(url, guid, title, published, summary, author)`。
- HtmlIndex：列表页 CSS 选择器（源级可配）→ 条目链接；**只抓列表页本身**，个体正文留给阶段 B。
- SiteCrawl：seed → robots/sitemap → BFS 队列（现有 `crawl_queue` 可复用）。
- ApiJson：站点 JSON 接口。
- Rsshub：`url` 是路由路径（如 `/zhihu/daily`），拼 `rsshub_base_url()` 即得 RSS。
  这是登录墙平台的**匿名主通道**（见 4.5），优先级高于直连。

产出写 `items`，`state=discovered`。

### 5.2 阶段 B：取页与渲染

- **render_policy 按源配置**，不再"先 HTTP 不行再 Chrome"（现在对 JS 站等于永远抓两次）。
  - `Never`：RSS 自带全文的源、纯静态站。
  - `Auto`：先 HTTP，薄/空壳再升级（现状行为）。
  - `Always`：已知 SPA/登录站。
- **保存 raw HTML**：`raw/<sha1(url)>.html.zst`（zstd 压缩；也可整段存 BLOB，见 7.3 权衡）。
  抽取失败、规则升级、用户手动"重新解析"都从这里重放，**不重抓网络**。
- 反检测/登录态复用现有 `flipboard_render`（`STEALTH_SCRIPT` / profile / 登录窗口）——这块已经做得不错，不改。
- **全局速率与并发**：
  - 全局令牌桶（默认 8 并发、按 host 4 并发、host 间隔 ≥ 500ms）；
  - 单次运行总页数预算按 `budget_weight` 分给各源；
  - 尊重 robots（现有 `texting_robots` 已引入）。

### 5.3 抽取并发：worker 池

现状（S3）是单 worker + 遇锁重建。改为：

- 池大小 `N = min(4, cpu/2)`，key 为 `__guaxi_extract_{i}__`，round-robin 派发。
- `with_python_worker_for_key` 的 `WouldBlock` 分支改为**等待**（含超时），不重建。
- 请求体瘦身：worker 只返回 `markdown + metadata`，`html` 与 `text` 就地派生，不再三份齐传。
- 失败计数：同一 worker 连续崩溃 2 次 → 从池里剔除并补一个新的（熔断而非重建整池）。

预期：抽取吞吐从"实际串行"变为 4 路并行，且不再反复冷启动。

### 5.4 优先级队列

`items` 带 `priority`：

1. 用户在读/收藏的文章（保证点开秒开）
2. 新发现条目（Discovery 刚拿到的）
3. 历史回填（`state=discovered` 的积压）
4. 重试（`state=failed` 的，指数退避）

调度器每 tick 按"预算 + 优先级"取件，而不是"每个源轮流抓 80 条"。

---

## 6. 内容识别、清洗、成 Markdown

这是质量的核心。分四层，每层都有明确产出与可观测指标。

### 6.1 第一层：页面类型判定（新增）

在抽取**之前**判断这是什么页面，决定后续策略。信号：JSON-LD `@type`、`og:type`、
正文长度/链接比、价格与折扣词、登录/验证码标记、评论结构、URL 模式。

| page_kind | 判定信号 | 处置 |
|---|---|---|
| `article` | `Article`/`NewsArticle`/`BlogPosting`；文本/链接比 > 0.5 | 正常抽取 |
| `index` | 列表结构、条目链接密度高、正文短 | 丢弃（或作为 Discovery 的二级入口） |
| `product` | `Product` schema、价格 + 折扣词、"加入购物车" | 丢弃 |
| `login_wall` | 现有 `looks_like_login_wall` 信号 + 正文极薄 | 记 `needs_login`，进待登录队列（现有机制） |
| `video` | `VideoObject`、播放器容器 | 只存元数据 + 封面，不强行抽正文 |
| `forum` | `DiscussionForumPosting`、嵌套回复 | 抽主帖，回复单独结构化 |
| `error` | 404/5xx 页、空白模板 | 记失败 |

产出 `page_kind` + `kind_reason`，落库。**这是"垃圾拦截"最有效的一层**，
比事后关键词匹配准确得多（Wired 促销页在 `product` 层就没了）。

### 6.2 第二层：源级收录策略（`AdmitPolicy`）

每个源可配，默认从站点类型推导：

```json
{
  "allow_kinds": ["article"],
  "deny_url_patterns": ["promo-code", "coupon", "-deals?", "-sale", "/shop/", "utm_"],
  "allow_url_patterns": [],
  "min_text_chars": 400,
  "min_quality": 35,
  "lang_allow": ["zh", "en"],
  "deny_title_patterns": ["^早报", "^周报", "盘点", "精选合集", "一图流"],
  "require_dated_url": false
}
```

`min_text_chars` / `min_quality` 在抽取之后生效；URL/标题规则在发现阶段就能拦，省一次网络请求。

### 6.3 第三层：正文抽取

1. **主引擎 trafilatura**，参数按站点调优：
   - 中文源：`favor_recall=true`；英文源：波动小，默认。
   - 跑两次（precision / recall）按 `(去重后字符数, 段落中位数)` 取更优的那份。
     trafilatura 单次约 50–150ms，双跑成本可接受，换来正文完整率显著提升。
2. **站点适配器**（覆盖 80% 高价值源）：为 arXiv / WordPress / Ghost / Substack / 主流 CMS
   写 CSS 选择器优先的抽取规则，准确率远高于通用算法。适配器表可配置、可热更新。
3. **Rust scraper 兜底**：仅在 trafilatura 不可用或产出 < 200 字符时启用（现有实现保留，
   但去掉"图片降级为链接列表"的历史包袱）。

### 6.4 第四层：块级清洗 → 干净 Markdown

这是"从 HTML 到高质量 Markdown"的实质工作，现在几乎是空缺。

1. **模板段自动学习（解决公众号尾巴）**
   同一源内统计段落 n-gram 频次：某段落在 ≥30% 的文章中出现 → 判为模板，剥离。
   自动适配新源，无需硬编码"欢迎关注"。剥离记录到 `stripped[]` 供审计。
2. **结构噪声剥离**：导航、面包屑、"相关阅读"、分享栏、评论区、版权页脚、
   订阅引导、"点击展开"。
3. **重复段落去重**（同文内）。
4. **图片**：
   - 绝对化、去 tracking 参数；
   - 剔除 1px / 占位 / 图标 / 装饰图（按尺寸 + src 特征）；
   - 保留 `alt` / `figcaption` → Markdown 图注；
   - **本地化**到 `_assets/<sha1>/`（见 7.4）。
5. **代码块**：`<pre><code class="language-x">` → 带语言标记的围栏。
6. **表格**：`<table>` → Markdown 表格（简单表）/ 保留 HTML（合并单元格等复杂表）。
7. **数学**：MathJax / KaTeX（`<span class="katex">`、`<math>`）→ `$...$` / `$$...$$`。
   **训练语料的关键资产，现在完全丢失**。
8. **规范化**：
   - 正文里的 `h1` 降为 `h2`（文档标题已在 frontmatter）；
   - 标题层级补齐、跳过空层；
   - 空白/标点规范化、去连续空行；
   - 相对链接绝对化。
9. 产出 `content_md` —— **唯一正文真源**。

### 6.5 第五层：元数据与质量分

**元数据**（多来源投票，冲突按可信度取）：title、author、published_at、lang（脚本检测 + 常见词表）、site、word_count、image_count。

**质量分 `quality` 0–100，可解释分项**：

| 分项 | 权重 | 信号 |
|---|---|---|
| 正文完整度 | 30 | 字符数、段落数、段落长度分布 |
| 噪声比 | 25 | 链接密度、模板段占比、剥离量 |
| 元数据完整度 | 15 | 标题/作者/时间/语言是否齐 |
| 标题-正文一致性 | 10 | 标题词在正文的覆盖 |
| 源信誉 | 10 | `source.quality_ema` |
| 用户兴趣 | 10 | 见下 |

每项落库（`quality_parts` JSON），调参时可直接归因。

**用户兴趣**：现在依赖 `feed.interest_profile`，内置源全为空 → 永远 -1。
改为三级：
1. 用户显式配置的 Collection 关键词；
2. 阅读历史 + 收藏的来源权重（前端 `buildSourcePreference` 已有，挪到后端）；
3. 认知层的 `interest` 表（`cognition_interest_upsert` 已存在）。

**LLM 增强（可选，按预算）**：只在 `quality` 处于中间带（如 40–75）或用户显式开启时调用，
单次返回 `{summary, tags, relevance, cleaned}`。低分直接丢、高分不必花钱。

### 6.6 去重与合并

- **URL 规范化**：去 utm/fbclid/锚点/AMP/trailing slash。
- **同文跨源**：正文 simhash（64 bit）+ 标题指纹；汉明距离 ≤ 3 判同。
  合并为一条 canonical，其余写 `aliases[]`（保留来源，便于"这条被几个源报道"）。
- **转载识别**：正文 ≥ 95% 相似 → 保留首发/高质量者。
- 现在的 `dedup_articles_by_identity` 是 `url|title` 全等，跨源同稿合并不了。

---

## 7. Markdown 的组织管理

### 7.1 目录结构（真源落盘）

```
<kbs_root>/Feeds/                          ← 新增顶层库，作为独立知识库
  _index.json                              ← collection → source 映射（可移植）
  <collection>/                            ← 如 "AI前沿"
    <source-slug>/                         ← 如 "arxiv-cs-ai"
      2026-11/
        20261104-attention-is-all-you-need-a3f9c1.md
  _assets/
    <sha1-prefix>/<filename>               ← 图片本地化
  _raw/                                    ← 可选：raw HTML（若不走 SQLite）
```

理由：
- 现在笔记写进 `<某个 KB>/Collected/`（`build_kb_note_path`，`flipboard.rs:2993`），
  **归属哪个 KB 语义不明**，且无法跨 KB 组织。
- 新建 `Feeds/` 作为一等知识库：自动进 `kgraph`（索引 `.md`）、自动进 `fulltext`（tantivy）、
  走现有 `backup`/`p2psync`（文件天然可同步）、可在库网站里浏览。
- `_index.json` 让整个目录可移植：换机器只需拷目录。

### 7.2 文件格式

```markdown
---
id: "a3f9c1e2..."
title: "Attention Is All You Need"
url: "https://arxiv.org/abs/1706.03762"
canonical_url: "https://arxiv.org/abs/1706.03762"
aliases: []
source: "arXiv cs.AI"
source_id: "builtin-ai-0"
author: ["Vaswani, Ashish", "..."]

published_at: "2017-06-12T00:00:00Z"
fetched_at: "2026-11-04T09:12:33Z"
extracted_at: "2026-11-04T09:12:35Z"

lang: "en"
word_count: 8412
image_count: 3

quality: 92
quality_parts: { completeness: 30, noise: 24, meta: 15, title_match: 9, source: 8, interest: 6 }

collection: ["AI前沿"]
tags: ["transformer", "attention", "NLP"]
summary: "提出完全基于注意力机制的 Transformer 架构，舍弃循环与卷积。"

page_kind: "article"
extractor: "trafilatura@0.16.1"
extractor_version: 3
license: null
---

## Introduction
...
```

- frontmatter 供索引/检索/导出；正文纯净。
- `extractor_version` 让"规则升级后哪些要重算"变成一条 SQL。

### 7.3 SQLite 的角色：索引 + 状态机 + 队列

```
items(
  id TEXT PK,               -- content 稳定哈希
  url, url_norm, canonical_url,
  source_id, collection_id,
  title, author, published_at, lang,
  page_kind, state,         -- discovered|fetched|extracted|ready|rejected|failed|needs_login
  quality, quality_parts,
  path,                     -- 指向 md 文件
  sha256_md, word_count, image_count,
  raw_path,                 -- 指向 raw html
  first_seen_at, updated_at,
  reject_reason, fail_reason, fail_count, next_retry_at,
  priority,
  -- 保留策略（见 7.5）
  opened_at, open_count, kept, starred,
  purge_after,              -- 计算好的下次清理时间；NULL = 永不清
  purged_at                 -- 软删除时间，24h 内可撤销
)
items_fts(...)              -- FTS5 或复用 tantivy
aliases(item_id, url, source_id)  -- 多源同文
```

- 正文**只在 Markdown 文件里**，SQLite 不存大字段 → DB 体积不再膨胀（当前 128MB/120 行的根因）。
- 检索可先落 FTS5（简单），或直接复用现有 tantivy 索引 Feeds 目录。建议**后者**：一套索引，避免两套。

### 7.4 图片本地化

- 下载到 `_assets/`，Markdown 用相对路径引用。
- 好处：① 离线自足（符合项目"release 包必须离线自足"的既有要求）；
  ② 源站删图不残缺；③ 训练语料图文配对完整；④ 不再依赖远程图片代理。
- 代价：磁盘增长。策略：单图 ≤ 8MB、每篇 ≤ 30 张、总容量水位告警（如 10GB）。
- 失败降级：下载失败保留远程 URL，并标记 `asset_missing`。

### 7.5 生命周期与保留策略（决策 1）

**核心原则：`ready` 不等于"永久保留"。磁盘上只该有"人看过的"和"即将被人看的"。**

本节是三道闸中的最后一道（回收）；前两道（拒收 + 日摄入上限）见 12.1。
三道闸必须一起上，只做回收等于"先囤再扔"，磁盘和索引都被无谓撑爆过。

双写 Markdown 之后，最大的风险不是抓不到，而是**囤积**：每天新增几千篇，一个月就是十几万文件，其中 99% 从没被读过。所以保留策略必须由**阅读行为**驱动，而不是由"处理成功"驱动。

引入 `opened_at` / `open_count` / `kept`（显式保留）/ `starred`（收藏）四个字段驱动决策：

| 状态 / 条件 | 保留期 | 说明 |
|---|---|---|
| `starred` 或已插入知识库/笔记 | **永久** | 用户的明确资产，永不清 |
| `kept=1`（用户点"保留"） | **永久** | 用户不想让它过期 |
| `open_count > 0`（读过） | **180 天**，之后降为未读规则 | 读过说明有价值，但不是永久 |
| `ready` 且**从未打开** | **14–30 天**（可配，默认 21 天） | 这是"囤积"的主要来源 |
| `rejected` | 7 天（保留以便调参复盘） | 带 `reject_reason` |
| `failed` | 30 天，期间指数退避重试 | 保留 raw 以便重放 |
| `needs_login` | 保留至用户登录或 30 天 | 登录后可批量重试 |

**关键设计：清理是按"未读且过期"分页批量执行的，不是一次性删除。**

- 每 tick 删最多 N 条（如 200），避免长事务阻塞 UI；
- 删除 Markdown 前先检查是否被引用（笔记 / 收藏 / 知识库 / 导出语料清单），被引用则标记 `keep` 并跳过；
- 图片按引用计数清理：`_assets` 下某图不再被任何 md 引用 → 删除；
- 删除后按需 `VACUUM`（或 `PRAGMA incremental_vacuum`），当前库里 98% 是空闲页。

**用户可配置**（放进新的 Feeds 配置页，见 9.2）：

```
未读文章保留期：[14 / 21 / 30 / 60 / 90 天 / 不清理]
已读文章保留期：[90 / 180 / 365 天 / 永久]
图片与 raw：    [随正文删除 / 保留 N 天 / 永久]
磁盘水位：      达到 __ GB 时，按"未读最旧优先"自动清理
```

**为什么不用"只保留高质量"？** 因为 `quality` 是启发式分数，不是用户判断。
按质量删会误删冷门但真正有价值的文章。按"读没读过"删则语义明确：你不看的东西才被回收。
质量分只用于**排序**（让好东西先被看到），不用于**删除**。

**UI 契约**：列表里每篇都要能一眼看出保留状态；清理前给一次汇总提示（"将清理 1,204 篇 21 天前未读的文章"），可撤销 24 小时（软删除 + 移动目录而非立即 unlink）。

同时：`cleanup_old_cached_articles_if_due` 加 **`VACUUM`**（当前 98% 是空闲页，126MB 空洞）。

---

## 8. 显示与查看

### 8.1 列表 = 检索，不是"抓取结果"

- `search_items(collection, query?, filters, sort, page)` 直接查索引：
  `state='ready' AND quality >= min`，排序 `quality DESC, published_at DESC`。
- 前端 `filteredArticles` 那套"关键字兜底过滤"（`flipboardStore.js:853`）删除——
  它会让一条含关键词的早报覆盖一堆查询，且和专题检索语义冲突。
- 筛选器：collection、语言、时间窗、来源、最低质量、字数。

### 8.2 卡片

- 标题、来源、时间、语言、字数；可选质量徽章。
- 摘要：`summary`（LLM）或正文首段，**保证纯文本**（数据侧契约）。
- `page_kind != article` 的条目不出现在阅读流（或单独 "速览" 视图）。

### 8.3 阅读器

- 直接渲染 Markdown（`content_md`），复用现有 `renderMarkdown`（GuaxiMarker）。
- **删除** `contentSource` 的三路兜底（`clean_content_html` / `content` / 纯文本），
  数据侧保证 `ready` 就是合法 Markdown。
- 图片走本地 `_assets`。
- 保留：划词翻译、双语对照、划词加笔记、收藏、插入编辑器（现有能力）。
- 新增：显示来源/发布时间/语言/质量；"原文"按钮；"重新抽取"（从 raw 重放）。

### 8.4 状态可见

卡片/列表上显式区分 `未处理 / 处理中 / 就绪 / 失败`，用户不会再"点开发现只有摘要"。
失败项给明确原因（登录墙 / 取页失败 / 抽取为空），而不是静默展示摘要冒充正文。

---

## 9. 配置入口：Feeds 独立配置页（决策 2）

`Feeds/` 作为**新建的顶层知识库**，位置可配置；与之配套的配置集中放在设置页新开的一页。

### 9.1 设置页新增「知识采集」页

现有设置页的结构是：`SidebarList.vue`（菜单项）→ `settingsStore.selectedMenu` →
`GuaxiSettingContent.vue` 的 `switch` 映射到组件。新页按同一约定接入，不动其他页面。

- 菜单 key：`feeds`，标签「知识采集」，归入「内容与库」分组。
- 组件：`components/SettingViewComponent/FeedsSetting.vue`（新文件）。
- 现有 `FlipboardFeedManager.vue`（知识源管理独立窗口，路由 `/flipboard-feeds`）**保留**，
  作为"源清单的批量操作台"；设置页这一页负责**全局策略**。两者读同一份配置。

### 9.2 页面内容（六块）

**① 存储位置**
- `Feeds/` 根目录：默认 `KnowledgeBases/Feeds`，可改（与 `set_knowledge_bases_root` 同级设置）。
- 显示当前占用：文章数 / 图片占用 / raw 占用 / 磁盘剩余。
- 「打开目录」「重新定位」按钮。

**② 保留策略**（对应 7.5）
- 未读文章保留期：14 / 21 / 30 / 60 / 90 天 / 不清理。
- 已读文章保留期：90 / 180 / 365 天 / 永久。
- 图片与 raw：随正文删除 / 保留 N 天 / 永久。
- 磁盘水位：达到 \_\_ GB 时按"未读最旧优先"清理。
- 「立即预览将清理哪些」→ 弹出汇总，不直接执行。

**③ 抓取调度**
- 全局并发、每 host 并发、每 host 最小间隔。
- 每日总页数预算；按源权重的分配预览。
- 后台抓取总开关（现有 `scrapingEnabled` 迁到这里）。
- 抓取时间段限制（如仅在 22:00–08:00 跑，避免占带宽）——新增能力。

**④ 登录与登录墙站点**（对应 4.5）
- 每个登录墙平台一行：匿名可用性（实测标注）、登录状态、登录按钮、登录后增量说明。
- 「匿名优先」总开关（默认开）。
- RSSHub sidecar 状态：运行/未运行、路由数、cookie 注入状态、重新加载登录态。
- 明确列出**不登录也能抓的**与**必须登录的**，避免用户误以为整个平台不可用。

**⑤ 内置源与清单**
- 当前 manifest 版本、Tier 分组开关（A/B）、单项覆盖。
- `tombstone` 列表：被用户删除的内置源，可恢复。
- 新增/修改源（复用现有 `FeedSourceConfigDialog`）。

**⑥ 诊断**
- 页面类型分布、质量分分布、未读积压趋势。
- 「导出诊断报告」：最近 N 条的处置轨迹，便于判断某个源是否该留。
- 「重建索引」「全量重放（从 raw 重新抽取）」「VACUUM」。

### 9.3 配置存储

- 全局策略写 `config/feeds_settings.json`（新文件，与现有 `flipboard_feeds.json` 并列）。
- 保留策略的 `purge_after` 由后端在每条入库/打开时**预先算好**，设置变更时批量重算，
  避免每次清理都全表比对。
- 所有配置项都要有默认值且向后兼容：旧配置文件缺失字段时按默认值补齐。

---

## 10. 与其他模块的交互

| 模块 | 现有 | 改造后 |
|---|---|---|
| **知识图谱** `kgraph` | 只索引 KB 内的 `.md` | `Feeds/` 作为 KB 自动索引；frontmatter 的 `tags` → 概念节点 |
| **全文检索** `fulltext`(tantivy+jieba) | 只索引用户选定目录 | `Feeds/` 常驻索引；中英文分词已具备 |
| **语义检索** `embedding` | 已有本地 embedding 服务 | 对 `ready && quality≥80` 建向量，用于"相似文章/去重辅助" |
| **认知层** `cognition::percept` | `save_articles_to_cache` + `persist_hydrated_article` 两处调用，要求 md ≥ 400 | **只在 `state=ready` 时调用一次**，传 `title/text(纯文本)/uri/trust(按源信誉)/source_id`；去重已有内容哈希 |
| **编辑器** `editorStore.insertContent` | 拼 `content` 纯文本 + 图片单独一行 | 直接插入 `content_md` 原文（含图片/公式/代码） |
| **KB 笔记** `create_kb_note_from_topic` | 聚合多篇 `content` 拼接 | 改为"收藏集导出"：按 collection 选取 `ready` 条目，输出标准 Markdown（保留各自 frontmatter 的必要字段） |
| **训练语料导出**（新） | 无 | `export_corpus`：JSONL/Parquet，字段见下 |
| **备份/同步** `backup` / `p2psync` | 文件级 | `Feeds/` 目录天然可同步，不需要额外协议 |
| **库网站** `library_site` | 浏览 Libraries | 可选把 `Feeds/` 挂进去，手机端阅读 |

### 10.1 训练语料导出契约

```jsonl
{"id":"a3f9c1e2","url":"https://…","title":"…","text":"<Markdown 全文>",
 "lang":"en","published_at":"2017-06-12T00:00:00Z","source":"arXiv cs.AI",
 "quality":92,"tags":["transformer","attention"],"word_count":8412,
 "sha256":"…","license":null}
```

- 过滤：`collection` / `lang` / `quality≥N` / 时间窗 / 来源白名单。
- 保证：已去重（canonical 唯一）、已剥模板、`page_kind=article`、图片路径已本地化（或可选剥离）。
- 两种形态：`text` 用 Markdown（保留结构，最适合指令微调）或纯文本（最适合继续预训练）。
- `license` 字段：从 `<link rel="license">` / 站点策略推断，未知留 `null` —— 训练用途下这个字段必须存在。

---

## 11. 迁移分期

每期都可独立上线、独立回归，不做"大爆炸"式重写。

### Phase 0 — 止血（不改架构，1–2 天）

不改表结构，直接修最刺眼的：

1. 删 `generate_mock_articles` 的生产调用（`:5476`、`:5564`）。
2. `description` / `content` 入库前统一去 HTML（`html_to_text`）。
3. 过滤**前置**到 SQL：`load_cached_articles` 增加 `WHERE` 条件而非事后 `retain`。
   `topic` 过滤改在 SQL 里做（修专题检索漏项）。
4. `AdmitPolicy` 的 URL 规则表（促销/优惠券/deal）在发现阶段直接拦。
5. `cleanup` 加 `VACUUM`。
6. **知乎热榜换域名**：`www.zhihu.com/api/v3/feed/topstory/...` → `api.zhihu.com/topstory/hot-lists/total`。
   实测匿名可得 30 条带摘要，是零成本的质量提升。
7. **接入 `rsshub:/zhihu/daily`**：实测匿名返回 30 条 × ~5KB 完整正文，今天最高性价比的全文源。

预期：Wired 18 条促销页消失，卡片不再出现 HTML 标签，专题检索不再漏老文章，
知乎内容从"401 全空"变成"热榜摘要 + 日报全文"。

### Phase 1 — 状态机 + 原文留存

引入 `items` 表与 `state`，A/B 两阶段拆开；raw HTML 落盘；worker 池；修 `try_lock`。
**不改前端**（前端仍读现有字段，由兼容层映射）。

### Phase 2 — 判定与清洗

页面类型判定、模板段学习、块级清洗、Markdown 规范化、质量分、去重合并。
这一期是"质量"的主体，可以内测：把质量分与丢弃理由做成开发者面板。

### Phase 3 — Markdown 双写 + Feeds 库

写文件、frontmatter、图片本地化、`_index.json`；接入 `kgraph` / `fulltext` / `backup`。

### Phase 4 — 前端改造

列表走检索、卡片契约、阅读器收敛、状态可见、源看板 + 单源诊断、
设置页「知识采集」配置页（第 9 节）。

### Phase 5 — 下游接线

认知层单点调用、编辑器插入、语料导出、语义检索。

---

## 12. 已确认的决策

以下为 2026-11-04 用户确认后的结论，实现以本节为准（与前文建议冲突时以本节为准）。

| # | 决策 | 结论 |
|---|---|---|
| 1 | Markdown 为真源 | **双写，Markdown 为准**；但必须**同时执行保留/清理策略**，不允许囤积从未被读过的文档（见 7.5）。 |
| 2 | `Feeds/` 位置 | **新建顶层知识库**，**路径可配置**；配置集中在设置页新开的「知识采集」页（见第 9 节）。 |
| 3 | 登录墙平台 | **默认启用**，走"匿名优先 + 登录加成"；先把不登录就能抓的路径做到位（见 4.5）。 |
| 4 | 聚合索引贴（早报/周报/一图流） | 默认丢弃，另设"速览"视图可选。 |
| 5 | 分类模型 | 保留 13 个分类作为默认 Collection，允许用户自建。 |
| 6 | 内置源分级 | 分级**保留为初始配置模板**，所有登录墙平台默认启用（取代原"Tier C 默认关"）。 |
| 7 | LLM 使用范围 | 只用于摘要 + 中间带质量评判，不用于全文清洗。 |
| 8 | "重放"入口 | 暴露给用户，放在阅读器右键与源诊断里。 |
| 9 | 图片本地化 | 本地化，带容量水位告警。 |
| 10 | 现有数据 | 直接从零重建（保留收藏 URL 白名单），不做行迁移。 |

### 12.1 决策 1 的补充约束：不囤积

用户明确要求："不要到最后存储了一大堆从没看过的文档。"
这一条不是"写完之后再清理"，而是**三道闸依次收口**，任何一道都能单独拦住囤积：

**闸一：入盘前先过质量门。** 只有 `state=ready` 且 `quality >= min_quality`（默认 35）
才写 Markdown 文件。被判定为 `rejected` 的（促销页、索引页、登录墙、正文过短）
只留一条 SQLite 索引行 + 原因，**不产生文件**。光这一道就砍掉了当前 84% 的垃圾量。

**闸二：每日摄入上限。** 每个 Collection 有 `daily_admit_cap`（默认 200 篇/天，
按质量分从高到低录取，超出的直接丢弃而不是排队）。这是**唯一能真正限制总量的手段** ——
清理只能回收，不能阻止增长。

**闸三：未读 21 天回收**（7.5 的保留策略）。过了闸一闸二仍然堆积的，
由"读过 / 收藏 / 保留"三个信号决定生死。

**为什么全部落盘而不是延迟落盘**：延迟落盘会让全文检索（tantivy 索引 Feeds 目录）
看不到未落盘的文章，检索结果与列表不一致。三道闸已经把文件量控制在
"每天最多 200 篇 × 21 天 = 4200 篇" 的量级，这个规模对磁盘和索引都完全可接受。

**可观测**：配置页要直接显示"本月新增 / 已读 / 待清理"，让用户能看见囤积是否在受控范围内。
如果 `daily_admit_cap` 经常被打满，说明源的质量或数量需要调整，而不是调大上限。

---

## 13. 风险与边界

- **合规**：遵守 robots、限速、深度上限；登录态仅用于用户自己有权访问的内容。不破解验证码。
- **磁盘**：本地化图片 + raw HTML 会显著增长。需要水位告警与"只保留 ready 的 raw"策略。
- **Python 运行时**：trafilatura 依赖托管运行时。worker 池化后内存占用上升（每个进程约 80–150MB），
  `N=4` 需在低内存机器上降级为 2。
- **RSSHub 版本与路由漂移**：`/zhihu/daily`、`/zhihu/hot` 等免登录路由依赖上游实现，
  可能随 RSSHub 升级失效。需要源健康看板能显示"路由失效"，并支持快速替换路由 URL。
- **登录墙的边界**：匿名路径只覆盖公开内容。知乎问题回答、专栏全文仍需登录；
  微博/小红书匿名覆盖很窄。不要向用户承诺"全平台免登录全文"。
- **Playwright 依赖**：部分 RSSHub 路由（bilibili 等）需 Playwright 浏览器，当前安装包未带。
  默认禁用并标注原因，不为个别路由增大安装包。
- **迁移数据**：现库 120 行且 98% 是空洞，**建议直接重建**而不是迁移（保留收藏 URL 白名单）。

---

## 13.5 渲染层：Moli 作为 Chromium/Playwright 的替代（已调研）

用户建议接入 [lexmount/moli](https://github.com/lexmount/moli)。结论与落地方案：

**它是什么**：Rust 编写的 headless 浏览器（`Apache-2.0`，22.5k star，2026-11 仍在日更），
预编译二进制 **约 44MB**（对比 Chromium 150–300MB + 独立 Playwright 驱动）。
支持三种自动化协议：**CDP**、WebDriver Classic、WebDriver BiDi，并自带 `serve` 子命令起协议服务端。

**集成方式的选择（重要）**

| 方式 | 可行性 | 说明 |
|---|---|---|
| 作为 **cargo 库** `path = "../moli/..."` | ❌ 不推荐 | 整个 workspace 有约 60 个 crate，核心 `moli-core` 硬依赖 `moli-renderer-v8` → `v8 = 152.2`（rusty_v8）。等于把 V8 编译进本项目的构建，与"构建 4 分钟"和 Windows 无 NASM 的既有约束直接冲突 |
| 作为 **sidecar 二进制 + CDP** | ✅ **推荐** | 与本项目既有的 RSSHub sidecar 模式完全一致：安装包内置 `moli` 可执行文件，按需以 `moli serve --port <n>` 启动，主程序通过 CDP 驱动。零编译侵入、体积可控、失败可降级 |

用户"以库形式集成以减少资源消耗、提升访问速度"的诉求，用 **sidecar + CDP** 同样能满足：
真正省资源的是 moli 的 on-demand 渲染设计（不下载/解析无关资源），而不是链接方式。

**但有一个必须先验证的前提**：moli 声称"结构优先、按需像素"，这正好是采集场景要的
（我们要的是 DOM，不是截图）。但它对**登录态 cookie 注入**、**反检测指纹**的支持程度未知，
而这两点恰恰是本项目 `flipboard_render` 现有的核心能力（stealth 脚本、持久 profile、登录窗口）。
因此接入顺序为：

1. **P0（现在）**：不动渲染层。现有 `flipboard_render`（chromiumoxide）继续工作。
2. **P1**：增加 `RenderBackend { Chromium, Moli }` 配置项，默认仍是 Chromium。
   用 moli sidecar 跑同一批 URL，对比：正文抽取成功率、耗时、内存、以及**登录态能否复用**。
3. **P2**：仅当 moli 在"登录态 + 反检测"上不劣于现状，才把默认后端切过去。

**不建议**现在就把 moli 设为默认：渲染层是整条管线的地基，换之前必须有对比数据。
如果 moli 无法复用现有 Chrome profile 的 cookie，切过去会让所有登录墙源直接失效——
这正是方案里花最多力气搞"匿名优先"的那部分。

---

## 14. 变更记录

- 2026-11-04：初稿。
- 2026-11-04：按用户确认更新决策 1/2/3 —— 保留策略由阅读行为驱动（7.5）、
  Feeds 配置页（第 9 节）、登录墙"匿名优先"策略（4.5，附本机 RSSHub 实测）。
  Phase 0 增加两项零成本的匿名抓取修复。
- 2026-11-04：**Phase 0 已实现**（见 14.1）。补充 Moli 渲染层调研（13.5）。
- 2026-11-04：Phase 0 落地时发现两处必须修正的设计假设：
  ① `utm_source` 不能当促销特征（会误杀整个爱范儿源）；
  ② `-deal` 出现在新闻 slug 里不能直接判促销（会误杀 Marvell 收购报道）。
  两者都已用测试固化。

### 14.1 Phase 0 实现清单（已完成）

| 项 | 文件 | 说明 |
|---|---|---|
| 新增准入策略模块 | `src-tauri/src/commands/flipboard_admit.rs`（新，588 行） | URL/标题/正文三级判定、跟踪参数规范化、按源类型推导默认策略；8 个单测 |
| RSS 解析阶段前置过滤 | `flipboard.rs::fetch_rss_feed_internal` | URL 与标题在解析时就判，命中直接不入库 |
| description 去 HTML | 同上 | 卡片不再把 `<p>#欢迎关注…</p>` 原样上屏 |
| 删除 mock 生产路径 | `refresh_articles_from_feeds`、`refresh_topic_knowledge_from_feeds`、`get_articles` | 无源时返回空列表；整段 `generate_mock_articles` 已删 |
| 列表排序改为质量优先 | `ARTICLE_ORDER_CLAUSE` | 有正文的排前，其次相关性，最后时间 |
| 专题检索改到 SQL | `load_cached_articles` | 修复"先 LIMIT 再过滤"导致漏老文章 |
| 去重 URL 规范化 | `dedup_articles_by_identity` | 跨源同稿（差 utm）可合并 |
| 促销 URL 拦截 | `AdmitPolicy` | 站点级 `gallery/`、`promo-code`、`coupon` 等 |
| 清理后 VACUUM | `cleanup_old_cached_articles_if_due` | 治 98% freelist |
| 知乎热榜换域名 | `fetch_zhihu_fallback` | `www.zhihu.com/api/v3` → `api.zhihu.com`（匿名 401→200） |
| 接入 RSSHub 数据源 | `collect_from_rsshub` + `curated_feeds` | 新增 4 个匿名路由源；`SourceKind::Rsshub` |
| feeds schema v4 | `FEEDS_SCHEMA_VERSION` | 只增不删的迁移，保留用户的启停选择 |

**真实数据验证**（用现有 120 条缓存回放）：
**89 条保留 / 31 条拒绝**——拒绝的正好是 22 条促销导购 + 9 条聚合索引贴；
Marvell 收购、"Best practices" 类技术文、所有正常新闻均正确保留。

### 14.2 Phase 1 实现清单（状态机 + 原文留存 + worker 池）

| 项 | 文件 | 说明 |
|---|---|---|
| 修 worker 并发缺陷 | `code_runtime.rs::with_python_worker_for_key` | 去掉 `try_lock` 失败即重建；改为**排队等待**。旧实现让并发抽取互相踩、反复冷启动 Python |
| 抽取 worker 池 | `code_runtime.rs::extract_article_via_python` | round-robin 分派到 `min(cpu/2, 4)` 个独立 worker，抽取不再串行 |
| 条目状态机 | `src-tauri/src/commands/flipboard_items.rs`（新） | `discovered→fetched→extracted→ready` / `rejected` / `failed` / `needs_login`；指数退避、claim_work、状态统计；4 个单测 |
| items 表 | `open_article_cache_db` → `ensure_items_schema` | 与 `article_cache` 分离：items 管状态与索引，article_cache 管内容 |
| raw HTML 留存 | `save_raw_html` / `load_raw_html` | zstd 压缩、内容寻址（sha256(url+html)），同一内容只存一份 |
| 重放入口 | `replay_article_extraction` 命令 | 从本地 raw 重新抽取，不重新请求站点 |
| 管线状态统计 | `get_pipeline_stats` 命令 | 各状态条目数，供看板/诊断 |
| 发现即入库 | `record_discovery`、`record_rejection` | RSS 解析后把发现/拒收写进状态机 |
| hydrate 推进状态 | `advance_hydration_state` | 抽到正文→extracted；过质量门→ready；登录墙→needs_login |
| URL 规范化统一 | `normalized_url_key` | 去重、items 键、raw 查找共用同一套规范化，避免各算各的 |

**验证**：12 个单测全绿（4 个状态机 + 8 个准入策略）；
在真实 120 条缓存上验证 items 建表、状态流转、URL 规范化后 120 条全部唯一。
`cargo check` 零警告。

**本阶段尚未接线**（`flipboard_items` 模块已标注 `allow(dead_code)`）：
`claim_work` / `mark_failed` / `set_state` 等原语是给下一步 **refine_tick**
（后台按状态批量取件、自动补抽正文）准备的。目前 hydrate 只在"用户打开文章"时推进状态，
**没有后台批量补全**——这是"84% 文章停在摘要态"彻底解决的最后一步。

**未能验证的部分**：本机没有安装 trafilatura，所以"抽取→写 raw→重放"的端到端链路
只做了编译与状态机层面的验证，没有跑真实网页。需要你在装有托管 Python 运行时的环境里
跑一次实际抓取确认。

### 14.3 Phase 1b：块级清洗 + 后台补抽（已完成）

| 项 | 文件 | 说明 |
|---|---|---|
| 块级清洗模块 | `src-tauri/src/commands/flipboard_clean.rs`（新） | 通用噪声规则 + 跨文章模板学习 + 后缀签名；16 个单测 |
| 模板索引持久化 | 同上 + `repos/flipboard_templates/<feed>.json` | 按来源分桶，随抓取逐步学习；有上限裁剪 |
| 清洗接入抽取 | `flipboard.rs::hydrate_article_readable_content` | 抽取后清洗；若削得太狠（<200 字）自动回退，绝不清空 |
| 后台补抽 | `flipboard.rs::run_refine_tick` | 按状态机批量取件补齐正文，与抓取循环分离、节奏更慢 |
| 租约回收 | `flipboard_items::reclaim_stale_leases` | `fetched` 状态 30 分钟租约，防止进程崩溃后条目永久卡死 |
| 调度接线 | `start_feed_scheduler_loop` | 补抽每轮 8 条 / 2 并发；无活时休眠 120s；随总开关一起停 |

**清洗的三层判定**（按风险从低到高）：

1. **通用噪声**：整段就是"关联阅读：""FAQs""Back to top"这类页面标签，或含强/弱噪声词。
   强信号（"点击展开全文"）中等长度也判；弱信号（"来源：""编辑："）只在 25 字以内的段落判。
2. **跨文章模板**：某段在 ≥30% 的文章里出现（至少 3 篇）→ 判为模板。
   自动适配新源，不用硬编码。
3. **后缀签名 + 营销语汇**：整段不同但结尾相同的推销块（实测少数派每篇的
   `> <某产品>[链接]，少数派为你呈现 🚀`）。签名取末尾 8 字，且必须同时满足
   "短块（≤80 字）+ 含营销词"，三者缺一不可。

**实现过程中被真实数据纠正的两个错误**：
- 初版把 `-deal` 当促销特征 → 误杀 Marvell 收购新闻（Phase 0，已修）；
- 初版后缀签名只看"结尾相同" → 误杀碰巧同结尾的正常正文。加"营销语汇"约束后才稳。

**真实数据验证**（9 篇少数派已抽取正文回放）：正确删除了 3 类模板
（Matrix 社区声明、作者观点免责声明、产品推销引用块），
而文章里真正的"参考资料"链接列表完好保留。

**剩余缺口**：`refine_tick` 已接线，但同样受"本机无 trafilatura"限制，
未经真实网页验证其吞吐与失败退避表现。

### 14.4 Phase 2 剩余：页面类型 + 质量分 + 去重（已完成）

| 项 | 文件 | 说明 |
|---|---|---|
| 页面类型判定 | `src-tauri/src/commands/flipboard_pagekind.rs`（新，361 行） | JSON-LD / og:type / 价格+购物车 / 链接密度 / URL 路径；8 种 PageKind；10 个单测 |
| 质量分 | `src-tauri/src/commands/flipboard_quality.rs`（新，429 行） | 四项加权（完整度 40 / 干净度 30 / 元数据 15 / 标题一致 15）+ 按页面类型封顶；10 个单测 |
| 去重合并 | `src-tauri/src/commands/flipboard_dedup.rs`（新，304 行） | 64 位 simhash + 汉明距离；跨源转载合并；9 个单测 |
| 判定前置 | `hydrate_article_readable_content` | 非文章类**在抽取前**就跳过，省一次抽取开销 |
| 质量接入 | 同上 + `advance_hydration_state` | 质量分与 page_kind 都写进 items，供排序/导出筛选 |

**最关键的一次实测纠正：结构分与页面类型必须联动。**

用真实 120 条缓存跑质量分，发现 Wired 的 `Uber Eats Promo Codes`（正文 1 万+ 字、
元数据齐全、链接很少）结构分 **96**，与真正的深度文章并列第一。

原因很直接：**质量分是结构打分**，"正文长、链接少、元数据全"这些条件和
"这是不是一篇文章"完全无关——促销页的 SEO 正文恰好满足全部条件。

修法不是调权重（调权重会连真文章一起压），而是**两类结果相乘**：
页面类型判为 `product` → 质量分封顶 15；`index` → 25；`login_wall` → 5。
修完重跑同一批数据：促销页最高 15 分，top 12 全部是真实文章。

**simhash 阈值同样被实测校准过。** 初版阈值取 6，把两篇**不同的**「早报」
判成了重复——它们的栏目排版、模板句高度相似，但报道的新闻完全不同。
这类"结构模板相似"会被 simhash 放大。收紧到 4 后，只有正文几乎逐字的转载
才会合并，并加了针对该案例的回归测试。

**真实数据验证**：
- 质量分：修正后促销页 ≤15，真实文章 92–96，中位数 66。
- 去重：120 条里阈值 4 下 0 组误合并；转载场景单测覆盖。

**尚未接线**：`flipboard_dedup` 已替换旧的 `dedup_articles_by_identity`，但
**只在列表合并时生效**；跨源同文写库时仍按 `(feed_id, url_norm)` 各存一行，
没有做"合并到 canonical 行 + 记 aliases"。后者属于 Phase 3 的存储改造。

**仍未验证**：页面类型判定依赖真实页面 HTML。测试用的是手工构造的
JSON-LD / og 片段；真实站点（尤其中文站）的 JSON-LD 覆盖率、以及
`looks_like_index_page` 的链接密度阈值是否合适，需要抓一批真实页面复核。

### 14.5 Phase 3：Markdown 双写与 Feeds 库（已完成）

| 项 | 文件 | 说明 |
|---|---|---|
| Feeds 库模块 | `src-tauri/src/commands/flipboard_feeds.rs`（新，约 900 行） | 目录布局、frontmatter、slug、图片本地化、每日计数、保留策略；17 个单测（含 2 个端到端） |
| 双写接线 | `hydrate_article_readable_content` | 质量门 + 每日上限通过后写盘，并把相对路径记进 `items.doc_path` |
| items 迁移 | `flipboard_items::ensure_items_columns` | 新增 `doc_path` / `opened_at` / `open_count` / `kept` 四列，幂等补列 |
| 已读信号 | `get_article_detail` → `record_opened` | 打开即记录——保留策略唯一可信的"有人看"信号 |
| 保留回收 | `purge_expired_markdown` | 接进既有 cleanup 循环；每轮限量 200；文件删后清空 `doc_path` |
| 设置命令 | `get/save_feeds_settings`、`get_feeds_storage_stats`、`purge_expired_documents` | 供设置页读写与手动触发 |

**目录布局**（实测生成）：
```
Feeds/ai/builtin-ai-0/2017-06/20170612-Attention-Is-All-You-Need.md
Feeds/_assets/ab/<sha256>.png
```
`Feeds/` 是知识库根下的顶层目录，因此被 `kgraph::kb_name_from_path`
自动识别为独立知识库——**不需要注册**，索引与全文检索直接生效。

**三道闸的实际落点**：
- 闸一（质量门）：`try_write_markdown` 里 `quality < min_quality`（默认 35）直接返回；
- 闸二（每日上限）：`DailyAdmitCounter` 按 collection 记，默认 200 篇/天；
- 闸三（未读回收）：`select_purge_candidates` 按 `opened_at` 分流，
  未读 21 天、已读 180 天。

**设计上刻意保留的取舍**：

1. **按"读没读过"删，不按质量分删。** 质量分是启发式判断，
   按它删会误杀冷门但真正有价值的文章。这是文档 7.5 已经论证过的原则，
   实现时严格遵守。
2. **写盘幂等。** 内容相同不重写文件——否则每次抓取都会改 mtime，
   进而触发 kgraph 全量重建。
3. **路径逃逸防护。** `remove_document` 拒绝绝对路径与 `..`，
   避免清理逻辑被坏数据带出库根。
4. **空目录自动清理。** 删掉某月最后一篇后连该月目录一起删，
   不留下大量空壳目录。

**实现中被测试抓到的 bug**：图片 URL 重写最初用 `](` 做全局替换，
把普通链接 `[原文](url)` 也改成了本地图片路径，链接直接断掉。
改为显式解析 `![alt](url)` 与 `<img src>`，普通链接不动。

**尚未完成**：
- **跨源同文仍未合并为 canonical 行**。去重只在列表合并时生效，
  写库仍按 `(feed_id, url_norm)` 各存一份 Markdown 文件。要彻底解决需要
  引入 canonical id 与 aliases 表，属于独立的存储改造。
- **软件删除（撤销）还没做**。设计里写了"24 小时可撤销"，
  当前是直接 unlink，只做了空目录清理。
- **未验证**：图片本地化需要真实网络下载，本机只有单测覆盖
  （禁用途与 URL 重写），没有实盘下载过真实图片。

### 14.6 trafilatura 实测与三处缺陷修复

用户提出"如果 trafilatura 必要，就放进默认安装包"，核实后发现：

**它早就在默认列表里**（`code_runtime.rs:135`），而且安装是幂等的——
老运行时升级后也会补装缺失包。所以不需要改列表。

真正的问题是**我此前误判了环境**：我拿系统 Python 测 `import trafilatura` 失败，
就断言"本机不可用"。实际上托管运行时 `utils/python_runtime/.venv`
里装的是 **trafilatura 2.3.1**，一直可用。改用托管运行时后立刻做了真实端到端验证。

**验证结果**（真实网页 `lilianweng.github.io`）：
- Markdown 55,184 字符，HTML 61,646 字符，24 张图，作者与发布日期都抽到了
- 标题、正文结构完整，无模板噪声

**由此发现并修复了三个真实缺陷**：

1. **`html` 字段是垃圾字符串。** trafilatura 2.x 的 `bare_extraction().body`
   返回的是 lxml `_Element` 而非 `str`，`str(body)` 会得到
   `"<Element body at 0x...>"`。此前因为 `markdown` 优先渲染，这个 bug 一直没暴露；
   一旦 markdown 为空、走 HTML 兜底，用户会看到一行无意义文本。
   修法：用 `etree.tostring(body, encoding="unicode", method="html")` 序列化。
   修复后 `html` 从垃圾变成 61KB 有效 HTML。

2. **链接里嵌了重复 URL。** trafilatura 处理文档内相对链接时产出
   `[text](<https://site/(https:/site/path>)`——绝对 URL 和原始相对路径拼在一起。
   实测影响约 0.6% 的链接（168 个里 1 个），但会让链接完全失效。
   修法：在 Rust 清洗层加 `fix_nested_links`，按括号深度扫描后替换。
   真实文章上验证：1 → 0。

3. **`language` 永远为空。** trafilatura 2.x 即使页面明写 `<html lang="en-us">`
   也返回 `None`。而质量分里语言占 25 分（元数据项的一档），一直拿不到。
   修法：Rust 侧从原始 HTML 的 `<html lang>` 兜底提取。
   实现时踩到一个 panic：按字节切片 `&html[..4000]` 会切在多字节字符中间，
   中文页面必崩；改成按字符取。

**同时修正了文档之前的一个错误结论**：14.2/14.3/14.5 里写的
"本机没有 trafilatura，无法端到端验证"是错的。实际上可以验证，
上述三处缺陷正是验证了才发现的。

**仍然成立的限制**：真实抓取依赖站点可访问。本次测试中
`ruanyifeng.com` 返回 403、`arstechnica.com` 返回 405，
说明部分中文源与反爬站点仍需要渲染层或登录态配合，
这不是 trafilatura 的能力问题。

### 14.7 补齐两个缺口：回收站与 canonical 合并

**一、删除改为软删除。** 之前保留策略是直接 `unlink`，文件删了就没了。
现在改为移动到 `_trash/<时间戳>/<原相对路径>`：

- 保留原目录结构，撤销时能原样放回；
- 宽限期（默认 24h）过后才物理清除；
- 原位已有同名文件时**拒绝恢复**，不覆盖新抓的文章；
- `restore_trashed_document` / `list_trashed_documents` 两个命令供 UI 调用。

**二、跨源同文合并为 canonical。** 之前同一篇被两个源抓到时会产生两个
Markdown 文件。现在 `items` 增加 `canonical_id` / `is_canonical` /
`alias_count` / `simhash` 四列：

- 写盘前先算正文 simhash，查是否已有代表行；
- 命中则只记别名（`is_canonical=0`），**不再写第二个文件**；
- 代表行累加 `alias_count`，前端卡片显示"×N源"。

阈值沿用 `flipboard_dedup::HAMMING_THRESHOLD`（4）。

**过程中修掉的两个真 bug**：

1. DDL 里写了 `///` 注释——SQLite 不支持，**建表会直接失败**。
   这个错误只会在真实运行时暴露，单测里如果建的是新表也会同样失败，
   所以测试立刻抓到了。
2. `link_as_alias` 的第二个 UPDATE 用了 `?3` 却只传两个参数，
   运行时报 "Wrong number of parameters"。参数编号跳号是手写 SQL 的常见坑。

**新增迁移测试**：先建出不含新列的旧表，再跑迁移，验证列被补上且新功能可用。
这类测试之前是缺的——单测建的都是新表，而生产库是旧的。

### 14.8 Phase 4：前端与检索

| 项 | 文件 | 说明 |
|---|---|---|
| 检索接口 | `search_items` 命令 | 走 items 状态机，只返回 `ready` + canonical；支持 collection/语言/时间窗/排序 |
| Store 扩展 | `flipboardStore.js` | 列表默认走 `search_items`，失败回落旧接口；新增回收站/统计/设置方法 |
| 卡片契约 | `ArticleCard.vue` | 摘要强制去标签；显示 `×N源` 转载标记 |
| 阅读器收敛 | `FlipboardDetailView.vue` | 渲染**优先 Markdown**（唯一经过清洗的形态），HTML/纯文本仅兼容旧数据 |
| 重放入口 | 同上 | 新增"从本地原文重放"按钮，不重新请求站点 |
| 配置页 | `FeedsSetting.vue`（新） | 存储/保留策略/质量门/回收站四块，接入设置页「内容与库」分组 |

### 14.9 Phase 5：下游接线

- **认知层门控**：`persist_hydrated_article` 送感知前先查 `items.state`，
  只在 `ready` 时送入。查不到状态时保守放行（感知层有内容哈希去重）。
  这样促销页（质量被封顶到 15 分）不会进大脑。
- **知识库笔记**：`create_kb_note_from_topic` 改用 `clean_content_markdown`，
  保留标题层级、列表、代码块与图片，而不是压平的纯文本。
- **语料导出**：新增 `export_corpus_jsonl`，输出 JSONL（逐条样本），
  每条带 `quality`/`lang`/`license`/`sha256`，支持按 collection/语言/质量/时间窗筛选。

### 14.10 统一验证结果

| 验证项 | 结果 |
|---|---|
| `cargo test --bin guaxi` | **486 通过，0 失败** |
| `cargo check --bin guaxi` | **零警告** |
| `npm run test:frontend` | **387 通过，0 失败** |
| `npm run check:reactivity` | 409 文件通过 |
| `npm run build`（生产构建） | 成功，4m48s |
| `check:reader-chunks` | 全部通过 |
| 真实库 schema 迁移 | 通过（`GUAXI_REAL_DB` 标记测试） |
| 真实网页抽取（trafilatura） | 55KB Markdown / 61KB HTML / 24 图 |
| search_items / corpus 导出 SQL | 在真实 120 条数据上跑通 |

**仍未验证**：
- 前端配置页与回收站只在编译层验证过，**没有在真实 Tauri 窗口里点过**；
- 图片本地化的真实下载（需要网络与真实图片 URL）；
- 端到端的"抓取→清洗→落盘→检索→导出"完整链路（受限于可用站点）。

### 14.11 信息源重构：从"什么都能抓"到"只要高质量知识"

用户指出：现有源质量差——大量小众站点、新闻快讯、热搜垃圾。
要的是知识性长文与专业内容，而不是资讯。

**问题不在抓取能力，在选源标准。** 旧清单 158 个源里混着大量新闻源
（TechCrunch、Engadget、36氪快讯、The Verge），它们结构上就是
"每天几十条短消息"，与知识长文抢同一个列表。

#### 新增的两个维度

`FeedSource` 增加：

| 字段 | 含义 |
|---|---|
| `content_kind` | knowledge / news / discussion / video / mixed |
| `tier` | core（精挑）/ standard / noisy |
| `verified_at` | 实测定级时间；0 = 未验证 |
| `last_probe*` | 最近探测结果、条目数、最新条目时间 |

`ContentKind::in_default_view()` 只对 knowledge / mixed 返回 true。
列表查询默认加 `content_kind IN ('knowledge','mixed')`，
资讯与热搜不会进主列表，除非用户关掉顶栏的「只要知识」开关。

#### 一个个实测

所有 `verified=true` 的内置源都在本机实际拉取过：HTTP 200、能解析出条目、
最新条目在近两个月内。实测结果（2026-11）：

**确认可用且活跃**：Simon Willison（当日）、Aeon（当日）、Solidot（当日）、
InfoQ 中文（当日）、Martin Fowler（一周内）、少数派（当日）、
Julia Evans、Josh W Comeau、Rust/Go 官方博客、LWN、Quanta、Nautilus、
STAT News、arXiv cs.AI/cs.LG、MIT News AI、Google AI Blog、Berkeley BAIR。

**确认失效/停更，已剔除**：
- `Distill`（rss 200，但最后更新 2021-09）—— 正是"很久不更新"的典型；
- `Bret Victor`（404）、`Anthropic News`（404）。

**本机网络不可达，标为待验证且默认不启用**：
Wikipedia、Reddit、YouTube、Hacker News(hnrss)、Lobsters、The Gradient。
这些源公认高质量，但无法在这台机器上验证。内置时 `verified=false`，
`enabled=false`——用户网络可通时一键开启即可。

这个取舍是刻意的：**宁可少给几个源，也不要让用户打开应用就看到一片抓取失败。**

#### 健康检查

新增 `probe_feeds` 命令：逐个实际拉取，报告 HTTP 码、条目数、最新条目时间，
并给出人话结论（"活跃" / "45 天未更新" / "疑似停更" / "网络不可达"）。
管理器顶部有「健康检查」按钮，结果直接写回源列表的"健康"列。

#### 管理界面

表格新增「类型」与「健康」两列；顶部加内容类型筛选；
主界面顶栏加「只要知识」开关（默认开）。

#### 未完成

- 知乎 / B站 / Quora / GitHub 这几类用户点名的源，本机要么网络不可达、
  要么需要登录态，无法实测，因此**没有**作为 `verified` 内置。
  它们可以通过「高级配置 → RSSHub 网页探测」按需添加，这条路是通的。
- RSSHub radar 已有实现（`rsshub_radar_detect`），本次未改动。

### 14.12 系统代理与 Atom 支持（本轮）

用户指出：Wikipedia / Reddit / YouTube / GitHub / Quora 是高质量源，必须支持；
这些站在大陆无法直连，已配置系统代理。

**两个卡点，都是真 bug，不是配置问题。**

#### 卡点一：抓取客户端从不读代理

`HTTP_CLIENT` 构造时没有设置任何代理，所以无论用户在系统里配了什么，
抓取永远直连。于是这些源全部表现为"网络不可达"——上一轮把它们标成
"待验证"正是这个原因。

修法：复用 `mail::net::system_proxy_url()`（环境变量优先，其次 Windows
注册表 / 各平台设置），在三个地方接上：

| 位置 | 说明 |
|---|---|
| `HTTP_CLIENT` | 主抓取客户端 |
| 远程图片代理 | 图片 CDN 在同一网络下同样需要代理 |
| RSSHub sidecar | 通过 `HTTP_PROXY` 等环境变量传给 node 子进程 |

**每处都必须配 `no_proxy`**：RSSHub 在 `127.0.0.1:1200`，本地图床在
`127.0.0.1:8888`；若不加排除，代理会把"连自己"也转发出去，本地服务全废。

实测（系统代理 `127.0.0.1:10810`）：

| 站点 | 直连 | 走代理 |
|---|---|---|
| Wikipedia API | 超时 | **200 / 88KB** |
| Reddit RSS | 超时 | **200 / 32KB**（有速率限制，偶发 429） |
| GitHub | 超时 | **200 / 603KB** |
| YouTube 频道 | 超时 | **200 / 15 条** |
| Quora | 超时 | 403（站点自己的反爬，非网络问题） |

#### 卡点二：`rss` crate 不认 Atom

`Channel::read_from` 只解析 RSS 2.0，遇到 Atom 的 `<feed>` 直接报
`the input did not begin with an rss tag`。而 YouTube 频道、大量个人博客
都是 Atom 格式——**它们从来没被成功抓取过**。

修法：改用 `feed-rs`（同时支持 RSS / Atom / JSON Feed，纯 Rust，
不引 openssl / aws-lc，符合 Windows 构建约束）。两处解析点都换：
`fetch_rss_feed_internal` 与 `probe_feed_url`。

feed-rs 还顺带消除了手写的时间格式判断：它把 RSS 的 `<pubDate>` 与
Atom 的 `<published>`/`<updated>` 统一成 `DateTime<Utc>`。

#### 内置源更新

| 源 | 状态 |
|---|---|
| Wikipedia 每日精选 / 历史上的今天 | 已验证（Atom，走代理） |
| Hacker News 100+ / Lobsters | 已验证 |
| The Gradient | 已验证 |
| YouTube 3Blue1Brown / Veritasium / Computerphile / Lex Fridman | 已验证（视频类） |
| GitHub Blog / Rust releases.atom | 已验证 |
| Reddit r/programming / r/MachineLearning | 已验证（注明可能 429） |
| Quora | **未内置**：站点对匿名抓取返回 403，需要登录态；已说明可走 RSSHub 路由 |

#### UI

管理器标题旁显示「代理已启用 / 未走代理」标签，抓取失败时能立刻判断
是网络问题还是源的问题。
