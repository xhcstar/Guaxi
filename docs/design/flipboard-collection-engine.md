# Flipboard 知识收集引擎：设计与实现说明

> 工业级"任意站点深度抓取 + 高质量图文 + 反检测"收集器。本文档对应代码逐一说明，便于维护与二次开发。

## 设计说明

把原先只能收集"固定片段"（标题 + 一张图）的 Flipboard 升级为一个**收集完整信息**的工具：给定一个主页地址，定期遍历整站，抓取感兴趣的内容，清洗成网页般排版的高质量图文，并支持登录墙 / 验证码站点。目标对标 Crawl4AI / MediaCrawler / trafilatura 的能力组合。

设计原则：

1. **分层可替换**：抓取（爬虫）/ 渲染（反检测）/ 抽取（清洗）/ 相关性 & LLM 增强 / 笔记收藏，各层独立。
2. **复用既有基础设施**：HTTP 客户端、SQLite 缓存、调度器、管理化 Python 运行时、硅基流动 LLM 通道、Tauri WebviewWindow，全部复用，不另起炉灶。
3. **务实降级**：每一层失败都有兜底（trafilatura→scraper、Chrome→HTTP、LLM→本地打分），保证链路不中断。
4. **MediaCrawler 式思路**：对签名加密 / 登录墙平台，不去逆向 API 签名，而是**驱动登录后的真实页面、从渲染后的 DOM 抽取内容**。

### 分层架构

```
FeedSource(source_type="site_crawl", interest_profile, crawl_cfg, render_profile)
        │  调度器 run_feed_scheduler_tick（复用，每 tick 给每源一个抓取预算）
        ▼
[1] 爬虫层   flipboard.rs::crawl_tick_for_feed / process_crawl_item
    robots/sitemap + 同域 BFS + SQLite crawl_queue 持久化队列 + 并发
        │ 候选 URL
        ▼
[2] 渲染层   flipboard_render.rs::fetch_page
    HTTP 优先 → 空壳/JS站/登录墙升级无头 Chrome(chromiumoxide)
    反检测 stealth + 人类行为 + 持久登录会话 + 登录墙检测
        │ 完整 HTML
        ▼
[3] 抽取层   flipboard_extract.rs::extract
    trafilatura(Python 金标准) 主 + scraper(Rust) 降级，保留内联图片
        │ ExtractedArticle
        ▼
[4] 相关性/LLM  flipboard.rs::local_relevance_score + flipboard_llm.rs::enhance
    关键词预筛 + 本地打分 + 可选 LLM(相关度/摘要/标签/清洗) + 阈值过滤
        ▼
   SQLite article_cache（含 clean_content_html/markdown/图片/relevance/tags/summary）
        ▼
[5] 前端  阅读器 + 笔记/收集/收藏夹 + 插入知识库
```

---

## 1. 数据模型与存储

**文件**：`src-tauri/src/commands/flipboard.rs`

- `Article`（结构体，约 595 行）：在原 12 字段基础上新增 `clean_content_html` / `clean_content_markdown` / `lead_image` / `content_images` / `relevance_score`(-1=未评分) / `tags` / `summary` / `render_mode`，均 `#[serde(default)]`，并实现 `Default`。`content` 保留为纯文本镜像（搜索 / KB 导出 / 旧行兼容）。
- `FeedSource`：新增 `interest_profile` / `relevance_threshold` / `crawl_cfg`(JSON) / `render_profile` / `llm_enhance`，同样带 `Default`。`FeedSourceInput` 对应新增可选字段。
- **SQLite 迁移**：`open_article_cache_db()` 内建 `article_cache` 与 `crawl_queue` 两张表；`ensure_article_columns()` 用 `PRAGMA table_info` 探测后逐列 `ALTER TABLE ADD COLUMN`（新列只追加末尾，避免 `row_to_article` 按位读取错位）。
- `save_articles_to_cache()` / `row_to_article()`：与新列同步；`content_images`/`tags` 以 JSON TEXT 存取。
- `persist_hydrated_article()`：按 `url` UPDATE 把深解析后的完整内容回写所有匹配缓存行 —— 实现"二次打开秒开"且卡片列表后续直接有完整图文。

---

## 2. 爬虫层（深度遍历 + 持久化队列）

**文件**：`src-tauri/src/commands/flipboard.rs`

- `crawl_queue` 表：`(feed_id, url, depth, status, kind, discovered_at, fetched_at, error)`，`PRIMARY KEY(feed_id,url)` + `INSERT OR IGNORE` 天然去重，持久化 → 重启可续爬。
- `CrawlConfig`（默认）：`max_depth=3`、`max_pages_per_tick=16`、`max_queue=2000`、`rate_delay_ms=400`、`respect_robots=true`，可由 `feed.crawl_cfg`(JSON) 覆盖（`CrawlConfig::from_feed`）。
- 队列操作：`seed_crawl_queue` / `claim_pending`（事务内标记 fetching 防并发重复）/ `enqueue_links`（同域过滤 + `is_noise_crawl_path` 噪声过滤 + `normalize_crawl_url` 去 fragment/utm + 深度&队列上限）/ `mark_crawl_done` / `mark_crawl_error` / `read_crawl_queue_stats`。
- 发现：`expand_from_sitemap`（读 `/robots.txt` 的 `Sitemap:` → `/sitemap.xml`，`collect_sitemap_locs` 递归一层 sitemap-index）。**注意**：`rusqlite::Connection` 是 `!Sync`，绝不能跨 `.await` 持有，所以 sitemap 收集返回 URL 列表后再开连接入队。
- `classify_crawl_url`：按 slug/数字 id/路径深度启发式判定"文章页 vs 导航页"，导航页只扩链接不抽正文。
- 调度接入：`collect_articles_for_feed` 对 `source_type=="site_crawl"` 走 `crawl_tick_for_feed`。
- **并发**：`crawl_tick_for_feed` 用 `futures_util::StreamExt::buffer_unordered(CRAWL_PAGE_CONCURRENCY=4)` 并行跑 `process_crawl_item`（每项独立处理、自行标记错误、返回 `Option<Article>`，不跨 await 持有连接）。
- 命令：`start_site_crawl` / `get_crawl_progress` / `reset_crawl_queue`（注册于 `main.rs`）。

---

## 3. 渲染层（反检测，本次重点）

**文件**：`src-tauri/src/commands/flipboard_render.rs`

引擎为 **chromiumoxide**（真无头 Chrome / CDP），首次用 `BrowserFetcher` 自动下载匹配版 Chromium 到 exe 同级 `chromium/`，或优先复用系统 Chrome/Edge（`find_system_chrome`）。共享 Browser 实例（`BROWSER: OnceCell<Arc<Mutex<Option<BrowserHandle>>>>`），按 profile 复用 / 切换重启。

参考 **undetected-chromedriver / puppeteer-extra-stealth / Crawl4AI / MediaCrawler** 的反检测能力，具体落点：

### 3.1 指纹规避 stealth 脚本 —— `STEALTH_SCRIPT`（约 74 行起）
导航前用 `page.add_init_script(STEALTH_SCRIPT)` 注入（映射 CDP `Page.addScriptToEvaluateOnNewDocument`，对每个新 document 生效）。覆盖：
- `navigator.webdriver` → false（并尝试从原型删除）
- `navigator.languages` / `plugins` / `mimeTypes`（真实 PDF 阅读器集合）
- 完整 `window.chrome`（runtime/app/csi/loadTimes）
- `permissions.query`（notifications 一致化）
- 硬件指纹：`hardwareConcurrency` / `deviceMemory` / `platform` / `maxTouchPoints` / `vendor`
- `navigator.userAgentData`（Client Hints，与 UA 版本一致，含 `getHighEntropyValues`）
- WebGL `getParameter` vendor/renderer 伪装（37445/37446）
- 移除 `cdc_` ChromeDriver 注入痕迹
- headless 下 `outerWidth/Height` 为 0 的修正
- `navigator.connection`

### 3.2 反自动化启动旗标 —— `launch_browser`（约 218 行起）
`--disable-blink-features=AutomationControlled`、`--disable-features=...AutomationControlled,Translate...`、`--disable-infobars`、`--disable-popup-blocking`、`--disable-notifications`、`--no-service-autorun`、`--password-store=basic`、`--use-mock-keychain`、现代 Chrome UA（不含 "HeadlessChrome"）、`--lang=zh-CN`、窗口 1366×900 等。可选代理 `render_proxy()`（环境变量 `GUAXI_RENDER_PROXY` / `GUAXI_HTTP_PROXY` → `--proxy-server`）。

### 3.3 人类行为模拟 —— `render_with_chrome`（约 300 行起）
渲染时执行一段 JS：随机步长滚动（280–900px）、随机间隔（120–360ms）、随机坐标 `mousemove` 派发，最后回滚顶部 + 700ms settle，触发懒加载并降低风控命中率。

### 3.4 登录会话持久化（MediaCrawler 式）
- 抓取与登录**共用同一 `user_data_dir`**（`chrome_profile_dir(profile)`），profile 一般取 `feed.id`。
- `open_login_window`：以**可见**窗口打开登录页，用户扫码 / 输密码 / 过验证码后，cookie 落盘到 profile。
- **关键修复**：Chrome 不允许两个实例同时占用同一 profile。`open_login_window` 登录前后各调一次 `reset_shared_browser()`：前者释放共享无头实例对该 profile 的占用，后者清空共享句柄，使下次抓取以**登录后的最新 cookie** 重新启动。这是登录站点能被抓到的核心。
- 命令：`open_login_session` / `ensure_chromium_ready`（`main.rs` 注册）；前端 FeedManager 每行"登录"按钮触发。

### 3.5 登录墙 / 验证码检测 —— `looks_like_block_page` + `RenderedPage.blocked`
渲染后扫描页面是否含 captcha / cloudflare / 滑块验证 / 请登录 / 扫码登录 等中英标记，置 `RenderedPage.blocked=true`。`process_crawl_item` 命中后 `mark_crawl_error` + `record_feed_health_failure("blocked", ...)`，前端"抓取进度/健康"即提示用户为该源登录。

### 3.6 混合抓取与熔断 —— `fetch_page`
HTTP 优先（`reqwest`）；命中已知平台（`host_needs_render`，含知乎/小红书/抖音/快手/B站/公众号/微博/贴吧/Twitter/Instagram 等 MediaCrawler 目标）或 `body_is_thin`（去标签可见文本 < 400 字 / SPA 空壳标记）则升级 Chrome 渲染；渲染失败回退 HTTP body。Chrome 连续启动失败达阈值触发熔断（`HEADLESS_DISABLED`），本会话干净降级到 HTTP，避免反复崩溃刷屏。

---

## 4. 抽取层（完整图文）

**文件**：`src-tauri/src/commands/flipboard_extract.rs`、`src-tauri/scripts/guaxi_python_worker.py`

- 主引擎 **trafilatura**（正文抽取学术金标准）：经常驻 Python worker 的 `extract_article` handler（`code_runtime::extract_article_via_python`），输出 Markdown + HTML + 元数据 + 图片列表。trafilatura 已加入 `code_runtime.rs::PYTHON_RUNTIME_RECOMMENDED_PACKAGES` 默认安装；缺失时 `maybe_install_trafilatura_once` 后台 pip 安装一次，本次临时降级。
- 降级引擎 **scraper(Rust)**：选取正文容器，**保留 `<img>` 内联**（修复老版本把图片降级成"图片资源链接：- url"文本列表的根因），URL 绝对化、噪声兜底清除。
- `markdown_to_html`：用既有 comrak 把 markdown 渲染为带内联图片的 HTML。
- 调用方：`hydrate_article_readable_content` 走 `flipboard_render::fetch_page`(HTTP→Chrome) 拿 HTML 后抽取；`get_article_detail(id, url, force)` 触发并 `persist_hydrated_article` 回写。

---

## 5. 相关性与 LLM 增强

**文件**：`src-tauri/src/commands/flipboard.rs`、`src-tauri/src/commands/flipboard_llm.rs`

- 关键词预筛：`filter_articles_by_keywords`（include/exclude）。
- 本地相关性：`local_relevance_score`（兴趣描述词元命中覆盖率 + 加权强度 → 0-100），`apply_relevance` 按 `relevance_threshold` 过滤。
- LLM 增强（可选，`feed.llm_enhance`）：`flipboard_llm::enhance` 单次调用硅基流动(DeepSeek/Qwen) 返回 `{relevance, summary, tags, cleaned}`，预算护栏 `LLM_CALLS`（上限 2000）；空 key 静默跳过。LLM 给分覆盖本地分，未给则回退本地。
- 后台预热（"两者结合"）：调度 tick 末 `preheat_thin_articles` 对非整站源、内容薄、相关性高的 ≤2 篇/源 提前深解析并落库。

---

## 6. 前端（阅读器 + 笔记/收集/收藏夹 + 知识库）

**文件**：`src/stores/flipboardStore.js`、`src/stores/flipboardCollectionsStore.js`、`src/components/SearchViewComponent/*.vue`

- 阅读器 `FlipboardDetailView.vue`：`formattedContent` 优先渲染 `clean_content_html`（内联图片 + 表格/引用/代码样式）；relevance/tags/summary chip；"重新深解析"（`store.reparseArticle` → `get_article_detail(force)`）；"在内置浏览器打开原网页"（`open_article_in_browser` 用 Tauri WebviewWindow External 打开真实页面）。
- 笔记 `SelectionNotesPanel.vue`：选中文字 → 卡片，按文章存 `Fliprepo/flipboard-notes/<id>.json`，支持手动加卡片。
- 收集/收藏夹 `CollectionsPanel.vue` + `flipboardCollectionsStore.js`：`index.json` 聚合全部笔记卡片；新增 `favorite` 字段 + `toggleFavorite` + `favoriteEntries`，"全部笔记 / 收藏夹"分段切换 + 每卡星标；`syncArticleNotes` 按 entry id 保留收藏标记（重编辑笔记不丢收藏）。整篇文章收藏（`flipboard_bookmarks`）保持独立，文案改"收藏文章"。
- 插入知识库：CollectionsPanel"插入文档"经 `editorStore.insertContent` 写入主界面打开的知识库文档；`search_articles` 检索覆盖 clean 内容/摘要/标签。
- 知识源管理 `FlipboardFeedManager.vue`：整站抓取对话框、每行登录/重爬、抓取进度列（6s 轮询）。

---

## 7. 新增依赖

- Rust：`chromiumoxide 0.9`（features `fetcher, native-tls, zip8`；用 native-tls 避开 rustls→aws-lc 的 cmake 依赖）、`texting_robots 0.2`。
- Python：`trafilatura`（经管理化运行时 pip 安装，不进 Cargo）。

---

## 8. 反检测的边界与对策（务实）

- **CDP 层指纹**：chromiumoxide 基于 CDP（`Runtime.enable` 等），极少数高级检测器可识别 CDP 接入；本方案覆盖的是绝大多数**指纹型**检测。若遇此类站点，建议用 `GUAXI_CHROME_PATH` 指向系统 Chrome + 登录会话。
- **TLS/JA3 指纹**：HTTP-first 路径用 `reqwest`，TLS 指纹非真实 Chrome；对反爬严格站点应走 Chrome 渲染路径（`host_needs_render` 命中或 `force_render`），其 TLS 即真实 Chrome。
- **验证码**：不自动破解。命中即 `blocked` 上报，由用户在可见登录窗口人工完成（扫码/滑块），会话持久化后续复用。
- **IP 风控**：支持 `GUAXI_RENDER_PROXY` 代理；大规模抓取建议配合住宅代理轮换。
- **合规**：遵守 robots、限速（`rate_delay_ms`）、深度/队列上限；登录态仅用于用户自己有权访问的内容。

---

## 9. 端到端验证

1. 重置/首启运行时 → 确认 trafilatura 自动装上；打开一篇文章 → 完整图文（非标题+一图）；二次打开秒开。
2. 整站抓取一个静态站 → 进度递增、卡片涌入、图片正常；再加一个 JS 站 → `render_mode` 含 `chrome`。
3. 对登录站点点"登录"完成登录（可见窗口）→ 重爬 → 正文非空；未登录时进度/健康提示 `blocked`。
4. 选中文字加笔记 → "全部笔记"出现；点 ☆ → "收藏夹"出现；重新编辑该文章笔记后收藏仍在。
5. 详情"在内置浏览器打开原网页"可打开真实页面；"插入文档"把笔记写进主界面知识库文档。

---

## 关键文件索引

| 关注点 | 文件 |
|---|---|
| 模型/迁移/爬虫/调度/相关性/命令 | `src-tauri/src/commands/flipboard.rs` |
| 反检测渲染（本次重点） | `src-tauri/src/commands/flipboard_render.rs` |
| 正文抽取 | `src-tauri/src/commands/flipboard_extract.rs` |
| LLM 增强 | `src-tauri/src/commands/flipboard_llm.rs` |
| Python worker / 运行时 | `src-tauri/scripts/guaxi_python_worker.py`、`src-tauri/src/commands/code_runtime.rs` |
| 命令注册 / 调度启动 | `src-tauri/src/main.rs` |
| 前端阅读器/收集/收藏夹/源管理 | `src/components/SearchViewComponent/*.vue`、`src/stores/flipboard*Store.js` |
