//! 出网工具：检索与抓取。
//!
//! 默认关闭。抓回来的内容一律当不可信资料，不回写知识库。
//!
//! `web_search` 与 `web_fetch` 的分工是刻意的：搜索只回答"哪几篇可能相关"，
//! 真读正文要单独抓这一篇。把它们合成一个工具会让模型在只想知道有没有资料时
//! 也把整页正文拉进上下文。

use super::*;
use serde_json::{json, Value};

pub(crate) fn handles(name: &str) -> bool {
    matches!(
        name,
        "guaxi_web_search" | "guaxi_web_fetch" | "guaxi_crawl_seed"
    )
}

pub(crate) async fn run(name: &str, a: &Value) -> Result<Value, String> {
    match name {
        "guaxi_web_search" => {
            // 需要 AppHandle（联网检索会向前端报进度）。没有句柄时明确报错，
            // 而不是静默返回空——空结果会让模型以为"网上没有"。
            let app = crate::cognition::runtime::app_handle()
                .ok_or_else(|| "AppHandle 未就绪，暂时无法联网检索".to_string())?;
            let r = crate::commands::chatview::search_web_duckduckgo(
                app,
                req_str(a, "query")?,
                Some(opt_usize(a, "maxResults", 8)),
            )
            .await?;
            Ok(serde_json::to_value(r).map_err(|e| e.to_string())?)
        }
        "guaxi_web_fetch" => fetch_page(a).await,
        "guaxi_crawl_seed" => {
            let url = req_str(a, "url")?;
            let feeds = crate::commands::flipboard::add_site(
                url.clone(),
                opt_str(a, "category").unwrap_or_else(|| "all".into()),
            )
            .await?;
            Ok(json!({
                "ok": true, "url": url,
                "feeds": feeds.iter().map(|f| json!({"id": f.id, "name": f.name})).collect::<Vec<_>>()
            }))
        }
        other => Err(format!("工具 {other} 尚未实现分发")),
    }
}

/// 抓一页并抽正文。
///
/// 抓取走 `flipboard_render::fetch_page`：HTTP 优先，遇到瘦壳/已知 JS 站自动
/// 升级到无头 Chrome（带登录态 profile），被登录墙/验证码挡住会在 `blocked` 里标出来。
/// 抽取走 `flipboard_extract::extract`：trafilatura 优先、scraper 兜底。
///
/// 两个都是既有实现，这里不另写一套抓取器——同一套逻辑在抓取队列上也跑，
/// 行为一致才好解释"为什么同一个链接两种结果不一样"。
async fn fetch_page(a: &Value) -> Result<Value, String> {
    let url = req_str(a, "url")?;
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("url 必须以 http:// 或 https:// 开头".to_string());
    }
    let max_chars = opt_usize(a, "maxChars", 20_000).clamp(500, 200_000);
    let force_render = opt_bool(a, "render", false);

    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/119.0 Safari/537.36",
        )
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| format!("构造 HTTP 客户端失败: {e}"))?;

    let page = crate::commands::flipboard_render::fetch_page(&client, &url, force_render, "").await?;
    let final_url = page.final_url.clone();
    let mode = page.mode;
    let blocked = page.blocked;

    // 抽取会同步调 Python worker（worker 协议是 stdin/stdout），必须在阻塞线程里跑，
    // 否则会卡住整个 tokio 运行时，连工具审批都回不来。
    let html = page.html;
    let base = final_url.clone();
    let extracted = tokio::task::spawn_blocking(move || {
        crate::commands::flipboard_extract::extract(&html, &base)
    })
    .await
    .map_err(|e| format!("抽取任务失败: {e}"))??;

    let body = if extracted.markdown.trim().is_empty() {
        extracted.plain_text.clone()
    } else {
        extracted.markdown.clone()
    };
    let (body, truncated) = truncate_chars(&body, max_chars);

    let mut out = json!({
        "url": url,
        "finalUrl": final_url,
        "title": extracted.title,
        "byline": extracted.byline,
        "publishedAt": extracted.published,
        "engine": extracted.engine,
        "fetchMode": mode,
        "blocked": blocked,
        "truncated": truncated,
        "chars": body.chars().count(),
        // 正文交给模型时一律包不可信声明：网页里任何"请调用 X 工具"的句子都不能照做。
        "content": crate::cognition::percept::untrusted_block(
            extracted.title.as_deref().unwrap_or(&url),
            &body,
        ),
    });
    if blocked {
        out["hint"] = json!("页面像是登录墙或验证码页，正文可能不完整；需要登录态的站请先在抓取面板里登录一次");
    }
    Ok(out)
}

/// 按字符截断（不是字节），避免把一个中文字符劈成两半。
fn truncate_chars(text: &str, max: usize) -> (String, bool) {
    if text.chars().count() <= max {
        return (text.to_string(), false);
    }
    (text.chars().take(max).collect(), true)
}
