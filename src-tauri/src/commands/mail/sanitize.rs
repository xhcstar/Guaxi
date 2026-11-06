//! 来信 HTML 清洗。
//!
//! 前端用 `v-html` 渲染邮件正文，而正文来自外部（任何人都能发一封给我）。
//! 清洗必须发生在把它交给渲染层之前，且不能靠正则——畸形标签、大小写混写、
//! 属性里夹 `>` 都能绕过粗糙的字符串替换。
//!
//! 这里用 `scraper`（项目已有依赖，html5ever 驱动）做一次真正的解析，然后：
//! - 删除危险元素（script / iframe / object / embed / form / base / meta / link）；
//! - 删除所有 `on*` 事件属性与 `style` 里的 `expression`/`javascript:`；
//! - 链接只保留 http/https/mailto/cid/tel，其余替换为空（属性直接丢弃）；
//! - 远端图片默认不加载：把 `src` 挪到 `data-guaxi-remote-src`，
//!   否则每封来信都是一个「已读 + IP」信标。

use scraper::Html;

const DROP_TAGS: &[&str] = &[
    // `style` 也丢掉：正文是直接 v-html 进主文档渲染的，邮件里的样式表会
    // 作用到整个应用；更常见的是 `@media (prefers-color-scheme: dark)` 在
    // 系统深色模式下把正文刷成深底，而文字仍用我们的深色纸面颜色，直接不可读。
    "script", "style", "iframe", "frame", "frameset", "object", "embed", "applet", "form",
    "input", "button", "select", "textarea", "base", "meta", "link", "title", "noscript",
    "template", "svg", "math",
];

pub fn sanitize(html: &str, block_remote: bool) -> String {
    if html.trim().is_empty() {
        return String::new();
    }
    let doc = Html::parse_fragment(html);
    let mut out = String::with_capacity(html.len());
    for node in doc.tree.root().children() {
        render_node(node, &mut out, block_remote);
    }
    out
}

fn render_node(node: ego_tree::NodeRef<'_, scraper::Node>, out: &mut String, block_remote: bool) {
    use scraper::Node;
    match node.value() {
        Node::Text(text) => escape_into(out, text),
        Node::Element(el) => {
            let tag = el.name().to_ascii_lowercase();
            if DROP_TAGS.contains(&tag.as_str()) {
                return;
            }
            out.push('<');
            out.push_str(&tag);
            for (name, value) in el.attrs() {
                let name = name.to_ascii_lowercase();
                let value = value.to_string();
                if !attr_allowed(&tag, &name, &value) {
                    continue;
                }
                if name == "href" && !is_safe_link(&value) {
                    continue;
                }
                if (name == "src" || name == "background") && !is_safe_image_src(&value) {
                    // 已经被封锁的远端图片写的是 `src=""`。二次清洗（老缓存重新过
                    // sanitize 时）不能把这个空占位当成非法地址丢掉：前端「显示图片」
                    // 要靠它把地址放回 src，丢了就只能看到碎图。
                    let blocked_placeholder = name == "src"
                        && value.trim().is_empty()
                        && el.attrs()
                            .any(|(attr, _)| attr.eq_ignore_ascii_case("data-guaxi-remote-src"));
                    if !blocked_placeholder {
                        continue;
                    }
                }
                if name == "src" && block_remote && is_remote_url(&value) {
                    out.push_str(" data-guaxi-remote-src=\"");
                    escape_attr_into(out, &value);
                    out.push('"');
                    out.push_str(" src=\"\"");
                    continue;
                }
                // 表格 / 单元格的 background 也是一张远端图片：不先挡住，
                // 打开邮件就等于给发件人回了 IP；「显示图片」时再还原。
                if name == "background" && block_remote && is_remote_url(&value) {
                    out.push_str(" data-guaxi-remote-bg=\"");
                    escape_attr_into(out, &value);
                    out.push('"');
                    continue;
                }
                out.push(' ');
                out.push_str(&name);
                out.push_str("=\"");
                escape_attr_into(out, &value);
                out.push('"');
            }
            if matches!(tag.as_str(), "img" | "br" | "hr" | "wbr") {
                out.push_str(" />");
                return;
            }
            out.push('>');
            for child in node.children() {
                render_node(child, out, block_remote);
            }
            out.push_str("</");
            out.push_str(&tag);
            out.push('>');
        }
        _ => {
            for child in node.children() {
                render_node(child, out, block_remote);
            }
        }
    }
}

fn is_safe_link(value: &str) -> bool {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("cid:")
        || lower.starts_with("tel:")
        || trimmed.starts_with('#')
}

fn is_remote_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn is_safe_image_src(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("cid:")
        || lower.starts_with("data:image/")
}

/// 属性白名单：未知属性一律丢弃；`on*` 永远丢弃。
fn attr_allowed(_tag: &str, name: &str, value: &str) -> bool {
    if name.starts_with("on") {
        return false;
    }
    // 邮件里的 class/id 没有样式表对应，留着只会让不同邮件互相污染。
    if matches!(
        name,
        "class" | "id" | "name" | "srcset" | "sizes" | "ping" | "formaction"
    ) {
        return false;
    }
    if name == "style" {
        let lower = value.to_ascii_lowercase();
        return !(lower.contains("expression(")
            || lower.contains("javascript:")
            || lower.contains("behavior:")
            || lower.contains("-moz-binding"));
    }
    // 表格的边框 / 单元格内边距 / 底色是邮件排版的一部分：Outlook 会原样渲染，
    // 我们以前一律剥掉（再由阅读区 CSS 统一成「无框紧凑表」），结果就是发件人
    // 精心排好的表格在我们这里丢了框线、间距和底色。这里保留原始属性，
    // 只在暗色主题下由阅读区/配色层做必要的颜色映射。
    matches!(
        name,
        "href"
            | "src"
            | "alt"
            | "title"
            | "width"
            | "height"
            | "align"
            | "valign"
            | "colspan"
            | "rowspan"
            | "cellpadding"
            | "cellspacing"
            | "border"
            | "bgcolor"
            | "background"
            | "color"
            | "face"
            | "size"
            | "dir"
            | "lang"
            | "target"
            | "rel"
            | "type"
            | "start"
            | "span"
            | "nowrap"
            | "role"
            | "aria-label"
            | "aria-hidden"
            | "datetime"
            | "cite"
            | "hreflang"
            | "clear"
            | "noshade"
            | "compact"
            | "summary"
            | "abbr"
            | "scope"
            | "download"
            | "data-guaxi-quote"
    ) || name.starts_with("data-")
}

fn escape_into(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

fn escape_attr_into(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

/// 取 HTML 的纯文本（列表摘要、纯文本视图）。
pub fn html_to_text(html: &str) -> String {
    let doc = Html::parse_fragment(html);
    let mut out = String::new();
    collect_text(doc.tree.root(), &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_text(node: ego_tree::NodeRef<'_, scraper::Node>, out: &mut String) {
    use scraper::Node;
    match node.value() {
        Node::Text(text) => out.push_str(text),
        Node::Element(el) => {
            let tag = el.name();
            if DROP_TAGS.contains(&tag) {
                return;
            }
            let block = matches!(
                tag,
                "p" | "div"
                    | "br"
                    | "tr"
                    | "li"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "blockquote"
                    | "table"
                    | "td"
            );
            if block && !out.ends_with(' ') {
                out.push(' ');
            }
            for child in node.children() {
                collect_text(child, out);
            }
            if block && !out.ends_with(' ') {
                out.push(' ');
            }
        }
        _ => {
            for child in node.children() {
                collect_text(child, out);
            }
        }
    }
}

/// 按字符（不是字节）截断，中文不会被切成半个字。
pub fn truncate_chars(text: &str, limit: usize) -> String {
    if limit == 0 {
        return String::new();
    }
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out: String = text.chars().take(limit).collect();
    out.push('…');
    out
}

/// 引用块标注：很多邮件用 `<div class="gmail_quote">` 包引用文，
/// class 被剥掉后无法再区分。清洗前把这些容器换成 `<blockquote data-guaxi-quote="1">`，
/// 阅读器就能折叠引用部分。
pub fn mark_quotes(html: &str) -> String {
    static QUOTE_OPEN: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(
            r#"(?i)<(div|blockquote)[^>]*class\s*=\s*"[^"]*(?:gmail_quote|moz-cite-prefix|yahoo_quoted|OutlookMessageHeader)[^"]*"[^>]*>"#,
        )
        .unwrap()
    });
    QUOTE_OPEN
        .replace_all(html, "<blockquote data-guaxi-quote=\"1\">")
        .into_owned()
}

/// 把正文约束进阅读容器的包裹层。邮件常带 `width="1000"` 的表格，直接塞进阅读区会横向溢出。
pub fn ensure_viewport(html: &str) -> String {
    format!("<div class=\"guaxi-mail-body\">{html}</div>")
}

/// 对「已经清洗过并缓存在库里」的正文补一次清洗。
///
/// 旧版本的清洗器会保留邮件里的 `<style>`，其中 `prefers-color-scheme: dark`
/// 的规则会把正文刷成深底，而文字仍是纸面的深色，结果整封信不可读。缓存里的
/// 正文不会因为换了清洗规则而自动更新，所以打开时对含 `<style>` 的旧缓存重洗一次。
pub fn resanitize_cached(html: &str) -> String {
    ensure_viewport(&sanitize(html, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_and_event_handlers() {
        let out = sanitize(r#"<div onclick="alert(1)">hi<script>alert(2)</script></div>"#, false);
        assert!(!out.contains("script"));
        assert!(!out.contains("onclick"));
        assert!(out.contains("hi"));
    }

    #[test]
    fn drops_javascript_href() {
        let out = sanitize(r#"<a href="javascript:alert(1)">x</a>"#, false);
        assert!(!out.contains("javascript:"));
        assert!(out.contains("<a>x</a>"));
    }

    #[test]
    fn keeps_https_link_and_mailto() {
        let out = sanitize(
            r#"<a href="https://example.com">a</a><a href="mailto:x@y.z">b</a>"#,
            false,
        );
        assert!(out.contains("https://example.com"));
        assert!(out.contains("mailto:x@y.z"));
    }

    #[test]
    fn blocks_remote_images_when_asked() {
        let out = sanitize(r#"<img src="https://tracker/pixel.gif">"#, true);
        assert!(out.contains("data-guaxi-remote-src=\"https://tracker/pixel.gif\""));
        assert!(out.contains("src=\"\""));
    }

    #[test]
    fn resanitize_keeps_blocked_image_placeholder() {
        // 老缓存重新清洗时，空的 src 是「已封锁图片」的占位，不能被当成非法地址丢掉。
        let out = resanitize_cached(
            r#"<div><img data-guaxi-remote-src="https://x/y.png" src="" /></div>"#,
        );
        assert!(out.contains("data-guaxi-remote-src=\"https://x/y.png\""));
        assert!(out.contains("src=\"\""));
    }

    #[test]
    fn keeps_cid_images() {
        let out = sanitize(r#"<img src="cid:logo@x">"#, true);
        assert!(out.contains("src=\"cid:logo@x\""));
    }

    #[test]
    fn drops_style_with_expression() {
        let out = sanitize(r#"<p style="width: expression(alert(1))">x</p>"#, false);
        assert!(!out.contains("expression"));
        assert!(out.contains(">x</p>"));
    }

    #[test]
    fn drops_style_blocks_entirely() {
        // 邮件里的样式表会作用到整个应用；深色模式的 media query 还会把正文刷黑。
        let out = sanitize(
            r#"<style>@media (prefers-color-scheme: dark){body{background:#111}}</style><p>hi</p>"#,
            false,
        );
        assert!(!out.contains("prefers-color-scheme"));
        assert!(!out.contains("<style"));
        assert!(out.contains("<p>hi</p>"));
    }


    #[test]
    fn html_to_text_collapses_space() {
        assert_eq!(html_to_text("<p>hello</p><p>world</p>"), "hello world");
    }

    #[test]
    fn truncate_counts_chars() {
        assert_eq!(truncate_chars("季度报告摘要", 3), "季度报…");
    }

    #[test]
    fn mark_quotes_rewrites_gmail_container() {
        let out = mark_quotes(r#"<div class="gmail_quote">old</div>"#);
        assert!(out.contains("data-guaxi-quote"));
    }
}
