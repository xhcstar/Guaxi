//! 页面类型判定（PageKind）：在**抽取之前**判断"这是什么页面"。
//!
//! 为什么必须前置：抽取器只会努力从任何页面里找"正文"，商品页、列表页、登录墙
//! 也能抽出若干段落——于是垃圾就进入了管线。用页面自身的结构化信号（JSON-LD、
//! `og:type`、价格词、链接密度）先把类型定下来，后续策略才有依据。
//!
//! 判定只在 raw HTML 上做，不需要渲染后的 DOM，也不消耗抽取预算。
//!
//! 设计取舍：
//! - **宁可判成 unknown，也不要误判成 product**。误判会永久丢内容；
//!   unknown 只是不施加额外策略。
//! - 信号是"多数的、结构化的"，不是关键词堆砌。单靠 `contains("价格")` 会
//!   把讲定价策略的文章误杀。

use scraper::{Html, Selector};

/// 页面类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    /// 正常文章
    Article,
    /// 列表/聚合页（早报、周报、归档、分类页）
    Index,
    /// 商品/促销页
    Product,
    /// 登录/验证墙
    LoginWall,
    /// 视频页
    Video,
    /// 论坛/问答帖
    Forum,
    /// 错误页（404/5xx/空白模板）
    Error,
    /// 无法判定：不施加额外策略，交给后续质量门
    Unknown,
}

impl PageKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            PageKind::Article => "article",
            PageKind::Index => "index",
            PageKind::Product => "product",
            PageKind::LoginWall => "login_wall",
            PageKind::Video => "video",
            PageKind::Forum => "forum",
            PageKind::Error => "error",
            PageKind::Unknown => "unknown",
        }
    }

    /// 是否值得抽取正文。非文章类默认不值得花抽取预算。
    ///
    /// 当前调用方用 `should_skip_extraction`（互补的判断）；保留此方法
    /// 是为了让"允许列表"与"跳过列表"两种写法都可就地取用，避免将来
    /// 有人重写时搞反语义。
    #[allow(dead_code)]
    pub fn is_readable(&self) -> bool {
        matches!(self, PageKind::Article | PageKind::Forum | PageKind::Unknown)
    }
}

/// 判定结果，附带理由（写进 items.page_kind 供诊断）。
#[derive(Debug, Clone)]
pub struct PageVerdict {
    pub kind: PageKind,
    pub reason: &'static str,
}

/// 对 raw HTML 做类型判定。`final_url` 用于辅助（登录跳转、/product/ 路径）。
pub fn classify_page(html: &str, final_url: &str) -> PageVerdict {
    let hay = html.trim();
    if hay.is_empty() {
        return PageVerdict { kind: PageKind::Error, reason: "empty body" };
    }
    let lower = hay.to_lowercase();

    // 1) JSON-LD：最可靠的信号。只看 `@type`，不看别的字段。
    if let Some(t) = first_jsonld_type(hay) {
        let tl = t.to_lowercase();
        if tl.contains("product") {
            return PageVerdict { kind: PageKind::Product, reason: "jsonld Product" };
        }
        if tl.contains("videoobject") {
            return PageVerdict { kind: PageKind::Video, reason: "jsonld VideoObject" };
        }
        if tl.contains("discussionforumposting") || tl.contains("qapage") {
            return PageVerdict { kind: PageKind::Forum, reason: "jsonld forum" };
        }
        if tl.contains("collectionpage") || tl.contains("itemlist") {
            return PageVerdict { kind: PageKind::Index, reason: "jsonld collection/itemlist" };
        }
        if tl.contains("article") || tl.contains("blogposting") || tl.contains("newsarticle") {
            return PageVerdict { kind: PageKind::Article, reason: "jsonld Article" };
        }
    }

    // 2) og:type
    if let Some(og) = meta_property(hay, "og:type") {
        let og = og.to_lowercase();
        if og.contains("product") || og.contains("product.item") {
            return PageVerdict { kind: PageKind::Product, reason: "og:type product" };
        }
        if og.contains("video") {
            return PageVerdict { kind: PageKind::Video, reason: "og:type video" };
        }
        if og.contains("article") || og.contains("blog") {
            return PageVerdict { kind: PageKind::Article, reason: "og:type article" };
        }
    }

    // 3) 登录墙：由 flipboard_render 的 blocked 判定负责（那里有渲染后上下文），
    //    这里只做 HTML 级的粗判，不重复整套信号。
    if looks_like_login_html(&lower) {
        return PageVerdict { kind: PageKind::LoginWall, reason: "login markers" };
    }

    // 4) 错误页
    if looks_like_error_page(&lower) {
        return PageVerdict { kind: PageKind::Error, reason: "error page markers" };
    }

    // 5) 商品页：schema 缺失时的兜底。要求**同时**命中价格与购买动作，
    //    否则"讲电商定价的文章"会被误杀。
    if looks_like_product_page(&lower) {
        return PageVerdict { kind: PageKind::Product, reason: "price + cart markers" };
    }

    // 6) 列表/聚合页：正文极薄 + 链接密度极高。
    if looks_like_index_page(hay) {
        return PageVerdict { kind: PageKind::Index, reason: "high link density" };
    }

    // 7) URL 兜底：/product/ 这类路径在 schema 缺失时仍是强信号。
    let u = final_url.to_lowercase();
    if u.contains("/product/") || u.contains("/products/") || u.contains("/item/") {
        return PageVerdict { kind: PageKind::Product, reason: "product url path" };
    }

    PageVerdict { kind: PageKind::Unknown, reason: "no decisive signal" }
}

/// 取第一个 JSON-LD 块里的 `@type`。
fn first_jsonld_type(html: &str) -> Option<String> {
    let sel = Selector::parse("script[type='application/ld+json']").ok()?;
    let doc = Html::parse_document(html);
    for el in doc.select(&sel) {
        let text = el.text().collect::<String>();
        // 宽松解析：JSON-LD 常有注释与多值，不追求严格 JSON。
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(text.trim()) {
            if let Some(t) = jsonld_type_from_value(&v) {
                return Some(t);
            }
        }
    }
    None
}

fn jsonld_type_from_value(v: &serde_json::Value) -> Option<String> {
    // @type 可能是字符串或数组
    if let Some(t) = v.get("@type") {
        if let Some(s) = t.as_str() {
            // 常见 `@graph` 结构里，顶层是 WebPage，实体在后面
            if s.eq_ignore_ascii_case("WebPage") {
                return None;
            }
            return Some(s.to_string());
        }
        if let Some(arr) = t.as_array() {
            for x in arr {
                if let Some(s) = x.as_str() {
                    if !s.eq_ignore_ascii_case("WebPage") {
                        return Some(s.to_string());
                    }
                }
            }
        }
    }
    // @graph 数组：逐个找第一个有意义的类型
    if let Some(graph) = v.get("@graph").and_then(|g| g.as_array()) {
        for node in graph {
            if let Some(t) = jsonld_type_from_value(node) {
                return Some(t);
            }
        }
    }
    None
}

fn meta_property(html: &str, property: &str) -> Option<String> {
    let sel = Selector::parse(&format!("meta[property='{}']", property)).ok()?;
    let doc = Html::parse_document(html);
    doc.select(&sel)
        .next()
        .and_then(|el| el.value().attr("content"))
        .map(|s| s.trim().to_string())
}

fn looks_like_login_html(lower: &str) -> bool {
    // 只认"页面很短且明确要求登录"的组合，避免长文里提到"登录"就误判。
    if lower.len() > 60_000 {
        return false;
    }
    const SIGNS: &[&str] = &[
        "sign in to continue",
        "log in to continue",
        "please sign in",
        "please log in",
        "请登录后",
        "登录后查看",
        "登录后继续",
        "扫码登录",
    ];
    SIGNS.iter().any(|s| lower.contains(s))
}

fn looks_like_error_page(lower: &str) -> bool {
    if lower.len() > 20_000 {
        return false;
    }
    const SIGNS: &[&str] = &[
        "404 not found",
        "page not found",
        "页面不存在",
        "页面未找到",
        "internal server error",
        "502 bad gateway",
        "503 service unavailable",
    ];
    SIGNS.iter().any(|s| lower.contains(s))
}

fn looks_like_product_page(lower: &str) -> bool {
    // 价格信号（含常见货币符号与结构化价格）
    let has_price = lower.contains("\"price\"")
        || lower.contains("¥")
        || lower.contains("$")
        || lower.contains("usd")
        || lower.contains("rmb");
    if !has_price {
        return false;
    }
    const BUY: &[&str] = &[
        "add to cart",
        "addtocart",
        "add-to-cart",
        "加入购物车",
        "立即购买",
        "buy now",
        "checkout",
        "in stock",
        "out of stock",
        "库存",
    ];
    BUY.iter().any(|b| lower.contains(b))
}

fn looks_like_index_page(html: &str) -> bool {
    let doc = Html::parse_document(html);
    let Ok(link_sel) = Selector::parse("a[href]") else {
        return false;
    };
    let Ok(para_sel) = Selector::parse("p") else {
        return false;
    };
    let links = doc.select(&link_sel).count();
    let paras = doc.select(&para_sel).count();
    let text_len = doc.root_element().text().collect::<String>().chars().count();

    // 链接多、段落少、正文短 → 列表页。
    // 阈值参考：正常文章每 100 字约有 0–3 个链接；列表页常 > 1 个/20 字。
    if text_len == 0 {
        return false;
    }
    let link_density = links as f32 / (text_len as f32 / 100.0).max(1.0);
    link_density > 3.0 && paras < 5 && text_len < 8000
}

/// 判定是否可以跳过抽取。用于 refine 前快速丢弃，省一次网络/抽取开销。
pub fn should_skip_extraction(kind: PageKind) -> bool {
    matches!(
        kind,
        PageKind::Product | PageKind::Index | PageKind::LoginWall | PageKind::Error | PageKind::Video
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonld_article() {
        let html = r#"<html><head><script type="application/ld+json">
          {"@context":"https://schema.org","@type":"NewsArticle","headline":"x"}
        </script></head><body><p>正文</p></body></html>"#;
        let v = classify_page(html, "https://x.com/a");
        assert_eq!(v.kind, PageKind::Article);
    }

    #[test]
    fn jsonld_graph_article() {
        let html = r#"<html><head><script type="application/ld+json">
          {"@context":"https://schema.org","@graph":[
            {"@type":"WebPage"},
            {"@type":"BlogPosting","headline":"y"}
          ]}
        </script></head><body>x</body></html>"#;
        assert_eq!(classify_page(html, "https://x.com/a").kind, PageKind::Article);
    }

    #[test]
    fn jsonld_product() {
        let html = r#"<html><head><script type="application/ld+json">
          {"@type":"Product","name":"Shoes","offers":{"price":"99"}}
        </script></head><body>x</body></html>"#;
        assert_eq!(classify_page(html, "https://x.com/p").kind, PageKind::Product);
    }

    #[test]
    fn og_video() {
        let html = r#"<html><head><meta property="og:type" content="video.other"></head><body>x</body></html>"#;
        assert_eq!(classify_page(html, "https://x.com/v").kind, PageKind::Video);
    }

    #[test]
    fn price_and_cart_makes_product() {
        let html = "<html><body><span>$49.99</span><button>Add to Cart</button></body></html>";
        assert_eq!(classify_page(html, "https://x.com/x").kind, PageKind::Product);
    }

    #[test]
    fn article_about_pricing_is_not_product() {
        // 反向保护：讲定价策略的文章不该被当成商品页。
        let html = "<html><body><article><h1>谈谈 SaaS 的定价策略</h1>\
                    <p>很多产品的价格由使用量决定，也有 $ 计价的订阅制。</p>\
                    <p>本文讨论这些模式背后的经济学。</p></article></body></html>";
        let v = classify_page(html, "https://blog.x.com/pricing-strategy");
        assert_ne!(v.kind, PageKind::Product, "pricing article misclassified as product");
    }

    #[test]
    fn login_wall() {
        let html = "<html><body><div>请登录后继续</div></body></html>";
        assert_eq!(classify_page(html, "https://x.com/a").kind, PageKind::LoginWall);
    }

    #[test]
    fn empty_is_error() {
        assert_eq!(classify_page("   ", "https://x.com").kind, PageKind::Error);
    }

    #[test]
    fn unknown_when_no_signal() {
        let html = "<html><body><p>一个普通页面，没有结构化信号。</p></body></html>";
        assert_eq!(classify_page(html, "https://x.com/a").kind, PageKind::Unknown);
    }

    #[test]
    fn readable_kinds() {
        assert!(PageKind::Article.is_readable());
        assert!(!PageKind::Product.is_readable());
        assert!(should_skip_extraction(PageKind::Index));
        assert!(!should_skip_extraction(PageKind::Forum));
    }
}
