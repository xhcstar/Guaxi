//! 网页 / URL 导入：把用户本来就能访问的页面转成听力资源或阅读段落。
//!
//! 只做公开页面抓取，不代管账号、不绕过登录/付费墙。页面是 JS 渲染时正文可能为空，
//! 前端会保留一个可编辑的正文框，用户可以直接粘贴。
//!
//! 抓取内容：
//! - 标题 / og:title
//! - `<article>` / `<main>` 正文，退化为 `<p>` 集合
//! - `<audio>` / `<source>` / `og:audio` / 直链 mp3、m4a 等
//! - `.vtt` / `.srt` 字幕

use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::commands::language_learning::{
    language_learning_record_translation, language_learning_upsert_audio_resource,
    AudioResource, AudioResourceInput, TranslationRecord, TranslationRecordInput,
};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct WebImportPreview {
    pub url: String,
    pub final_url: String,
    pub title: String,
    pub site: String,
    pub text: String,
    pub subtitle_text: String,
    pub word_count: usize,
    pub audio_urls: Vec<String>,
    pub subtitle_urls: Vec<String>,
    pub image_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebImportInput {
    pub url: String,
    pub title: Option<String>,
    pub text: Option<String>,
    pub subtitle_text: Option<String>,
    pub audio_url: Option<String>,
    pub learning_lang: Option<String>,
    pub native_lang: Option<String>,
    pub import_listening: bool,
    pub import_reading: bool,
    pub translate: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct WebImportResult {
    pub audio: Option<AudioResource>,
    pub record: Option<TranslationRecord>,
}

fn normalize_lang(raw: Option<String>, fallback: &str) -> String {
    let value = raw.unwrap_or_default().trim().to_lowercase();
    if value.is_empty() || value == "auto" {
        fallback.to_string()
    } else {
        value
    }
}

fn proxy_client(target: &str) -> Result<Client, String> {
    let config = crate::commands::resource_hunter::store::get_setting("proxy")
        .ok()
        .flatten()
        .and_then(|value| {
            serde_json::from_str::<crate::commands::resource_hunter::model::ProxyConfig>(&value).ok()
        })
        .unwrap_or_default();
    crate::commands::resource_hunter::proxy::build_client_for(&config, target)
}

fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
}

fn strip_tags(raw: &str) -> String {
    let tag = Regex::new(r"(?is)<[^>]+>").unwrap();
    let text = tag.replace_all(raw, " ");
    decode_entities(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn first_capture(re: &Regex, html: &str) -> String {
    re.captures(html)
        .and_then(|caps| caps.get(1))
        .map(|m| collapse(m.as_str()))
        .unwrap_or_default()
}

fn collapse(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(raw: &str, max_chars: usize) -> String {
    if raw.chars().count() <= max_chars {
        return raw.to_string();
    }
    raw.chars().take(max_chars).collect()
}

fn extract_title(html: &str) -> String {
    let og = Regex::new(r#"(?is)<meta[^>]+(?:property|name)=["']og:title["'][^>]+content=["']([^"']+)["']"#).unwrap();
    let title = first_capture(&og, html);
    if !title.is_empty() {
        return decode_entities(&title);
    }
    let re = Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap();
    decode_entities(&first_capture(&re, html))
}

fn extract_text(html: &str) -> String {
    // regex crate 不支持反向引用，这里按标签逐个列出，避免 `<script>...</script>` 误配。
    let cleaner = Regex::new(
        r"(?is)<script[^>]*>.*?</script>|<style[^>]*>.*?</style>|<noscript[^>]*>.*?</noscript>|<svg[^>]*>.*?</svg>|<template[^>]*>.*?</template>",
    )
    .unwrap();
    let cleaned = cleaner.replace_all(html, " ");
    for container in ["article", "main"] {
        let re = Regex::new(&format!(r"(?is)<{}(?:\s[^>]*)?>(.*?)</{}>", container, container)).unwrap();
        for cap in re.captures_iter(&cleaned).take(3) {
            let raw = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
            let text = strip_tags(raw);
            if text.chars().count() >= 180 {
                return text;
            }
        }
    }
    let p_re = Regex::new(r"(?is)<p[^>]*>(.*?)</p>").unwrap();
    let mut chunks = Vec::new();
    for cap in p_re.captures_iter(&cleaned).take(600) {
        let text = strip_tags(cap.get(1).map(|m| m.as_str()).unwrap_or_default());
        if text.chars().count() >= 20 {
            chunks.push(text);
        }
    }
    if !chunks.is_empty() {
        return chunks.join("\n");
    }
    // 退化为整页文本；导航、版权声明也会混进来，但至少给用户一个可编辑的起点。
    strip_tags(&cleaned)
}

fn collect_urls(re: &Regex, html: &str, base: &Url) -> Vec<String> {
    let mut out = Vec::new();
    for cap in re.captures_iter(html) {
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or_default().trim();
        if raw.is_empty() || raw.starts_with("data:") {
            continue;
        }
        let resolved = base
            .join(&html_unescape(raw))
            .map(|u| u.to_string())
            .unwrap_or_else(|_| raw.to_string());
        if !out.iter().any(|x| x == &resolved) {
            out.push(resolved);
        }
    }
    out
}

fn html_unescape(raw: &str) -> String {
    decode_entities(raw).replace("\\/", "/")
}

fn extract_audio_urls(html: &str, base: &Url) -> Vec<String> {
    let audio_ext = Regex::new(r#"(?is)(?:src|data-src|content)=["']([^"']+\.(?:mp3|m4a|wav|ogg|flac|mp4|webm)(?:\?[^"']*)?)["']"#).unwrap();
    let mut out = collect_urls(&audio_ext, html, base);
    let anchor = Regex::new(r#"(?is)<a[^>]+href=["']([^"']+\.(?:mp3|m4a|wav|ogg|flac)(?:\?[^"']*)?)["']"#).unwrap();
    for url in collect_urls(&anchor, html, base) {
        if !out.iter().any(|x| x == &url) {
            out.push(url);
        }
    }
    out.sort_by_key(|url| {
        let lower = url.to_lowercase();
        if lower.ends_with(".mp3") || lower.contains(".mp3?") {
            0
        } else {
            1
        }
    });
    out.truncate(6);
    out
}

fn extract_subtitle_urls(html: &str, base: &Url) -> Vec<String> {
    let re = Regex::new(r#"(?is)(?:src|data-src|href)=["']([^"']+\.(?:vtt|srt)(?:\?[^"']*)?)["']"#).unwrap();
    let mut out = collect_urls(&re, html, base);
    out.truncate(4);
    out
}

fn extract_image(html: &str, base: &Url) -> String {
    let re = Regex::new(r#"(?is)<meta[^>]+(?:property|name)=["']og:image["'][^>]+content=["']([^"']+)["']"#).unwrap();
    let raw = first_capture(&re, html);
    if raw.is_empty() {
        return String::new();
    }
    base.join(&html_unescape(&raw))
        .map(|u| u.to_string())
        .unwrap_or(raw)
}

fn word_count(text: &str) -> usize {
    let mut cjk = 0;
    let mut latin = 0;
    let mut in_word = false;
    for ch in text.chars() {
        if ('\u{4e00}'..='\u{9fff}').contains(&ch) {
            cjk += 1;
            in_word = false;
        } else if ch.is_alphanumeric() {
            if !in_word {
                latin += 1;
                in_word = true;
            }
        } else {
            in_word = false;
        }
    }
    cjk + latin
}

fn parse_subtitle_text(raw: &str) -> String {
    let cleaner = Regex::new(r"(?is)<[^>]+>").unwrap();
    let mut lines = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("WEBVTT")
            || line.starts_with("NOTE")
            || line.contains("-->")
            || line.chars().all(|c| c.is_ascii_digit())
        {
            continue;
        }
        let text = cleaner.replace_all(line, " ").trim().to_string();
        if !text.is_empty() && !lines.iter().any(|x| x == &text) {
            lines.push(text);
        }
    }
    lines.join("\n")
}

#[tauri::command]
pub async fn language_learning_preview_web_import(
    url: String,
) -> Result<WebImportPreview, String> {
    let raw = url.trim();
    let parsed = Url::parse(raw).map_err(|_| "请输入合法的 http/https 地址".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("只支持 http/https 页面".to_string());
    }
    let client = proxy_client(raw)?;
    let resp = client
        .get(raw)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("打开页面失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("打开页面失败，状态码: {}", resp.status()));
    }
    let final_url = resp.url().to_string();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_lowercase();
    if !content_type.contains("html") && !content_type.is_empty() {
        return Err(format!("该地址不是网页（{}）", content_type));
    }
    let html = resp.text().await.map_err(|e| format!("读取页面失败: {}", e))?;
    let base = Url::parse(&final_url).unwrap_or(parsed);
    let title = extract_title(&html);
    // 单个页面正文最多保留 6 万字符，避免把整站 HTML 或超长目录塞进学习库。
    let text = truncate_chars(&extract_text(&html), 60_000);
    let audio_urls = extract_audio_urls(&html, &base);
    let subtitle_urls = extract_subtitle_urls(&html, &base);
    let mut subtitle_text = String::new();
    if let Some(subtitle_url) = subtitle_urls.first() {
        if let Ok(sub_client_resp) = proxy_client(subtitle_url)?.get(subtitle_url).timeout(std::time::Duration::from_secs(15)).send().await {
            if sub_client_resp.status().is_success() {
                if let Ok(raw_sub) = sub_client_resp.text().await {
                    subtitle_text = truncate_chars(&parse_subtitle_text(&raw_sub), 80_000);
                }
            }
        }
    }
    let site = base.host_str().unwrap_or_default().to_string();
    Ok(WebImportPreview {
        word_count: word_count(&text),
        url: raw.to_string(),
        final_url,
        title,
        site,
        text,
        subtitle_text,
        audio_urls,
        subtitle_urls,
        image_url: extract_image(&html, &base),
    })
}

fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for sentence in text.split_inclusive(|c| c == '。' || c == '！' || c == '？' || c == '.' || c == '!' || c == '?') {
        let piece = sentence.trim();
        if piece.is_empty() {
            continue;
        }
        if !current.is_empty() && current.chars().count() + piece.chars().count() > max_chars {
            out.push(current.trim().to_string());
            current.clear();
        }
        current.push_str(piece);
        if current.chars().count() >= max_chars {
            out.push(current.trim().to_string());
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

async fn translate_text(text: &str, from: &str, to: &str) -> String {
    if text.trim().is_empty() || from == to {
        return String::new();
    }
    let chunks = chunk_text(text, 1600);
    if chunks.is_empty() {
        return String::new();
    }
    match crate::commands::translator_free::translate_text_free(
        chunks.clone(),
        Some(from.to_string()),
        Some(to.to_string()),
    )
    .await
    {
        Ok(result) if result.success => result.translated_texts.join("\n"),
        _ => String::new(),
    }
}

#[tauri::command]
pub async fn language_learning_import_web_content(
    app: tauri::AppHandle,
    input: WebImportInput,
) -> Result<WebImportResult, String> {
    let learning_lang = normalize_lang(input.learning_lang.clone(), "en");
    let native_lang = normalize_lang(input.native_lang.clone(), "zh");
    let title = input
        .title
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    let text = input.text.clone().unwrap_or_default().trim().to_string();
    let subtitle_text = input.subtitle_text.clone().unwrap_or_default().trim().to_string();
    let audio_url = input.audio_url.clone().unwrap_or_default().trim().to_string();
    let page_url = input.url.trim().to_string();
    if !input.import_listening && !input.import_reading {
        return Err("请至少选择导入听力或阅读".to_string());
    }
    if input.import_listening && audio_url.is_empty() {
        return Err("该页面没有识别到音频地址".to_string());
    }
    if input.import_reading && text.is_empty() {
        return Err("页面正文为空，请直接粘贴正文后再导入".to_string());
    }
    let translated = if input.translate && input.import_reading {
        translate_text(&text, &learning_lang, &native_lang).await
    } else {
        String::new()
    };
    // record_translation 要求译文非空；翻译服务暂时不可用时先留占位，
    // 用户可以在阅读条目里继续编辑，不要把整次导入回滚掉。
    let translated_for_record = if translated.trim().is_empty() {
        "[待翻译]".to_string()
    } else {
        translated
    };
    let mut result = WebImportResult::default();
    if input.import_listening {
        let transcript = if subtitle_text.chars().count() >= 40 {
            subtitle_text
        } else {
            text.clone()
        };
        let notes = format!("来源页: {}", page_url);
        let saved = language_learning_upsert_audio_resource(
            app.clone(),
            AudioResourceInput {
                id: None,
                title: if title.is_empty() {
                    page_url.clone()
                } else {
                    title.clone()
                },
                source_kind: "remote".to_string(),
                source_uri: audio_url,
                provider: Some("web".to_string()),
                topic: Some(
                    Url::parse(&page_url)
                        .ok()
                        .and_then(|u| u.host_str().map(|x| x.to_string()))
                        .unwrap_or_default(),
                ),
                language_tag: Some(learning_lang.clone()),
                transcript: Some(transcript),
                sync_json_uri: Some(String::new()),
                notes: Some(notes),
                duration_sec: Some(0.0),
            },
            Some(learning_lang.clone()),
        )?;
        result.audio = Some(saved);
    }
    if input.import_reading {
        let record = language_learning_record_translation(
            app,
            TranslationRecordInput {
                source_text: text,
                translated_text: translated_for_record,
                source_lang: Some(learning_lang),
                target_lang: Some(native_lang),
                engine: Some("Web".to_string()),
                record_type: Some("paragraph".to_string()),
                word_meta: None,
            },
        )?;
        result.record = Some(record);
    }
    Ok(result)
}
