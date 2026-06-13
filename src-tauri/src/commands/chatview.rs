use crate::commands::settingview;
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use futures_util::StreamExt;
use rand::Rng;
use reqwest::multipart::{Form, Part};
use reqwest::{Client, ClientBuilder};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
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

const LOCAL_DEEPSEEK_MODEL_FILENAME: &str = "DeepSeek-R1-0528-Qwen3-8B-IQ4_XS.gguf";
const LOCAL_DEEPSEEK_SERVER_URL: &str = "http://127.0.0.1:18080";
const LOCAL_DEEPSEEK_CHAT_COMPLETIONS_URL: &str = "http://127.0.0.1:18080/v1/chat/completions";
const LOCAL_QWEN_MODEL_FILENAME: &str = "Qwen3.6-35B-A3B-UD-Q4_K_M.gguf";
const LOCAL_QWEN_SERVER_URL: &str = "http://127.0.0.1:18082";
const LOCAL_QWEN_CHAT_COMPLETIONS_URL: &str = "http://127.0.0.1:18082/v1/chat/completions";
const SILICONFLOW_CHAT_COMPLETIONS_URL: &str = "https://api.siliconflow.cn/v1/chat/completions";
const SILICONFLOW_DEEPSEEK_QWEN_MODEL: &str = "deepseek-ai/DeepSeek-R1-0528-Qwen3-8B";
const SILICONFLOW_AUDIO_SPEECH_URL: &str = "https://api.siliconflow.cn/v1/audio/speech";
const SILICONFLOW_AUDIO_TRANSCRIPTIONS_URL: &str = "https://api.siliconflow.cn/v1/audio/transcriptions";
const SILICONFLOW_AUDIO_TTS_DEFAULT_MODEL: &str = "FunAudioLLM/CosyVoice2-0.5B";
const SILICONFLOW_AUDIO_TTS_DEFAULT_VOICE: &str = "alex";
const SILICONFLOW_AUDIO_STT_DEFAULT_MODEL: &str = "FunAudioLLM/SenseVoiceSmall";
const SHERPA_ONNX_TTS_DEFAULT_EXE_WIN: &str = "utils/sherpa-onnx/sherpa-onnx-offline-tts.exe";
const SHERPA_ONNX_TTS_DEFAULT_EXE_UNIX: &str = "utils/sherpa-onnx/sherpa-onnx-offline-tts";
const SHERPA_ONNX_TTS_DEFAULT_MODEL: &str = "utils/Models/tts/kokoro_multi_lang/model.onnx";
const SHERPA_ONNX_TTS_DEFAULT_TOKENS: &str = "utils/Models/tts/kokoro_multi_lang/tokens.txt";
const SHERPA_ONNX_ASR_DEFAULT_EXE_WIN: &str = "utils/sherpa-onnx/sherpa-onnx-offline.exe";
const SHERPA_ONNX_ASR_DEFAULT_EXE_UNIX: &str = "utils/sherpa-onnx/sherpa-onnx-offline";

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

fn reset_stream_cancel_flag() {
    STREAM_CANCEL_REQUESTED.store(false, Ordering::SeqCst);
}

pub async fn get_answer_from_siliconflow(
    question: &str,
    sessionid: &str,
) -> Result<ChatResponse, String> {
    info!(
        "get_answer_from_siliconflow: Received request: {} {}",
        question, sessionid
    );

    let api_key = settingview::get_llm_config().siliconflow_api_key;
    if api_key.trim().is_empty() {
        return Err("硅基流动 API Key 未配置".to_string());
    }

    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let answer_content = openai_compat_chat(
        SILICONFLOW_CHAT_COMPLETIONS_URL,
        &api_key,
        SILICONFLOW_DEEPSEEK_QWEN_MODEL,
        question,
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
    sessionid: &str,
) -> Result<(), String> {
    let api_key = settingview::get_llm_config().siliconflow_api_key;
    if api_key.trim().is_empty() {
        return Err("硅基流动 API Key 未配置".to_string());
    }

    info!(
        "get_answer_from_siliconflow_stream: Received request: {:?} {}",
        question, sessionid
    );

    let request_body = json!({
        "model": SILICONFLOW_DEEPSEEK_QWEN_MODEL,
        "messages": math_prompt_messages(question),
        "stream": true,
    });

    openai_compat_sse_stream(&app, SILICONFLOW_CHAT_COMPLETIONS_URL, &api_key, request_body).await
}

fn request_stream_cancel() {
    STREAM_CANCEL_REQUESTED.store(true, Ordering::SeqCst);
}

fn is_stream_cancel_requested() -> bool {
    STREAM_CANCEL_REQUESTED.load(Ordering::SeqCst)
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
    #[serde(flatten)]
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
    #[serde(flatten)]
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
        ChatModel::CHATGPT => get_answer_from_chatgpt(prompt, sessionid).await,
        ChatModel::DEEPSEEK => get_answer_from_deepseek(prompt, sessionid).await,
        ChatModel::QWEN => get_answer_from_qwen(prompt, sessionid).await,
        ChatModel::KIMI => get_answer_from_kimi(prompt, sessionid).await,
        ChatModel::DOUBAO => get_answer_from_doubao(prompt, sessionid).await,
        ChatModel::LOCALQWEN => get_answer_from_siliconflow(prompt, sessionid).await,
        ChatModel::LOCALDEEPSEEK => get_answer_from_local_deepseek(prompt, sessionid).await,
        ChatModel::LOCALQWEN3 => get_answer_from_local_qwen(prompt, sessionid).await,
        ChatModel::YUANBAO_WEB | ChatModel::CHATGPT_WEB => {
            Err("当前网页模型不支持知识处理 Agent 命令，请切换到 API 模型".to_string())
        }
    }
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

#[derive(Debug, Clone)]
struct SherpaOnnxTtsCliConfig {
    exe_path: PathBuf,
    model_path: PathBuf,
    tokens_path: PathBuf,
    voices_path: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    lexicon: Option<PathBuf>,
    rule_fsts: Option<String>,
    runtime_provider: String,
    sid: i32,
    speed: f32,
}

#[derive(Debug, Clone, Copy)]
enum SherpaOnnxTtsCliMode {
    Kokoro,
    Vits,
}

fn detect_sherpa_tts_modes(cfg: &SherpaOnnxTtsCliConfig, mode_hint: &str) -> Vec<SherpaOnnxTtsCliMode> {
    let hint = mode_hint.trim().to_lowercase();
    if hint == "vits" {
        return vec![SherpaOnnxTtsCliMode::Vits];
    }
    if hint == "kokoro" {
        return vec![SherpaOnnxTtsCliMode::Kokoro];
    }
    let model_name = cfg
        .model_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let model_dir_name = cfg
        .model_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let likely_kokoro = model_name.contains("kokoro")
        || model_dir_name.contains("kokoro")
        || cfg.voices_path.as_ref().map(|p| p.exists()).unwrap_or(false);
    if likely_kokoro {
        vec![SherpaOnnxTtsCliMode::Kokoro, SherpaOnnxTtsCliMode::Vits]
    } else {
        vec![SherpaOnnxTtsCliMode::Vits, SherpaOnnxTtsCliMode::Kokoro]
    }
}

fn resolve_exe_dir() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| format!("获取可执行目录失败: {}", e))?;
    exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "无法获取可执行目录".to_string())
}

fn resolve_sherpa_default_exe() -> Result<PathBuf, String> {
    let rel = if cfg!(target_os = "windows") {
        SHERPA_ONNX_TTS_DEFAULT_EXE_WIN
    } else {
        SHERPA_ONNX_TTS_DEFAULT_EXE_UNIX
    };
    bundled_resources::ensure_relative_dir(rel)
}

fn resolve_sherpa_default_asr_exe() -> Result<PathBuf, String> {
    let rel = if cfg!(target_os = "windows") {
        SHERPA_ONNX_ASR_DEFAULT_EXE_WIN
    } else {
        SHERPA_ONNX_ASR_DEFAULT_EXE_UNIX
    };
    bundled_resources::ensure_relative_dir(rel)
}

fn resolve_sherpa_default_model_paths() -> Result<(PathBuf, PathBuf), String> {
    Ok((
        bundled_resources::ensure_relative_dir(SHERPA_ONNX_TTS_DEFAULT_MODEL)?,
        bundled_resources::ensure_relative_dir(SHERPA_ONNX_TTS_DEFAULT_TOKENS)?,
    ))
}

fn resolve_path_with_exe_dir(raw: &str) -> Option<PathBuf> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    let p = PathBuf::from(text);
    if p.exists() {
        return Some(p);
    }
    let exe_dir = resolve_exe_dir().ok()?;
    let joined = exe_dir.join(text);
    if joined.exists() {
        return Some(joined);
    }
    Some(PathBuf::from(text))
}

fn parse_speaker_id(voice: &str) -> i32 {
    let s = voice.trim();
    if s.is_empty() {
        return 0;
    }
    s.parse::<i32>().unwrap_or(0)
}

fn normalize_sherpa_runtime_provider(provider: &str) -> String {
    match provider.trim().to_lowercase().as_str() {
        "cuda" | "gpu" => "cuda".to_string(),
        "coreml" => "coreml".to_string(),
        _ => "cpu".to_string(),
    }
}

fn guess_model_files_from_dir(dir: &Path) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    if !dir.exists() || !dir.is_dir() {
        return (None, None, None);
    }
    let mut model = None;
    let mut tokens = None;
    let mut data_dir = None;
    let entries = match fs::read_dir(dir) {
        Ok(v) => v,
        Err(_) => return (None, None, None),
    };
    for it in entries {
        let entry = match it {
            Ok(v) => v,
            Err(_) => continue,
        };
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if p.is_file() && name.ends_with(".onnx") && model.is_none() {
            model = Some(p.clone());
        } else if p.is_file() && name == "tokens.txt" && tokens.is_none() {
            tokens = Some(p.clone());
        } else if p.is_dir() && (name.contains("espeak-ng-data") || name.contains("dict")) && data_dir.is_none() {
            data_dir = Some(p.clone());
        }
    }
    (model, tokens, data_dir)
}

fn resolve_sherpa_onnx_tts_cli_config(request: &TextToSpeechRequest) -> Result<SherpaOnnxTtsCliConfig, String> {
    let endpoint = request.common.endpoint.trim();
    let exe_env = env::var("GUAXI_SHERPA_ONNX_TTS_EXE").unwrap_or_default();
    let model_env = env::var("GUAXI_SHERPA_ONNX_TTS_MODEL").unwrap_or_default();
    let tokens_env = env::var("GUAXI_SHERPA_ONNX_TTS_TOKENS").unwrap_or_default();
    let data_env = env::var("GUAXI_SHERPA_ONNX_TTS_DATA_DIR").unwrap_or_default();
    let lexicon_env = env::var("GUAXI_SHERPA_ONNX_TTS_LEXICON").unwrap_or_default();
    let rule_fsts_env = env::var("GUAXI_SHERPA_ONNX_TTS_RULE_FSTS").unwrap_or_default();
    let voices_env = env::var("GUAXI_SHERPA_ONNX_TTS_VOICES").unwrap_or_default();

    let endpoint_path = if endpoint.is_empty() { None } else { resolve_path_with_exe_dir(endpoint) };
    let endpoint_is_exe = endpoint_path
        .as_ref()
        .map(|p| {
            if !p.is_file() {
                return false;
            }
            let ext = p
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or_default()
                .to_lowercase();
            if ext == "exe" {
                return true;
            }
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_lowercase().contains("sherpa-onnx-offline-tts"))
                .unwrap_or(false)
        })
        .unwrap_or(false);
    let endpoint_is_dir = endpoint_path.as_ref().map(|p| p.is_dir()).unwrap_or(false);

    let exe_path = if endpoint_is_exe {
        endpoint_path.clone().unwrap()
    } else if !exe_env.trim().is_empty() {
        PathBuf::from(exe_env.trim())
    } else {
        resolve_sherpa_default_exe()?
    };
    if !exe_path.exists() {
        return Err(format!("未找到 sherpa-onnx tts 可执行文件: {}", exe_path.to_string_lossy()));
    }

    let requested_model_path = resolve_path_with_exe_dir(request.common.model.trim());
    let model_dir = if endpoint_is_dir {
        endpoint_path.clone()
    } else if let Some(p) = requested_model_path.clone() {
        if p.is_dir() { Some(p) } else { None }
    } else {
        None
    };
    let guessed = model_dir
        .as_ref()
        .map(|d| guess_model_files_from_dir(d))
        .unwrap_or((None, None, None));

    let defaults = resolve_sherpa_default_model_paths()?;
    info!(
        endpoint_raw = request.common.endpoint.trim(),
        endpoint_resolved = endpoint_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        endpoint_is_exe,
        endpoint_is_dir,
        requested_model_raw = request.common.model.trim(),
        requested_model_resolved = requested_model_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        inferred_model_dir = model_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        guessed_model = guessed
            .0
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        guessed_tokens = guessed
            .1
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        guessed_data_dir = guessed
            .2
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        default_model = defaults.0.to_string_lossy().as_ref(),
        default_tokens = defaults.1.to_string_lossy().as_ref(),
        "resolving sherpa-onnx tts cli config"
    );
    let model_path = if let Some(p) = requested_model_path {
        if p.is_file() { p } else if !request.common.model.trim().is_empty() { PathBuf::from(request.common.model.trim()) } else { defaults.0.clone() }
    } else if !model_env.trim().is_empty() {
        PathBuf::from(model_env.trim())
    } else if let Some(p) = guessed.0 {
        p
    } else if defaults.0.exists() {
        defaults.0.clone()
    } else {
        return Err("未找到 sherpa-onnx 模型文件，请检查 utils/Models/tts/kokoro_multi_lang/model.onnx 或设置 common.model/GUAXI_SHERPA_ONNX_TTS_MODEL".to_string());
    };
    if !model_path.exists() {
        return Err(format!("sherpa-onnx 模型文件不存在: {}", model_path.to_string_lossy()));
    }
    let model_base_dir = model_path.parent().map(|p| p.to_path_buf());

    let request_tokens = resolve_path_with_exe_dir(request.common.tokens.trim());
    let tokens_path = if let Some(p) = request_tokens {
        p
    } else if !tokens_env.trim().is_empty() {
        PathBuf::from(tokens_env.trim())
    } else if let Some(p) = guessed.1 {
        p
    } else if defaults.1.exists() {
        defaults.1.clone()
    } else {
        model_path
            .parent()
            .map(|p| p.join("tokens.txt"))
            .ok_or_else(|| "无法推断 tokens.txt 路径".to_string())?
    };
    if !tokens_path.exists() {
        return Err(format!("tokens.txt 不存在: {}", tokens_path.to_string_lossy()));
    }

    let voices_path = if !voices_env.trim().is_empty() {
        Some(PathBuf::from(voices_env.trim()))
    } else {
        let guessed = model_path
            .parent()
            .map(|p| p.join("voices.bin"));
        match guessed {
            Some(p) if p.exists() => Some(p),
            _ => None,
        }
    };

    let data_dir = if !data_env.trim().is_empty() {
        Some(PathBuf::from(data_env.trim()))
    } else if let Some(p) = guessed.2 {
        Some(p)
    } else {
        let fallback = model_base_dir
            .as_ref()
            .map(|p| p.join("espeak-ng-data"));
        match fallback {
            Some(p) if p.exists() => Some(p),
            _ => None,
        }
    };
    let lexicon = if !lexicon_env.trim().is_empty() {
        Some(PathBuf::from(lexicon_env.trim()))
    } else {
        let mut candidate: Option<PathBuf> = None;
        if let Some(base) = model_base_dir.as_ref() {
            for name in ["lexicon-zh.txt", "lexicon-us-en.txt", "lexicon-gb-en.txt", "lexicon.txt"] {
                let p = base.join(name);
                if p.exists() {
                    candidate = Some(p);
                    break;
                }
            }
        }
        candidate
    };
    let rule_fsts = if rule_fsts_env.trim().is_empty() {
        None
    } else {
        Some(rule_fsts_env.trim().to_string())
    };

    info!(
        exe_path = exe_path.to_string_lossy().as_ref(),
        model_path = model_path.to_string_lossy().as_ref(),
        model_is_file = model_path.is_file(),
        tokens_path = tokens_path.to_string_lossy().as_ref(),
        tokens_exists = tokens_path.exists(),
        voices_path = voices_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        data_dir = data_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        lexicon = lexicon
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        "sherpa-onnx tts cli config resolved"
    );

    Ok(SherpaOnnxTtsCliConfig {
        exe_path,
        model_path,
        tokens_path,
        voices_path,
        data_dir,
        lexicon,
        rule_fsts,
        runtime_provider: normalize_sherpa_runtime_provider(&request.common.runtime_provider),
        sid: parse_speaker_id(&request.voice),
        speed: request.speed.unwrap_or(1.0).max(0.2).min(3.0),
    })
}

fn run_sherpa_onnx_cli_once(
    cfg: &SherpaOnnxTtsCliConfig,
    text: &str,
    output_wav: &Path,
    mode: SherpaOnnxTtsCliMode,
) -> Result<(), String> {
    let mut cmd = Command::new(&cfg.exe_path);
    let mut push_opt = |name: &str, value: &str| {
        cmd.arg(format!("--{}={}", name, value));
    };
    match mode {
        SherpaOnnxTtsCliMode::Kokoro => {
            push_opt("kokoro-model", cfg.model_path.to_string_lossy().as_ref());
            push_opt("kokoro-tokens", cfg.tokens_path.to_string_lossy().as_ref());
            if let Some(v) = cfg.voices_path.as_ref() {
                push_opt("kokoro-voices", v.to_string_lossy().as_ref());
            }
            if let Some(d) = cfg.data_dir.as_ref() {
                push_opt("kokoro-data-dir", d.to_string_lossy().as_ref());
            }
            if let Some(l) = cfg.lexicon.as_ref() {
                push_opt("kokoro-lexicon", l.to_string_lossy().as_ref());
            }
        }
        SherpaOnnxTtsCliMode::Vits => {
            push_opt("vits-model", cfg.model_path.to_string_lossy().as_ref());
            push_opt("vits-tokens", cfg.tokens_path.to_string_lossy().as_ref());
            if let Some(d) = cfg.data_dir.as_ref() {
                push_opt("vits-data-dir", d.to_string_lossy().as_ref());
            }
            if let Some(l) = cfg.lexicon.as_ref() {
                push_opt("vits-lexicon", l.to_string_lossy().as_ref());
            }
        }
    }
    if let Some(r) = cfg.rule_fsts.as_ref() {
        push_opt("tts-rule-fsts", r);
    }
    push_opt("provider", cfg.runtime_provider.as_str());
    push_opt("print-args", "false");
    push_opt("sid", cfg.sid.to_string().as_str());
    push_opt("speed", format!("{}", cfg.speed).as_str());
    push_opt("output-filename", output_wav.to_string_lossy().as_ref());
    cmd.arg(text);
    info!(
        mode = format!("{:?}", mode),
        exe = cfg.exe_path.to_string_lossy().as_ref(),
        model = cfg.model_path.to_string_lossy().as_ref(),
        tokens = cfg.tokens_path.to_string_lossy().as_ref(),
        voices = cfg
            .voices_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        data_dir = cfg
            .data_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        lexicon = cfg
            .lexicon
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        runtime_provider = cfg.runtime_provider.as_str(),
        output_wav = output_wav.to_string_lossy().as_ref(),
        "sherpa-onnx cli args prepared"
    );
    apply_no_window_in_release(&mut cmd);
    let output = cmd.output().map_err(|e| format!("执行 sherpa-onnx tts 失败: {}", e))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let code = output
            .status
            .code()
            .map(|value| format!("{value} (0x{:08X})", value as u32))
            .unwrap_or_else(|| "terminated by signal".to_string());
        let detail = if !stderr.is_empty() {
            stderr.clone()
        } else if !stdout.is_empty() {
            format!("stdout: {}", stdout)
        } else {
            format!(
                "进程无输出，退出码={}. 这通常表示运行时 provider 或其依赖库初始化失败。",
                code
            )
        };
        warn!(
            mode = format!("{:?}", mode),
            exit_code = code.as_str(),
            stdout = stdout.as_str(),
            stderr = stderr.as_str(),
            "sherpa-onnx cli execution failed"
        );
        return Err(format!("sherpa-onnx tts 失败({:?}): {}", mode, detail));
    }
    Ok(())
}

async fn audio_text_to_speech_sherpa_onnx(request: TextToSpeechRequest) -> Result<TextToSpeechResponse, String> {
    let text = request.input.trim().to_string();
    if text.is_empty() {
        return Err("input 不能为空".to_string());
    }
    let cfg = resolve_sherpa_onnx_tts_cli_config(&request)?;
    info!(
        exe = cfg.exe_path.to_string_lossy().as_ref(),
        model = cfg.model_path.to_string_lossy().as_ref(),
        tokens = cfg.tokens_path.to_string_lossy().as_ref(),
        sid = cfg.sid,
        speed = cfg.speed,
        text_len = text.len(),
        "audio_text_to_speech using sherpa-onnx"
    );
    let modes = detect_sherpa_tts_modes(&cfg, &request.common.mode_hint);
    let output_wav = env::temp_dir().join(format!("guaxi_sherpa_tts_{}.wav", Utc::now().timestamp_millis()));
    let mut last_err = String::new();
    for mode in modes {
        match run_sherpa_onnx_cli_once(&cfg, &text, &output_wav, mode) {
            Ok(_) => {
                last_err.clear();
                break;
            }
            Err(e) => {
                warn!(mode = format!("{:?}", mode), error = e.as_str(), "sherpa-onnx tts attempt failed");
                if cfg.runtime_provider == "cuda" {
                    let mut cpu_cfg = cfg.clone();
                    cpu_cfg.runtime_provider = "cpu".to_string();
                    warn!(
                        mode = format!("{:?}", mode),
                        error = e.as_str(),
                        "sherpa-onnx cuda provider failed, retrying with cpu"
                    );
                    match run_sherpa_onnx_cli_once(&cpu_cfg, &text, &output_wav, mode) {
                        Ok(_) => {
                            last_err.clear();
                            break;
                        }
                        Err(cpu_err) => {
                            warn!(
                                mode = format!("{:?}", mode),
                                error = cpu_err.as_str(),
                                "sherpa-onnx cpu fallback failed"
                            );
                            last_err = format!("{}\nCPU fallback also failed: {}", e, cpu_err);
                        }
                    }
                } else {
                    last_err = e;
                }
            }
        }
    }
    if !last_err.is_empty() {
        return Err(last_err);
    }
    if !output_wav.exists() {
        return Err("sherpa-onnx 未生成音频文件".to_string());
    }
    let bytes = fs::read(&output_wav).map_err(|e| format!("读取生成音频失败: {}", e))?;
    let _ = fs::remove_file(&output_wav);
    Ok(TextToSpeechResponse {
        audio_base64: general_purpose::STANDARD.encode(bytes),
        content_type: "audio/wav".to_string(),
        trace_id: "local-sherpa-onnx".to_string(),
        model: cfg.model_path.to_string_lossy().to_string(),
        response_format: "wav".to_string(),
    })
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
    if let Some(speed) = request.speed {
        body["speed"] = json!(speed);
    }
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

fn infer_audio_file_ext(file_name: &str, mime_type: &str) -> String {
    let name = file_name.trim().to_lowercase();
    if name.ends_with(".wav") {
        return ".wav".to_string();
    }
    if name.ends_with(".mp3") {
        return ".mp3".to_string();
    }
    if name.ends_with(".m4a") {
        return ".m4a".to_string();
    }
    if name.ends_with(".flac") {
        return ".flac".to_string();
    }
    let mime = mime_type.trim().to_lowercase();
    if mime.contains("wav") {
        return ".wav".to_string();
    }
    if mime.contains("mpeg") || mime.contains("mp3") {
        return ".mp3".to_string();
    }
    if mime.contains("mp4") || mime.contains("m4a") {
        return ".m4a".to_string();
    }
    if mime.contains("flac") {
        return ".flac".to_string();
    }
    ".wav".to_string()
}

fn resolve_speech_input_file(request: &SpeechToTextRequest) -> Result<(PathBuf, bool), String> {
    if !request.file_path.trim().is_empty() {
        let p = PathBuf::from(request.file_path.trim());
        if !p.exists() {
            return Err(format!("音频文件不存在: {}", p.to_string_lossy()));
        }
        return Ok((p, false));
    }
    if request.file_base64.trim().is_empty() {
        return Err("file_path 或 file_base64 至少提供一个".to_string());
    }
    let bytes = general_purpose::STANDARD
        .decode(request.file_base64.trim())
        .map_err(|e| format!("file_base64 解码失败: {}", e))?;
    let ext = infer_audio_file_ext(&request.file_name, &request.mime_type);
    let path = env::temp_dir().join(format!(
        "guaxi_sherpa_stt_{}{}",
        Utc::now().timestamp_millis(),
        ext
    ));
    fs::write(&path, bytes).map_err(|e| format!("写入临时音频失败: {}", e))?;
    Ok((path, true))
}

fn parse_sherpa_asr_text(stdout: &str, stderr: &str) -> String {
    for line in stdout.lines().rev() {
        let x = line.trim();
        if x.is_empty() {
            continue;
        }
        if let Some((left, right)) = x.split_once(':') {
            let key = left.trim().to_lowercase();
            if key.contains("result") || key.contains("text") || key.contains("transcript") {
                let v = right.trim();
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
        if !x.starts_with('[') && !x.to_lowercase().contains("sherpa-onnx") {
            return x.to_string();
        }
    }
    for line in stderr.lines().rev() {
        let x = line.trim();
        if x.is_empty() {
            continue;
        }
        if let Some((left, right)) = x.split_once(':') {
            let key = left.trim().to_lowercase();
            if key.contains("result") || key.contains("text") || key.contains("transcript") {
                let v = right.trim();
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }
    String::new()
}

async fn audio_speech_to_text_sherpa_onnx(request: SpeechToTextRequest) -> Result<SpeechToTextResponse, String> {
    let _ = bundled_resources::ensure_relative_dir("utils/sherpa-onnx")?;
    let configured = settingview::get_asr_config();
    let endpoint = request.common.endpoint.trim();
    let exe_path = if !endpoint.is_empty() {
        PathBuf::from(endpoint)
    } else if !configured.exe.trim().is_empty() {
        resolve_path_with_exe_dir(&configured.exe).unwrap_or_else(|| PathBuf::from(configured.exe.trim()))
    } else {
        resolve_sherpa_default_asr_exe()?
    };
    if !exe_path.exists() {
        return Err(format!(
            "未找到 sherpa-onnx asr 可执行文件: {}",
            exe_path.to_string_lossy()
        ));
    }
    let model = {
        let from_req = request.common.model.trim();
        if !from_req.is_empty() {
            from_req.to_string()
        } else {
            configured.model.trim().to_string()
        }
    };
    let model_path = resolve_path_with_exe_dir(&model).unwrap_or_else(|| PathBuf::from(model.as_str()));
    let (input_path, should_cleanup_input) = resolve_speech_input_file(&request)?;
    let mut cmd = Command::new(&exe_path);
    cmd.arg("--debug=false");
    if !model.trim().is_empty() {
        if model_path.is_dir() {
            let mut added = false;
            for (key, name) in [
                ("tokens", "tokens.txt"),
                ("encoder", "encoder.onnx"),
                ("decoder", "decoder.onnx"),
                ("joiner", "joiner.onnx"),
                ("paraformer", "model.onnx"),
                ("nemo-ctc", "model.onnx"),
                ("whisper-encoder", "encoder.int8.onnx"),
                ("whisper-decoder", "decoder.int8.onnx"),
            ] {
                let p = model_path.join(name);
                if p.exists() {
                    cmd.arg(format!("--{}={}", key, p.to_string_lossy()));
                    added = true;
                }
            }
            if !added {
                return Err(format!(
                    "ASR 模型目录未识别到可用模型文件: {}",
                    model_path.to_string_lossy()
                ));
            }
        } else {
            cmd.arg(format!("--model={}", model_path.to_string_lossy()));
            let tokens = model_path
                .parent()
                .map(|x| x.join("tokens.txt"));
            if let Some(tp) = tokens {
                if tp.exists() {
                    cmd.arg(format!("--tokens={}", tp.to_string_lossy()));
                }
            }
        }
    }
    cmd.arg(input_path.to_string_lossy().to_string());
    apply_no_window_in_release(&mut cmd);
    let output = cmd.output().map_err(|e| format!("执行 sherpa-onnx asr 失败: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if should_cleanup_input {
        let _ = fs::remove_file(&input_path);
    }
    if !output.status.success() {
        return Err(format!("sherpa-onnx asr 失败: {}", stderr.trim()));
    }
    let text = parse_sherpa_asr_text(&stdout, &stderr);
    if text.trim().is_empty() {
        return Err("sherpa-onnx 未返回可解析文本，请检查 ASR 模型配置".to_string());
    }
    Ok(SpeechToTextResponse {
        text: text.trim().to_string(),
        trace_id: "local-sherpa-onnx".to_string(),
        model: if model.trim().is_empty() {
            model_path.to_string_lossy().to_string()
        } else {
            model
        },
    })
}

#[tauri::command]
pub async fn audio_speech_to_text(request: SpeechToTextRequest) -> Result<SpeechToTextResponse, String> {
    let provider = resolve_audio_provider(&request.common.provider)?;
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

fn is_cjk(ch: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&ch)
        || ('\u{3400}'..='\u{4DBF}').contains(&ch)
        || ('\u{F900}'..='\u{FAFF}').contains(&ch)
}

fn flush_cjk_token_buf(buf: &mut String, token_set: &mut HashSet<String>) {
    if buf.chars().count() >= 3 {
        let full = buf.to_lowercase();
        token_set.insert(full.clone());
        let chars: Vec<char> = full.chars().collect();
        if chars.len() >= 4 {
            for window in chars.windows(3) {
                let part: String = window.iter().collect();
                token_set.insert(part);
            }
        }
    }
    buf.clear();
}

fn flush_ascii_token_buf(buf: &mut String, token_set: &mut HashSet<String>) {
    let cleaned = buf.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    if cleaned.len() >= 4 {
        token_set.insert(cleaned.to_lowercase());
    }
    buf.clear();
}

#[derive(Default)]
struct RelevanceTokens {
    strong: Vec<String>,
    weak: Vec<String>,
}

fn unique_relevance_tokens(query: &str) -> RelevanceTokens {
    let mut strong_set = HashSet::new();
    let mut weak_set = HashSet::new();
    let mut cjk_buf = String::new();
    let mut ascii_buf = String::new();

    for ch in query.chars() {
        if is_cjk(ch) {
            if !ascii_buf.is_empty() {
                flush_ascii_token_buf(&mut ascii_buf, &mut strong_set);
            }
            cjk_buf.push(ch);
            continue;
        }
        if ch.is_ascii_alphanumeric() {
            if !cjk_buf.is_empty() {
                flush_cjk_token_buf(&mut cjk_buf, &mut strong_set);
            }
            ascii_buf.push(ch);
            continue;
        }
        if !cjk_buf.is_empty() {
            flush_cjk_token_buf(&mut cjk_buf, &mut strong_set);
        }
        if !ascii_buf.is_empty() {
            flush_ascii_token_buf(&mut ascii_buf, &mut strong_set);
        }
    }
    if !cjk_buf.is_empty() {
        flush_cjk_token_buf(&mut cjk_buf, &mut strong_set);
    }
    if !ascii_buf.is_empty() {
        flush_ascii_token_buf(&mut ascii_buf, &mut strong_set);
    }

    let compact = query
        .chars()
        .filter(|c| is_cjk(*c) || c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    if compact.chars().count() >= 4 {
        strong_set.insert(compact);
    }

    for token in strong_set.clone() {
        if token.chars().all(is_cjk) {
            let chars: Vec<char> = token.chars().collect();
            if chars.len() >= 3 {
                for window in chars.windows(2) {
                    weak_set.insert(window.iter().collect::<String>());
                }
            }
        }
    }
    let weak = weak_set
        .into_iter()
        .filter(|x| !strong_set.contains(x))
        .collect::<Vec<_>>();
    RelevanceTokens {
        strong: strong_set.into_iter().collect(),
        weak,
    }
}

fn web_result_relevance_score(item: &WebSearchItem, query: &str, tokens: &RelevanceTokens) -> (i32, usize, usize) {
    let text = format!("{} {} {}", item.title, item.snippet, item.url).to_lowercase();
    let compact_query = query
        .chars()
        .filter(|c| is_cjk(*c) || c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase();

    let mut score = 0i32;
    let mut strong_hits = 0usize;
    let mut weak_hits = 0usize;
    if compact_query.len() >= 4 && text.contains(&compact_query) {
        score += 12;
        strong_hits += 1;
    }
    for token in &tokens.strong {
        if token.len() >= 3 && text.contains(token) {
            score += if token.chars().all(|c| is_cjk(c)) { 3 } else { 2 };
            strong_hits += 1;
        }
    }
    for token in &tokens.weak {
        if token.len() >= 2 && text.contains(token) {
            score += 1;
            weak_hits += 1;
        }
    }
    (score, strong_hits, weak_hits)
}

fn is_preferred_host(host: &str) -> bool {
    const PREFERRED: [&str; 28] = [
        "microsoft.com",
        "learn.microsoft.com",
        "stackoverflow.com",
        "github.com",
        "docs.github.com",
        "mozilla.org",
        "developer.mozilla.org",
        "wikipedia.org",
        "arxiv.org",
        "python.org",
        "pypi.org",
        "nodejs.org",
        "npmjs.com",
        "rust-lang.org",
        "crates.io",
        "golang.org",
        "go.dev",
        "kubernetes.io",
        "docker.com",
        "openai.com",
        "deepseek.com",
        "qwenlm.ai",
        "moonshot.cn",
        "cloudflare.com",
        "aws.amazon.com",
        "cloud.google.com",
        "azure.microsoft.com",
        "tauri.app",
    ];
    PREFERRED.iter().any(|suffix| host == *suffix || host.ends_with(&format!(".{}", suffix)))
}

fn rank_and_filter_web_results(items: Vec<WebSearchItem>, query: &str, limit: usize) -> Vec<WebSearchItem> {
    let tokens = unique_relevance_tokens(query);
    let mut ranked_items: Vec<(WebSearchItem, i32, bool)> = Vec::new();

    for item in items {
        let url_text = item.url.trim().to_lowercase();
        if url_text.is_empty() {
            continue;
        }
        if url_text.contains("baidu.com") || url_text.contains("m.baidu.com") {
            continue;
        }
        let host = Url::parse(&url_text)
            .ok()
            .and_then(|u| u.host_str().map(|x| x.to_lowercase()))
            .unwrap_or_default();
        let preferred = !host.is_empty() && is_preferred_host(&host);
        let (score, strong_hits, weak_hits) = web_result_relevance_score(&item, query, &tokens);
        if strong_hits == 0 && weak_hits < 3 {
            continue;
        }
        ranked_items.push((item, score, preferred));
    }

    ranked_items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));
    let mut out = ranked_items
        .into_iter()
        .map(|(item, _, _)| item)
        .collect::<Vec<_>>();
    out.truncate(limit);
    out
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
        Ok(items) if !items.is_empty() => {
            let ranked = rank_and_filter_web_results(items, &q, cap);
            if !ranked.is_empty() {
                return Ok(ranked);
            }
            errors.push("bing filtered empty result".to_string());
        }
        Ok(_) => errors.push("bing empty result".to_string()),
        Err(e) => errors.push(e),
    }

    match search_web_google(&client, &q, cap).await {
        Ok(items) if !items.is_empty() => {
            let ranked = rank_and_filter_web_results(items, &q, cap);
            if !ranked.is_empty() {
                return Ok(ranked);
            }
            errors.push("google filtered empty result".to_string());
        }
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
                    let ranked = rank_and_filter_web_results(hits, &q, cap);
                    if !ranked.is_empty() {
                        return Ok(ranked);
                    }
                    errors.push("duckduckgo filtered empty result".to_string());
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

#[tauri::command]
pub async fn cancel_chat_stream() -> Result<(), String> {
    request_stream_cancel();
    info!("cancel_chat_stream: cancellation requested");
    Ok(())
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
        "messages": math_prompt_messages(question),
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
    "你必须使用稳定、可渲染的 Markdown 数学语法输出内容，并严格遵守以下规则：\n\
1. 行内数学公式只能使用 $...$。\n\
2. 块级数学公式只能使用 $$...$$，并且必须严格写成以下形式：\n\
$$\n\
公式内容\n\
$$\n\
其中前后的 $$ 必须各自单独占一行。\n\
3. 严禁使用 \\[...\\]、\\(...\\)、\\begin{equation}...\\end{equation}、\\begin{align}...\\end{align}、\\begin{gather}...\\end{gather} 等 LaTeX 环境。\n\
4. 严禁用反引号包裹任何数学内容。不要输出 `\\Phi_E`、`I_{enc}`、`\\mu_0`、`d\\vec{l}` 这样的写法；参数说明中的数学符号也必须写成 $\\Phi_E$、$I_{enc}$、$\\mu_0$、$d\\vec{l}$。\n\
5. 严禁在数学公式内部混用 Markdown 强调语法，不要输出 **\\vec{E}**、*\\Phi_E*、__\\mu_0__ 这类形式。\n\
6. 如果在列表项中插入块级公式，公式前后必须保留空行；参数解释优先使用普通列表文本，数学量用 $...$ 包裹。\n\
7. 若给出代码，代码块正常使用 Markdown fenced code block，并添加必要注释；但数学内容绝不能放进代码块或反引号中。"
}

fn math_prompt_messages(question: &str) -> Value {
    json!([
        {"role": "system", "content": stable_math_system_prompt()},
        {"role": "user", "content": question}
    ])
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
    let dedented = normalize_block_math_indentation(&layout);

    if raw.contains("\\[")
        || raw.contains("\\]")
        || raw.contains("\\(")
        || raw.contains("\\)")
        || raw.contains("```")
        || out != tightened
        || tightened != layout
        || layout != dedented
    {
        info!(
            "normalize_math_symbols: raw={} out={} tightened={} layout={} dedented={}",
            excerpt_for_log(&raw),
            excerpt_for_log(&out),
            excerpt_for_log(&tightened),
            excerpt_for_log(&layout),
            excerpt_for_log(&dedented)
        );
    }

    dedented
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
) -> Result<String, String> {
    let request_body = json!({
        "model": model,
        "messages": math_prompt_messages(question),
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
            {"role": "system", "content": stable_math_system_prompt()},
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
        "messages": math_prompt_messages(question),
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
        "messages": math_prompt_messages(question),
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
        "messages": math_prompt_messages(question),
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
    sessionid: &str,
) -> Result<(), String> {
    let rsp = get_answer_from_qwen(question, sessionid).await?;
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
        "messages": [
            {"role": "system", "content": stable_math_system_prompt()},
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
            {"role": "system", "content": stable_math_system_prompt()},
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
        "messages": [
            {"role": "system", "content": stable_math_system_prompt()},
            {"role": "user", "content": question}
        ],
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
    reset_stream_cancel_flag();
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
            match get_answer_from_siliconflow_stream(app, &request.question, &request.sessionid)
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
        ChatModel::LOCALQWEN3 => {
            match get_answer_from_local_qwen_stream(app, &request.question, &request.sessionid)
                .await
            {
                Ok(()) => {
                    info!("get_chat_answer_stream LOCALQWEN3: respond stream successfully");
                    Ok(())
                }
                Err(err) => Err(format!("Error calling Local Qwen API: {}", err)),
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
            match get_answer_from_siliconflow(&request.question, &request.sessionid).await {
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
        ChatModel::LOCALQWEN3 => {
            match get_answer_from_local_qwen(&request.question, &request.sessionid).await {
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
