//! 内容准入策略（AdmitPolicy）：决定"哪条内容值得进入采集管线"。
//!
//! 设计原则：**判定先于抓取/抽取**。能靠 URL、标题、长度判掉的东西，
//! 就不要先抓回来、存下来、再在展示时过滤掉 —— 那是拿带宽和磁盘换垃圾。
//!
//! 本模块只做纯函数判定，不碰网络与数据库，便于单测与后续规则热更新。
//! 规则来源：
//! - 站点类型推导的默认策略（见 `AdmitPolicy::for_source_kind`）
//! - 用户对单个源的覆盖（见 `FeedSource` 的相关字段）
//!
//! 与既有 `is_globally_muted` 的关系：后者是"内容级"兜底，本模块是"来源级"前置。
//! 两者互补，`is_globally_muted` 继续保留用于抽取后的二次校验。

use serde::{Deserialize, Serialize};

/// 拒收原因。落库用于诊断"这个源为什么没产出"，也是看板的核心数据。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    /// 促销/优惠券/折扣页
    Promo,
    /// 列表/聚合/索引页（早报、周报、盘点、精选合集）
    IndexPage,
    /// 商品页
    Product,
    /// 登录/验证墙
    LoginWall,
    /// 正文过短，不像一篇文章
    TooShort,
    /// 语言不在允许范围
    Language,
    /// 站点/路径级黑名单（tag、category、search、静态资源等）
    NoisePath,
    /// 标题命中噪声模式
    NoiseTitle,
    /// 全局静音词（用户配置）
    Muted,
}

impl RejectReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            RejectReason::Promo => "promo",
            RejectReason::IndexPage => "index_page",
            RejectReason::Product => "product",
            RejectReason::LoginWall => "login_wall",
            RejectReason::TooShort => "too_short",
            RejectReason::Language => "language",
            RejectReason::NoisePath => "noise_path",
            RejectReason::NoiseTitle => "noise_title",
            RejectReason::Muted => "muted",
        }
    }
}

/// 准入结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Accept,
    Reject(RejectReason),
}

impl Verdict {
    pub fn is_accept(&self) -> bool {
        matches!(self, Verdict::Accept)
    }
}

/// 单个源的准入策略。字段全部有默认值，`serde(default)` 保证旧配置兼容。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct AdmitPolicy {
    /// 允许的页面类型；空表示"不按类型限制"（由调用方自行判断）。
    pub allow_kinds: Vec<String>,
    /// URL 命中即拒（子串匹配，小写比对）。
    pub deny_url_patterns: Vec<String>,
    /// 标题命中即拒（正则，小写比对）。
    pub deny_title_patterns: Vec<String>,
    /// 去标签后的最小正文字符数；0 表示不限制。
    pub min_text_chars: usize,
    /// 允许的语言前缀（如 ["zh", "en"]）；空表示不限制。
    pub lang_allow: Vec<String>,
    /// 是否要求 URL 里带日期（用于识别文章页，多数源不必开）。
    pub require_dated_url: bool,
}

impl Default for AdmitPolicy {
    fn default() -> Self {
        AdmitPolicy {
            allow_kinds: vec!["article".to_string()],
            deny_url_patterns: Vec::new(),
            deny_title_patterns: Vec::new(),
            min_text_chars: 400,
            lang_allow: Vec::new(),
            require_dated_url: false,
        }
    }
}

// ===== 内置规则表 =====

/// 通用促销/电商 URL 片段。这些在大站（Wired/TechCrunch 等）会混进 RSS，
/// 是当前缓存里 36% 垃圾的直接来源。
///
/// 只放**无歧义**的词。像 `-deal` / `-sale` 这类在正常新闻里也会出现
/// （实测："In one of the largest deals in tech history, Marvell buys Lumentum"），
/// 放在 URL 规则里会误杀真实新闻，因此改由标题规则配合判定（见 `judge_item`）。
const PROMO_URL_PATTERNS: &[&str] = &[
    "promo-code",
    "promocode",
    "promo_codes",
    "coupon",
    "voucher",
    "/deals/",
    "/shop/",
    "/store/",
    "/product/",
    "/products/",
    "/cart",
    "/checkout",
];

/// 跟踪参数：**不是**促销信号，而是应当在规范化时剥离的东西。
/// 实测：爱范儿等源给每条 RSS 链接都带 `utm_source=rss`，
/// 若把它当促销特征会把整个源的正常文章全部误杀。
const TRACKING_QUERY_KEYS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "fbclid",
    "gclid",
    "spm",
    "from",
    "ref",
];

/// 去掉跟踪参数与 fragment，得到用于去重/判定的规范化 URL。
/// 只动 query 与 fragment，不改路径与主机。
pub fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let Ok(mut url) = url::Url::parse(trimmed) else {
        return trimmed.to_string();
    };
    url.set_fragment(None);
    if url.query().is_some() {
        let kept: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| {
                let k = k.to_lowercase();
                !TRACKING_QUERY_KEYS.iter().any(|t| k == *t || k.starts_with(&format!("{}_", t)))
            })
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if kept.is_empty() {
            url.set_query(None);
        } else {
            url.query_pairs_mut().clear().extend_pairs(kept.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        }
    }
    url.to_string()
}

/// 标题里的促销特征。比 URL 精确得多：真实新闻不会写"Save $25""20% Off"。
/// 每条都要求**购买意图**，而不是泛泛的 deal/sale 字样。
const PROMO_TITLE_PATTERNS: &[&str] = &[
    r"(?i)\bpromo\s*codes?\b",
    r"(?i)\bcoupon\s*codes?\b",
    r"(?i)\bvoucher\b",
    r"(?i)\d+\s*%\s*off\b",
    r"(?i)\bsave\s*\$?\s*\d+",
    r"(?i)\b(best|top)\b.{0,40}\bdeals?\b",
    r"(?i)\bdeals?\s*:\s*save",
    r"(?i)\b(deal|sale)\s+(of the day|ends|today)\b",
    r"(?i)\b(black friday|cyber monday)\b",
    // —— 以下针对实测仍漏网的 Wired 类导购 ——
    // "Bose QC Headphones (2nd Gen) Sale: Lowest Price Ahead of Black Friday"
    r"(?i)\bsale\s*:\s*(lowest|best)\s+price",
    // "Google Pixel Watch 5 Sale: Its Lowest Price Yet"
    r"(?i)\bsale\s*:\s*its\s+lowest\s+price",
    // "7 Best Travel Mugs (2026): Insulated…" 这类"Best X"榜单，
    // 只对"购物类名词"生效，避免误伤 "Best practices for…" 这种技术文。
    r"(?i)^\s*\d+\s+best\s+(space\s+heaters?|travel\s+mugs?|alexa\s+speakers?|chromebooks?|sunrise\s+alarm\s+clocks?|gifts?|headphones?|earbuds?|laptops?|monitors?|keyboards?|mice|webcams?|routers?)\b",
];

/// 可在 URL 里出现的"半歧义"促销词：必须同时满足标题也有购买意图才拒。
const AMBIGUOUS_PROMO_URL: &[&str] = &["-deal", "-deals", "-sale", "-sales", "-discount"];

/// 标题里的聚合/索引页特征。这些是"索引"不是"内容"，应单独走速览视图。
const INDEX_TITLE_PATTERNS: &[&str] = &[
    "早报",
    "晚报",
    "周报",
    "日报",
    "月报",
    "派早报",
    "派晚报",
    "本周",
    "本月",
    "本周看什么",
    "盘点",
    "精选合集",
    "一图流",
    "值得关注",
    "值得一看",
    "新闻速递",
];

/// 纯导航/功能性路径，永远不是文章。
const NOISE_PATH_PREFIXES: &[&str] = &[
    "/tag/",
    "/tags/",
    "/category/",
    "/categories/",
    "/search",
    "/login",
    "/signin",
    "/signup",
    "/register",
    "/account",
    "/wp-admin",
    "/wp-login",
    "/author/",
    "/page/",
    "/feed",
    "/rss",
];

/// 静态资源后缀。
const NOISE_PATH_SUFFIXES: &[&str] = &[
    ".css", ".js", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico", ".pdf", ".zip", ".mp4",
    ".xml",
];

// ===== 判定入口 =====

/// URL 级判定：在**抓取之前**调用，命中即不抓。
/// 判定前先规范化（去跟踪参数），避免 `?utm_source=rss` 之类的杂质影响结论。
pub fn judge_url(url: &str, policy: &AdmitPolicy) -> Verdict {
    let normalized = normalize_url(url);
    let u = if normalized.is_empty() {
        url.trim().to_lowercase()
    } else {
        normalized.to_lowercase()
    };
    if u.is_empty() {
        return Verdict::Reject(RejectReason::NoisePath);
    }

    // 用户自定义拒绝规则优先。
    for p in &policy.deny_url_patterns {
        let p = p.trim().to_lowercase();
        if !p.is_empty() && u.contains(&p) {
            return Verdict::Reject(RejectReason::Promo);
        }
    }

    // 路径噪声：只看 path 部分，避免 query 里的 "feed" 误判。
    if let Some(path) = u.split(['?', '#']).next() {
        if NOISE_PATH_PREFIXES.iter().any(|p| path.contains(p)) {
            return Verdict::Reject(RejectReason::NoisePath);
        }
        if NOISE_PATH_SUFFIXES.iter().any(|s| path.ends_with(s)) {
            return Verdict::Reject(RejectReason::NoisePath);
        }
    }

    // 促销页。
    if PROMO_URL_PATTERNS.iter().any(|p| u.contains(p)) {
        return Verdict::Reject(RejectReason::Promo);
    }
    // 注意：`-deal`/`-sale` 这类半歧义词**不在这里单独判**——正常新闻也会命中
    // （"the largest deals in tech history"）。它们由 `judge_item` 结合标题判定。

    if policy.require_dated_url && !looks_dated(&u) {
        return Verdict::Reject(RejectReason::NoisePath);
    }

    Verdict::Accept
}

/// 标题+摘要级判定（不传 URL 的便捷形式）。生产路径用 [`judge_item_with_url`]，
/// 因为它能额外处理"半歧义促销 URL + 购买意图标题"的组合判定。
#[cfg(test)]
pub fn judge_item(title: &str, summary: &str, policy: &AdmitPolicy) -> Verdict {
    judge_item_with_url(title, summary, None, policy)
}

/// 带 URL 的标题判定。规则顺序即优先级：
/// 1. 用户自定义标题规则
/// 2. 促销标题（真正的促销文案，与 URL 无关也拒）
/// 3. 半歧义 URL 词 + 购买意图标题 → 促销
/// 4. 聚合/索引贴
pub fn judge_item_with_url(
    title: &str,
    summary: &str,
    url: Option<&str>,
    policy: &AdmitPolicy,
) -> Verdict {
    let t = title.trim();
    if t.is_empty() {
        return Verdict::Reject(RejectReason::NoiseTitle);
    }

    // 用户自定义标题模式。
    for p in &policy.deny_title_patterns {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        if let Ok(re) = regex::Regex::new(&format!("(?i){}", p)) {
            if re.is_match(t) {
                return Verdict::Reject(RejectReason::NoiseTitle);
            }
        }
    }

    // 强促销标题：文案本身就是导购，无论 URL 长什么样都不该进阅读流。
    if PROMO_TITLE_PATTERNS
        .iter()
        .any(|p| regex::Regex::new(p).map(|re| re.is_match(t)).unwrap_or(false))
    {
        return Verdict::Reject(RejectReason::Promo);
    }

    // 半歧义 URL 词（如 `/story/dyson-vacuum-deals-nov-2026/`）：
    // 只有当标题同时带购买意图（价格/折扣/省钱）时才判促销。
    // 这样 "largest deals in tech history"（真实新闻）不会被误杀。
    if let Some(u) = url {
        let l = u.to_lowercase();
        let has_ambiguous = AMBIGUOUS_PROMO_URL.iter().any(|s| l.contains(s));
        if has_ambiguous && title_has_commerce_intent(t) {
            return Verdict::Reject(RejectReason::Promo);
        }
    }

    // 聚合/索引贴。只在标题命中时判，正文里出现这些词不算。
    if INDEX_TITLE_PATTERNS.iter().any(|p| t.contains(p)) {
        return Verdict::Reject(RejectReason::IndexPage);
    }

    let _ = summary; // 摘要暂不参与判定，保留参数以便后续扩展
    Verdict::Accept
}

/// 标题是否带"在卖东西"的信号。只给半歧义 URL 规则做二次确认用，所以宁可漏也不要误伤。
///
/// 关键区别：**新闻会提到金额，但不会写折扣话术**。
/// - 促销标题："Best Dyson Vacuum Deals: Save $200"、"20% Off Brooks"、"Chewy Coupons: $20 Off"
/// - 真实新闻："In one of the largest deals in tech history, Marvell buys Lumentum for $11.5B"
///
/// 因此要求出现**明确的折扣表达**（off/save/coupon/discount/percent），
/// 单独的 `$`、`deal`、`buy` 都不算——它们在新闻里太常见。
fn title_has_commerce_intent(title: &str) -> bool {
    let t = title.to_lowercase();
    const DISCOUNT_WORDS: &[&str] = &[
        " off", "save $", "save up", "% off", "coupon", "promo", "discount", "voucher",
        "clearance", "markdown", "price drop",
    ];
    if DISCOUNT_WORDS.iter().any(|w| t.contains(w)) {
        return true;
    }
    // "20% off" 的紧凑写法（百分号紧跟 off）。
    if let Ok(re) = regex::Regex::new(r"\d+\s*%") {
        if re.is_match(&t) && t.contains("off") {
            return true;
        }
    }
    false
}

/// 正文级判定：抽取之后调用，判断"这算不算一篇文章"。
/// `text_chars` 是去标签后的有效字符数（调用方可用 `strip_html_len`）。
pub fn judge_content(text_chars: usize, lang: Option<&str>, policy: &AdmitPolicy) -> Verdict {
    if policy.min_text_chars > 0 && text_chars < policy.min_text_chars {
        return Verdict::Reject(RejectReason::TooShort);
    }
    if !policy.lang_allow.is_empty() {
        let l = lang.unwrap_or("").trim().to_lowercase();
        let ok = policy
            .lang_allow
            .iter()
            .any(|a| l.starts_with(a.trim().to_lowercase().as_str()));
        if !ok {
            return Verdict::Reject(RejectReason::Language);
        }
    }
    Verdict::Accept
}

/// URL 是否带日期（`/2026/11/04/`、`/2026-11-04-`、`20261104`）。
fn looks_dated(u: &str) -> bool {
    static RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(20\d{2})[/-]?(0[1-9]|1[0-2])[/-]?(0[1-9]|[12]\d|3[01])").unwrap()
    });
    RE.is_match(u)
}

/// 按源类型给出默认策略。站点类型决定了它会不会混促销页、要不要更严。
pub fn default_policy_for(source_type: &str, url: &str) -> AdmitPolicy {
    let st = source_type.trim().to_lowercase();
    let host = url.to_lowercase();

    let mut policy = AdmitPolicy::default();

    // 会混电商内容的科技媒体，必须显式拒绝促销 URL（实测 Wired 36% 是促销页）。
    if host.contains("wired.com")
        || host.contains("techcrunch.com")
        || host.contains("theverge.com")
        || host.contains("engadget.com")
        || host.contains("arstechnica.com")
    {
        policy.deny_url_patterns = vec![
            "promo-code".into(),
            "coupon".into(),
            "/deals/".into(),
            "/shop/".into(),
            "/store/".into(),
            "gallery/".into(),
        ];
        // 这类站"评测/导购"也常混进去，要求正文更实在。
        policy.min_text_chars = 600;
    }

    // 学术源：正文短不等于垃圾，但要求确实有内容。
    if host.contains("arxiv.org") {
        policy.min_text_chars = 300;
        policy.deny_title_patterns = vec!["^\\s*$".into()];
    }

    // 问答站：单条回答可能很短，放宽下限。
    if host.contains("stackexchange.com") || host.contains("stackoverflow.com") {
        policy.min_text_chars = 200;
    }

    // 整站抓取：用更严的正文门槛，因为发现阶段没有任何人工筛选。
    if st == "site_crawl" {
        policy.min_text_chars = policy.min_text_chars.max(600);
    }

    // RSSHub 路由：多为聚合流（热榜/快讯），单条可能很短；但也不该放进来一屏标题。
    // 对知乎日报这种正文级路由，正文门槛反而要够高才算"一篇文章"。
    if st == "rsshub" {
        if host.contains("zhihu/daily") {
            policy.min_text_chars = 500;
        } else if host.contains("newsflashes") || host.contains("hot") {
            // 快讯/热榜本身就是短条目，门前降到 150；这些走"速览"而非深度阅读。
            policy.min_text_chars = 150;
        } else {
            policy.min_text_chars = 300;
        }
    }

    policy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_tracking_params() {
        assert_eq!(
            normalize_url("https://www.ifanr.com/1686623?utm_source=rss&utm_medium=rss&utm_campaign="),
            "https://www.ifanr.com/1686623"
        );
        // 非跟踪参数要保留（可能是文章身份的一部分）。
        assert!(normalize_url("https://x.com/a?id=123&utm_source=rss")
            .ends_with("/a?id=123"));
        assert_eq!(normalize_url("https://x.com/a#frag"), "https://x.com/a");
    }

    #[test]
    fn tracking_params_are_not_promo() {
        // 回归：一旦把 utm_source 当促销特征，整个爱范儿源都会被误杀。
        let p = default_policy_for("rss", "https://www.ifanr.com/feed");
        assert!(judge_url("https://www.ifanr.com/1686623?utm_source=rss", &p).is_accept());
    }

    #[test]
    fn rejects_wired_promo_urls() {
        let p = default_policy_for("rss", "https://www.wired.com/feed/rss");
        // 无歧义的促销 URL：只看 URL 就能拒。
        for u in [
            "https://www.wired.com/story/uber-eats-promo-code/",
            "https://www.wired.com/story/hotels-com-coupon/",
        ] {
            assert!(
                matches!(judge_url(u, &p), Verdict::Reject(RejectReason::Promo)),
                "should reject {}",
                u
            );
        }
        // 半歧义 `-deals` 需要标题配合；标题带价格信号时拒。
        assert!(matches!(
            judge_item_with_url(
                "Best Dyson Vacuum Deals (2026): Dyson Spot+Scrub Ai",
                "",
                Some("https://www.wired.com/story/dyson-vacuum-deals-nov-2026/"),
                &p
            ),
            Verdict::Reject(RejectReason::Promo)
        ));
        assert!(judge_url("https://www.wired.com/story/a-real-article/", &p).is_accept());
    }

    #[test]
    fn does_not_reject_real_news_with_deal_in_slug() {
        // 实测误伤案例：Marvell 115 亿美元收购是真实新闻，slug 里有 "deals"。
        let p = default_policy_for("rss", "https://techcrunch.com/feed/");
        let verdict = judge_item_with_url(
            "In one of the largest deals in tech history, Marvell buys Lumentum for $11.5B",
            "",
            Some("https://techcrunch.com/2026/11/02/in-one-of-the-largest-deals-in-tech-history-marvell-buys-lumentum-for-115b/"),
            &p,
        );
        assert!(verdict.is_accept(), "real news must not be rejected, got {:?}", verdict);
    }

    #[test]
    fn rejects_promo_titles_without_url() {
        let p = AdmitPolicy::default();
        for t in [
            "Uber Eats Promo Codes: $15 Off November 2026",
            "20% Off Brooks Promo Code",
            "Chewy Promo Codes: $20 Off",
            "Wayfair Coupons: Up to 80% Off",
            // 实测漏网的导购标题
            "Bose QC Headphones (2nd Gen) Sale: Lowest Price Ahead of Black Friday",
            "Google Pixel Watch 5 Sale: Its Lowest Price Yet (2026)",
            "7 Best Travel Mugs (2026): Insulated, Leakproof, and Self-Heating",
            "8 Best Space Heaters (2026): Tested, Measured, and Mistreated",
        ] {
            assert!(
                matches!(judge_item(t, "", &p), Verdict::Reject(RejectReason::Promo)),
                "should reject promo title {}",
                t
            );
        }
        // "Best X" 榜单规则不能误伤技术文。
        for t in [
            "Best practices for async Rust",
            "The best way to structure a Rust workspace",
            "Best practices for database migrations",
        ] {
            assert!(
                judge_item(t, "", &p).is_accept(),
                "must not reject technical article: {}",
                t
            );
        }
    }

    #[test]
    fn rejects_index_titles() {
        let p = AdmitPolicy::default();
        for t in ["派早报：某某发布", "早报｜苹果营收增长", "本周看什么 | 9 部作品"] {
            assert!(
                matches!(judge_item(t, "", &p), Verdict::Reject(RejectReason::IndexPage)),
                "should reject {}",
                t
            );
        }
        assert!(judge_item("Attention Is All You Need", "", &p).is_accept());
    }

    #[test]
    fn rejects_noise_paths() {
        let p = AdmitPolicy::default();
        assert!(judge_url("https://example.com/tag/rust", &p).is_accept() == false);
        assert!(judge_url("https://example.com/a.css", &p).is_accept() == false);
        assert!(judge_url("https://example.com/posts/real-article", &p).is_accept());
    }

    #[test]
    fn content_length_gate() {
        let p = AdmitPolicy::default();
        assert!(matches!(
            judge_content(100, None, &p),
            Verdict::Reject(RejectReason::TooShort)
        ));
        assert!(judge_content(800, None, &p).is_accept());
    }
}
