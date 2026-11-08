use crate::commands::llm_provider;
use crate::commands::settingview;
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use futures_util::StreamExt;
use rand::Rng;
use reqwest::multipart::{Form, Part};
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::convert::TryFrom;
use std::env;
use std::fs;
use std::fs::OpenOptions;
#[cfg(all(windows, not(debug_assertions)))]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration as StdDuration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::time::{sleep, timeout, Duration};
use tracing::{debug, error, info, warn};
use url::Url;
// use std::error::Error;
use once_cell::sync::Lazy;

use crate::guaxiservices::bundled_resources;
use crate::guaxiservices::process_shutdown;

const LOCAL_DEEPSEEK_MODEL_FILENAME: &str = "DeepSeek-R1-0528-Qwen3-8B-IQ4_XS.gguf";
const LOCAL_DEEPSEEK_SERVER_URL: &str = "http://127.0.0.1:18080";
const LOCAL_DEEPSEEK_CHAT_COMPLETIONS_URL: &str = "http://127.0.0.1:18080/v1/chat/completions";
const LOCAL_QWEN_MODEL_FILENAME: &str = "Qwen3.6-35B-A3B-UD-Q4_K_M.gguf";
const LOCAL_QWEN_SERVER_URL: &str = "http://127.0.0.1:18082";
const LOCAL_QWEN_CHAT_COMPLETIONS_URL: &str = "http://127.0.0.1:18082/v1/chat/completions";
const SILICONFLOW_AUDIO_SPEECH_URL: &str = "https://api.siliconflow.cn/v1/audio/speech";
const SILICONFLOW_AUDIO_TRANSCRIPTIONS_URL: &str = "https://api.siliconflow.cn/v1/audio/transcriptions";
const SILICONFLOW_AUDIO_TTS_DEFAULT_MODEL: &str = "FunAudioLLM/CosyVoice2-0.5B";
const SILICONFLOW_AUDIO_TTS_DEFAULT_VOICE: &str = "alex";
const SILICONFLOW_AUDIO_STT_DEFAULT_MODEL: &str = "FunAudioLLM/SenseVoiceSmall";

#[cfg(all(windows, not(debug_assertions)))]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(all(windows, not(debug_assertions)))]
fn apply_no_window_in_release(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn apply_no_window_in_release(_cmd: &mut Command) {
}

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
static LOCAL_QWEN_PROCESS: Lazy<Mutex<LocalDeepseekProcessState>> =
    Lazy::new(|| Mutex::new(LocalDeepseekProcessState { child: None }));
static STREAM_CANCEL_REQUESTED: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

/// 关闭本地 llama.cpp 推理服务（deepseek / qwen 各占一个 llama-server 进程）。
///
/// 设计说明：这两个进程是“按需常驻”的——用户用过一次本地大模型后就一直挂着等下次提问，
/// 主程序退出时必须显式收掉，否则会残留占用显存/内存。llama-server 自身还会拉起
/// 若干工作线程与子进程，所以走 `terminate_child`（先结束进程树再用句柄兜底）。
/// 停掉一个本地模型进程。抽出来是为了让 DeepSeek 与 Qwen 能**各自**停止 ——
/// 它们监听不同端口（18080 / 18082），排障时常常只想动其中一个。
fn shutdown_one_local_llm(label: &str, holder: &Lazy<Mutex<LocalDeepseekProcessState>>) {
    match holder.lock() {
        Ok(mut guard) => {
            if let Some(mut child) = guard.child.take() {
                process_shutdown::terminate_child(label, &mut child);
            }
        }
        Err(_) => {
            warn!(label, "[chatview] 本地模型进程锁损坏，退出时无法回收");
        }
    }
}

pub fn shutdown_local_deepseek_server() {
    shutdown_one_local_llm("llama-server(deepseek)", &LOCAL_DEEPSEEK_PROCESS);
}

pub fn shutdown_local_qwen_server() {
    shutdown_one_local_llm("llama-server(qwen)", &LOCAL_QWEN_PROCESS);
}

/// 两个都停。沿用原来的语义，应用退出链仍在用它。
pub fn shutdown_local_llm_servers() {
    shutdown_local_deepseek_server();
    shutdown_local_qwen_server();
}

pub(crate) fn reset_stream_cancel_flag() {
    STREAM_CANCEL_REQUESTED.store(false, Ordering::SeqCst);
}

pub async fn get_answer_from_siliconflow(
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_siliconflow: Received request: {} {}",
        question, sessionid
    );

    let provider = llm_provider::resolve_chat(5)?;

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        &provider.endpoint,
        &provider.api_key,
        &provider.model,
        question,
        history,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_siliconflow_stream(
    app: AppHandle,
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<(), String> {
    let provider = llm_provider::resolve_chat(5)?;

    info!(
        "get_answer_from_siliconflow_stream: Received request: {:?} {}",
        question, sessionid
    );

    let request_body = json!({
        "model": provider.model,
        "messages": build_chat_messages(history, question),
        "stream": true,
    });

    openai_compat_sse_stream(&app, &provider.endpoint, &provider.api_key, request_body).await
}

pub async fn get_answer_from_custom_stream(
    app: AppHandle,
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<(), String> {
    let provider = llm_provider::resolve_chat(10)?;
    info!(
        "get_answer_from_custom_stream: url={} model={} sid={}",
        provider.endpoint, provider.model, sessionid
    );
    let request_body = json!({
        "model": provider.model,
        "messages": build_chat_messages(history, question),
        "stream": true,
    });
    openai_compat_sse_stream(&app, &provider.endpoint, &provider.api_key, request_body).await
}

pub async fn get_answer_from_custom(
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<ChatResponse, String> {
    let provider = llm_provider::resolve_chat(10)?;
    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };
    let answer_content = openai_compat_chat(
        &provider.endpoint,
        &provider.api_key,
        &provider.model,
        question,
        history,
    )
    .await?;
    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_cc_switch_stream(
    app: AppHandle,
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<(), String> {
    let provider = llm_provider::resolve_chat(11)?;
    info!(
        "get_answer_from_cc_switch_stream: url={} model={} sid={}",
        provider.endpoint, provider.model, sessionid
    );
    let request_body = json!({
        "model": provider.model,
        "messages": build_chat_messages(history, question),
        "stream": true,
    });
    openai_compat_sse_stream(&app, &provider.endpoint, &provider.api_key, request_body).await
}

pub async fn get_answer_from_cc_switch(
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<ChatResponse, String> {
    let provider = llm_provider::resolve_chat(11)?;
    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };
    let answer_content = openai_compat_chat(
        &provider.endpoint,
        &provider.api_key,
        &provider.model,
        question,
        history,
    )
    .await?;
    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

fn request_stream_cancel() {
    STREAM_CANCEL_REQUESTED.store(true, Ordering::SeqCst);
}
fn is_stream_cancel_requested() -> bool {
    STREAM_CANCEL_REQUESTED.load(Ordering::SeqCst)
}

/// 供 codex_agent 复用同一个取消开关：前端的停止按钮只调 `cancel_chat_stream`，
/// 两个 Agent 引擎必须看同一个标志位，否则 codex 引擎下点停止不会真的停。
pub(crate) fn stream_cancel_requested() -> bool {
    is_stream_cancel_requested()
}

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
    LOCALQWEN3,
    YUANBAO_WEB,
    CHATGPT_WEB,
    CUSTOM,
    /// 走 cc-switch 本地代理，具体供应商由 cc-switch 决定。
    CCSWITCH,
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
            9 => Ok(ChatModel::LOCALQWEN3),
            10 => Ok(ChatModel::CUSTOM),
            11 => Ok(ChatModel::CCSWITCH),
            _ => Err("Invalid ChatModel ID".to_string()),
        }
    }
}

/// 一条历史对话消息（多轮上下文）。role 取 "user" / "assistant"（其它角色会被忽略）。
#[derive(Deserialize, Debug, Clone)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ChatRequest {
    modelid: i32,
    question: String,
    topicid: i32,
    sessionid: String,
    /// 可选的多轮对话历史（不含本次 question）。前端不传时为空，行为与旧版一致。
    #[serde(default)]
    history: Vec<ChatMsg>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct KbMetaAgentRequest {
    pub modelid: i32,
    #[serde(default)]
    pub sessionid: String,
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub kb_name: String,
    #[serde(default)]
    pub rel_path: String,
    #[serde(default)]
    pub kb_meta: Value,
    #[serde(default)]
    pub file_meta: Value,
    #[serde(default)]
    pub kb_context_files: Value,
    #[serde(default)]
    pub file_content: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct AudioCommonRequest {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub runtime_provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub tokens: String,
    #[serde(default)]
    pub mode_hint: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct TextToSpeechRequest {
    /// 前端把音频公共参数放在 `request.common` 下，所以这里就是普通的嵌套字段。
    ///
    /// 别改回 `#[serde(flatten)]`：那样只认 `{"provider":…,"input":…}` 的平铺形状，
    /// 而前端发的 `{"common":{…},"input":…}` 会被当成未知字段丢掉（serde 默认忽略未知字段、
    /// 不报错），provider/model/tokens 静默变成空串。
    #[serde(default)]
    pub common: AudioCommonRequest,
    pub input: String,
    #[serde(default)]
    pub voice: String,
    #[serde(default)]
    pub response_format: String,
    pub sample_rate: Option<u32>,
    pub stream: Option<bool>,
    pub speed: Option<f32>,
    pub gain: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Serialize, Debug, Clone)]
pub struct TextToSpeechResponse {
    pub audio_base64: String,
    pub content_type: String,
    pub trace_id: String,
    pub model: String,
    pub response_format: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct SpeechToTextRequest {
    /// 与 `TextToSpeechRequest::common` 同理，前端发的是 `request.common` 这个嵌套对象。
    #[serde(default)]
    pub common: AudioCommonRequest,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub file_base64: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub mime_type: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct SpeechToTextResponse {
    pub text: String,
    pub trace_id: String,
    pub model: String,
}

#[derive(Serialize, Debug)]
pub struct ChatResponse {
    pub answer: String,
    pub sessionid: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct KbMetaAgentResponse {
    pub sessionid: String,
    pub modelid: i32,
    pub task: String,
    pub result: Value,
    pub raw_answer: String,
}

fn normalize_kb_agent_task(task: &str) -> String {
    let t = task.trim().to_lowercase();
    if t == "file" {
        "file".to_string()
    } else {
        "kb".to_string()
    }
}

fn find_first_json_segment(raw: &str) -> Option<String> {
    let starts: Vec<usize> = raw
        .char_indices()
        .filter_map(|(i, ch)| if ch == '{' || ch == '[' { Some(i) } else { None })
        .collect();
    if starts.is_empty() {
        return None;
    }

    for start in starts {
        let mut stack: Vec<char> = Vec::new();
        let mut in_string = false;
        let mut escaped = false;

        for (i, ch) in raw.char_indices().skip_while(|(idx, _)| *idx < start) {
            if in_string {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == '"' {
                    in_string = false;
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '{' => stack.push('}'),
                '[' => stack.push(']'),
                '}' | ']' => {
                    if stack.pop() != Some(ch) {
                        stack.clear();
                        break;
                    }
                    if stack.is_empty() {
                        let end = i + ch.len_utf8();
                        return Some(raw[start..end].to_string());
                    }
                }
                _ => {}
            }
        }
    }

    None
}

fn parse_kb_agent_json(raw_answer: &str) -> Result<Value, String> {
    let trimmed = raw_answer.trim();
    if trimmed.is_empty() {
        return Err("模型未返回内容".to_string());
    }

    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        return Ok(v);
    }

    if trimmed.starts_with("```") {
        let mut lines = trimmed.lines();
        let _ = lines.next();
        let mut body = lines.collect::<Vec<_>>().join("\n");
        if let Some(pos) = body.rfind("```") {
            body.truncate(pos);
        }
        if let Ok(v) = serde_json::from_str::<Value>(body.trim()) {
            return Ok(v);
        }
    }

    if let Some(seg) = find_first_json_segment(trimmed) {
        if let Ok(v) = serde_json::from_str::<Value>(seg.trim()) {
            return Ok(v);
        }
    }

    Err("模型返回内容不是有效 JSON".to_string())
}

fn alias_object_value(map: &serde_json::Map<String, Value>, names: &[&str]) -> Option<Value> {
    for name in names {
        if let Some(v) = map.get(*name) {
            return Some(v.clone());
        }
    }
    None
}

fn normalize_kb_agent_result(value: Value) -> Value {
    let Some(obj) = value.as_object() else {
        return value;
    };

    let mut out = serde_json::Map::new();

    if let Some(v) = alias_object_value(
        obj,
        &["knowledge_base_type", "knowledgeBaseType", "kb_type", "kbType"],
    ) {
        out.insert("knowledge_base_type".to_string(), v);
    }
    if let Some(v) = alias_object_value(obj, &["file_tags", "fileTags"]) {
        out.insert("file_tags".to_string(), v);
    }
    if let Some(v) = alias_object_value(obj, &["related_nodes", "relatedNodes"]) {
        out.insert("related_nodes".to_string(), v);
    }
    if let Some(v) = alias_object_value(obj, &["kb_tags", "kbTags"]) {
        out.insert("kb_tags".to_string(), v);
    }
    if let Some(v) = alias_object_value(obj, &["summary"]) {
        out.insert("summary".to_string(), v);
    }

    if let Some(nodes_v) = alias_object_value(obj, &["structure_nodes", "structureNodes", "nodes"]) {
        if let Some(nodes) = nodes_v.as_array() {
            let mut mapped = Vec::new();
            for n in nodes {
                if let Some(nobj) = n.as_object() {
                    let mut one = serde_json::Map::new();
                    if let Some(v) = alias_object_value(nobj, &["stable_id", "stableId", "id", "node_id"]) {
                        one.insert("stable_id".to_string(), v);
                    }
                    if let Some(v) = alias_object_value(nobj, &["title", "name"]) {
                        one.insert("title".to_string(), v);
                    }
                    if let Some(v) = alias_object_value(nobj, &["level"]) {
                        one.insert("level".to_string(), v);
                    }
                    if let Some(v) = alias_object_value(nobj, &["tags"]) {
                        one.insert("tags".to_string(), v);
                    }
                    if let Some(v) = alias_object_value(nobj, &["related_nodes", "relatedNodes"]) {
                        one.insert("related_nodes".to_string(), v);
                    }
                    if let Some(v) = alias_object_value(nobj, &["attrs_patch", "attrsPatch", "attrs"]) {
                        one.insert("attrs_patch".to_string(), v);
                    }
                    if !one.is_empty() {
                        mapped.push(Value::Object(one));
                    }
                }
            }
            out.insert("structure_nodes".to_string(), Value::Array(mapped));
        }
    }

    if let Some(edges_v) = alias_object_value(obj, &["knowledge_relations", "knowledgeRelations", "relations"]) {
        if let Some(edges) = edges_v.as_array() {
            let mut mapped = Vec::new();
            for e in edges {
                if let Some(eobj) = e.as_object() {
                    let mut one = serde_json::Map::new();
                    if let Some(v) = alias_object_value(eobj, &["from_stable_id", "fromStableId", "from"]) {
                        one.insert("from_stable_id".to_string(), v);
                    }
                    if let Some(v) = alias_object_value(eobj, &["to_stable_id", "toStableId", "to"]) {
                        one.insert("to_stable_id".to_string(), v);
                    }
                    if let Some(v) = alias_object_value(eobj, &["relation", "type", "label"]) {
                        one.insert("relation".to_string(), v);
                    }
                    if !one.is_empty() {
                        mapped.push(Value::Object(one));
                    }
                }
            }
            out.insert("knowledge_relations".to_string(), Value::Array(mapped));
        }
    }

    if out.is_empty() {
        value
    } else {
        Value::Object(out)
    }
}

fn trim_rel_path_to_label(rel_path: &str) -> String {
    let p = rel_path.trim().replace('\\', "/");
    if p.is_empty() {
        return String::new();
    }
    p.rsplit('/').next().unwrap_or("").trim_end_matches(".md").trim_end_matches(".markdown").to_string()
}

fn fallback_file_tags_from_request(request: &KbMetaAgentRequest) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    if let Some(arr) = request.file_meta.get("file_tags").and_then(|v| v.as_array()) {
        for v in arr {
            if let Some(s) = v.as_str() {
                let t = s.trim();
                if !t.is_empty() && !out.contains(&t.to_string()) {
                    out.push(t.to_string());
                }
            }
        }
    }

    if out.is_empty() {
        if let Some(arr) = request.file_meta.get("structure_nodes").and_then(|v| v.as_array()) {
            for n in arr.iter().take(8) {
                if let Some(title) = n.get("title").and_then(|v| v.as_str()) {
                    let t = title.trim();
                    if !t.is_empty() && !out.contains(&t.to_string()) {
                        out.push(t.to_string());
                    }
                }
                if out.len() >= 5 {
                    break;
                }
            }
        }
    }

    if out.is_empty() {
        let label = trim_rel_path_to_label(&request.rel_path);
        if !label.is_empty() {
            out.push(label);
        }
    }

    out.truncate(5);
    out
}

fn fallback_related_nodes_from_request(request: &KbMetaAgentRequest) -> Vec<String> {
    let self_rel = request.rel_path.trim().replace('\\', "/");
    let self_parent = self_rel.rsplit_once('/').map(|(a, _)| a.to_string()).unwrap_or_default();
    let mut same_parent: Vec<String> = Vec::new();
    let mut others: Vec<String> = Vec::new();

    if let Some(arr) = request.kb_context_files.as_array() {
        for one in arr {
            let rel = one
                .get("rel_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .replace('\\', "/");
            if rel.is_empty() || rel == self_rel {
                continue;
            }
            let label = trim_rel_path_to_label(&rel);
            if label.is_empty() {
                continue;
            }
            let parent = rel.rsplit_once('/').map(|(a, _)| a.to_string()).unwrap_or_default();
            if !self_parent.is_empty() && parent == self_parent {
                if !same_parent.contains(&label) {
                    same_parent.push(label);
                }
            } else if !others.contains(&label) {
                others.push(label);
            }
        }
    }

    let mut out = same_parent;
    out.extend(others);
    out.truncate(5);
    out
}

fn ensure_non_empty_file_fields(request: &KbMetaAgentRequest, mut result: Value) -> Value {
    let Some(obj) = result.as_object_mut() else {
        return result;
    };

    let tags_len = obj
        .get("file_tags")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if tags_len == 0 {
        let fallback = fallback_file_tags_from_request(request)
            .into_iter()
            .map(Value::String)
            .collect::<Vec<_>>();
        obj.insert("file_tags".to_string(), Value::Array(fallback));
    }

    let rel_len = obj
        .get("related_nodes")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if rel_len == 0 {
        let fallback = fallback_related_nodes_from_request(request)
            .into_iter()
            .map(Value::String)
            .collect::<Vec<_>>();
        obj.insert("related_nodes".to_string(), Value::Array(fallback));
    }

    result
}

fn is_prompt_too_large_error(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("max_prompt_tokens")
        || m.contains("prompt_tokens")
        || m.contains("prompt too long")
        || m.contains("token limit")
        || m.contains("context length")
}

fn shorten_text(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut out = String::new();
    for ch in s.chars().take(max_chars) {
        out.push(ch);
    }
    out
}

fn shrink_kb_agent_request_for_retry(request: &mut KbMetaAgentRequest, round: usize) -> bool {
    let (max_file_content, max_ctx_files, max_excerpt, max_outline) = match round {
        0 => (12000usize, 24usize, 1200usize, 20usize),
        1 => (8000usize, 14usize, 700usize, 12usize),
        2 => (5000usize, 8usize, 400usize, 8usize),
        _ => (3000usize, 4usize, 200usize, 5usize),
    };

    let mut changed = false;

    let trimmed_fc = shorten_text(request.file_content.as_str(), max_file_content);
    if trimmed_fc.len() < request.file_content.len() {
        request.file_content = trimmed_fc;
        changed = true;
    }

    let Some(arr) = request.kb_context_files.as_array() else {
        return changed;
    };

    let mut compact: Vec<Value> = Vec::new();
    for one in arr.iter().take(max_ctx_files) {
        if let Some(obj) = one.as_object() {
            let mut m = serde_json::Map::new();
            if let Some(v) = obj.get("rel_path") {
                m.insert("rel_path".to_string(), v.clone());
            }
            if let Some(v) = obj.get("file_tags") {
                m.insert("file_tags".to_string(), v.clone());
            }
            if let Some(v) = obj.get("related_nodes") {
                m.insert("related_nodes".to_string(), v.clone());
            }
            let excerpt = obj
                .get("content_excerpt")
                .and_then(|v| v.as_str())
                .map(|s| shorten_text(s, max_excerpt))
                .unwrap_or_default();
            m.insert("content_excerpt".to_string(), Value::String(excerpt));

            let outline = obj
                .get("structure_outline")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().take(max_outline).cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            m.insert("structure_outline".to_string(), Value::Array(outline));
            compact.push(Value::Object(m));
        }
    }

    if compact.len() != arr.len() {
        changed = true;
    }
    request.kb_context_files = Value::Array(compact);
    changed
}

fn build_kb_agent_prompt(request: &KbMetaAgentRequest, task: &str) -> Result<String, String> {
    let kb_name = request.kb_name.trim();
    if kb_name.is_empty() {
        return Err("kb_name 不能为空".to_string());
    }

    let kb_meta_json =
        serde_json::to_string_pretty(&request.kb_meta).map_err(|e| format!("kb_meta 序列化失败: {}", e))?;
    let file_meta_json = if request.file_meta.is_null() {
        "null".to_string()
    } else {
        serde_json::to_string_pretty(&request.file_meta)
            .map_err(|e| format!("file_meta 序列化失败: {}", e))?
    };
    let kb_context_files_json = if request.kb_context_files.is_null() {
        "[]".to_string()
    } else {
        serde_json::to_string_pretty(&request.kb_context_files)
            .map_err(|e| format!("kb_context_files 序列化失败: {}", e))?
    };
    let file_content = request.file_content.trim().to_string();
    let file_content_json = if file_content.is_empty() {
        "\"\"".to_string()
    } else {
        serde_json::to_string(&file_content).map_err(|e| format!("file_content 序列化失败: {}", e))?
    };

    let schema_text = if task == "file" {
        r#"{
  "knowledge_base_type": "string，可选",
  "file_tags": ["string"],
  "related_nodes": ["string"],
  "structure_nodes": [
    {
      "stable_id": "string，可选",
      "title": "string，可选",
      "level": 1,
      "tags": ["string"],
      "related_nodes": ["string"],
      "attrs_patch": { "key": "value" }
    }
  ],
  "knowledge_relations": [
    {
      "from_stable_id": "string",
      "to_stable_id": "string",
      "relation": "string"
    }
  ],
  "summary": "string"
}"#
    } else {
        r#"{
  "knowledge_base_type": "string",
  "kb_tags": ["string"],
  "summary": "string"
}"#
    };

    Ok(format!(
        "你是知识工程助手。请根据输入的知识库元数据做“知识属性配置、分类归纳”，输出严格 JSON，不要输出任何额外文字。\n\n任务类型: {task}\n知识库: {kb_name}\n文件相对路径: {rel_path}\n\n输出 JSON Schema:\n{schema}\n\n规则:\n1. 只返回一个 JSON 对象，不能使用 markdown 代码块。\n2. 结果必须可被 JSON.parse 解析。\n3. 不确定时给出保守建议，不要编造事实。\n4. tags/related_nodes 去重、简短、语义清晰。\n5. 文件任务中必须同时关注 file_tags、文件 related_nodes、structure_nodes 的 tags 与 related_nodes，不要只返回 attrs_patch。\n6. 节点关联关系要体现“文件级知识主题”与“段落/小节级知识分类描述”的映射，可通过 structure_nodes[*].related_nodes 与 knowledge_relations 表达。\n7. 必须结合整个知识库上下文（kb_context_files）进行分类与关系判断，不要只看单文件。\n8. 文件任务必须给出非空 file_tags 和 non-empty related_nodes（至少各 1 条）。\n9. 若结构节点无法可靠判断，可少给或不改 structure_nodes，但 file_tags 与 related_nodes 不能为空。\n\n输入 kb_meta:\n{kb_meta}\n\n输入 kb_context_files:\n{kb_context_files}\n\n输入 file_meta:\n{file_meta}\n\n输入 file_content:\n{file_content}\n",
        task = task,
        kb_name = kb_name,
        rel_path = request.rel_path.trim(),
        schema = schema_text,
        kb_meta = kb_meta_json,
        kb_context_files = kb_context_files_json,
        file_meta = file_meta_json,
        file_content = file_content_json
    ))
}

/// 通用 LLM 补全入口(供知识图谱抽取等复用)。按 modelid 路由到对应后端,
/// 远程 API 失败时自动回退本地 DeepSeek,体现“本地优先 + 外部 API 预留”的分层后端。
pub async fn kg_llm_complete(
    modelid: i32,
    prompt: &str,
    sessionid: &str,
) -> Result<(String, String), String> {
    let model = ChatModel::try_from(modelid).unwrap_or(ChatModel::LOCALDEEPSEEK);
    match kb_agent_call_model(model, prompt, sessionid).await {
        Ok(rsp) => Ok((rsp.answer, rsp.sessionid)),
        Err(e) => {
            // 远程/付费后端失败 → 回退本地模型,保证可用性
            warn!("kg_llm_complete modelid={} 失败,回退本地 DeepSeek: {}", modelid, e);
            let rsp = kb_agent_call_model(ChatModel::LOCALDEEPSEEK, prompt, sessionid).await?;
            Ok((rsp.answer, rsp.sessionid))
        }
    }
}

async fn kb_agent_call_model(model: ChatModel, prompt: &str, sessionid: &str) -> Result<ChatResponse, String> {
    match model {
        ChatModel::CHATGPT => get_answer_from_chatgpt(prompt, &[], sessionid).await,
        ChatModel::DEEPSEEK => get_answer_from_deepseek(prompt, &[], sessionid).await,
        ChatModel::QWEN => get_answer_from_qwen(prompt, &[], sessionid).await,
        ChatModel::KIMI => get_answer_from_kimi(prompt, &[], sessionid).await,
        ChatModel::DOUBAO => get_answer_from_doubao(prompt, &[], sessionid).await,
        ChatModel::LOCALQWEN => get_answer_from_siliconflow(prompt, &[], sessionid).await,
        ChatModel::LOCALDEEPSEEK => get_answer_from_local_deepseek(prompt, &[], sessionid).await,
        ChatModel::LOCALQWEN3 => get_answer_from_local_qwen(prompt, &[], sessionid).await,
        ChatModel::CUSTOM => get_answer_from_custom(prompt, &[], sessionid).await,
        ChatModel::CCSWITCH => get_answer_from_cc_switch(prompt, &[], sessionid).await,
        ChatModel::YUANBAO_WEB | ChatModel::CHATGPT_WEB => {
            Err("当前网页模型不支持知识处理 Agent 命令，请切换到 API 模型".to_string())
        }
    }
}

/// AI 影像工坊的文稿编译请求。
/// modelid 完全复用聊天后端，因此云端供应商、自定义 OpenAI 兼容接口和本地模型都可使用。
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StoryStudioCompileRequest {
    pub modelid: i32,
    #[serde(default)]
    pub sessionid: String,
    #[serde(default)]
    pub source_name: String,
    pub markdown: String,
    #[serde(default)]
    pub genre: String,
    #[serde(default)]
    pub visual_style: String,
    pub target_duration_seconds: Option<u32>,
    #[serde(default)]
    pub production_mode: String,
    #[serde(default)]
    pub additional_direction: String,
}

/// Story IR 同时返回原始模型文本，方便开发期诊断模型是否遵循 JSON 约束。
/// 前端默认只展示已解析的 story_ir，不会执行 raw_answer 中的任何代码或指令。
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StoryStudioCompileResponse {
    pub story_ir: Value,
    pub raw_answer: String,
    pub sessionid: String,
    pub warnings: Vec<String>,
}

fn build_story_studio_prompt(request: &StoryStudioCompileRequest) -> Result<String, String> {
    let markdown = request.markdown.trim();
    if markdown.is_empty() {
        return Err("Markdown 文稿为空".to_string());
    }
    // 约 150 万中文字符已远超一次故事编译的合理上下文，提前阻断可避免本地模型内存失控。
    if markdown.chars().count() > 1_500_000 {
        return Err("文稿过长，请先按卷、章或场景拆分后再生成 Story IR".to_string());
    }

    let source_json = serde_json::to_string(markdown).map_err(|e| e.to_string())?;
    let source_name_json = serde_json::to_string(request.source_name.trim()).map_err(|e| e.to_string())?;
    let genre_json = serde_json::to_string(request.genre.trim()).map_err(|e| e.to_string())?;
    let style_json = serde_json::to_string(request.visual_style.trim()).map_err(|e| e.to_string())?;
    let mode_json = serde_json::to_string(request.production_mode.trim()).map_err(|e| e.to_string())?;
    let direction_json =
        serde_json::to_string(request.additional_direction.trim()).map_err(|e| e.to_string())?;
    let duration = request.target_duration_seconds.unwrap_or(60).clamp(5, 10_800);

    Ok(format!(
        r#"你是 Guaxi AI 影像工坊的故事编译器。你的任务是把 Markdown 文稿编译为可由 Blender 5.2 LTS 执行的 Story IR。

只返回一个严格 JSON 对象，不要 Markdown 代码块，不要解释，不要生成 Python。必须保持原文事实与叙事意图；史书和地理文稿中不得虚构日期、人物、地名、引用或因果关系，不确定信息写入 researchGaps。

输入配置：
- sourceName: {source_name_json}
- genre: {genre_json}
- visualStyle: {style_json}
- productionMode: {mode_json}
- targetDurationSeconds: {duration}
- additionalDirection: {direction_json}

输出必须符合以下结构。允许添加字段，但这些字段不可缺少：
{{
  "schemaVersion": "1.0",
  "metadata": {{
    "title": "string",
    "logline": "string",
    "genre": "history|geography|novel|documentary|education|other",
    "visualStyle": "string",
    "language": "zh-CN",
    "fps": 24,
    "targetDurationSeconds": number,
    "targetBlenderVersion": "5.2 LTS"
  }},
  "research": {{
    "facts": [{{"id":"fact_001","claim":"string","sourceAnchor":"原文标题或段落","confidence":0.0}}],
    "researchGaps": ["string"],
    "sensitiveDepictions": ["string"]
  }},
  "entities": {{
    "characters": [{{"id":"char_001","name":"string","description":"string","appearance":"string","wardrobe":"string","assetHints":["string"]}}],
    "locations": [{{"id":"loc_001","name":"string","description":"string","geo":{{"latitude":null,"longitude":null}},"era":"string","assetHints":["string"]}}],
    "props": [{{"id":"prop_001","name":"string","description":"string","assetHints":["string"]}}]
  }},
  "production": {{
    "renderEngine": "BLENDER_EEVEE",
    "resolution": {{"width":1280,"height":720,"percentage":50}},
    "colorManagement": "AgX",
    "audio": {{"dialogue":true,"narration":true,"music":true,"soundEffects":true}},
    "deliverables": ["storyboard","preview_blend","preview_mp4","subtitle_srt"]
  }},
  "scenes": [
    {{
      "id": "scene_001",
      "title": "string",
      "summary": "string",
      "locationId": "loc_001或空字符串",
      "time": "string",
      "durationSeconds": number,
      "sourceAnchors": ["原文标题或段落"],
      "shots": [
        {{
          "id":"shot_001",
          "durationSeconds": number,
          "shotType":"wide|medium|closeup|aerial|map|insert",
          "cameraMovement":"static|pan|tilt|dolly|orbit|tracking",
          "description":"画面中可观察到的内容",
          "characters":["char_001"],
          "action":"string",
          "dialogue":[{{"speakerId":"char_001","text":"string"}}],
          "narration":"string",
          "onScreenText":"string",
          "assetRequirements":["string"],
          "transition":"cut|fade|dissolve|wipe"
        }}
      ]
    }}
  ],
  "assetPlan": [{{"id":"asset_001","type":"character|environment|prop|map|archive|material","description":"string","searchTags":["string"],"licenseRequired":true}}],
  "audioPlan": {{"narrationStyle":"string","musicStyle":"string","soundEffects":["string"]}},
  "publishing": {{"aspectRatios":["16:9"],"platforms":["video-platform"],"needsFactReview":true,"needsRightsReview":true}}
}}

规则：
1. 所有 id 在文档内稳定且唯一；关联必须引用已存在 id。
2. scenes 至少一项，每个 scene 至少一个 shot；总时长尽量接近目标时长。
3. 镜头描述必须可视化，不使用“表达了、体现了”代替画面动作。
4. 史书保留年代、制度、人物关系和史料不确定性；地理内容优先规划地图、地形、路线与尺度参照。
5. 小说区分叙述、对白、动作和内心活动；内心活动优先转为旁白或可见行为。
6. 所有资产只给需求和标签，不假定文件已经存在。
7. 不要在 JSON 字符串中放未转义换行，不要尾随逗号。

Markdown 原文 JSON 字符串：
{source_json}"#
    ))
}

/// 将 Markdown 编译为 Story IR。该命令是“规划层”，不启动 Blender，也不会写入用户文件。
#[tauri::command]
pub async fn story_studio_compile_markdown(
    request: StoryStudioCompileRequest,
) -> Result<StoryStudioCompileResponse, String> {
    let prompt = build_story_studio_prompt(&request)?;
    let sessionid = if request.sessionid.trim().is_empty() {
        generate_session_id()
    } else {
        request.sessionid.trim().to_string()
    };
    info!(
        modelid = request.modelid,
        source = %request.source_name,
        markdown_chars = request.markdown.chars().count(),
        "[StoryStudio] compiling markdown to Story IR"
    );

    let (raw_answer, response_sessionid) =
        kg_llm_complete(request.modelid, &prompt, &sessionid).await?;
    let parsed = parse_kb_agent_json(&raw_answer)
        .map_err(|e| format!("模型未返回有效 Story IR：{e}"))?;
    let (story_ir, warnings) = crate::commands::story_ir::validate_and_normalize_story_ir(parsed)?;
    let scene_count = story_ir
        .get("scenes")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    info!(
        modelid = request.modelid,
        scene_count,
        warning_count = warnings.len(),
        "[StoryStudio] Story IR compiled"
    );
    Ok(StoryStudioCompileResponse {
        story_ir,
        raw_answer,
        sessionid: response_sessionid,
        warnings,
    })
}

#[derive(Deserialize, Debug, Clone)]
pub struct PythonCodegenRequest {
    #[serde(default)]
    pub modelid: i32,
    #[serde(default)]
    pub sessionid: String,
    pub instruction: String,
    #[serde(default)]
    pub context: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct PythonCodegenResponse {
    pub code: String,
    pub raw_answer: String,
    pub sessionid: String,
}

fn build_python_codegen_prompt(request: &PythonCodegenRequest) -> Result<String, String> {
    let instruction = request.instruction.trim();
    if instruction.is_empty() {
        return Err("代码生成指令为空".to_string());
    }
    let mut prompt = String::new();
    prompt.push_str(
        "你是 Guaxi 探索者笔记本里的 Python 编程助手。根据用户的自然语言需求生成可直接运行的 Python 代码。\n\
         要求:\n\
         1. 只输出一个 ```python 代码块,块外不要任何解释文字。\n\
         2. 代码要完整、可直接运行;需要第三方库时正常 import。\n\
         3. 在关键处用简短行内注释说明选用了哪个库/API 以及为什么(这是用户最需要的)。\n\
         4. 不臆造不存在的 API;不确定的外部输入用合理的占位或示例值。\n\
         5. 运行环境已预装常用数据科学库:pandas、numpy、matplotlib、plotly、scipy、\n\
            scikit-learn、statsmodels、seaborn、sympy、pyarrow、duckdb、polars、openpyxl、Pillow 等。\n\
            优先使用这些已装库;若确实需要其他第三方库,请在代码第一行用注释标注需要 pip 安装的包名,例如 `# 需要: requests`。\n",
    );
    if !request.context.trim().is_empty() {
        prompt.push_str("\n当前 cell 已有内容(供参考,可基于它续写或改写):\n```python\n");
        prompt.push_str(request.context.trim());
        prompt.push_str("\n```\n");
    }
    prompt.push_str("\n用户需求:\n");
    prompt.push_str(instruction);
    Ok(prompt)
}

fn strip_python_code_fence(answer: &str) -> String {
    // 推理模型(DeepSeek-R1 等)可能带 <think>...</think>,取最后一个 </think> 之后的正式回答。
    let mut text = answer.trim();
    if let Some(pos) = text.rfind("</think>") {
        text = text[pos + "</think>".len()..].trim();
    }
    // 从 ```python / ``` 代码围栏里抽取纯代码;没有围栏则原样返回。
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        let body_start = after.find('\n').map(|nl| nl + 1).unwrap_or(0);
        let body = &after[body_start..];
        if let Some(end) = body.find("```") {
            return body[..end].trim_matches('\n').to_string();
        }
        return body.trim_matches('\n').to_string();
    }
    text.to_string()
}

// PLACEHOLDER_PYTHON_CODEGEN_COMMAND

#[tauri::command]
pub async fn python_codegen_from_prompt(
    request: PythonCodegenRequest,
) -> Result<PythonCodegenResponse, String> {
    let prompt = build_python_codegen_prompt(&request)?;
    let sessionid = if request.sessionid.trim().is_empty() {
        generate_session_id()
    } else {
        request.sessionid.trim().to_string()
    };
    info!(
        modelid = request.modelid,
        instruction_chars = request.instruction.chars().count(),
        has_context = !request.context.trim().is_empty(),
        "[PythonCodegen] generating python from natural language"
    );
    let (raw_answer, response_sessionid) =
        kg_llm_complete(request.modelid, &prompt, &sessionid).await?;
    let code = strip_python_code_fence(&raw_answer);
    Ok(PythonCodegenResponse {
        code,
        raw_answer,
        sessionid: response_sessionid,
    })
}

/// 文档 Agent 请求：默认走本地 LocalQwen(id 9)，也可指定任意已接入模型。
/// 仅承载“结构化任务”（classify/tag）；散文类任务(write/translate/organize)由前端复用流式聊天通道。
#[derive(Deserialize, Debug, Clone)]
pub struct DocAgentRequest {
    #[serde(default)]
    pub modelid: Option<i32>,
    #[serde(default)]
    pub sessionid: String,
    pub task: String,
    #[serde(default)]
    pub doc_text: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub target_lang: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct DocAgentResponse {
    pub sessionid: String,
    pub modelid: i32,
    pub task: String,
    pub result: Value,
    pub raw_answer: String,
}

fn build_doc_agent_prompt(task: &str, doc_text: &str, categories: &[String]) -> String {
    let doc = doc_text.trim();
    match task {
        "classify" => {
            let cat_hint = if categories.is_empty() {
                "可自行给出最贴切的单一分类名称（中文，2~6 字）。".to_string()
            } else {
                format!("只能从以下候选分类中选择最贴切的一个：{}", categories.join("、"))
            };
            format!(
                "你是文档分类助手。请阅读下面的文档内容，判断它最合适的单一分类。\n\
                 {cat_hint}\n\
                 只返回 JSON，不要输出任何额外文字、解释或代码块标记。\n\
                 JSON 格式：{{\"category\":\"string\",\"confidence\":number,\"reason\":\"string\"}}\n\
                 其中 confidence 取 0~1 之间的小数，reason 用一句中文说明理由。\n\n\
                 文档内容：\n{doc}"
            )
        }
        _ => {
            // 默认按打标签处理
            format!(
                "你是文档打标签助手。请阅读下面的文档内容，提炼最具代表性的标签。\n\
                 只返回 JSON，不要输出任何额外文字、解释或代码块标记。\n\
                 JSON 格式：{{\"tags\":[\"string\"],\"reason\":\"string\"}}\n\
                 约束：标签为中文短词，去重，最多 8 个，按重要性排序；reason 用一句中文说明依据。\n\n\
                 文档内容：\n{doc}"
            )
        }
    }
}

fn normalize_doc_agent_result(task: &str, value: Value) -> Value {
    let Some(obj) = value.as_object() else {
        return value;
    };

    if task == "classify" {
        let category = alias_object_value(obj, &["category", "type", "label", "分类"])
            .and_then(|v| v.as_str().map(|s| s.trim().to_string()))
            .unwrap_or_default();
        let confidence = alias_object_value(obj, &["confidence", "score", "置信度"])
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let reason = alias_object_value(obj, &["reason", "reasoning", "理由"])
            .and_then(|v| v.as_str().map(|s| s.trim().to_string()))
            .unwrap_or_default();
        return json!({
            "category": category,
            "confidence": confidence,
            "reason": reason,
        });
    }

    // tag
    let raw_tags = alias_object_value(obj, &["tags", "labels", "keywords", "标签"]);
    let mut tags: Vec<String> = Vec::new();
    if let Some(Value::Array(arr)) = raw_tags {
        for item in arr {
            let t = match item {
                Value::String(s) => s.trim().to_string(),
                other => other.as_str().unwrap_or("").trim().to_string(),
            };
            if t.is_empty() || tags.iter().any(|x| x == &t) {
                continue;
            }
            tags.push(t);
            if tags.len() >= 8 {
                break;
            }
        }
    }
    let reason = alias_object_value(obj, &["reason", "reasoning", "依据"])
        .and_then(|v| v.as_str().map(|s| s.trim().to_string()))
        .unwrap_or_default();
    json!({ "tags": tags, "reason": reason })
}

/// 嵌入式文档 Agent：分类 / 打标签 等结构化任务。
/// 通过 kg_llm_complete 路由模型（默认 LocalQwen id 9，远程失败自动回退本地）。
#[tauri::command]
pub async fn doc_agent_process(request: DocAgentRequest) -> Result<DocAgentResponse, String> {
    let modelid = request.modelid.unwrap_or(9);
    let task = match request.task.trim().to_lowercase().as_str() {
        "classify" => "classify",
        _ => "tag",
    };
    if request.doc_text.trim().is_empty() {
        return Err("文档内容为空，无法执行 Agent 任务".to_string());
    }
    info!(
        "doc_agent_process: modelid={} task={} doc_len={}",
        modelid,
        task,
        request.doc_text.len()
    );

    let prompt = build_doc_agent_prompt(task, &request.doc_text, &request.categories);
    let mut last_error: Option<String> = None;
    let mut last_session = request.sessionid.clone();

    for round in 0..3usize {
        let (answer, session) = match timeout(
            Duration::from_secs(300),
            kg_llm_complete(modelid, &prompt, &last_session),
        )
        .await
        {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err("文档 Agent 响应超时（>300s）".to_string()),
        };
        last_session = session.clone();

        match parse_kb_agent_json(&answer) {
            Ok(parsed) => {
                let result = normalize_doc_agent_result(task, parsed);
                return Ok(DocAgentResponse {
                    sessionid: session,
                    modelid,
                    task: task.to_string(),
                    result,
                    raw_answer: answer,
                });
            }
            Err(e) => {
                warn!("doc_agent_process parse failed round={} err={}", round, e);
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "文档 Agent 未返回有效 JSON".to_string()))
}

#[tauri::command]
pub async fn kbmeta_agent_process(request: KbMetaAgentRequest) -> Result<KbMetaAgentResponse, String> {
    let _model = ChatModel::try_from(request.modelid).map_err(|e| format!("Invalid model ID: {}", e))?;
    let task = normalize_kb_agent_task(&request.task);
    info!(
        "kbmeta_agent_process: modelid={} task={} kb_name={} rel_path={}",
        request.modelid, task, request.kb_name, request.rel_path
    );

    let mut work = request.clone();
    let mut last_error: Option<String> = None;

    for round in 0..5usize {
        let prompt = build_kb_agent_prompt(&work, &task)?;
        let model = ChatModel::try_from(request.modelid).map_err(|e| format!("Invalid model ID: {}", e))?;
        let rsp = match timeout(
            Duration::from_secs(90),
            kb_agent_call_model(model, &prompt, &work.sessionid),
        )
        .await
        {
            Ok(inner) => match inner {
                Ok(v) => v,
                Err(e) => {
                    if is_prompt_too_large_error(&e) {
                        let shrunk = shrink_kb_agent_request_for_retry(&mut work, round);
                        warn!(
                            "kbmeta_agent_process prompt too large round={} rel_path={} shrunk={}",
                            round, work.rel_path, shrunk
                        );
                        last_error = Some(e.clone());
                        if shrunk {
                            continue;
                        }
                    }
                    return Err(e);
                }
            },
            Err(_) => {
                let timeout_err = format!(
                    "上游响应超时（>90s），rel_path={} round={}",
                    work.rel_path, round
                );
                warn!("kbmeta_agent_process {}", timeout_err);
                last_error = Some(timeout_err.clone());
                let shrunk = shrink_kb_agent_request_for_retry(&mut work, round);
                if shrunk {
                    continue;
                }
                return Err(timeout_err);
            }
        };

        let parsed = match parse_kb_agent_json(&rsp.answer) {
            Ok(v) => v,
            Err(e) => {
                last_error = Some(e.clone());
                if round < 2 {
                    let shrunk = shrink_kb_agent_request_for_retry(&mut work, round + 1);
                    warn!(
                        "kbmeta_agent_process parse failed round={} rel_path={} shrunk={}",
                        round, work.rel_path, shrunk
                    );
                    if shrunk {
                        continue;
                    }
                }
                return Err(e);
            }
        };

        let normalized = normalize_kb_agent_result(parsed);
        let normalized = if task == "file" {
            ensure_non_empty_file_fields(&work, normalized)
        } else {
            normalized
        };
        info!(
            "kbmeta_agent_process: normalized_result_keys={:?}",
            normalized
                .as_object()
                .map(|m| m.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
        );

        return Ok(KbMetaAgentResponse {
            sessionid: rsp.sessionid,
            modelid: request.modelid,
            task,
            result: normalized,
            raw_answer: rsp.answer,
        });
    }

    if task == "file" {
        let fallback = ensure_non_empty_file_fields(&work, json!({}));
        warn!(
            "kbmeta_agent_process fallback rel_path={} reason={}",
            work.rel_path,
            last_error.unwrap_or_else(|| "unknown".to_string())
        );
        return Ok(KbMetaAgentResponse {
            sessionid: work.sessionid.clone(),
            modelid: request.modelid,
            task,
            result: fallback,
            raw_answer: String::new(),
        });
    }

    Err(last_error.unwrap_or_else(|| "知识处理失败".to_string()))
}

fn resolve_audio_provider(provider: &str) -> Result<String, String> {
    let p = provider.trim().to_lowercase();
    if p.is_empty() {
        return Ok("sherpa-onnx".to_string());
    }
    if p == "siliconflow" {
        return Ok("siliconflow".to_string());
    }
    if p == "sherpa-onnx" || p == "sherpa_onnx" || p == "sherpa" {
        return Ok("sherpa-onnx".to_string());
    }
    Err(format!("不支持的音频 provider: {}", provider))
}

fn resolve_audio_tts_model(model: &str) -> String {
    let m = model.trim();
    if m.is_empty() {
        SILICONFLOW_AUDIO_TTS_DEFAULT_MODEL.to_string()
    } else {
        m.to_string()
    }
}

fn resolve_audio_stt_model(model: &str) -> String {
    let m = model.trim();
    if m.is_empty() {
        SILICONFLOW_AUDIO_STT_DEFAULT_MODEL.to_string()
    } else {
        m.to_string()
    }
}

fn resolve_audio_tts_voice(voice: &str) -> String {
    let v = voice.trim();
    if v.is_empty() {
        SILICONFLOW_AUDIO_TTS_DEFAULT_VOICE.to_string()
    } else {
        v.to_string()
    }
}

fn build_siliconflow_voice_candidates(voice: &str, model: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let primary = resolve_audio_tts_voice(voice);
    if !primary.is_empty() {
        out.push(primary.clone());
    }
    if let Some((_, suffix)) = primary.rsplit_once(':') {
        let s = suffix.trim();
        if !s.is_empty() {
            out.push(s.to_string());
        }
    }
    let model_trimmed = model.trim();
    if !model_trimmed.is_empty() {
        out.push(format!("{}:{}", model_trimmed, SILICONFLOW_AUDIO_TTS_DEFAULT_VOICE));
    }
    out.push(SILICONFLOW_AUDIO_TTS_DEFAULT_VOICE.to_string());
    if out.is_empty() {
        out.push(SILICONFLOW_AUDIO_TTS_DEFAULT_VOICE.to_string());
    }
    let mut dedup: Vec<String> = Vec::new();
    for item in out {
        if !dedup.iter().any(|x| x == &item) {
            dedup.push(item);
        }
    }
    dedup
}

fn resolve_audio_api_key(api_key: &str) -> Result<String, String> {
    let inline = api_key.trim();
    if !inline.is_empty() {
        return Ok(inline.to_string());
    }
    let cfg = settingview::get_llm_config();
    let key = cfg.siliconflow_api_key.trim().to_string();
    if key.is_empty() {
        return Err("硅基流动 API Key 未配置".to_string());
    }
    Ok(key)
}

fn extract_trace_id(headers: &reqwest::header::HeaderMap) -> String {
    headers
        .get("x-siliconcloud-trace-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn parse_speaker_id(voice: &str) -> i32 {
    let s = voice.trim();
    if s.is_empty() {
        return 0;
    }
    s.parse::<i32>().unwrap_or(0)
}

async fn audio_text_to_speech_sherpa_onnx(request: TextToSpeechRequest) -> Result<TextToSpeechResponse, String> {
    let text = request.input.trim().to_string();
    if text.is_empty() {
        return Err("input 不能为空".to_string());
    }

    let speaker = resolve_tts_speaker(&request.voice);
    // 请求没带语速时用设置页的全局朗读语速（0.5–2.0）；带上就以请求为准。
    let speed = request
        .speed
        .filter(|value| *value > 0.0)
        .map(|value| value.clamp(0.5, 2.0))
        .unwrap_or_else(|| crate::commands::settingview::get_tts_config().speed as f32);
    // 与 ASR 同一条规矩：请求没带模型/tokens 就用设置里的默认值。
    // 各朗读入口只传自己关心的字段，不该因为漏传一个路径就整条链路失败。
    let configured = settingview::get_tts_config();
    let model = if request.common.model.trim().is_empty() {
        configured.model.clone()
    } else {
        request.common.model.clone()
    };
    let tokens = if request.common.tokens.trim().is_empty() {
        configured.tokens.clone()
    } else {
        request.common.tokens.clone()
    };
    if model.trim().is_empty() {
        return Err(
            "未配置 TTS 模型：请在设置页指定模型文件（默认 utils/Models/tts/kokoro_multi_lang/model.onnx），\
             或运行 tools/fetch_sherpa_onnx_libs.mjs 下载默认模型"
                .to_string(),
        );
    }

    // 日志放在解析之前：解析失败会提前返回，日志若在它后面就不会留下任何线索。
    info!(
        requested_model = request.common.model.trim(),
        requested_tokens = request.common.tokens.trim(),
        runtime_provider = request.common.runtime_provider.trim(),
        mode_hint = request.common.mode_hint.trim(),
        speaker,
        speed,
        text_len = text.len(),
        "audio_text_to_speech using sherpa-onnx in-process"
    );

    // 合成在进程内完成，模型只加载一次；cuda 失败时按老规矩回退 cpu。
    let mut last_err = String::new();
    for provider in provider_attempts(&request.common.runtime_provider) {
        let config = crate::guaxiservices::speech::resolve_tts_engine_config(
            &model,
            &tokens,
            &request.common.mode_hint,
            &provider,
            None,
        )?;

        match crate::guaxiservices::speech::synthesize_wav(config, text.clone(), speaker, speed).await {
            Ok(wav) => {
                return Ok(TextToSpeechResponse {
                    audio_base64: general_purpose::STANDARD.encode(wav),
                    content_type: "audio/wav".to_string(),
                    trace_id: "local-sherpa-onnx".to_string(),
                    model: model.clone(),
                    response_format: "wav".to_string(),
                })
            }
            Err(err) => {
                warn!(
                    provider = provider.as_str(),
                    error = err.as_str(),
                    "sherpa-onnx 合成失败"
                );
                last_err = err;
            }
        }
    }
    Err(last_err)
}

/// 先按请求的 provider 试，cuda 失败再回退 cpu——与旧 CLI 路径的回退语义一致。
/// 请求没给 voice 时回落到设置里的默认音色，
/// 这样各朗读入口不必各自硬编码一个编号。
fn resolve_tts_speaker(voice: &str) -> i32 {
    let requested = voice.trim();
    if !requested.is_empty() {
        return parse_speaker_id(requested);
    }
    parse_speaker_id(&settingview::get_tts_config().speaker_id)
}

/// 先按请求的 provider 试，cuda 失败再回退 cpu。
///
/// 注意归一化走的是 speech 模块里**带 CUDA 可用性探测**的那一份：
/// 直接请求 cuda 而实际缺少驱动/EP 库时，onnxruntime 的 C++ 异常会穿透 FFI
/// 直接把进程 abort，回退分支根本没机会跑。所以必须在请求之前就降级。
fn provider_attempts(requested: &str) -> Vec<String> {
    let provider = crate::guaxiservices::speech::normalize_provider(requested);
    if provider == "cuda" {
        // 已确认 CUDA 可用，但推理仍可能因显存/驱动版本失败，保留一次 cpu 重试。
        vec![provider, "cpu".to_string()]
    } else {
        vec![provider]
    }
}

#[tauri::command]
pub async fn audio_text_to_speech(request: TextToSpeechRequest) -> Result<TextToSpeechResponse, String> {
    let provider = resolve_audio_provider(&request.common.provider)?;
    info!(
        requested_provider = request.common.provider.trim(),
        resolved_provider = provider.as_str(),
        requested_model = request.common.model.trim(),
        has_custom_endpoint = !request.common.endpoint.trim().is_empty(),
        "audio_text_to_speech provider resolved"
    );
    if provider == "sherpa-onnx" {
        return audio_text_to_speech_sherpa_onnx(request).await;
    }
    let api_key = resolve_audio_api_key(&request.common.api_key)?;
    let model = resolve_audio_tts_model(&request.common.model);
    let voice_candidates = build_siliconflow_voice_candidates(&request.voice, &model);
    let endpoint = {
        let custom = request.common.endpoint.trim();
        if custom.is_empty() {
            SILICONFLOW_AUDIO_SPEECH_URL.to_string()
        } else {
            custom.to_string()
        }
    };
    let input = request.input.trim().to_string();
    if input.is_empty() {
        return Err("input 不能为空".to_string());
    }

    let mut body = json!({
        "model": model,
        "input": input,
        "stream": request.stream.unwrap_or(false)
    });

    if let Some(max_tokens) = request.max_tokens {
        body["max_tokens"] = json!(max_tokens);
    }
    if let Some(sample_rate) = request.sample_rate {
        body["sample_rate"] = json!(sample_rate);
    }
    // 与本地 sherpa 路径一致：请求没带语速时用设置页的全局朗读语速。
    let speed = request
        .speed
        .filter(|value| *value > 0.0)
        .unwrap_or_else(|| crate::commands::settingview::get_tts_config().speed as f32)
        .clamp(0.5, 2.0);
    body["speed"] = json!(speed);
    if let Some(gain) = request.gain {
        body["gain"] = json!(gain);
    }
    body["voice"] = json!(voice_candidates[0].clone());
    if !request.response_format.trim().is_empty() {
        body["response_format"] = json!(request.response_format.trim());
    }

    let client = ClientBuilder::new()
        .timeout(StdDuration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let mut headers_opt: Option<reqwest::header::HeaderMap> = None;
    let mut bytes_opt: Option<Vec<u8>> = None;
    let mut last_error = String::new();
    let mut attempts: Vec<Option<String>> = voice_candidates.into_iter().map(Some).collect();
    attempts.push(None);
    for i in 0..attempts.len() {
        match &attempts[i] {
            Some(voice) => {
                body["voice"] = json!(voice);
            }
            None => {
                if let Some(obj) = body.as_object_mut() {
                    obj.remove("voice");
                }
            }
        }
        let response = client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if response.status().is_success() {
            headers_opt = Some(response.headers().clone());
            bytes_opt = Some(response.bytes().await.map_err(|e| e.to_string())?.to_vec());
            break;
        }
        let status = response.status();
        let err_text = response.text().await.unwrap_or_default();
        last_error = format!("文本转语音失败: {} {}", status, err_text);
        let invalid_voice = err_text.contains("Invalid voice") || err_text.contains("invalid voice");
        if !invalid_voice || i + 1 >= attempts.len() {
            return Err(last_error);
        }
    }
    let headers = match headers_opt {
        Some(h) => h,
        None => return Err(last_error),
    };
    let bytes = match bytes_opt {
        Some(b) => b,
        None => return Err(last_error),
    };
    let content_type = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let trace_id = extract_trace_id(&headers);
    let audio_base64 = general_purpose::STANDARD.encode(bytes);

    Ok(TextToSpeechResponse {
        audio_base64,
        content_type,
        trace_id,
        model,
        response_format: request.response_format.trim().to_string(),
    })
}

async fn audio_speech_to_text_sherpa_onnx(request: SpeechToTextRequest) -> Result<SpeechToTextResponse, String> {
    let configured = settingview::get_asr_config();
    let model = {
        let from_req = request.common.model.trim();
        if !from_req.is_empty() {
            from_req.to_string()
        } else {
            configured.model.trim().to_string()
        }
    };
    if model.trim().is_empty() {
        return Err(
            "未配置 ASR 模型：请在设置中指定模型目录，或运行 tools/download_asr_model 下载到 utils/Models/asr/sense-voice".to_string(),
        );
    }

    // 音频直接在内存里交给识别器：不再落临时文件，也不必再读回来。
    let wav_bytes = if !request.file_path.trim().is_empty() {
        let path = PathBuf::from(request.file_path.trim());
        fs::read(&path).map_err(|e| {
            format!("读取音频文件失败（{}）: {}", path.to_string_lossy(), e)
        })?
    } else if !request.file_base64.trim().is_empty() {
        general_purpose::STANDARD
            .decode(request.file_base64.trim())
            .map_err(|e| format!("file_base64 解码失败: {}", e))?
    } else {
        return Err("file_path 或 file_base64 至少提供一个".to_string());
    };

    let config = crate::guaxiservices::speech::resolve_asr_engine_config(
        &model,
        &request.common.runtime_provider,
    )?;
    info!(
        model = model.as_str(),
        bytes = wav_bytes.len(),
        "audio_speech_to_text sherpa-onnx in-process"
    );

    let text = crate::guaxiservices::speech::transcribe_wav(config, wav_bytes).await?;
    if text.trim().is_empty() {
        return Err("识别结果为空，请检查音频内容或 ASR 模型配置".to_string());
    }
    info!(text_len = text.trim().len(), "audio_speech_to_text sherpa-onnx done");

    Ok(SpeechToTextResponse {
        text: text.trim().to_string(),
        trace_id: "local-sherpa-onnx".to_string(),
        model,
    })
}

#[tauri::command]
pub async fn audio_speech_to_text(request: SpeechToTextRequest) -> Result<SpeechToTextResponse, String> {
    let provider = resolve_audio_provider(&request.common.provider)?;
    info!(
        requested_provider = request.common.provider.trim(),
        resolved_provider = provider.as_str(),
        requested_model = request.common.model.trim(),
        has_file_path = !request.file_path.trim().is_empty(),
        has_file_base64 = !request.file_base64.trim().is_empty(),
        file_base64_len = request.file_base64.trim().len(),
        mime_type = request.mime_type.trim(),
        "audio_speech_to_text provider resolved"
    );
    if provider == "sherpa-onnx" {
        return audio_speech_to_text_sherpa_onnx(request).await;
    }
    let api_key = resolve_audio_api_key(&request.common.api_key)?;
    let model = resolve_audio_stt_model(&request.common.model);
    let endpoint = {
        let custom = request.common.endpoint.trim();
        if custom.is_empty() {
            SILICONFLOW_AUDIO_TRANSCRIPTIONS_URL.to_string()
        } else {
            custom.to_string()
        }
    };

    let file_bytes = if !request.file_path.trim().is_empty() {
        fs::read(request.file_path.trim()).map_err(|e| e.to_string())?
    } else if !request.file_base64.trim().is_empty() {
        general_purpose::STANDARD
            .decode(request.file_base64.trim())
            .map_err(|e| format!("file_base64 解码失败: {}", e))?
    } else {
        return Err("file_path 或 file_base64 至少提供一个".to_string());
    };

    let file_name = if !request.file_name.trim().is_empty() {
        request.file_name.trim().to_string()
    } else if !request.file_path.trim().is_empty() {
        Path::new(request.file_path.trim())
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.wav")
            .to_string()
    } else {
        "audio.wav".to_string()
    };
    let mime_type = if request.mime_type.trim().is_empty() {
        "application/octet-stream".to_string()
    } else {
        request.mime_type.trim().to_string()
    };

    let part = Part::bytes(file_bytes)
        .file_name(file_name)
        .mime_str(&mime_type)
        .map_err(|e| e.to_string())?;
    let form = Form::new().text("model", model.clone()).part("file", part);

    let client = ClientBuilder::new()
        .timeout(StdDuration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(&endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        error!(
            status = %status,
            body = body.as_str(),
            "audio_speech_to_text remote request failed"
        );
        return Err(format!("语音转文本失败: {} {}", status, body));
    }

    let headers = response.headers().clone();
    let trace_id = extract_trace_id(&headers);
    let payload: Value = response.json().await.map_err(|e| e.to_string())?;
    let text = payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    info!(
        trace_id = trace_id.as_str(),
        text_len = text.trim().len(),
        "audio_speech_to_text remote transcription done"
    );

    Ok(SpeechToTextResponse {
        text,
        trace_id,
        model,
    })
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

fn local_deepseek_model_path() -> Result<PathBuf, String> {
    bundled_resources::ensure_relative_dir(&format!(
        "utils/Models/chat/{}",
        LOCAL_DEEPSEEK_MODEL_FILENAME
    ))
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

fn resolve_llama_server_path() -> Result<PathBuf, String> {
    let llama_dir = bundled_resources::ensure_relative_dir("utils/llama")?;
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
    let model_path = local_deepseek_model_path()?;
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
            let server_path = resolve_llama_server_path()?;
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

            apply_no_window_in_release(&mut command);

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

fn local_qwen_model_path() -> Result<PathBuf, String> {
    bundled_resources::ensure_relative_dir(&format!(
        "utils/Models/chat/{}",
        LOCAL_QWEN_MODEL_FILENAME
    ))
}

async fn is_local_qwen_server_ready() -> bool {
    let client = match ClientBuilder::new().no_proxy().build() {
        Ok(c) => c,
        Err(_) => return false,
    };

    if let Ok(resp) = client
        .get(format!("{}/health", LOCAL_QWEN_SERVER_URL))
        .send()
        .await
    {
        if resp.status().is_success() {
            return true;
        }
    }

    if let Ok(resp) = client
        .get(format!("{}/v1/models", LOCAL_QWEN_SERVER_URL))
        .send()
        .await
    {
        if resp.status().is_success() {
            return true;
        }
    }

    false
}

fn refresh_local_qwen_process_state() {
    if let Ok(mut guard) = LOCAL_QWEN_PROCESS.lock() {
        if let Some(child) = guard.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => guard.child = None,
                Ok(None) => {}
                Err(_) => guard.child = None,
            }
        }
    }
}

async fn ensure_local_qwen_server_started() -> Result<PathBuf, String> {
    let exe = exe_dir()?;
    let model_path = local_qwen_model_path()?;
    let log_dir = exe.join("logs");
    fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    let log_path = log_dir.join("local_qwen_server.log");
    if !model_path.is_file() {
        return Err(format!(
            "LocalQwen model not found: {}",
            model_path.to_string_lossy()
        ));
    }

    if is_local_qwen_server_ready().await {
        return Ok(model_path);
    }

    refresh_local_qwen_process_state();

    {
        let mut guard = LOCAL_QWEN_PROCESS
            .lock()
            .map_err(|_| "local qwen process lock poisoned".to_string())?;
        if guard.child.is_none() {
            let server_path = resolve_llama_server_path()?;
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
                .arg("18082")
                // 不显式指定 --n-gpu-layers：让 llama.cpp 按可用显存自动决定 GPU 层数，
                // 其余层回退 CPU。低显存机器(如 6GB)上强行固定 -ngl 会触发 fit 失败并中止加载。
                .arg("--ctx-size")
                .arg("8192")
                .arg("--batch-size")
                .arg("512")
                .arg("--ubatch-size")
                .arg("512")
                // 不使用 --mlock：低内存机器上锁定 20GB 模型进物理内存可能失败或抖动，
                // 默认 mmap 按需分页更稳妥。
                .arg("--flash-attn")
                .arg("on")
                .arg("--jinja")
                .arg("--temp")
                .arg("0.7")
                .arg("--top-p")
                .arg("0.9")
                .arg("--repeat-penalty")
                .arg("1.1")
                .stdout(Stdio::from(log_file))
                .stderr(Stdio::from(log_file_err))
                .stdin(Stdio::null());

            apply_no_window_in_release(&mut command);

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

    // 低配机器加载 35B 模型（多为 CPU 分页 + 部分 GPU 卸载）可能很慢，给足 10 分钟。
    for _ in 0..1200 {
        if is_local_qwen_server_ready().await {
            return Ok(model_path);
        }
        if let Ok(mut guard) = LOCAL_QWEN_PROCESS.lock() {
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
        sleep(Duration::from_millis(500)).await;
    }

    if let Ok(mut guard) = LOCAL_QWEN_PROCESS.lock() {
        if let Some(mut child) = guard.child.take() {
            let _ = child.kill();
        }
    }

    Err(format!(
        "LocalQwen service start timeout. log={}",
        log_path.to_string_lossy()
    ))
}

#[tauri::command]
pub async fn ensure_local_qwen_service() -> Result<LocalDeepseekServiceStatus, String> {
    let model_path = ensure_local_qwen_server_started().await?;
    Ok(LocalDeepseekServiceStatus {
        ready: true,
        endpoint: LOCAL_QWEN_CHAT_COMPLETIONS_URL.to_string(),
        model_path: model_path.to_string_lossy().to_string(),
    })
}

/// 联网检索（智能助手 / 认知层工具共用）。
///
/// 真正的实现在 `commands::web_search`：多引擎并发 + 正文摘录。这里只保留命令名与
/// 返回类型，兼容既有调用方。所有引擎都空时才退回本地 Python Agent 的 web_search
/// （它能用用户配置的 Tavily/Brave key 或 ddgs）。
#[tauri::command]
pub async fn search_web_duckduckgo(
    app: AppHandle,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<WebSearchItem>, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let cap = limit.unwrap_or(5).clamp(1, 8);

    let items = crate::commands::web_search::search(&q, cap).await;
    if !items.is_empty() {
        return Ok(items
            .into_iter()
            .map(|item| WebSearchItem {
                title: item.title,
                snippet: item.snippet,
                url: item.url,
            })
            .collect());
    }

    match search_web_agent_server_fallback(&app, &q, cap).await {
        Ok(fallback) if !fallback.is_empty() => {
            info!("search_web: agent_server fallback produced results");
            Ok(fallback)
        }
        Ok(_) => Ok(vec![]),
        Err(err) => {
            warn!("search_web: agent_server fallback failed: {}", err);
            Ok(vec![])
        }
    }
}

/// 所有引擎都没抓到时，复用本地 Python Agent 的 web_search 工具兜底。
async fn search_web_agent_server_fallback(
    app: &AppHandle,
    query: &str,
    limit: usize,
) -> Result<Vec<WebSearchItem>, String> {
    info!(
        "search_web: trying agent_server fallback query_len={} limit={}",
        query.len(),
        limit
    );
    crate::commands::agent_server::ensure_agent_server_started(app).await?;

    let client = ClientBuilder::new()
        .no_proxy()
        .timeout(StdDuration::from_secs(25))
        .build()
        .map_err(|e| format!("build agent search client failed: {}", e))?;
    let payload = json!({
        "name": "web_search",
        "args": {
            "query": query,
            "max_results": limit,
        },
        "work_dir": "",
    });

    let resp = client
        .post("http://127.0.0.1:9876/tools/exec")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("agent search request failed: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("agent search status={} body={}", status, text));
    }

    let wrapper = resp
        .json::<Value>()
        .await
        .map_err(|e| format!("agent search response parse failed: {}", e))?;
    let result_raw = wrapper
        .get("result")
        .and_then(|v| v.as_str())
        .unwrap_or("{}");
    let result = serde_json::from_str::<Value>(result_raw)
        .map_err(|e| format!("agent search result parse failed: {}", e))?;
    if let Some(err) = result.get("error").and_then(|v| v.as_str()) {
        warn!("search_web: agent_server fallback reported error: {}", err);
    }

    let mut items = Vec::new();
    if let Some(results) = result.get("results").and_then(|v| v.as_array()) {
        for item in results.iter().take(limit) {
            let title = item
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let url = item
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let snippet = item
                .get("snippet")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if title.is_empty() || url.is_empty() {
                continue;
            }
            items.push(WebSearchItem { title, snippet, url });
        }
    }

    info!(
        "search_web: agent_server fallback returned count={}",
        items.len()
    );
    Ok(items)
}

#[tauri::command]
pub async fn cancel_chat_stream() -> Result<(), String> {
    request_stream_cancel();
    // 取消：设置 Rust 侧停止标志（Python Agent Server 由 SSE 循环内通知）
    info!("cancel_chat_stream: cancellation requested");
    Ok(())
}

// 调用 DeepSeek API 获取回答
pub async fn get_answer_from_deepseek(
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<ChatResponse, String> {
    let provider = llm_provider::resolve_chat(1)?;
    let api_key = &provider.api_key;
    let url = &provider.endpoint;
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
        "model": provider.model,
        "messages": build_chat_messages(history, question),
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

#[allow(dead_code)]
fn local_math_system_prompt() -> &'static str {
    "输出数学公式时请严格遵循：行内公式用 $...$；块级公式用 $$...$$ 且 $$ 必须独占一行。块级公式格式必须为：\n$$\n公式内容\n$$\n并且闭合 $$ 后下一行开始正文，严禁在同一行继续文字。若处于列表项中，公式块前后都要留空行。不要用反引号包裹任何数学公式。若提供代码，必须带注释。"
}

fn stable_math_system_prompt() -> &'static str {
    // 设计思路：
    // 1. 从模型输出源头约束数学格式，减少前端/后端后处理的不确定性。
    // 2. 同时规范主公式和参数说明中的数学符号，避免模型把数学量写进反引号。
    // 3. 统一禁止不稳定的 LaTeX 环境写法，强制收敛到 $...$ / $$...$$。
    "输出数学时严格遵守：\n\
1. 行内公式只用 $...$；块级公式只用 $$...$$，且前后的 $$ 各自单独占一行。\n\
2. 禁止使用 \\[...\\]、\\(...\\) 及 \\begin{equation}/align/gather 等 LaTeX 环境。\n\
3. 禁止用反引号或 Markdown 强调(**...**/*...*)包裹数学内容；参数说明里的数学符号也要写成 $...$。\n\
4. 代码用 Markdown fenced code block，但数学内容绝不能放进代码块或反引号。"
}

/// 构造带多轮上下文的 messages：[system] + 过滤后的 history + [user: question]。
/// 仅保留 role 为 user/assistant 且 content 非空的历史条目，配合 cache_prompt 复用 KV 前缀。
fn build_chat_messages(history: &[ChatMsg], question: &str) -> Value {
    let mut msgs: Vec<Value> = Vec::with_capacity(history.len() + 2);
    msgs.push(json!({"role": "system", "content": stable_math_system_prompt()}));
    for m in history {
        let role = match m.role.as_str() {
            "user" | "assistant" => m.role.as_str(),
            _ => continue,
        };
        let content = m.content.trim();
        if content.is_empty() {
            continue;
        }
        msgs.push(json!({"role": role, "content": content}));
    }
    msgs.push(json!({"role": "user", "content": question}));
    Value::Array(msgs)
}

fn normalize_math_symbols(content: String) -> String {
    if content.is_empty() {
        return content;
    }

    // 设计说明（为什么不再是一串全文正则）：
    // 旧实现在**流式的每一个分片**上跑「把每个 $$ 前后都补换行」的规则，
    // 于是一句 “块级公式使用 $$...$$ 开启新行” 会被拆成跨行块，后半句变成块级公式的
    // 同行尾随文字，markdown-it 找不到闭合就一路吞到下一个 $$，整段正文变成一条
    // MathJax 失败后的转义文本（用户看到的就是“后面所有条目挤成一行”）。
    //
    // 现在的规则按「整行」判断，且只在真正属于数学定界符时改写：
    // - 行首 `$$ ... $$`（单行）与独占一行的 `$$` 视为块级公式，原样保留；
    // - 句中成对的 `$$...$$` 收敛为行内 `$...$`（markdown-it 只认行首的块级 $$）；
    // - 行内 `$ ... $` 去掉内侧空白，否则 mathjax3 不会接管；
    // - 代码围栏与块级公式内部一律不动。
    let raw = content;
    let mut out_lines: Vec<String> = Vec::new();
    let mut in_fence = false;
    let mut in_block_math = false;

    for line in raw.split('\n') {
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out_lines.push(line.to_string());
            continue;
        }
        if in_fence {
            out_lines.push(line.to_string());
            continue;
        }

        if trimmed == "$$" {
            in_block_math = !in_block_math;
            out_lines.push(line.to_string());
            continue;
        }
        if in_block_math {
            out_lines.push(line.to_string());
            continue;
        }

        // 整行的 \[ ... \] 收敛成块级公式。
        if trimmed.starts_with(r"\[") && trimmed.ends_with(r"\]") && trimmed.len() >= 4 {
            let body = trimmed[2..trimmed.len() - 2].trim();
            let indent = " ".repeat(line.len() - trimmed.len());
            out_lines.push(format!("{indent}$$"));
            out_lines.push(body.to_string());
            out_lines.push(format!("{indent}$$"));
            continue;
        }

        // 行首 $$ 后直接跟内容、本行又没有闭合：拆成块首 + 内容行。
        if trimmed.starts_with("$$") {
            let rest = &trimmed[2..];
            let rest_trim = rest.trim();
            if rest_trim.is_empty() || rest_trim.ends_with("$$") {
                out_lines.push(line.to_string());
                continue;
            }
            let indent = " ".repeat((line.len() - trimmed.len()).min(3));
            out_lines.push(format!("{indent}$$"));
            out_lines.push(rest.to_string());
            continue;
        }

        out_lines.extend(normalize_math_line(line));
    }

    let normalized = out_lines.join("\n");
    let dedented = normalize_block_math_indentation(&normalized);

    if dedented != raw {
        info!(
            "normalize_math_symbols: raw={} norm={}",
            excerpt_for_log(&raw),
            excerpt_for_log(&dedented)
        );
    }

    dedented
}

/// 把一行里「反引号代码段之外」的部分做定界符收敛。
fn normalize_math_line(line: &str) -> Vec<String> {
    let mut rebuilt = String::new();
    for (is_code, text) in split_inline_code(line) {
        if is_code {
            rebuilt.push_str(&text);
        } else {
            rebuilt.push_str(&normalize_math_text(&text));
        }
    }
    vec![rebuilt]
}

/// 判断某个位置上的字符是否被奇数个反斜杠转义（`\$` 不是数学定界符）。
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

/// 把一段行内文本里的 LaTeX 定界符统一成 `$` / `$$`：
/// - `\(...\)` -> `$...$`，`\[...\]` -> `$$...$$`
/// - 句中成对的 `$$...$$` 收敛成行内 `$...$`
/// - 行内 `$ ... $` 去掉内侧空白（否则 mathjax3 不会接管）
fn normalize_math_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let converted = text
        .replace(r"\(", "$")
        .replace(r"\)", "$")
        .replace(r"\[", "$$")
        .replace(r"\]", "$$");
    let chars: Vec<char> = converted.chars().collect();
    let mut out = String::with_capacity(converted.len());
    let mut i = 0usize;

    while i < chars.len() {
        let ch = chars[i];
        if ch != '$' || is_math_delimiter_escaped(&chars, i) {
            out.push(ch);
            i += 1;
            continue;
        }

        let is_double = i + 1 < chars.len() && chars[i + 1] == '$';
        let open_len = if is_double { 2 } else { 1 };
        let closer = find_math_closer(&chars, i + open_len, is_double);

        match closer {
            Some(end) => {
                let body: String = chars[i + open_len..end].iter().collect();
                let body = body.trim();
                if body.is_empty() {
                    out.push('$');
                    i += 1;
                    continue;
                }
                out.push('$');
                out.push_str(body);
                out.push('$');
                i = end + open_len;
            }
            None => {
                out.push('$');
                i += 1;
            }
        }
    }

    out
}

/// 从 `from` 开始找匹配的 `$` / `$$`；块级只认 `$$` 闭合，行内跳过成对的 `$$`。
fn find_math_closer(chars: &[char], from: usize, double: bool) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] == '$' && !is_math_delimiter_escaped(chars, i) {
            let is_double = i + 1 < chars.len() && chars[i + 1] == '$';
            if double {
                if is_double {
                    return Some(i);
                }
                i += 1;
                continue;
            }
            if is_double {
                i += 2;
                continue;
            }
            return Some(i);
        }
        i += 1;
    }
    None
}

/// 把一行按反引号拆成「代码段 / 普通文本」片段；未闭合的反引号按代码段处理（不改写）。
fn split_inline_code(line: &str) -> Vec<(bool, String)> {
    let mut segments: Vec<(bool, String)> = Vec::new();
    let mut current = String::new();
    let mut in_code = false;
    let mut tick_run = 0usize;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] != '`' {
            current.push(chars[i]);
            i += 1;
            continue;
        }

        let mut run = 0usize;
        while i + run < chars.len() && chars[i + run] == '`' {
            run += 1;
        }

        if !in_code {
            if !current.is_empty() {
                segments.push((false, std::mem::take(&mut current)));
            }
            in_code = true;
            tick_run = run;
        } else if run == tick_run {
            current.push_str(&"`".repeat(run));
            segments.push((true, std::mem::take(&mut current)));
            in_code = false;
            tick_run = 0;
            i += run;
            continue;
        }

        current.push_str(&"`".repeat(run));
        i += run;
    }

    if !current.is_empty() {
        segments.push((in_code, current));
    }
    segments
}

fn normalize_block_math_indentation(input: &str) -> String {
    if input.is_empty() || !input.contains("$$") {
        return input.to_string();
    }

    // 设计思路：
    // 规范化后的块级数学公式如果仍然保留列表缩进，例如：
    // 1.
    //     $$
    //     \oint ...
    //     $$
    // markdown-it 很容易把它识别为代码块，从而导致前端看到 LaTeX 源码。
    // 这里在非代码围栏上下文中，把 $$ 块内部统一去掉前导缩进，使其稳定落入数学块解析。
    let mut out = String::with_capacity(input.len());
    let mut in_code_fence = false;
    let mut in_math_block = false;

    for line in input.split_inclusive('\n') {
        let has_newline = line.ends_with('\n');
        let core = if has_newline {
            &line[..line.len() - 1]
        } else {
            line
        };

        let trimmed = core.trim();
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            out.push_str(core);
            if has_newline {
                out.push('\n');
            }
            continue;
        }

        if in_code_fence {
            out.push_str(core);
            if has_newline {
                out.push('\n');
            }
            continue;
        }

        if trimmed == "$$" {
            out.push_str(trimmed);
            if has_newline {
                out.push('\n');
            }
            in_math_block = !in_math_block;
            continue;
        }

        if in_math_block {
            out.push_str(core.trim_start());
        } else {
            out.push_str(core);
        }

        if has_newline {
            out.push('\n');
        }
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
        if is_stream_cancel_requested() {
            info!("{}: stream canceled by user", tag);
            return Ok(());
        }
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
                if is_stream_cancel_requested() {
                    info!("{}: stream canceled before emit", tag);
                    return Ok(());
                }
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
        if is_stream_cancel_requested() {
            info!("{}: stream canceled before tail emit", tag);
            return Ok(());
        }
        app.emit("modle-streaming-respond-output", &out)
            .map_err(|e| e.to_string())?;
    }

    if let Some(closer) = fence.closing_delta() {
        if is_stream_cancel_requested() {
            info!("{}: stream canceled before closer emit", tag);
            return Ok(());
        }
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
    history: &[ChatMsg],
) -> Result<String, String> {
    let request_body = json!({
        "model": model,
        "messages": build_chat_messages(history, question),
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
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_chatgpt: Received request: {} {}",
        question, sessionid
    );

    let provider = llm_provider::resolve_chat(0)?;

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        &provider.endpoint,
        &provider.api_key,
        &provider.model,
        question,
        history,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_kimi(question: &str, history: &[ChatMsg], sessionid: &str) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_kimi: Received request: {} {}",
        question, sessionid
    );

    let provider = llm_provider::resolve_chat(3)?;

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        &provider.endpoint,
        &provider.api_key,
        &provider.model,
        question,
        history,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_qwen(question: &str, history: &[ChatMsg], sessionid: &str) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_qwen: Received request: {} {}",
        question, sessionid
    );

    let provider = llm_provider::resolve_chat(2)?;

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        &provider.endpoint,
        &provider.api_key,
        &provider.model,
        question,
        history,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_doubao(
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_doubao: Received request: {} {}",
        question, sessionid
    );

    let provider = llm_provider::resolve_chat(4)?;

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        &provider.endpoint,
        &provider.api_key,
        &provider.model,
        question,
        history,
    )
    .await?;

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
    })
}

pub async fn get_answer_from_local_deepseek(
    question: &str,
    history: &[ChatMsg],
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
        "messages": build_chat_messages(history, question),
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
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<(), String> {
    info!(
        "get_answer_from_chatgpt_stream: Received request: {} {}",
        question, sessionid
    );

    let provider = llm_provider::resolve_chat(0)?;

    let request_body = json!({
        "model": provider.model,
        "messages": build_chat_messages(history, question),
        "stream": true,
    });

    openai_compat_sse_stream(&app, &provider.endpoint, &provider.api_key, request_body).await
}
pub async fn get_answer_from_deepseek_stream(
    app: AppHandle,
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<(), String> {
    let provider = llm_provider::resolve_chat(1)?;

    info!(
        "get_answer_from_deepseek_stream: Received request: {:?} {}",
        question, sessionid
    );

    let request_body = json!({
        "model": provider.model,
        "messages": build_chat_messages(history, question),
        "stream": true,
    });

    openai_compat_sse_stream(&app, &provider.endpoint, &provider.api_key, request_body).await
}

pub async fn get_answer_from_kimi_stream(
    app: AppHandle,
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<(), String> {
    let provider = llm_provider::resolve_chat(3)?;

    info!(
        "get_answer_from_kimi_stream: Received request: {:?} {}",
        question, sessionid
    );

    let request_body = json!({
        "model": provider.model,
        "messages": build_chat_messages(history, question),
        "temperature": 0.3,
        "stream": true,
    });

    openai_compat_sse_stream(&app, &provider.endpoint, &provider.api_key, request_body).await
}

pub async fn get_answer_from_doubao_stream(
    app: AppHandle,
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<(), String> {
    let rsp = get_answer_from_doubao(question, history, sessionid).await?;
    if is_stream_cancel_requested() {
        info!("get_answer_from_doubao_stream: canceled before emit");
        return Ok(());
    }
    app.emit("modle-streaming-respond-output", &rsp.answer)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_answer_from_qwen_stream(
    app: AppHandle,
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<(), String> {
    let rsp = get_answer_from_qwen(question, history, sessionid).await?;
    if is_stream_cancel_requested() {
        info!("get_answer_from_qwen_stream: canceled before emit");
        return Ok(());
    }
    app.emit("modle-streaming-respond-output", &rsp.answer)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_answer_from_local_qwen_stream(
    app: AppHandle,
    question: &str,
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<(), String> {
    ensure_local_qwen_server_started().await?;
    let url = LOCAL_QWEN_CHAT_COMPLETIONS_URL;
    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let request_body = json!({
        "model": "local-qwen3",
        "messages": build_chat_messages(history, question),
        "stream": true,  // 启用流式传输
        "session_id": session_id,
        // 复用 llama.cpp 的 prompt 前缀缓存，跳过对相同 system 前缀的重复评估。
        "cache_prompt": true,
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
        if is_stream_cancel_requested() {
            info!("get_answer_from_local_qwen_stream: canceled by user");
            return Ok(());
        }
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
                if is_stream_cancel_requested() {
                    info!("get_answer_from_local_qwen_stream: canceled before emit");
                    return Ok(());
                }
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
        if is_stream_cancel_requested() {
            info!("get_answer_from_local_qwen_stream: canceled before tail emit");
            return Ok(());
        }

        app.emit("modle-streaming-respond-output", &out)
            .map_err(|e| e.to_string())?;
        debug!(
            "get_answer_from_local_qwen_stream: Emitting remaining buffered data: {}",
            out
        );
        debug!("get_answer_from_local_qwen_stream: emit event modle-streaming-respond-output");
    }

    if let Some(closer) = fence.closing_delta() {
        if is_stream_cancel_requested() {
            info!("get_answer_from_local_qwen_stream: canceled before closer emit");
            return Ok(());
        }
        app.emit("modle-streaming-respond-output", &closer)
            .map_err(|e| e.to_string())?;
    }

    log_model_reply("local_qwen_stream model=qwen3:8b", &session_id, &full_raw);

    Ok(())
}

pub async fn get_answer_from_local_deepseek_stream(
    app: AppHandle,
    question: &str,
    history: &[ChatMsg],
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
        "messages": build_chat_messages(history, question),
        "stream": true,
        // 复用 llama.cpp 的 prompt 前缀缓存，跳过对相同 system 前缀的重复评估。
        "cache_prompt": true,
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
        if is_stream_cancel_requested() {
            info!("get_answer_from_local_deepseek_stream: canceled by user");
            return Ok(());
        }
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
                    if is_stream_cancel_requested() {
                        info!("get_answer_from_local_deepseek_stream: canceled before emit");
                        return Ok(());
                    }
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
            if is_stream_cancel_requested() {
                info!("get_answer_from_local_deepseek_stream: canceled before tail emit");
                return Ok(());
            }
            app.emit("modle-streaming-respond-output", &out)
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(closer) = fence.closing_delta() {
        if is_stream_cancel_requested() {
            info!("get_answer_from_local_deepseek_stream: canceled before closer emit");
            return Ok(());
        }
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
    history: &[ChatMsg],
    sessionid: &str,
) -> Result<ChatResponse, String> {
    ensure_local_qwen_server_started().await?;
    let url = LOCAL_QWEN_CHAT_COMPLETIONS_URL;
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
        "model": "local-qwen3",
        "messages": build_chat_messages(history, question),
        "stream": false
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
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Error calling Local Qwen API: {} {}", status, body));
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

    Ok(ChatResponse {
        answer: answer_content,
        sessionid: session_id,
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
        "deepseek" => Some("https://chat.deepseek.com"),
        "kimi" => Some("https://kimi.moonshot.cn"),
        // 通义千问已迁移到 qianwen.com；旧域名 301 跳转会让登录态落在另一个 origin 上。
        "qwen" => Some("https://www.qianwen.com/"),
        "doubao" => Some("https://www.doubao.com/chat/"),
        "claude" => Some("https://claude.ai/new"),
        _ => None,
    }
}

/// 网页模型窗口和 web-model-chat-push 事件使用的 model_id。
/// 这些 id 只用于网页会话，不会进入普通对话的 ChatModel 分发。
pub fn web_provider_model_id(provider: &str) -> Option<i32> {
    match provider.trim().to_lowercase().as_str() {
        "yuanbao" => Some(7),
        "chatgpt" => Some(8),
        "deepseek" => Some(21),
        "kimi" => Some(22),
        "qwen" => Some(23),
        "doubao" => Some(24),
        "claude" => Some(25),
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
    pub provider: String,
    pub model_id: i32,
    pub topic_id: i32,
    pub session_id: String,
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<String>,
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
    const ENDPOINT = 'http://127.0.0.1:8888/api/web-model-chat';

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
        // 千问/部分站点用 Slate 之类的富文本编辑器：直接改 textContent 只改 DOM，
        // 编辑器内部状态仍是空的，发送按钮会一直禁用。优先用 paste 事件让编辑器自己写入。
        let written = false;
        try {{
          if (typeof DataTransfer === 'function' && typeof ClipboardEvent === 'function') {{
            el.focus();
            const dt = new DataTransfer();
            dt.setData('text/plain', valueText);
            const paste = new ClipboardEvent('paste', {{ bubbles: true, cancelable: true, clipboardData: dt }});
            el.dispatchEvent(paste);
            written = paste.defaultPrevented;
          }}
        }} catch (e) {{}}
        if (!written) {{
          try {{
            el.focus();
            el.textContent = '';
            if (document.execCommand) {{
              document.execCommand('selectAll', false, null);
              document.execCommand('insertText', false, valueText);
            }} else {{
              el.textContent = valueText;
            }}
            written = true;
          }} catch (e) {{
            try {{ el.textContent = valueText; written = true; }} catch (e2) {{ return false; }}
          }}
        }}

        if (written) {{
          try {{ el.dispatchEvent(new InputEvent('input', {{ bubbles: true, data: valueText }})); }} catch (e) {{ el.dispatchEvent(new Event('input', {{ bubbles: true }})); }}
        }}
        return written;
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

    let filled = false;
    let sent = false;
    let filledAt = 0;

    // 发送按钮要等编辑器状态同步后才会启用（千问是 Slate，写入后约 1~2 秒），
    // 所以这里轮询而不是固定重试三次：最多等 40 秒。
    function sendNow() {{
      if (sent) return true;
      const input = findInput();
      if (!input) return false;

      if (!filled) {{
        openAttachmentEntrances(input);
        filled = setValue(input, appendAttachmentHint(text));
        if (filled) filledAt = Date.now();
      }}
      if (!filled) return false;

      // 页面重新渲染把内容清空了就重贴一次，避免一直等一个空输入框。
      if (Date.now() - filledAt > 2500) {{
        const current = (input.innerText || input.textContent || '').trim();
        if (!current) {{ filled = false; filledAt = 0; return false; }}
      }}

      if (provider === 'yuanbao') {{
        const ybtn = document.querySelector('#yuanbao-send-btn');
        if (ybtn && !isDisabled(ybtn) && click(ybtn)) {{ sent = true; return true; }}
      }}

      const btn = bestSendButton(input);
      if (btn && click(btn)) {{ sent = true; return true; }}

      // 没有可用按钮时才退回表单/回车，避免在编辑器状态未同步时发出空消息。
      if (trySubmitForm(input)) {{ sent = true; return true; }}
      return false;
    }}

    function reportSend(ok) {{
      try {{
        window.__GUAXI_LAST_PROMPT__ = text;
        fetch(ENDPOINT, {{
          method: 'POST',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify({{
            provider: provider,
            model_id: window.__GUAXI_MODEL_ID__,
            topic_id: window.__GUAXI_TOPIC_ID__,
            session_id: window.__GUAXI_SESSION_ID__,
            status: ok ? 'sent' : 'send_failed',
            messages: []
          }})
        }});
      }} catch (e) {{}}
    }}

    let attempts = 0;
    function tick() {{
      attempts += 1;
      let ok = false;
      try {{ ok = sendNow(); }} catch (e) {{}}
      if (ok) {{ reportSend(true); return; }}
      if (attempts >= 80) {{ reportSend(false); return; }}
      setTimeout(tick, 500);
    }}

    tick();
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
    reset_stream_cancel_flag();
    // ── 认知层接地：把用户自己的知识库/收件箱/历史问答召回成带引证的上下文，
    // 前置到问题上。失败或无证据时**原样**走原链路，不改变任何既有行为。
    let mut request = request;
    let episode_id = crate::cognition::hook_chat_start(&app, &mut request.question, &request.sessionid, request.modelid).await;
    let model =
        ChatModel::try_from(request.modelid).map_err(|e| format!("Invalid model ID: {}", e))?;
    info!(
        "Received request: model_id:{} question:{} session_id:{}",
        request.modelid, request.question, request.sessionid
    );

    let dispatch_result = match model {
        ChatModel::CHATGPT => {
            match get_answer_from_chatgpt_stream(app, &request.question, &request.history, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream CHATGPT: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling ChatGPT API: {}", err)),
            }
        }
        ChatModel::DEEPSEEK => {
            match get_answer_from_deepseek_stream(app, &request.question, &request.history, &request.sessionid).await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream DEEPSEEK: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling DeepSeek API: {}", err)),
            }
        }
        ChatModel::KIMI => {
            match get_answer_from_kimi_stream(app, &request.question, &request.history, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream KIMI: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Kimi API: {}", err)),
            }
        }
        ChatModel::QWEN => {
            match get_answer_from_qwen_stream(app, &request.question, &request.history, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream QWEN: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Qwen API: {}", err)),
            }
        }
        ChatModel::DOUBAO => {
            match get_answer_from_doubao_stream(app, &request.question, &request.history, &request.sessionid).await {
                Ok(()) => {
                    info!("get_chat_answer_stream DOUBAO: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Doubao API: {}", err)),
            }
        }
        ChatModel::LOCALQWEN => {
            match get_answer_from_siliconflow_stream(app, &request.question, &request.history, &request.sessionid)
                .await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream LOCALQWEN: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling SiliconFlow API: {}", err)),
            }
        }
        ChatModel::LOCALDEEPSEEK => {
            match get_answer_from_local_deepseek_stream(app, &request.question, &request.history, &request.sessionid)
                .await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream LOCALDEEPSEEK: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Local Model API: {}", err)),
            }
        }
        ChatModel::LOCALQWEN3 => {
            match get_answer_from_local_qwen_stream(app, &request.question, &request.history, &request.sessionid)
                .await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream LOCALQWEN3: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Local Qwen API: {}", err)),
            }
        }
        ChatModel::CUSTOM => {
            match get_answer_from_custom_stream(app, &request.question, &request.history, &request.sessionid)
                .await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream CUSTOM: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Custom API: {}", err)),
            }
        }
        ChatModel::CCSWITCH => {
            match get_answer_from_cc_switch_stream(
                app,
                &request.question,
                &request.history,
                &request.sessionid,
            )
            .await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream CCSWITCH: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling cc-switch proxy: {}", err)),
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
    };
    // 失败即刻收尾；成功时留给前端在流结束后带着完整答案调 cognition_chat_finish，
    // 这样才能拿到答案文本回填"哪条引证真的被用上了"——那是检索权重学习唯一的免费标签。
    if let Err(e) = &dispatch_result {
        crate::cognition::hook_chat_failed(&episode_id, e);
    }
    dispatch_result
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
            match get_answer_from_chatgpt(&request.question, &request.history, &request.sessionid).await {
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
            match get_answer_from_deepseek(&request.question, &request.history, &request.sessionid).await {
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
            match get_answer_from_kimi(&request.question, &request.history, &request.sessionid).await {
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
            match get_answer_from_qwen(&request.question, &request.history, &request.sessionid).await {
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
            match get_answer_from_doubao(&request.question, &request.history, &request.sessionid).await {
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
            match get_answer_from_siliconflow(&request.question, &request.history, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer LOCALQWEN: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling SiliconFlow API: {}", err)),
            }
        }
        ChatModel::LOCALDEEPSEEK => {
            match get_answer_from_local_deepseek(&request.question, &request.history, &request.sessionid).await {
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
        ChatModel::LOCALQWEN3 => {
            match get_answer_from_local_qwen(&request.question, &request.history, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer LOCALQWEN3: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Local Qwen API: {}", err)),
            }
        }
        ChatModel::CUSTOM => {
            match get_answer_from_custom(&request.question, &request.history, &request.sessionid).await {
                Ok(answer) => {
                    info!(
                        "get_chat_answer CUSTOM: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling Custom API: {}", err)),
            }
        }
        ChatModel::CCSWITCH => {
            match get_answer_from_cc_switch(&request.question, &request.history, &request.sessionid)
                .await
            {
                Ok(answer) => {
                    info!(
                        "get_chat_answer CCSWITCH: respond ChatResponse  {} {}",
                        answer.answer, answer.sessionid
                    );
                    Ok(answer)
                }
                Err(err) => Err(format!("Error calling cc-switch proxy: {}", err)),
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

// ===================== Agent（工具调用，OpenAI function calling）=====================

#[derive(Deserialize)]
pub struct AgentRequest {
    modelid: i32,
    question: String,
    #[serde(default)]
    topicid: i32,
    #[serde(default)]
    sessionid: String,
    #[serde(default)]
    history: Vec<ChatMsg>,
    /// 活动仓库根路径（read_file 越权防护）。
    #[serde(default)]
    repo_path: String,
    /// 全部已配置知识库根路径，作为 Agent 文件工具的多根白名单（跨库 @ 引用读取）。
    #[serde(default)]
    repo_roots: Vec<String>,
    /// 用户是否手动开启联网能力。关闭时 Python Agent Server 不会暴露网络工具。
    #[serde(default)]
    web_search_enabled: bool,
    /// 上一轮 codex 返回的 thread id。codex 引擎下用它 resume 同一个会话线程，
    /// 内置 Python 引擎忽略该字段（它靠 history 维持上下文）。
    #[serde(default)]
    thread_id: String,
}


#[derive(Deserialize)]
pub struct AgentDecisionRequest {
    call_id: String,
    approved: bool,
    #[serde(default)]
    args: Option<Value>,
}

/// Forward agent write confirm to Python Agent Server.
#[tauri::command]
pub async fn agent_tool_decision(request: AgentDecisionRequest) -> Result<(), String> {
    let client = reqwest::Client::new();
    let _resp = client
        .post("http://127.0.0.1:9876/agent/confirm")
        .json(&json!({"call_id": request.call_id, "approved": request.approved, "edited_args": request.args}))
        .send()
        .await
        .map_err(|e| format!("Agent confirm failed: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn agent_chat_stream(app: AppHandle, request: AgentRequest) -> Result<(), String> {
    info!(
        "Enter agent_chat_stream model_id:{} web_search_enabled:{} q:{}",
        request.modelid, request.web_search_enabled, request.question
    );

    // 网页版模型不支持工具调用，降级为普通对话
    if request.modelid == 7 || request.modelid == 8 {
        warn!(
            "agent_chat_stream: modelid {} 为网页版模型，降级为普通对话",
            request.modelid
        );
        let chat_req = ChatRequest {
            modelid: request.modelid,
            question: request.question.clone(),
            topicid: request.topicid,
            sessionid: request.sessionid.clone(),
            history: request.history.clone(),
        };
        return get_chat_answer_stream(app, chat_req).await;
    }

    reset_stream_cancel_flag();

    // 认知层：记一段 Agent 轨迹，并把召回结果作为提示交给前端（不改写问题——
    // Agent 可以自己调 guaxi_recall 拿更精确的东西，前置一大段反而挤占它的工具预算）。
    let cog_episode =
        crate::cognition::hook_agent_start(&app, &request.question, &request.sessionid, request.modelid)
            .await;
    let _ = &cog_episode;

    // Agent 引擎分流。三档是一条降级链，不是互斥选项：
    //   app-server（常驻，支持 steer / 审批 / 计划 / diff）
    //     → codex exec（一轮一进程）
    //       → 内置 Python Agent
    // 任意一环起不来就退到下一环，保证这个功能不会因为某个引擎的问题整体不可用。
    //
    // 注意：右侧助手走的是这条单轮链路，用 app-server 只能拿到「发一句、等结束」，
    // 收益有限且多一个常驻进程；工作台（AgentWorkspace）自己直接调 app-server 的命令。
    // 所以这里只把 `codex` 与 `builtin` 两档当作单轮引擎，`app-server` 落到 `codex`。
    let engine = settingview::get_llm_config().agent_engine;
    if engine.trim() != "builtin" {
        let codex_request = crate::commands::codex_agent::CodexAgentRequest {
            question: request.question.clone(),
            sessionid: request.sessionid.clone(),
            repo_path: request.repo_path.clone(),
            repo_roots: request.repo_roots.clone(),
            thread_id: request.thread_id.clone(),
            web_search_enabled: request.web_search_enabled,
        };
        match crate::commands::codex_agent::run_codex_agent(&app, &codex_request).await {
            Ok(()) => {
                log_model_reply(
                    "agent_chat_stream via codex",
                    &request.sessionid,
                    "[Codex response via JSONL - see event stream for content]",
                );
                return Ok(());
            }
            Err(err) => {
                warn!(
                    "agent_chat_stream: codex 引擎不可用，回退到内置 Python Agent。原因: {}",
                    err
                );
                // 回退前重置取消标志：codex 那一轮可能已经把它置位（例如用户中途点了停止）。
                reset_stream_cancel_flag();
            }
        }
    }

    // 确保 Python Agent Server 已运行
    crate::commands::agent_server::ensure_agent_server_started(&app).await?;

    // 构建请求体
    let history_json: Vec<Value> = request.history.iter().map(|m| {
        json!({"role": m.role, "content": m.content})
    }).collect();

    let body = json!({
        "chat_id": request.sessionid,
        "model_id": request.modelid,
        "question": request.question,
        "history": history_json,
        "work_dir": request.repo_path,
        "allowed_roots": request.repo_roots,
        "web_search_enabled": request.web_search_enabled,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post("http://127.0.0.1:9876/agent/chat")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Agent 服务请求失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Agent 服务返回错误: {} {}", status, text));
    }

    // 读取 SSE 流并转发到 Tauri 前端
    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        if is_stream_cancel_requested() {
            // 通知 agent server 取消
            let _ = client
                .post("http://127.0.0.1:9876/agent/cancel")
                .json(&json!({"chat_id": request.sessionid}))
                .send()
                .await;
            info!("agent_chat_stream: canceled by user");
            return Ok(());
        }

        let chunk_bytes = match chunk {
            Ok(bytes) => bytes,
            Err(e) => {
                // 设计说明：Agent 已经进入 SSE 流式阶段后，底层连接偶发 EOF 时，
                // 直接返回 Err 会让前端只看到 invoke 失败，当前回答也无法承载原因。
                // 这里改为把传输错误转成可展示的 Agent 步骤和正文提示，让用户能在同一轮对话里定位问题。
                let msg = format!("Agent SSE 流读取中断: {}", e);
                warn!("agent_chat_stream stream read error: {}", msg);
                let ev = json!({
                    "type": "error",
                    "name": "agent_stream",
                    "status": "error",
                    "message": msg.clone(),
                });
                let _ = app.emit("agent-step", &ev);
                let _ = app.emit(
                    "modle-streaming-respond-output",
                    format!("\n\n> ⚠️ Agent 流中断：{}\n", msg),
                );
                log_model_reply(
                    "agent_chat_stream interrupted via python service",
                    &request.sessionid,
                    &msg,
                );
                return Ok(());
            }
        };

        let chunk_str = match std::str::from_utf8(&chunk_bytes) {
            Ok(s) => s.to_string(),
            Err(_) => continue,
        };
        buffer.push_str(&chunk_str);

        // 解析 SSE 事件行
        while let Some(pos) = buffer.find("\n\n") {
            let event_block = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();

            for line in event_block.lines() {
                let data = line.strip_prefix("data: ").unwrap_or("");
                if data.is_empty() { continue; }

                let ev: Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let ev_type = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");

                match ev_type {
                    "text" => {
                        let content = ev.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        if !content.is_empty() {
                            let _ = app.emit("modle-streaming-respond-output", content);
                        }
                    }
                    "tool_call" | "tool_result" => {
                        let _ = app.emit("agent-step", &ev);
                    }
                    "tool_confirm" => {
                        let _ = app.emit("agent-tool-confirm", &ev);
                    }
                    "error" => {
                        let msg = ev.get("message").and_then(|v| v.as_str()).unwrap_or("未知错误");
                        warn!("agent_chat_stream error: {}", msg);
                        let frontend_ev = json!({
                            "type": "error",
                            "name": "agent_stream",
                            "status": "error",
                            "message": msg,
                        });
                        let _ = app.emit("agent-step", &frontend_ev);
                        let _ = app.emit(
                            "modle-streaming-respond-output",
                            format!("\n\n> ⚠️ Agent 执行失败：{}\n", msg),
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    // 记录日志
    log_model_reply(
        "agent_chat_stream via python service",
        &request.sessionid,
        "[Agent response via SSE - see event stream for content]",
    );

    Ok(())
}
