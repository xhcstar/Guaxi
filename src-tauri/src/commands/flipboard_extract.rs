//! 工业级正文抽取层。
//!
//! 目标：把"原始页面 HTML"清洗成"高质量图文"——保留正文段落与**内联图片**，
//! 去除导航/广告/评论等噪声。这是修复"收集到的都是固定片段、图片被降级成
//! 纯文本链接列表"问题的核心。
//!
//! 双引擎：
//! 1. 主引擎 trafilatura（Python，正文抽取学术金标准）——经常驻 worker 调用，
//!    输出 Markdown + 元数据 + 图片列表。见 [`crate::commands::code_runtime`]。
//! 2. 降级引擎（纯 Rust `scraper`）——trafilatura 不可用/抽取失败时启用。
//!    与旧 `assemble_readable_article` 的关键差异：**保留 `<img>` 内联**而非
//!    降级成 `图片资源链接：- url` 文本列表。

use crate::commands::code_runtime;
use scraper::{Html, Selector};
use std::collections::HashSet;
use url::Url;

/// 抽取结果。`clean_html` 供前端 v-html 渲染（内联图片），`markdown` 供
/// KB 导出 / LLM 输入，`plain_text` 作为搜索与旧字段镜像。
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // published/lang 等字段将在 P2/P6 相关性与 LLM 增强阶段消费
pub struct ExtractedArticle {
    pub title: Option<String>,
    pub clean_html: String,
    pub markdown: String,
    pub plain_text: String,
    pub lead_image: Option<String>,
    pub images: Vec<String>,
    pub byline: Option<String>,
    pub published: Option<String>,
    pub lang: Option<String>,
    pub excerpt: Option<String>,
    /// 实际使用的抽取引擎："trafilatura" | "scraper"
    pub engine: &'static str,
}

/// 抽取入口：先试 trafilatura，失败回退到本地 scraper 抽取。
///
/// `base_url` 用于把相对链接绝对化。该函数在阻塞上下文里调用 Python worker，
/// 因此调用方应放在 `spawn_blocking` 中（worker 协议是同步 stdin/stdout）。
pub fn extract(html: &str, base_url: &str) -> Result<ExtractedArticle, String> {
    if html.trim().is_empty() {
        return Err("empty html".to_string());
    }

    match code_runtime::extract_article_via_python(html, base_url) {
        Ok(article) => match build_from_trafilatura(&article, base_url) {
            Ok(mut extracted) if !extracted.clean_html.trim().is_empty() => {
                // trafilatura 2.x 经常不填 language（实测页面有 lang="en-us" 却返回 None）。
                // 从原始 HTML 的 <html lang> 兜底取一次——质量分里的语言项靠它。
                if extracted.lang.as_deref().map(|l| l.trim().is_empty()).unwrap_or(true) {
                    extracted.lang = html_lang_attr(html);
                }
                return Ok(extracted);
            }
            _ => {
                tracing::warn!("flipboard_extract: trafilatura produced empty content, fallback to scraper");
            }
        },
        Err(e) => {
            // trafilatura 未安装 → 后台触发一次安装（幂等，仅一次），本次先用 scraper 兜底。
            if e.contains("trafilatura unavailable") {
                maybe_install_trafilatura_once();
            }
            tracing::warn!("flipboard_extract: trafilatura failed ({}), fallback to scraper", e);
        }
    }

    extract_with_scraper(html, base_url)
}

/// 从 `<html lang="...">` 取语言。取不到返回 None。
///
/// trafilatura 2.x 的 `language` 字段实测经常为空（页面明写 `lang="en-us"` 也返回 None），
/// 而质量分里语言占一档，所以补这个兜底。
fn html_lang_attr(html: &str) -> Option<String> {
    // 只看文档开头的 4000 个**字符**（不是字节）——按字节切片会切在多字节
    // 字符中间直接 panic，中文页面必踩。取不到边界就安全回退到整个字符串。
    let head: String = if html.chars().count() > 4000 {
        html.chars().take(4000).collect()
    } else {
        html.to_string()
    };
    let lower = head.to_lowercase();
    let start = lower.find("<html")?;
    let tag_end = lower[start..].find('>')? + start;
    let tag = &head[start..tag_end];
    let tag_lower = tag.to_lowercase();
    let attr = tag_lower.find("lang=")?;
    let rest = &tag[attr + 5..];
    let rest = rest.trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        // 未加引号的写法
        let value: String = rest
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '>')
            .collect();
        return if value.is_empty() { None } else { Some(value) };
    }
    let inner = &rest[1..];
    let end = inner.find(quote)?;
    let value = inner[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// 首次发现 trafilatura 缺失时，后台异步安装一次（不阻塞当前抓取）。
fn maybe_install_trafilatura_once() {
    use std::sync::Once;
    static INSTALL_ONCE: Once = Once::new();
    INSTALL_ONCE.call_once(|| {
        std::thread::spawn(|| {
            tracing::info!("flipboard_extract: trafilatura missing, installing in background...");
            match code_runtime::ensure_trafilatura_installed() {
                Ok(true) => tracing::info!("flipboard_extract: trafilatura installed"),
                Ok(false) => tracing::warn!("flipboard_extract: trafilatura install reported failure"),
                Err(e) => tracing::warn!("flipboard_extract: trafilatura install error: {}", e),
            }
        });
    });
}

/// 把 trafilatura worker 的 JSON 结果转成 ExtractedArticle，并把 Markdown 渲染为
/// 干净 HTML（内联图片）。
fn build_from_trafilatura(article: &serde_json::Value, base_url: &str) -> Result<ExtractedArticle, String> {
    let get_str = |k: &str| -> Option<String> {
        article
            .get(k)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let markdown = get_str("markdown").unwrap_or_default();
    let body_html = get_str("html").unwrap_or_default();
    let plain_text = get_str("text").unwrap_or_default();

    let mut images: Vec<String> = article
        .get("images")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str())
                .filter_map(|s| absolutize(base_url, s))
                .collect()
        })
        .unwrap_or_default();
    dedup_in_place(&mut images);

    let lead_image = get_str("lead_image")
        .and_then(|s| absolutize(base_url, &s))
        .or_else(|| images.first().cloned());

    // 优先用 trafilatura 的正文 HTML；否则把 markdown 渲染成 HTML。
    let clean_html = if !body_html.is_empty() {
        absolutize_html_assets(&body_html, base_url)
    } else if !markdown.is_empty() {
        crate::commands::flipboard_extract::markdown_to_html(&markdown, base_url)
    } else {
        String::new()
    };

    let plain = if !plain_text.is_empty() {
        plain_text
    } else {
        strip_html_to_text(&clean_html)
    };

    Ok(ExtractedArticle {
        title: get_str("title"),
        clean_html,
        markdown,
        plain_text: plain,
        lead_image,
        images,
        byline: get_str("byline"),
        published: get_str("published"),
        // trafilatura 2.x 经常不填 language（实测页面有 lang="en-us" 也返回 None），
        // 所以这里再从原始 HTML 的 <html lang> 兜底取一次。
        lang: get_str("lang"),
        excerpt: get_str("excerpt"),
        engine: "trafilatura",
    })
}

/// 纯 Rust 降级抽取：选取正文容器，保留段落/标题/列表/图片为内联 HTML。
fn extract_with_scraper(html: &str, base_url: &str) -> Result<ExtractedArticle, String> {
    let document = Html::parse_document(html);

    let title = first_text(&document, "h1")
        .or_else(|| meta_content(&document, "meta[property='og:title']"))
        .filter(|s| !s.is_empty());

    let excerpt = meta_content(&document, "meta[property='og:description']")
        .or_else(|| meta_content(&document, "meta[name='description']"))
        .filter(|s| !s.is_empty());

    // 选取最可能的正文根容器。
    let container_selectors = [
        "article",
        "main",
        ".post-content",
        ".entry-content",
        ".article-content",
        ".RichText",
        ".markdown-body",
        ".content",
        "body",
    ];
    let mut root_html = String::new();
    for sel in &container_selectors {
        if let Ok(s) = Selector::parse(sel) {
            if let Some(el) = document.select(&s).next() {
                root_html = el.inner_html();
                if root_html.len() > 200 {
                    break;
                }
            }
        }
    }
    if root_html.is_empty() {
        root_html = html.to_string();
    }

    // 在正文根容器内提取有序的块（段落/标题/列表项/图片），重建为干净 HTML。
    let frag = Html::parse_fragment(&root_html);
    let block_sel = Selector::parse("p, h1, h2, h3, h4, blockquote, li, pre, img").unwrap();

    let mut blocks: Vec<String> = Vec::new();
    let mut images: Vec<String> = Vec::new();
    let mut img_seen: HashSet<String> = HashSet::new();
    let mut text_seen: HashSet<String> = HashSet::new();

    for el in frag.select(&block_sel) {
        let tag = el.value().name();
        if tag == "img" {
            if let Some(src) = img_src(&el) {
                if let Some(abs) = absolutize(base_url, &src) {
                    if img_seen.insert(abs.clone()) {
                        blocks.push(format!("<img src=\"{}\" loading=\"lazy\" />", escape_attr(&abs)));
                        images.push(abs);
                    }
                }
            }
            continue;
        }

        let text = compact_ws(&el.text().collect::<String>());
        if is_noise(&text) {
            continue;
        }
        let key = text.to_lowercase();
        if !text_seen.insert(key) {
            continue;
        }
        let safe = escape_html(&text);
        let html_block = match tag {
            "h1" | "h2" => format!("<h2>{}</h2>", safe),
            "h3" | "h4" => format!("<h3>{}</h3>", safe),
            "blockquote" => format!("<blockquote>{}</blockquote>", safe),
            "li" => format!("<li>{}</li>", safe),
            "pre" => format!("<pre>{}</pre>", safe),
            _ => format!("<p>{}</p>", safe),
        };
        blocks.push(html_block);
    }

    let clean_html = blocks.join("\n");
    let plain_text = blocks
        .iter()
        .filter(|b| !b.starts_with("<img"))
        .map(|b| strip_html_to_text(b))
        .collect::<Vec<_>>()
        .join("\n\n");

    if clean_html.trim().is_empty() {
        return Err("scraper extraction produced empty content".to_string());
    }

    let lead_image = images.first().cloned().or_else(|| {
        meta_content(&document, "meta[property='og:image']").and_then(|s| absolutize(base_url, &s))
    });

    Ok(ExtractedArticle {
        title,
        markdown: html_blocks_to_markdown(&blocks),
        clean_html,
        plain_text,
        lead_image,
        images,
        byline: meta_content(&document, "meta[name='author']").filter(|s| !s.is_empty()),
        published: meta_content(&document, "meta[property='article:published_time']")
            .filter(|s| !s.is_empty()),
        lang: None,
        excerpt,
        engine: "scraper",
    })
}

// ===== 辅助函数 =====

/// 把 Markdown 渲染为 HTML（用既有 comrak 依赖），并绝对化图片/链接。
pub fn markdown_to_html(markdown: &str, base_url: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.render.r#unsafe = true; // 保留图片标签
    let html = comrak::markdown_to_html(markdown, &options);
    absolutize_html_assets(&html, base_url)
}

fn html_blocks_to_markdown(blocks: &[String]) -> String {
    blocks
        .iter()
        .map(|b| {
            if let Some(rest) = b.strip_prefix("<img src=\"") {
                let url = rest.split('"').next().unwrap_or("");
                format!("![]({})", url)
            } else {
                strip_html_to_text(b)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// 把 HTML 里的相对 src/href 绝对化。
fn absolutize_html_assets(html: &str, base_url: &str) -> String {
    let base = match Url::parse(base_url) {
        Ok(u) => u,
        Err(_) => return html.to_string(),
    };
    // 轻量正则替换 src=" / href=" 中的相对 URL。
    let re = regex::Regex::new(r#"(src|href)\s*=\s*"([^"]*)""#).unwrap();
    re.replace_all(html, |caps: &regex::Captures| {
        let attr = &caps[1];
        let val = &caps[2];
        match base.join(val) {
            Ok(abs) => format!("{}=\"{}\"", attr, abs),
            Err(_) => format!("{}=\"{}\"", attr, val),
        }
    })
    .to_string()
}

fn absolutize(base_url: &str, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with("data:") {
        return None;
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(raw.to_string());
    }
    Url::parse(base_url)
        .ok()
        .and_then(|base| base.join(raw).ok())
        .map(|u| u.to_string())
}

fn dedup_in_place(items: &mut Vec<String>) {
    let mut seen = HashSet::new();
    items.retain(|x| seen.insert(x.clone()));
}

fn first_text(doc: &Html, selector: &str) -> Option<String> {
    Selector::parse(selector).ok().and_then(|s| {
        doc.select(&s)
            .next()
            .map(|el| compact_ws(&el.text().collect::<String>()))
    })
}

fn meta_content(doc: &Html, selector: &str) -> Option<String> {
    Selector::parse(selector).ok().and_then(|s| {
        doc.select(&s)
            .next()
            .and_then(|el| el.value().attr("content"))
            .map(|c| c.trim().to_string())
    })
}

fn img_src(el: &scraper::ElementRef) -> Option<String> {
    el.value()
        .attr("src")
        .or_else(|| el.value().attr("data-src"))
        .or_else(|| el.value().attr("data-original"))
        .or_else(|| el.value().attr("data-actualsrc"))
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
}

fn compact_ws(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn is_noise(text: &str) -> bool {
    let s = text.to_lowercase();
    s.is_empty()
        || s.len() < 16
        || s.contains("广告")
        || s.contains("赞助")
        || s.contains("cookie")
        || s.contains("隐私政策")
        || s.contains("相关阅读")
        || s.contains("推荐阅读")
        || s.contains("登录后")
        || s.contains("点击展开")
        || s.contains("版权所有")
}

fn strip_html_to_text(html: &str) -> String {
    let frag = Html::parse_fragment(html);
    compact_ws(&frag.root_element().text().collect::<String>())
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_html_lang_double_quotes() {
        let html = r#"<!DOCTYPE html><html lang="en-us"><head></head><body>x</body></html>"#;
        assert_eq!(html_lang_attr(html), Some("en-us".to_string()));
    }

    #[test]
    fn reads_html_lang_single_quotes_and_uppercase() {
        let html = r#"<HTML LANG='zh-CN'><body>x</body></HTML>"#;
        assert_eq!(html_lang_attr(html), Some("zh-CN".to_string()));
    }

    #[test]
    fn returns_none_without_lang() {
        let html = "<html><body>no lang here</body></html>";
        assert_eq!(html_lang_attr(html), None);
    }

    #[test]
    fn only_looks_at_document_head() {
        // 正文深处出现 lang= 不应该被当页面语言
        let mut html = String::from("<html><body>");
        html.push_str(&"填".repeat(5000));
        html.push_str(r#"<pre lang="python">code</pre></body></html>"#);
        assert_eq!(html_lang_attr(&html), None);
    }
}
