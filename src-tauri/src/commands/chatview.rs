use crate::commands::settingview;
use chrono::Utc;
use futures_util::StreamExt;
use rand::Rng;
use reqwest::{Client, ClientBuilder};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::convert::TryFrom;
use std::env;
use std::fs;
use std::fs::OpenOptions;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration as StdDuration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};
use url::Url;
// use std::error::Error;
use once_cell::sync::Lazy;

const LOCAL_DEEPSEEK_MODEL_FILENAME: &str = "DeepSeek-R1-0528-Qwen3-8B-IQ4_XS.gguf";
const LOCAL_DEEPSEEK_SERVER_URL: &str = "http://127.0.0.1:18080";
const LOCAL_DEEPSEEK_CHAT_COMPLETIONS_URL: &str = "http://127.0.0.1:18080/v1/chat/completions";

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Default)]
struct StreamBlockTracker {
    in_fence: bool,
    at_line_start: bool,
    leading_spaces: usize,
    tick_run: usize,
    in_inline_math: bool,
    in_block_math: bool,
    backslash_run: usize,
}

impl StreamBlockTracker {
    fn push(&mut self, s: &str) {
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0usize;
        while i < chars.len() {
            let ch = chars[i];
            if ch == '\n' {
                self.at_line_start = true;
                self.leading_spaces = 0;
                self.tick_run = 0;
                self.backslash_run = 0;
                i += 1;
                continue;
            }

            if self.at_line_start {
                if ch == ' ' {
                    if self.leading_spaces < 3 {
                        self.leading_spaces += 1;
                        self.backslash_run = 0;
                        i += 1;
                        continue;
                    }
                    self.at_line_start = false;
                    self.tick_run = 0;
                } else if ch == '`' {
                    self.tick_run += 1;
                    if self.tick_run == 3 {
                        self.in_fence = !self.in_fence;
                    }
                    self.backslash_run = 0;
                    i += 1;
                    continue;
                } else {
                    self.at_line_start = false;
                    self.tick_run = 0;
                }
            }

            let escaped = self.backslash_run % 2 == 1;
            if !self.in_fence && ch == '$' && !escaped {
                let next_is_dollar = i + 1 < chars.len() && chars[i + 1] == '$';
                if self.in_block_math {
                    if next_is_dollar {
                        self.in_block_math = false;
                        i += 2;
                        self.backslash_run = 0;
                        continue;
                    }
                } else if self.in_inline_math {
                    if !next_is_dollar {
                        self.in_inline_math = false;
                        i += 1;
                        self.backslash_run = 0;
                        continue;
                    }
                } else {
                    if next_is_dollar {
                        self.in_block_math = true;
                        i += 2;
                        self.backslash_run = 0;
                        continue;
                    }
                    self.in_inline_math = true;
                    i += 1;
                    self.backslash_run = 0;
                    continue;
                }
            }

            if ch == '\\' {
                self.backslash_run += 1;
            } else {
                self.backslash_run = 0;
            }

            i += 1;
        }
    }

    fn is_blocked(&self) -> bool {
        self.in_fence || self.in_inline_math || self.in_block_math
    }
}

struct LocalDeepseekProcessState {
    child: Option<Child>,
}

static LOCAL_DEEPSEEK_PROCESS: Lazy<Mutex<LocalDeepseekProcessState>> =
    Lazy::new(|| Mutex::new(LocalDeepseekProcessState { child: None }));

#[derive(Serialize)]
pub struct LocalDeepseekServiceStatus {
    pub ready: bool,
    pub endpoint: String,
    pub model_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebSearchItem {
    pub title: String,
    pub snippet: String,
    pub url: String,
}

#[allow(non_camel_case_types)]
enum ChatModel {
    CHATGPT,
    DEEPSEEK,
    QWEN,
    KIMI,
    DOUBAO,
    LOCALQWEN,
    LOCALDEEPSEEK,
    YUANBAO_WEB,
    CHATGPT_WEB,
}

impl TryFrom<i32> for ChatModel {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ChatModel::CHATGPT),
            1 => Ok(ChatModel::DEEPSEEK),
            2 => Ok(ChatModel::QWEN),
            3 => Ok(ChatModel::KIMI),
            4 => Ok(ChatModel::DOUBAO),
            5 => Ok(ChatModel::LOCALQWEN),
            6 => Ok(ChatModel::LOCALDEEPSEEK),
            7 => Ok(ChatModel::YUANBAO_WEB),
            8 => Ok(ChatModel::CHATGPT_WEB),
            _ => Err("Invalid ChatModel ID".to_string()),
        }
    }
}

#[derive(Deserialize)]
pub struct ChatRequest {
    modelid: i32,
    question: String,
    topicid: i32,
    sessionid: String,
}

#[derive(Serialize, Debug)]
pub struct ChatResponse {
    answer: String,
    sessionid: String,
}

#[derive(Deserialize)]
pub struct DeepSeekMessage {
    content: String,
}

#[derive(Deserialize)]
pub struct DeepSeekChoice {
    message: DeepSeekMessage,
}

#[derive(Deserialize)]
pub struct DeepSeekResponse {
    choices: Vec<DeepSeekChoice>,
}

#[derive(Deserialize)]
struct LocalDeepSeekResponse {
    message: DeepSeekMessage,
}

async fn collect_content_from_local_deepseek(data: &str) -> Result<String, String> {
    let mut full_answer = String::new();

    // 分割每一行
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // 解析每一行的 JSON
        let response: LocalDeepSeekResponse =
            serde_json::from_str(line).map_err(|e| e.to_string())?;

        // 提取 content 字段并拼接
        full_answer.push_str(&response.message.content);
    }

    // 返回完整的回答
    Ok(full_answer)
}

fn stream_extract_content(line: &str) -> Option<String> {
    let mut s = line.trim();
    if s.is_empty() {
        return None;
    }

    if let Some(rest) = s.strip_prefix("data:") {
        s = rest.trim();
    }

    if s.is_empty() || s == "[DONE]" {
        return None;
    }

    let v: Value = serde_json::from_str(s).ok()?;

    if let Some(c) = v.pointer("/message/content").and_then(|v| v.as_str()) {
        if !c.is_empty() {
            return Some(c.to_string());
        }
    }

    if let Some(c) = v
        .pointer("/choices/0/delta/content")
        .and_then(|v| v.as_str())
    {
        if !c.is_empty() {
            return Some(c.to_string());
        }
    }

    if let Some(c) = v
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
    {
        if !c.is_empty() {
            return Some(c.to_string());
        }
    }

    None
}

fn drain_complete_lines(buf: &mut String) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(idx) = buf.find('\n') {
        let line = buf[..idx].to_string();
        buf.drain(..=idx);
        lines.push(line);
    }
    lines
}

#[derive(Default)]
struct CodeFenceTracker {
    in_fence: bool,
    at_line_start: bool,
    leading_spaces: usize,
    tick_run: usize,
    last_char: Option<char>,
}

impl CodeFenceTracker {
    fn push(&mut self, s: &str) {
        for ch in s.chars() {
            self.last_char = Some(ch);

            if ch == '\n' {
                self.at_line_start = true;
                self.leading_spaces = 0;
                self.tick_run = 0;
                continue;
            }

            if self.at_line_start {
                if ch == ' ' {
                    if self.leading_spaces < 3 {
                        self.leading_spaces += 1;
                        continue;
                    }
                    self.at_line_start = false;
                    self.tick_run = 0;
                    continue;
                }

                if ch == '`' {
                    self.tick_run += 1;
                    if self.tick_run == 3 {
                        self.in_fence = !self.in_fence;
                    }
                    continue;
                }

                self.at_line_start = false;
                self.tick_run = 0;
                continue;
            }

            self.tick_run = 0;
        }
    }

    fn closing_delta(&self) -> Option<String> {
        if !self.in_fence {
            return None;
        }

        let needs_newline = match self.last_char {
            Some('\n') | None => false,
            _ => true,
        };

        Some(if needs_newline {
            "\n```\n".to_string()
        } else {
            "```\n".to_string()
        })
    }
}

fn generate_session_id() -> String {
    // 获取当前时间戳（秒级）
    let timestamp = Utc::now().timestamp();

    // 生成一个 4 位随机数
    let random_number: u32 = rand::rng().random_range(1000..9999);

    // 将时间戳和随机数组合成 session_id
    format!("session_{}_{}", timestamp, random_number)
}

fn exe_dir() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| e.to_string())?;
    exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or("failed to locate executable directory".to_string())
}

fn local_deepseek_model_path(exe: &Path) -> PathBuf {
    exe.join("utils")
        .join("Models")
        .join("chat")
        .join(LOCAL_DEEPSEEK_MODEL_FILENAME)
}

fn find_file_by_name(root: &Path, name: &str, max_depth: usize) -> Option<PathBuf> {
    if max_depth == 0 || !root.is_dir() {
        return None;
    }

    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file()
            && p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case(name))
                .unwrap_or(false)
        {
            return Some(p);
        }

        if p.is_dir() {
            if let Some(found) = find_file_by_name(&p, name, max_depth - 1) {
                return Some(found);
            }
        }
    }

    None
}

fn resolve_llama_server_path(exe: &Path) -> Result<PathBuf, String> {
    let llama_dir = exe.join("utils").join("llama");
    if !llama_dir.exists() {
        return Err(format!(
            "llama directory not found: {}",
            llama_dir.to_string_lossy()
        ));
    }

    let candidates = if cfg!(windows) {
        vec!["llama-server.exe", "llama_server.exe"]
    } else {
        vec!["llama-server", "llama_server"]
    };

    for name in candidates {
        let direct = llama_dir.join(name);
        if direct.is_file() {
            return Ok(direct);
        }
        if let Some(found) = find_file_by_name(&llama_dir, name, 6) {
            return Ok(found);
        }
    }

    Err(format!(
        "llama-server executable not found under {}",
        llama_dir.to_string_lossy()
    ))
}

async fn is_local_deepseek_server_ready() -> bool {
    let client = match ClientBuilder::new().no_proxy().build() {
        Ok(c) => c,
        Err(_) => return false,
    };

    if let Ok(resp) = client
        .get(format!("{}/health", LOCAL_DEEPSEEK_SERVER_URL))
        .send()
        .await
    {
        if resp.status().is_success() {
            return true;
        }
    }

    if let Ok(resp) = client
        .get(format!("{}/v1/models", LOCAL_DEEPSEEK_SERVER_URL))
        .send()
        .await
    {
        if resp.status().is_success() {
            return true;
        }
    }

    false
}

fn refresh_local_deepseek_process_state() {
    if let Ok(mut guard) = LOCAL_DEEPSEEK_PROCESS.lock() {
        if let Some(child) = guard.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => guard.child = None,
                Ok(None) => {}
                Err(_) => guard.child = None,
            }
        }
    }
}

async fn ensure_local_deepseek_server_started() -> Result<PathBuf, String> {
    let exe = exe_dir()?;
    let model_path = local_deepseek_model_path(&exe);
    let log_dir = exe.join("logs");
    fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    let log_path = log_dir.join("local_deepseek_server.log");
    if !model_path.is_file() {
        return Err(format!(
            "LocalDeepseek model not found: {}",
            model_path.to_string_lossy()
        ));
    }

    if is_local_deepseek_server_ready().await {
        return Ok(model_path);
    }

    refresh_local_deepseek_process_state();

    {
        let mut guard = LOCAL_DEEPSEEK_PROCESS
            .lock()
            .map_err(|_| "local deepseek process lock poisoned".to_string())?;
        if guard.child.is_none() {
            let server_path = resolve_llama_server_path(&exe)?;
            let log_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .map_err(|e| format!("failed to open llama-server log file: {}", e))?;
            let log_file_err = log_file
                .try_clone()
                .map_err(|e| format!("failed to clone log handle: {}", e))?;
            let mut command = Command::new(&server_path);
            command
                .arg("--model")
                .arg(&model_path)
                .arg("--host")
                .arg("127.0.0.1")
                .arg("--port")
                .arg("18080")
                .arg("--threads")
                .arg("-1")
                .arg("--n-gpu-layers")
                .arg("99")
                .arg("--ctx-size")
                .arg("16384")
                .stdout(Stdio::from(log_file))
                .stderr(Stdio::from(log_file_err))
                .stdin(Stdio::null());

            #[cfg(windows)]
            command.creation_flags(CREATE_NO_WINDOW);

            if let Some(parent) = server_path.parent() {
                command.current_dir(parent);
            }

            let child = command.spawn().map_err(|e| {
                format!(
                    "failed to start llama-server: {} ({}), log={}",
                    e,
                    server_path.to_string_lossy(),
                    log_path.to_string_lossy()
                )
            })?;
            guard.child = Some(child);
        }
    }

    for _ in 0..120 {
        if is_local_deepseek_server_ready().await {
            return Ok(model_path);
        }
        if let Ok(mut guard) = LOCAL_DEEPSEEK_PROCESS.lock() {
            if let Some(child) = guard.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    guard.child = None;
                    return Err(format!(
                        "llama-server exited early with status {}. log={}",
                        status,
                        log_path.to_string_lossy()
                    ));
                }
            }
        }
        sleep(Duration::from_millis(250)).await;
    }

    if let Ok(mut guard) = LOCAL_DEEPSEEK_PROCESS.lock() {
        if let Some(mut child) = guard.child.take() {
            let _ = child.kill();
        }
    }

    Err(format!(
        "LocalDeepseek service start timeout. log={}",
        log_path.to_string_lossy()
    ))
}

#[tauri::command]
pub async fn ensure_local_deepseek_service() -> Result<LocalDeepseekServiceStatus, String> {
    let model_path = ensure_local_deepseek_server_started().await?;
    Ok(LocalDeepseekServiceStatus {
        ready: true,
        endpoint: LOCAL_DEEPSEEK_CHAT_COMPLETIONS_URL.to_string(),
        model_path: model_path.to_string_lossy().to_string(),
    })
}

fn collect_duckduckgo_topics(items: &Vec<Value>, out: &mut Vec<WebSearchItem>, limit: usize) {
    for item in items {
        if out.len() >= limit {
            return;
        }

        if let Some(nested) = item.get("Topics").and_then(|v| v.as_array()) {
            collect_duckduckgo_topics(&nested.to_vec(), out, limit);
            continue;
        }

        let title = item
            .get("Text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let url = item
            .get("FirstURL")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if title.is_empty() || url.is_empty() {
            continue;
        }

        out.push(WebSearchItem {
            title: title.clone(),
            snippet: title,
            url,
        });
    }
}

fn normalize_text(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

async fn search_web_bing(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<WebSearchItem>, String> {
    let params = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("q", query)
        .append_pair("setlang", "zh-Hans")
        .finish();
    let request_url = format!("https://www.bing.com/search?{}", params);

    let response = client
        .get(&request_url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36",
        )
        .send()
        .await
        .map_err(|e| format!("bing request failed: {}", e))?;
    let html = response
        .text()
        .await
        .map_err(|e| format!("bing response parse failed: {}", e))?;

    let document = Html::parse_document(&html);
    let item_sel = Selector::parse("li.b_algo").map_err(|e| format!("bing selector parse failed: {}", e))?;
    let link_sel = Selector::parse("h2 a").map_err(|e| format!("bing link selector parse failed: {}", e))?;
    let snippet_sel = Selector::parse("p").map_err(|e| format!("bing snippet selector parse failed: {}", e))?;

    let mut items = Vec::new();
    for item in document.select(&item_sel) {
        if items.len() >= limit {
            break;
        }
        let Some(link) = item.select(&link_sel).next() else {
            continue;
        };
        let url = link.value().attr("href").unwrap_or("").trim().to_string();
        if url.is_empty() || !url.starts_with("http") {
            continue;
        }
        let title = normalize_text(&link.text().collect::<Vec<_>>().join(" "));
        if title.is_empty() {
            continue;
        }
        let snippet = item
            .select(&snippet_sel)
            .next()
            .map(|node| normalize_text(&node.text().collect::<Vec<_>>().join(" ")))
            .unwrap_or_default();

        items.push(WebSearchItem { title, snippet, url });
    }
    Ok(items)
}

async fn search_web_google(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<WebSearchItem>, String> {
    let params = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("q", query)
        .append_pair("hl", "zh-CN")
        .append_pair("num", &limit.to_string())
        .finish();
    let request_url = format!("https://www.google.com/search?{}", params);

    let response = client
        .get(&request_url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36",
        )
        .send()
        .await
        .map_err(|e| format!("google request failed: {}", e))?;
    let html = response
        .text()
        .await
        .map_err(|e| format!("google response parse failed: {}", e))?;

    let document = Html::parse_document(&html);
    let item_sel = Selector::parse("div.g, div.MjjYud")
        .map_err(|e| format!("google selector parse failed: {}", e))?;
    let link_sel = Selector::parse("a[href]").map_err(|e| format!("google link selector parse failed: {}", e))?;
    let h3_sel = Selector::parse("h3").map_err(|e| format!("google title selector parse failed: {}", e))?;
    let snippet_sel = Selector::parse("div.VwiC3b, span.aCOpRe, div.IsZvec")
        .map_err(|e| format!("google snippet selector parse failed: {}", e))?;

    let mut items = Vec::new();
    for item in document.select(&item_sel) {
        if items.len() >= limit {
            break;
        }

        let Some(link) = item.select(&link_sel).next() else {
            continue;
        };
        let url = link.value().attr("href").unwrap_or("").trim().to_string();
        if url.is_empty() || !url.starts_with("http") {
            continue;
        }

        let title = item
            .select(&h3_sel)
            .next()
            .map(|node| normalize_text(&node.text().collect::<Vec<_>>().join(" ")))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| normalize_text(&link.text().collect::<Vec<_>>().join(" ")));
        if title.is_empty() {
            continue;
        }

        let snippet = item
            .select(&snippet_sel)
            .next()
            .map(|node| normalize_text(&node.text().collect::<Vec<_>>().join(" ")))
            .unwrap_or_default();

        items.push(WebSearchItem { title, snippet, url });
    }
    Ok(items)
}

#[tauri::command]
pub async fn search_web_duckduckgo(
    query: String,
    limit: Option<usize>,
) -> Result<Vec<WebSearchItem>, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(vec![]);
    }

    let cap = limit.unwrap_or(5).clamp(1, 8);
    let client = ClientBuilder::new()
        .no_proxy()
        .timeout(StdDuration::from_secs(10))
        .build()
        .map_err(|e| format!("build web search client failed: {}", e))?;
    info!("search_web: query_len={} limit={}", q.len(), cap);

    let mut errors: Vec<String> = Vec::new();

    match search_web_bing(&client, &q, cap).await {
        Ok(items) if !items.is_empty() => return Ok(items),
        Ok(_) => errors.push("bing empty result".to_string()),
        Err(e) => errors.push(e),
    }

    match search_web_google(&client, &q, cap).await {
        Ok(items) if !items.is_empty() => return Ok(items),
        Ok(_) => errors.push("google empty result".to_string()),
        Err(e) => errors.push(e),
    }

    let params = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("q", &q)
        .append_pair("format", "json")
        .append_pair("no_html", "1")
        .append_pair("skip_disambig", "1")
        .finish();
    let request_url = format!("https://api.duckduckgo.com/?{}", params);

    match client.get(&request_url).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(payload) => {
                let mut hits = vec![];
                if let Some(items) = payload.get("RelatedTopics").and_then(|v| v.as_array()) {
                    collect_duckduckgo_topics(&items.to_vec(), &mut hits, cap);
                }

                if hits.is_empty() {
                    if let Some(text) = payload.get("AbstractText").and_then(|v| v.as_str()) {
                        let abstract_text = text.trim().to_string();
                        if !abstract_text.is_empty() {
                            hits.push(WebSearchItem {
                                title: payload
                                    .get("Heading")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("搜索摘要")
                                    .to_string(),
                                snippet: abstract_text,
                                url: payload
                                    .get("AbstractURL")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            });
                        }
                    }
                }

                if !hits.is_empty() {
                    return Ok(hits);
                }
                errors.push("duckduckgo empty result".to_string());
            }
            Err(e) => errors.push(format!("duckduckgo parse failed: {}", e)),
        },
        Err(e) => errors.push(format!("duckduckgo request failed: {}", e)),
    }

    warn!("search_web all engines failed: {}", errors.join(" | "));
    Ok(vec![])
}

// 调用 DeepSeek API 获取回答
pub async fn get_answer_from_deepseek(
    question: &str,
    sessionid: &str,
) -> Result<ChatResponse, String> {
    let api_key = settingview::get_llm_config().deepseek_api_key;
    if api_key.trim().is_empty() {
        return Err("DeepSeek API Key 未配置".to_string());
    }

    let url = "https://api.deepseek.com/chat/completions";
    let session_id;

    info!(
        "get_answer_from_deepseek: Received request: {:?} {}",
        question, sessionid
    );

    if sessionid.is_empty() {
        session_id = generate_session_id();
        info!("get_answer_from_deepseek: sessionid is empty, generating a new one.");
    } else {
        session_id = sessionid.to_string();
        info!("get_answer_from_deepseek: session_id: {}", session_id);
    }

    let request_body = json!({
        "model": "deepseek-chat",
        "messages": [{"role": "user", "content": question}],
        // "session_id": session_id
    });

    info!(
        "get_answer_from_deepseek: request_body: {:?} ",
        request_body
    );

    let client = Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        error!(
            "get_answer_from_deepseek: response error: {:?} ",
            response.status()
        );
        eprintln!("Request failed: {:?}", response.status());
    } else {
        info!("get_answer_from_deepseek: response: {:?} ", response);
    }

    let mut stream = response.bytes_stream();
    let mut full_data = Vec::new();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => full_data.extend_from_slice(&chunk),
            Err(e) => {
                eprintln!("Error while reading stream: {}", e);
            }
        }
    }
    debug!("get_answer_from_deepseek: full_data: {:?} ", full_data);

    let data = String::from_utf8(full_data).expect("Failed to convert bytes to string");
    info!("get_answer_from_deepseek: Data: {:?} ", data);

    let mut chat_response: DeepSeekResponse =
        serde_json::from_str(&data).expect("Failed to parse JSON");

    info!(
        "get_chat_answer: chat_response {} ",
        chat_response.choices[0].message.content
    );

    if let Some(message) = chat_response
        .choices
        .get_mut(0)
        .map(|choice| &mut choice.message)
    {
        message.content = normalize_math_symbols(message.content.clone());
    }

    info!(
        "Answer replaced for math symble: {}",
        chat_response.choices[0].message.content
    );

    let answer_content = chat_response
        .choices
        .get(0)
        .map(|choice| choice.message.content.clone())
        .unwrap_or_else(|| "No answer received.".to_string());

    let chatrsp = ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    };

    Ok(chatrsp)
}

fn excerpt_for_log(s: &str) -> String {
    const LIMIT: usize = 400;
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= LIMIT {
            out.push('…');
            break;
        }
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out
}

fn log_model_reply(tag: &str, session_id: &str, raw: &str) {
    let raw_trimmed = raw.trim();
    if raw_trimmed.is_empty() {
        return;
    }

    let norm = normalize_math_symbols(raw.to_string());

    info!(
        "model_reply tag={} session_id={} raw_len={} norm_len={} raw_excerpt={} norm_excerpt={} ",
        tag,
        session_id,
        raw.len(),
        norm.len(),
        excerpt_for_log(raw_trimmed),
        excerpt_for_log(norm.trim())
    );
}

fn local_math_system_prompt() -> &'static str {
    "输出数学公式时请严格遵循：行内公式用 $...$；块级公式用 $$...$$ 且 $$ 必须独占一行。块级公式格式必须为：\n$$\n公式内容\n$$\n并且闭合 $$ 后下一行开始正文，严禁在同一行继续文字。若处于列表项中，公式块前后都要留空行。不要用反引号包裹任何数学公式。若提供代码，必须带注释。"
}

fn normalize_math_symbols(content: String) -> String {
    if content.is_empty() {
        return content;
    }

    let raw = content;
    let out = raw
        .replace("\\[", "$$")
        .replace("\\]", "$$")
        .replace("\\(", "$")
        .replace("\\)", "$");
    let tightened = tighten_math_spacing(&out);
    let layout = soften_block_math_boundaries(&tightened);

    if raw.contains("\\[")
        || raw.contains("\\]")
        || raw.contains("\\(")
        || raw.contains("\\)")
        || raw.contains("```")
        || out != tightened
        || tightened != layout
    {
        info!(
            "normalize_math_symbols: raw={} out={} tightened={} layout={}",
            excerpt_for_log(&raw),
            excerpt_for_log(&out),
            excerpt_for_log(&tightened),
            excerpt_for_log(&layout)
        );
    }

    layout
}

fn trim_output_whitespace(out: &mut String) {
    while out
        .chars()
        .last()
        .map(|ch| ch.is_whitespace())
        .unwrap_or(false)
    {
        out.pop();
    }
}

fn is_math_delimiter_escaped(chars: &[char], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    let mut backslashes = 0usize;
    let mut i = index;
    while i > 0 {
        i -= 1;
        if chars[i] == '\\' {
            backslashes += 1;
        } else {
            break;
        }
    }
    backslashes % 2 == 1
}

fn tighten_math_spacing(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    let mut state = 0u8;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '$' && !is_math_delimiter_escaped(&chars, i) {
            let next_is_dollar = i + 1 < chars.len() && chars[i + 1] == '$';

            if state == 0 {
                if next_is_dollar {
                    out.push('$');
                    out.push('$');
                    i += 2;
                    while i < chars.len() && chars[i].is_whitespace() {
                        i += 1;
                    }
                    state = 2;
                    continue;
                }

                out.push('$');
                i += 1;
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                state = 1;
                continue;
            }

            if state == 1 && !next_is_dollar {
                trim_output_whitespace(&mut out);
                out.push('$');
                i += 1;
                state = 0;
                continue;
            }

            if state == 2 && next_is_dollar {
                trim_output_whitespace(&mut out);
                out.push('$');
                out.push('$');
                i += 2;
                state = 0;
                continue;
            }
        }

        out.push(ch);
        i += 1;
    }

    out
}

fn soften_block_math_boundaries(input: &str) -> String {
    if input.is_empty() || !input.contains("$$") {
        return input.to_string();
    }

    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len() + 8);
    let mut i = 0usize;
    let mut in_fence = false;
    let mut at_line_start = true;
    let mut leading_spaces = 0usize;
    let mut tick_run = 0usize;

    let is_line_break = |ch: char| ch == '\n' || ch == '\r';

    while i < chars.len() {
        let ch = chars[i];

        if ch == '\n' || ch == '\r' {
            out.push(ch);
            at_line_start = true;
            leading_spaces = 0;
            tick_run = 0;
            i += 1;
            continue;
        }

        if at_line_start {
            if ch == ' ' && leading_spaces < 3 {
                leading_spaces += 1;
                out.push(ch);
                i += 1;
                continue;
            }

            if ch == '`' {
                tick_run += 1;
                out.push(ch);
                i += 1;
                if tick_run == 3 {
                    in_fence = !in_fence;
                    at_line_start = false;
                }
                continue;
            }

            at_line_start = false;
            tick_run = 0;
        }

        if !in_fence && ch == '$' && i + 1 < chars.len() && chars[i + 1] == '$' {
            let prev = out.chars().last();
            if let Some(p) = prev {
                if !p.is_whitespace() && !is_line_break(p) {
                    out.push('\n');
                }
            }

            out.push('$');
            out.push('$');
            i += 2;

            if i < chars.len() {
                let next = chars[i];
                if !next.is_whitespace() && !is_line_break(next) {
                    out.push('\n');
                }
            }

            continue;
        }

        out.push(ch);
        i += 1;
    }

    out
}

async fn openai_compat_sse_stream(
    app: &AppHandle,
    url: &str,
    api_key: &str,
    request_body: Value,
) -> Result<(), String> {
    let client = Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Upstream error: {} {}", status, body));
    }

    let model = request_body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let session_id = request_body
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let tag = format!("openai_compat_sse_stream model={} url={}", model, url);
    let mut stream = response.bytes_stream();
    let mut carry = String::new();
    let mut buffer = String::new();
    let mut full_raw = String::new();
    let mut fence = CodeFenceTracker::default();
    let mut block = StreamBlockTracker::default();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                carry.push_str(&String::from_utf8_lossy(&data));

                for line in drain_complete_lines(&mut carry) {
                    if let Some(content) = stream_extract_content(&line) {
                        if content.contains('$')
                            || content.contains('`')
                            || content.contains("\\[")
                            || content.contains("\\]")
                            || content.contains("\\(")
                            || content.contains("\\)")
                            || content.contains("```")
                        {
                            info!("{}: piece={}", tag, excerpt_for_log(&content));
                        }

                        full_raw.push_str(&content);
                        fence.push(&content);
                        block.push(&content);
                        buffer.push_str(&content);
                    }
                }

                if buffer.is_empty() {
                    continue;
                }

                if block.is_blocked() {
                    continue;
                }

                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }

                let out = normalize_math_symbols(buffer.clone());
                app.emit("modle-streaming-respond-output", &out)
                    .map_err(|e| e.to_string())?;

                buffer.clear();
            }
            Err(e) => return Err(format!("Error while reading stream: {}", e)),
        }
    }

    let tail = carry.trim();
    if !tail.is_empty() {
        if let Some(content) = stream_extract_content(tail) {
            if content.contains('$')
                || content.contains('`')
                || content.contains("\\[")
                || content.contains("\\]")
                || content.contains("\\(")
                || content.contains("\\)")
                || content.contains("```")
            {
                info!("{}: tail_piece={}", tag, excerpt_for_log(&content));
            }

            full_raw.push_str(&content);
            fence.push(&content);
            block.push(&content);
            buffer.push_str(&content);
        }
    }

    if !buffer.is_empty() {
        let out = normalize_math_symbols(buffer);
        app.emit("modle-streaming-respond-output", &out)
            .map_err(|e| e.to_string())?;
    }

    if let Some(closer) = fence.closing_delta() {
        app.emit("modle-streaming-respond-output", &closer)
            .map_err(|e| e.to_string())?;
    }

    log_model_reply(&tag, &session_id, &full_raw);

    Ok(())
}

async fn openai_compat_chat(
    url: &str,
    api_key: &str,
    model: &str,
    question: &str,
) -> Result<String, String> {
    let request_body = json!({
        "model": model,
        "messages": [{"role": "user", "content": question}],
    });

    let client = Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Upstream error: {} {}", status, body));
    }

    let data = response.text().await.map_err(|e| e.to_string())?;
    let mut chat_response: DeepSeekResponse =
        serde_json::from_str(&data).map_err(|e| e.to_string())?;

    let content = chat_response
        .choices
        .get_mut(0)
        .map(|c| normalize_math_symbols(c.message.content.clone()))
        .unwrap_or_else(|| "No answer received.".to_string());

    Ok(content)
}

pub async fn get_answer_from_chatgpt(
    question: &str,
    sessionid: &str,
) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_chatgpt: Received request: {} {}",
        question, sessionid
    );

    let api_key = settingview::get_llm_config().chatgpt_api_key;
    if api_key.trim().is_empty() {
        return Err("ChatGPT API Key 未配置".to_string());
    }

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        "https://api.openai.com/v1/chat/completions",
        &api_key,
        "gpt-4o-mini",
        question,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_kimi(question: &str, sessionid: &str) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_kimi: Received request: {} {}",
        question, sessionid
    );

    let api_key = settingview::get_llm_config().kimi_api_key;
    if api_key.trim().is_empty() {
        return Err("Kimi API Key 未配置".to_string());
    }

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        "https://api.moonshot.cn/v1/chat/completions",
        &api_key,
        "moonshot-v1-8k",
        question,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_qwen(question: &str, sessionid: &str) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_qwen: Received request: {} {}",
        question, sessionid
    );

    let api_key = settingview::get_llm_config().qwen_api_key;
    if api_key.trim().is_empty() {
        return Err("Qwen API Key 未配置".to_string());
    }

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
        &api_key,
        "qwen-plus",
        question,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_doubao(
    question: &str,
    sessionid: &str,
) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_doubao: Received request: {} {}",
        question, sessionid
    );

    let api_key = settingview::get_llm_config().doubao_api_key;
    if api_key.trim().is_empty() {
        return Err("豆包 API Key 未配置".to_string());
    }

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        "https://ark.cn-beijing.volces.com/api/v3/chat/completions",
        &api_key,
        "doubao-pro-32k",
        question,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_local_deepseek(
    question: &str,
    sessionid: &str,
) -> Result<ChatResponse, String> {
    ensure_local_deepseek_server_started().await?;
    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    info!(
        "get_answer_from_local_deepseek: Received request: {} {}",
        question, sessionid
    );

    let request_body = json!({
        "model": "local-deepseek-r1",
        "messages": [
            {"role": "system", "content": local_math_system_prompt()},
            {"role": "user", "content": question}
        ],
        "stream": false
    });

    info!(
        "get_answer_from_local_deepseek: request_body: {:?} ",
        request_body
    );

    // 禁用系统代理
    let client = ClientBuilder::new()
        .no_proxy() // 禁用代理
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(LOCAL_DEEPSEEK_CHAT_COMPLETIONS_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Error calling Local DeepSeek API: {} {}",
            status, body
        ));
    }

    let data = response.text().await.map_err(|e| e.to_string())?;
    let mut chat_response: DeepSeekResponse =
        serde_json::from_str(&data).map_err(|e| e.to_string())?;

    if let Some(message) = chat_response
        .choices
        .get_mut(0)
        .map(|choice| &mut choice.message)
    {
        message.content = normalize_math_symbols(message.content.clone());
    }

    let answer_content = chat_response
        .choices
        .first()
        .map(|choice| choice.message.content.clone())
        .unwrap_or_else(|| "No answer received.".to_string());

    let chatrsp = ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    };
    info!("get_answer_from_local_deepseek: Answer: {:?}", chatrsp);
    Ok(chatrsp)
}

pub async fn get_answer_from_chatgpt_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    info!(
        "get_answer_from_chatgpt_stream: Received request: {} {}",
        question, sessionid
    );

    let api_key = settingview::get_llm_config().chatgpt_api_key;
    if api_key.trim().is_empty() {
        return Err("ChatGPT API Key 未配置".to_string());
    }

    let request_body = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": question}],
        "stream": true,
    });

    openai_compat_sse_stream(
        &app,
        "https://api.openai.com/v1/chat/completions",
        &api_key,
        request_body,
    )
    .await
}
pub async fn get_answer_from_deepseek_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    let api_key = settingview::get_llm_config().deepseek_api_key;
    if api_key.trim().is_empty() {
        return Err("DeepSeek API Key 未配置".to_string());
    }

    info!(
        "get_answer_from_deepseek_stream: Received request: {:?} {}",
        question, sessionid
    );

    let request_body = json!({
        "model": "deepseek-chat",
        "messages": [{"role": "user", "content": question}],
        "stream": true,
    });

    openai_compat_sse_stream(
        &app,
        "https://api.deepseek.com/chat/completions",
        &api_key,
        request_body,
    )
    .await
}

pub async fn get_answer_from_kimi_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    let api_key = settingview::get_llm_config().kimi_api_key;
    if api_key.trim().is_empty() {
        return Err("Kimi API Key 未配置".to_string());
    }

    info!(
        "get_answer_from_kimi_stream: Received request: {:?} {}",
        question, sessionid
    );

    let request_body = json!({
        "model": "moonshot-v1-8k",
        "messages": [{"role": "user", "content": question}],
        "temperature": 0.3,
        "stream": true,
    });

    openai_compat_sse_stream(
        &app,
        "https://api.moonshot.cn/v1/chat/completions",
        &api_key,
        request_body,
    )
    .await
}

pub async fn get_answer_from_doubao_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    let rsp = get_answer_from_doubao(question, sessionid).await?;
    app.emit("modle-streaming-respond-output", &rsp.answer)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_answer_from_qwen_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    let rsp = get_answer_from_qwen(question, sessionid).await?;
    app.emit("modle-streaming-respond-output", &rsp.answer)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_answer_from_local_qwen_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    let url = "http://localhost:11434/api/chat";
    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let request_body = json!({
        "model": "qwen3:8b",
        "messages": [
            {"role": "system", "content": local_math_system_prompt()},
            {"role": "user", "content": question}
        ],
        "stream": true,  // 启用流式传输
        "session_id": session_id,
    });

    info!(
        "get_answer_from_local_qwen_stream: request_body: {:?}",
        request_body
    );

    // 禁用系统代理
    let client = ClientBuilder::new()
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!(
            "Error calling Local Qwen API: {:?}",
            response.status()
        ));
    }

    let mut stream = response.bytes_stream();
    let mut carry = String::new();
    let mut buffer = String::new();
    let mut full_raw = String::new();
    let mut fence = CodeFenceTracker::default();
    let mut block = StreamBlockTracker::default();

    // 流式输出数据
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                carry.push_str(&String::from_utf8_lossy(&data));

                for line in drain_complete_lines(&mut carry) {
                    if let Some(content) = stream_extract_content(&line) {
                        if content.contains('$')
                            || content.contains('`')
                            || content.contains("\\[")
                            || content.contains("\\]")
                            || content.contains("\\(")
                            || content.contains("\\)")
                            || content.contains("```")
                        {
                            info!(
                                "get_answer_from_local_qwen_stream: piece={}",
                                excerpt_for_log(&content)
                            );
                        }

                        full_raw.push_str(&content);
                        fence.push(&content);
                        block.push(&content);
                        buffer.push_str(&content);
                    }
                }

                if buffer.is_empty() {
                    continue;
                }

                if block.is_blocked() {
                    continue;
                }

                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }

                let out = normalize_math_symbols(buffer.clone());
                app.emit("modle-streaming-respond-output", &out)
                    .map_err(|e| e.to_string())?;

                buffer.clear();
            }
            Err(e) => {
                return Err(format!("Error while reading stream: {}", e));
            }
        }
    }

    let tail = carry.trim();
    if !tail.is_empty() {
        if let Some(content) = stream_extract_content(tail) {
            fence.push(&content);
            block.push(&content);
            buffer.push_str(&content);
        }
    }

    // 流结束后，处理剩余的缓冲区数据
    if !buffer.is_empty() {
        let out = normalize_math_symbols(buffer);

        app.emit("modle-streaming-respond-output", &out)
            .map_err(|e| e.to_string())?;
        debug!(
            "get_answer_from_local_qwen_stream: Emitting remaining buffered data: {}",
            out
        );
        debug!("get_answer_from_local_qwen_stream: emit event modle-streaming-respond-output");
    }

    if let Some(closer) = fence.closing_delta() {
        app.emit("modle-streaming-respond-output", &closer)
            .map_err(|e| e.to_string())?;
    }

    log_model_reply("local_qwen_stream model=qwen3:8b", &session_id, &full_raw);

    Ok(())
}

pub async fn get_answer_from_local_deepseek_stream(
    app: AppHandle,
    question: &str,
    sessionid: &str,
) -> Result<(), String> {
    ensure_local_deepseek_server_started().await?;
    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let request_body = json!({
        "model": "local-deepseek-r1",
        "messages": [
            {"role": "system", "content": local_math_system_prompt()},
            {"role": "user", "content": question}
        ],
        "stream": true,
    });

    info!(
        "get_answer_from_local_deepseek_stream: request_body: {:?}",
        request_body
    );

    // 禁用系统代理
    let client = ClientBuilder::new()
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(LOCAL_DEEPSEEK_CHAT_COMPLETIONS_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Error calling Local DeepSeek API: {} {}",
            status, body
        ));
    }

    let mut stream = response.bytes_stream();
    let mut carry = String::new();
    let mut full_raw = String::new();
    let mut buffer = String::new();
    let mut fence = CodeFenceTracker::default();
    let mut block = StreamBlockTracker::default();

    // Stream each delta immediately so UI can render token-by-token.
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                carry.push_str(&String::from_utf8_lossy(&data));

                for line in drain_complete_lines(&mut carry) {
                    if let Some(content) = stream_extract_content(&line) {
                        if content.contains('$')
                            || content.contains('`')
                            || content.contains("\\[")
                            || content.contains("\\]")
                            || content.contains("\\(")
                            || content.contains("\\)")
                            || content.contains("```")
                        {
                            info!(
                                "get_answer_from_local_deepseek_stream: piece={}",
                                excerpt_for_log(&content)
                            );
                        }

                        full_raw.push_str(&content);
                        fence.push(&content);
                        block.push(&content);
                        buffer.push_str(&content);
                    }
                }

                if buffer.is_empty() {
                    continue;
                }

                if block.is_blocked() {
                    continue;
                }

                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }

                let out = normalize_math_symbols(buffer.clone());
                if !out.is_empty() {
                    app.emit("modle-streaming-respond-output", &out)
                        .map_err(|e| e.to_string())?;
                    debug!(
                        "get_answer_from_local_deepseek_stream: Streaming data: {:?}",
                        out
                    );
                }

                buffer.clear();
            }
            Err(e) => {
                return Err(format!("Error while reading stream: {}", e));
            }
        }
    }

    let tail = carry.trim();
    if !tail.is_empty() {
        if let Some(content) = stream_extract_content(tail) {
            fence.push(&content);
            block.push(&content);
            full_raw.push_str(&content);
            buffer.push_str(&content);
        }
    }

    if !buffer.is_empty() {
        let out = normalize_math_symbols(buffer);
        if !out.is_empty() {
            app.emit("modle-streaming-respond-output", &out)
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(closer) = fence.closing_delta() {
        app.emit("modle-streaming-respond-output", &closer)
            .map_err(|e| e.to_string())?;
    }

    log_model_reply(
        "local_deepseek_stream model=deepseek-r1:8b",
        &session_id,
        &full_raw,
    );

    Ok(())
}

pub async fn get_answer_from_local_qwen(
    question: &str,
    sessionid: &str,
) -> Result<ChatResponse, String> {
    let url = "http://localhost:11434/api/chat";
    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    info!(
        "get_answer_from_local_qwen: Received request: {} {}",
        question, sessionid
    );

    let request_body = json!({
        "model": "qwen3:8b",
        "messages": [
            {"role": "system", "content": local_math_system_prompt()},
            {"role": "user", "content": question}
        ],
        "stream": false,
        "session_id": session_id
    });

    let client = ClientBuilder::new()
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!(
            "Error calling Local Qwen API: {:?}",
            response.status()
        ));
    }

    let data = response.text().await.map_err(|e| e.to_string())?;
    let answer_content = collect_content_from_local_deepseek(&data).await?;

    Ok(ChatResponse {
        answer: normalize_math_symbols(answer_content),
        sessionid: request_body["session_id"]
            .as_str()
            .unwrap_or("")
            .to_string(),
    })
}

fn sanitize_filename_component(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if s.is_empty() {
        return s;
    }
    for ch in ['\\', '/', ':', '*', '?', '"', '<', '>', '|'] {
        s = s.replace(ch, "_");
    }
    s
}

fn ensure_chat_record_root() -> Result<PathBuf, String> {
    let root = PathBuf::from(settingview::get_chat_record_root());
    if !root.exists() {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    }
    Ok(root)
}

#[derive(Deserialize)]
pub struct ExportChatSessionRequest {
    pub model_id: Option<i32>,
    pub model_name: Option<String>,
    pub topic_id: Option<i32>,
    pub session_id: Option<String>,
    pub topic_title: Option<String>,
    pub content: Value,
}

#[tauri::command]
pub async fn export_chat_session(request: ExportChatSessionRequest) -> Result<String, String> {
    let root = ensure_chat_record_root()?;

    let model = sanitize_filename_component(request.model_name.as_deref().unwrap_or("chat"));
    let sid = sanitize_filename_component(request.session_id.as_deref().unwrap_or(""));

    let ts = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let file_name = if sid.trim().is_empty() {
        format!("{}_{}.json", ts, model)
    } else {
        format!("{}_{}_{}.json", ts, model, sid)
    };

    let file_path = root.join(file_name);

    let payload = json!({
        "exported_at": Utc::now().to_rfc3339(),
        "model_id": request.model_id,
        "model_name": model,
        "topic_id": request.topic_id,
        "session_id": request.session_id,
        "topic_title": request.topic_title,
        "content": request.content,
    });

    let data = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    fs::write(&file_path, data).map_err(|e| e.to_string())?;

    Ok(file_path.to_string_lossy().to_string())
}

#[derive(Deserialize)]
pub struct SaveChatSessionRequest {
    pub model_id: Option<i32>,
    pub model_name: Option<String>,
    pub topic_id: Option<i32>,
    pub session_id: Option<String>,
    pub topic_title: Option<String>,
    pub content: Value,
}

#[tauri::command]
pub async fn save_chat_session(request: SaveChatSessionRequest) -> Result<String, String> {
    let root = ensure_chat_record_root()?;
    let sessions_dir = root.join("sessions");
    if !sessions_dir.exists() {
        fs::create_dir_all(&sessions_dir).map_err(|e| e.to_string())?;
    }

    let model = sanitize_filename_component(request.model_name.as_deref().unwrap_or("chat"));

    let sid_raw = if let Some(sid) = request.session_id.as_deref() {
        sid.to_string()
    } else {
        format!(
            "model{}_topic{}",
            request.model_id.unwrap_or(-1),
            request.topic_id.unwrap_or(-1)
        )
    };
    let sid = sanitize_filename_component(&sid_raw);

    let file_name = if model.trim().is_empty() {
        format!("{}.json", sid)
    } else {
        format!("{}_{}.json", sid, model)
    };

    let file_path = sessions_dir.join(file_name);

    let payload = json!({
        "saved_at": Utc::now().to_rfc3339(),
        "model_id": request.model_id,
        "model_name": request.model_name,
        "topic_id": request.topic_id,
        "session_id": request.session_id,
        "topic_title": request.topic_title,
        "content": request.content,
    });

    let data = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    fs::write(&file_path, data).map_err(|e| e.to_string())?;

    Ok(file_path.to_string_lossy().to_string())
}

fn web_model_url(provider: &str) -> Option<&'static str> {
    match provider {
        "yuanbao" => Some("https://yuanbao.tencent.com"),
        "chatgpt" => Some("https://chatgpt.com"),
        _ => None,
    }
}

fn web_model_capture_init_script(provider: &str) -> String {
    let provider_key = provider.trim().to_lowercase();
    let provider_json = serde_json::to_string(&provider_key).unwrap_or_else(|_| "\"\"".to_string());
    let endpoint_json = serde_json::to_string("http://127.0.0.1:8888/api/web-model-chat")
        .unwrap_or_else(|_| "\"\"".to_string());

    include_str!("../web_scripts/web_model_capture_init.js")
        .replace("__GUAXI_PROVIDER_JSON__", &provider_json)
        .replace("__GUAXI_ENDPOINT_JSON__", &endpoint_json)
}

async fn ensure_web_model_window(app: &tauri::AppHandle, provider_key: &str) -> Result<(), String> {
    let url = web_model_url(provider_key).ok_or("Unsupported provider".to_string())?;
    let label = format!("web-model-{}", provider_key);

    if app.get_webview_window(&label).is_some() {
        return Ok(());
    }

    let external = Url::parse(url).map_err(|e| e.to_string())?;
    let init_script = web_model_capture_init_script(provider_key);

    WebviewWindowBuilder::new(app, &label, WebviewUrl::External(external))
        .title(url)
        .inner_size(1100.0, 780.0)
        .min_inner_size(820.0, 560.0)
        .initialization_script(init_script.as_str())
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    Ok(())
}

#[derive(Deserialize)]
pub struct WebModelSendRequest {
    provider: String,
    model_id: i32,
    topic_id: i32,
    session_id: String,
    content: String,
    #[serde(default)]
    attachments: Vec<String>,
}

#[tauri::command]
pub async fn web_model_send_message(
    app: tauri::AppHandle,
    request: WebModelSendRequest,
) -> Result<(), String> {
    let provider_key = request.provider.trim().to_lowercase();
    ensure_web_model_window(&app, &provider_key).await?;

    let label = format!("web-model-{}", provider_key);
    let w = app
        .get_webview_window(&label)
        .ok_or("web model window not found".to_string())?;

    let content_json = serde_json::to_string(&request.content).map_err(|e| e.to_string())?;
    let session_json = serde_json::to_string(&request.session_id).map_err(|e| e.to_string())?;
    let attachments_json = serde_json::to_string(&request.attachments).map_err(|e| e.to_string())?;

    let js = format!(
        r#"(() => {{
  try {{
    window.__GUAXI_MODEL_ID__ = {model_id};
    window.__GUAXI_TOPIC_ID__ = {topic_id};
    window.__GUAXI_SESSION_ID__ = {session_id};

    const provider = {provider};
    const text = {content};
    const attachments = Array.isArray({attachments}) ? {attachments} : [];

    function findInput() {{
      const sendBtn = document.querySelector('#yuanbao-send-btn');
      if (provider === 'yuanbao' && sendBtn) {{
        const scope = (sendBtn.closest('form') || sendBtn.closest('main, [role="main"], body') || document.body);
        return (
          scope.querySelector('textarea') ||
          scope.querySelector('[contenteditable="true"]') ||
          scope.querySelector('[role="textbox"]') ||
          document.querySelector('textarea') ||
          document.querySelector('[contenteditable="true"]') ||
          document.querySelector('[role="textbox"]')
        );
      }}

      if (provider === 'chatgpt') {{
        return (
          document.querySelector('#prompt-textarea') ||
          document.querySelector('textarea[data-testid="prompt-textarea" i]') ||
          document.querySelector('textarea') ||
          document.querySelector('[contenteditable="true"]') ||
          document.querySelector('[role="textbox"]')
        );
      }}

      return (
        document.querySelector('#prompt-textarea') ||
        document.querySelector('textarea') ||
        document.querySelector('[contenteditable="true"]') ||
        document.querySelector('[role="textbox"]')
      );
    }}

    function setNativeValue(el, value) {{
      try {{
        const proto = el instanceof HTMLTextAreaElement
          ? HTMLTextAreaElement.prototype
          : HTMLInputElement.prototype;
        const desc = Object.getOwnPropertyDescriptor(proto, 'value');
        if (desc && typeof desc.set === 'function') {{
          desc.set.call(el, value);
          return true;
        }}
      }} catch (e) {{}}
      try {{
        el.value = value;
        return true;
      }} catch (e) {{}}
      return false;
    }}

    function setValue(el, valueText) {{
      if (!el) return false;

      try {{ el.focus(); }} catch (e) {{}}

      if (typeof el.value === 'string') {{
        if (!setNativeValue(el, valueText)) return false;
        try {{ el.dispatchEvent(new InputEvent('input', {{ bubbles: true, data: valueText }})); }} catch (e) {{ el.dispatchEvent(new Event('input', {{ bubbles: true }})); }}
        try {{ el.dispatchEvent(new Event('change', {{ bubbles: true }})); }} catch (e) {{}}
        return true;
      }}

      if (el.isContentEditable) {{
        try {{
          el.textContent = '';
          if (document.execCommand) {{
            document.execCommand('selectAll', false, null);
            document.execCommand('insertText', false, valueText);
          }} else {{
            el.textContent = valueText;
          }}
        }} catch (e) {{
          try {{ el.textContent = valueText; }} catch (e2) {{ return false; }}
        }}

        try {{ el.dispatchEvent(new InputEvent('input', {{ bubbles: true, data: valueText }})); }} catch (e) {{ el.dispatchEvent(new Event('input', {{ bubbles: true }})); }}
        return true;
      }}

      return false;
    }}

    function isDisabled(el) {{
      if (!el) return true;
      if (el.disabled) return true;
      const aria = (el.getAttribute && el.getAttribute('aria-disabled')) || '';
      if (aria.toString().toLowerCase() === 'true') return true;
      return false;
    }}

    function click(el) {{
      if (!el) return false;
      if (isDisabled(el)) return false;
      try {{ el.click(); return true; }} catch (e) {{}}
      try {{
        el.dispatchEvent(new MouseEvent('click', {{ bubbles: true, cancelable: true, view: window }}));
        return true;
      }} catch (e) {{}}
      return false;
    }}

    function findUploadButtons(input) {{
      const root = (input && (input.closest('form') || input.closest('main, [role="main"], body'))) || document.body;
      const nodes = Array.from(root.querySelectorAll('button, [role="button"], label, a, div'));
      const out = [];
      for (const node of nodes) {{
        if (!node) continue;
        const label = ((node.getAttribute && (node.getAttribute('aria-label') || node.getAttribute('title') || node.getAttribute('data-testid') || node.getAttribute('id'))) || '').toString();
        const txt = ((node.innerText || node.textContent) || '').toString().trim();
        const cls = (node.className || '').toString();
        if (/上传|附件|attach|upload|paperclip|plus|添加文件/i.test(label) || /上传|附件|attach|upload/i.test(txt) || /attach|upload|paperclip/i.test(cls)) {{
          out.push(node);
        }}
      }}
      return out.slice(0, 4);
    }}

    function openAttachmentEntrances(input) {{
      if (!attachments.length) return;
      const candidates = findUploadButtons(input);
      for (const el of candidates) {{
        try {{ click(el); }} catch (e) {{}}
      }}
    }}

    function appendAttachmentHint(base) {{
      if (!attachments.length) return base;
      const list = attachments.map((p, i) => `${{i + 1}}. ${{p}}`).join('\n');
      return `${{base}}\n\n[本地附件路径]\n${{list}}\n[/本地附件路径]\n请结合这些附件路径中指向的文件完成回答。`;
    }}

    function bestSendButton(input) {{
      const root = (input && (input.closest('form') || input.closest('main, [role="main"], body'))) || document.body;
      const inputRect = input && input.getBoundingClientRect ? input.getBoundingClientRect() : null;
      const nodes = Array.from(root.querySelectorAll('button, [role="button"], a'));

      let best = null;
      let bestScore = 0;

      for (const b of nodes) {{
        if (!b || isDisabled(b)) continue;

        const label = ((b.getAttribute && (b.getAttribute('aria-label') || b.getAttribute('title') || b.getAttribute('data-testid') || b.getAttribute('id'))) || '').toString();
        const txt = ((b.innerText || b.textContent) || '').toString().trim();
        const cls = (b.className || '').toString();
        const type = (b.getAttribute && b.getAttribute('type')) || '';

        let score = 0;

        if (/语音|voice|speech/i.test(label) || /语音|voice|speech/i.test(txt) || /speech|voice/i.test(cls)) score -= 200;

        if ((b.id || '') === 'yuanbao-send-btn') score += 220;
        if (/send|发送|提交|确定/i.test(label)) score += 90;
        if (/send|发送|提交|确定/i.test(txt)) score += 80;
        if (/send/i.test(cls)) score += 35;
        if (type.toString().toLowerCase() === 'submit') score += 70;
        if (label.toString().toLowerCase().includes('send')) score += 30;
        if (label.includes('发送')) score += 30;
        if (b.querySelector && b.querySelector('svg')) score += 5;

        if (inputRect && b.getBoundingClientRect) {{
          const r = b.getBoundingClientRect();
          const dx = (r.left - inputRect.right);
          const dy = Math.abs(r.top - inputRect.top);
          const dist = Math.sqrt(dx * dx + dy * dy);
          if (Number.isFinite(dist)) score -= Math.min(50, dist / 10);
          if (r.width <= 80 && r.height <= 80) score += 5;
        }}

        if (score > bestScore) {{
          bestScore = score;
          best = b;
        }}
      }}

      return bestScore > 10 ? best : null;
    }}

    function trySubmitForm(input) {{
      const form = input && (input.form || (input.closest && input.closest('form')));
      if (!form) return false;
      try {{
        if (typeof form.requestSubmit === 'function') {{
          form.requestSubmit();
          return true;
        }}
      }} catch (e) {{}}
      try {{
        form.dispatchEvent(new Event('submit', {{ bubbles: true, cancelable: true }}));
        return true;
      }} catch (e) {{}}
      return false;
    }}

    function tryEnter(input) {{
      try {{
        const down = new KeyboardEvent('keydown', {{ key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }});
        const up = new KeyboardEvent('keyup', {{ key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }});
        input.dispatchEvent(down);
        input.dispatchEvent(up);
        return true;
      }} catch (e) {{}}
      return false;
    }}

    function sendNow() {{
      const input = findInput();
      if (!input) return;

      openAttachmentEntrances(input);

      const finalText = appendAttachmentHint(text);
      if (!setValue(input, finalText)) return;

      if (trySubmitForm(input)) return;

      if (provider === 'yuanbao') {{
        const ybtn = document.querySelector('#yuanbao-send-btn');
        if (ybtn && click(ybtn)) return;
      }}

      const btn = bestSendButton(input);
      if (btn && click(btn)) return;

      tryEnter(input);
    }}

    sendNow();
    setTimeout(sendNow, 120);
    setTimeout(sendNow, 420);
    setTimeout(sendNow, 900);
  }} catch (e) {{}}
}})();"#,
        model_id = request.model_id,
        topic_id = request.topic_id,
        session_id = session_json,
        provider = serde_json::to_string(&provider_key).unwrap_or_else(|_| "\"\"".to_string()),
        content = content_json,
        attachments = attachments_json
    );

    w.eval(js.as_str()).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn open_web_model_window(app: tauri::AppHandle, provider: String) -> Result<(), String> {
    let provider_key = provider.trim().to_lowercase();
    ensure_web_model_window(&app, &provider_key).await?;

    let label = format!("web-model-{}", provider_key);
    if let Some(w) = app.get_webview_window(&label) {
        w.show().map_err(|e| format!("显示窗口失败: {}", e))?;
        w.set_focus().ok();
    }

    Ok(())
}

#[tauri::command]
pub async fn prepare_web_model_window(
    app: tauri::AppHandle,
    provider: String,
) -> Result<(), String> {
    let provider_key = provider.trim().to_lowercase();
    ensure_web_model_window(&app, &provider_key).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_chat_answer_stream(app: AppHandle, request: ChatRequest) -> Result<(), String> {
    info!("Enter get_chat_answer_stream");
    let model =
        ChatModel::try_from(request.modelid).map_err(|e| format!("Invalid model ID: {}", e))?;
    info!(
        "Received request: model_id:{} question:{} session_id:{}",
        request.modelid, request.question, request.sessionid
    );

    match model {
        ChatModel::CHATGPT => {
            match get_answer_from_chatgpt_stream(app, &request.question, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream CHATGPT: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling ChatGPT API: {}", err)),
            }
        }
        ChatModel::DEEPSEEK => {
            match get_answer_from_deepseek_stream(app, &request.question, &request.sessionid).await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream DEEPSEEK: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling DeepSeek API: {}", err)),
            }
        }
        ChatModel::KIMI => {
            match get_answer_from_kimi_stream(app, &request.question, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream KIMI: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Kimi API: {}", err)),
            }
        }
        ChatModel::QWEN => {
            match get_answer_from_qwen_stream(app, &request.question, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream QWEN: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Qwen API: {}", err)),
            }
        }
        ChatModel::DOUBAO => {
            match get_answer_from_doubao_stream(app, &request.question, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream DOUBAO: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Doubao API: {}", err)),
            }
        }
        ChatModel::LOCALQWEN => {
            match get_answer_from_local_qwen_stream(app, &request.question, &request.sessionid)
                .await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream LOCALQWEN: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Local Qwen API: {}", err)),
            }
        }
        ChatModel::LOCALDEEPSEEK => {
            match get_answer_from_local_deepseek_stream(app, &request.question, &request.sessionid)
                .await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream LOCALDEEPSEEK: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Local Model API: {}", err)),
            }
        }
        ChatModel::YUANBAO_WEB => {
            let session_id = if request.sessionid.trim().is_empty() {
                generate_session_id()
            } else {
                request.sessionid.to_string()
            };

            web_model_send_message(
                app,
                WebModelSendRequest {
                    provider: "yuanbao".to_string(),
                    model_id: request.modelid,
                    topic_id: request.topicid,
                    session_id,
                    content: request.question.to_string(),
                    attachments: vec![],
                },
            )
            .await?;

            Ok(())
        }
        ChatModel::CHATGPT_WEB => {
            let session_id = if request.sessionid.trim().is_empty() {
                generate_session_id()
            } else {
                request.sessionid.to_string()
            };

            web_model_send_message(
                app,
                WebModelSendRequest {
                    provider: "chatgpt".to_string(),
                    model_id: request.modelid,
                    topic_id: request.topicid,
                    session_id,
                    content: request.question.to_string(),
                    attachments: vec![],
                },
            )
            .await?;

            Ok(())
        }
    }
}

#[tauri::command]
pub async fn get_chat_answer(request: ChatRequest) -> Result<ChatResponse, String> {
    info!("Enter get_chat_answer");
    let model =
        ChatModel::try_from(request.modelid).map_err(|e| format!("Invalid model ID: {}", e))?;
    info!(
        "Received request: model_id:{} question:{} session_id:{}",
        request.modelid, request.question, request.sessionid
    );

    match model {
        ChatModel::CHATGPT => {
            match get_answer_from_chatgpt(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer CHATGPT: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling ChatGPT API: {}", err)),
            }
        }
        ChatModel::DEEPSEEK => {
            match get_answer_from_deepseek(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer DEEPSEEK: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling DeepSeek API: {}", err)),
            }
        }
        ChatModel::KIMI => {
            match get_answer_from_kimi(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer KIMI: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Kimi API: {}", err)),
            }
        }
        ChatModel::QWEN => {
            match get_answer_from_qwen(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer QWEN: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Qwen API: {}", err)),
            }
        }
        ChatModel::DOUBAO => {
            match get_answer_from_doubao(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer DOUBAO: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Doubao API: {}", err)),
            }
        }
        ChatModel::LOCALQWEN => {
            match get_answer_from_local_qwen(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer LOCALQWEN: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Local Qwen API: {}", err)),
            }
        }
        ChatModel::LOCALDEEPSEEK => {
            match get_answer_from_local_deepseek(&request.question, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer LOCALDEEPSEEK: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Local Model API: {}", err)),
            }
        }
        ChatModel::YUANBAO_WEB => Ok(ChatResponse {
            answer: "该模型为网页模式，请调用 open_web_model_window(provider='yuanbao') 进行对话。"
                .to_string(),
            sessionid: if request.sessionid.is_empty() {
                generate_session_id()
            } else {
                request.sessionid
            },
        }),
        ChatModel::CHATGPT_WEB => Ok(ChatResponse {
            answer: "该模型为网页模式，请调用 open_web_model_window(provider='chatgpt') 进行对话。"
                .to_string(),
            sessionid: if request.sessionid.is_empty() {
                generate_session_id()
            } else {
                request.sessionid
            },
        }),
    }
}
