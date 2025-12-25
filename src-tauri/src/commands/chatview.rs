use chrono::Utc;
use futures_util::StreamExt;
use rand::Rng;
use regex::Regex;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use crate::commands::settingview;
use serde_json::{json, Value};
use std::convert::TryFrom;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::{debug, error, info};
use url::Url;
// use std::error::Error;

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

fn generate_session_id() -> String {
    // 获取当前时间戳（秒级）
    let timestamp = Utc::now().timestamp();

    // 生成一个 4 位随机数
    let random_number: u32 = rand::rng().random_range(1000..9999);

    // 将时间戳和随机数组合成 session_id
    format!("session_{}_{}", timestamp, random_number)
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

    // 替换 `\[` 和 `\]` 及相关换行符
    let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
    // 替换 \( 和 \) 及相关换行符
    let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();

    // 对 content 进行替换
    if let Some(message) = chat_response
        .choices
        .get_mut(0)
        .map(|choice| &mut choice.message)
    {
        message.content = re_block.replace_all(&message.content, "$$").to_string();
        message.content = re_inline.replace_all(&message.content, "$").to_string();
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

fn normalize_math_symbols(mut content: String) -> String {
    let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
    let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();
    if !content.is_empty() {
        content = re_block.replace_all(&content, "$").to_string();
        content = re_inline.replace_all(&content, "$").to_string();
    }
    content
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
    let mut chat_response: DeepSeekResponse = serde_json::from_str(&data).map_err(|e| e.to_string())?;

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
    let url = "http://localhost:11434/api/chat";
    let session_id;

    info!(
        "get_answer_from_local_deepseek: Received request: {} {}",
        question, sessionid
    );

    if sessionid.is_empty() {
        session_id = generate_session_id();
        info!("get_answer_from_local_deepseek: sessionid is empty, generating a new one.");
    } else {
        session_id = sessionid.to_string();
        info!("get_answer_from_local_deepseek: session_id: {}", session_id);
    }

    let request_body = json!({
        "model": "deepseek-r1:8b",
        "messages": [{"role": "user", "content": question}],
        "stream": false,
        "session_id": session_id
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
        .post(url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        error!(
            "get_answer_from_local_deepseek: response error: {:?} ",
            response.status()
        );
        eprintln!("Request failed: {:?}", response.status());
    } else {
        info!("get_answer_from_local_deepseek: response: {:?} ", response);
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
    debug!(
        "get_answer_from_local_deepseek: full_data: {:?} ",
        full_data
    );

    let data = String::from_utf8(full_data).expect("Failed to convert bytes to string");
    info!("get_answer_from_local_deepseek: Data: {:?} ", data);

    // // 替换 `\[` 和 `\]` 及相关换行符
    // let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
    // // 替换 \( 和 \) 及相关换行符
    // let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();

    // // 对 content 进行替换
    // if let Some(message) = chat_response
    //     .choices
    //     .get_mut(0)
    //     .map(|choice| &mut choice.message)
    // {
    //     message.content = re_block.replace_all(&message.content, "$$").to_string();
    //     message.content = re_inline.replace_all(&message.content, "$").to_string();
    // }

    // info!("get_answer_from_local_deepseek: Answer replaced for math symble: {}", chat_response.choices[0].message.content);

    // let answer_content = chat_response
    //     .choices
    //     .get(0)
    //     .map(|choice| choice.message.content.clone())
    //     .unwrap_or_else(|| "No answer received.".to_string());

    // 使用?运算符传播错误
    let answer_content = collect_content_from_local_deepseek(&data).await?;

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
    let rsp = get_answer_from_chatgpt(question, sessionid).await?;
    app.emit("modle-streaming-respond-output", &rsp.answer)
        .map_err(|e| e.to_string())?;
    Ok(())
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

    let url = "https://api.deepseek.com/chat/completions";
    let session_id;

    info!(
        "get_answer_from_deepseek_stream: Received request: {:?} {}",
        question, sessionid
    );

    if sessionid.is_empty() {
        session_id = generate_session_id();
        info!("get_answer_from_deepseek_stream: sessionid is empty, generating a new one.");
    } else {
        session_id = sessionid.to_string();
        info!(
            "get_answer_from_deepseek_stream: session_id: {}",
            session_id
        );
    }

    let request_body = json!({
        "model": "deepseek-chat",
        "messages": [{"role": "user", "content": question}],
        "stream": true,
    });

    info!(
        "get_answer_from_deepseek_stream: request_body: {:?} ",
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
        return Err(format!(
            "Error calling DeepSeek API: {:?}",
            response.status()
        ));
    }

    let mut stream = response.bytes_stream();
    let mut carry = String::new();
    let mut buffer = String::new();

    // 流式输出数据
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                let chunk_str = String::from_utf8_lossy(&data).to_string();
                let mut answer_content = collect_content_from_local_deepseek(&chunk_str).await?;

                buffer.push_str(&answer_content);
                debug!(
                    "get_answer_from_local_deepseek_stream: origin buffer: {:?}",
                    buffer
                );
                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }
                debug!(
                    "get_answer_from_local_deepseek_stream: buffer: {:?}",
                    buffer
                );
                // 替换 `\[` 和 `\]` 及相关换行符
                let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
                // 替换 \( 和 \) 及相关换行符
                let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();
                // 对 content 进行替换
                if !buffer.is_empty() {
                    buffer = re_block.replace_all(&buffer, "$$").to_string();
                    buffer = re_inline.replace_all(&buffer, "$").to_string();
                }

                app.emit("modle-streaming-respond-output", &buffer)
                    .map_err(|e| e.to_string())?;
                info!(
                    "get_answer_from_local_deepseek_stream: Streaming data: {:?}",
                    buffer
                );
                debug!("get_answer_from_local_deepseek_stream: emit event modle-streaming-respond-output");

                buffer.clear();
            }
            Err(e) => {
                return Err(format!("Error while reading stream: {}", e));
            }
        }
    }

    // 流结束后，处理剩余的缓冲区数据
    let tail = carry.trim();
    if !tail.is_empty() {
        if let Some(content) = stream_extract_content(tail) {
            buffer.push_str(&content);
        }
    }

    if !buffer.is_empty() {
        // 替换 `\[` 和 `\]` 及相关换行符
        let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
        // 替换 \( 和 \) 及相关换行符
        let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();

        buffer = re_block.replace_all(&buffer, "$$").to_string();
        buffer = re_inline.replace_all(&buffer, "$").to_string();

        // 触发事件处理剩余数据
        app.emit("modle-streaming-respond-output", &buffer)
            .map_err(|e| e.to_string())?;
        debug!(
            "get_answer_from_local_deepseek_stream: Emitting remaining buffered data: {}",
            buffer
        );
        debug!("get_answer_from_local_deepseek_stream: emit event modle-streaming-respond-output");
    }

    Ok(())
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

    let url = "https://api.moonshot.cn/v1/chat/completions";
    let session_id;

    info!(
        "get_answer_from_kimi_stream: Received request: {:?} {}",
        question, sessionid
    );

    if sessionid.is_empty() {
        session_id = generate_session_id();
        info!("get_answer_from_kimi_stream: sessionid is empty, generating a new one.");
    } else {
        session_id = sessionid.to_string();
        info!("get_answer_from_kimi_stream: session_id: {}", session_id);
    }

    let request_body = json!({
        "model": "moonshot-v1-8k",
        "messages": [{"role": "user", "content": question}],
        "temperature": 0.3
    });

    info!(
        "get_answer_from_kimi_stream: request_body: {:?} ",
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

    info!("Construct response");

    if !response.status().is_success() {
        info!("Error calling kimi API:");
        return Err(format!("Error calling kimi API: {:?}", response.status()));
    }

    let mut stream = response.bytes_stream();
    let mut carry = String::new();
    let mut buffer = String::new();

    info!("get_answer_from_kimi_stream: Receive Streaming data");
    // 流式输出数据
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                let chunk_str = String::from_utf8_lossy(&data).to_string();
                info!("get_answer_from_kimi_stream: data chunk_str: {:?}", data);
                let mut answer_content: DeepSeekResponse =
                    serde_json::from_str(&chunk_str).expect("Failed to parse JSON");

                info!(
                    "get_chat_answer: chat_response {} ",
                    answer_content.choices[0].message.content
                );
                buffer.push_str(&answer_content.choices[0].message.content);
                info!("get_answer_from_kimi_stream: origin buffer: {:?}", buffer);
                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }
                info!("get_answer_from_kimi_stream: buffer: {:?}", buffer);
                // 替换 `\[` 和 `\]` 及相关换行符
                let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
                // 替换 \( 和 \) 及相关换行符
                let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();
                // 对 content 进行替换
                if !buffer.is_empty() {
                    buffer = re_block.replace_all(&buffer, "$$").to_string();
                    buffer = re_inline.replace_all(&buffer, "$").to_string();
                }

                app.emit("modle-streaming-respond-output", &buffer)
                    .map_err(|e| e.to_string())?;
                info!("get_answer_from_kimi_stream: Streaming data: {:?}", buffer);
                info!("get_answer_from_kimi_stream: emit event modle-streaming-respond-output");

                buffer.clear();
            }
            Err(e) => {
                return Err(format!("Error while reading stream: {}", e));
            }
        }
    }

    // 流结束后，处理剩余的缓冲区数据
    let tail = carry.trim();
    if !tail.is_empty() {
        if let Some(content) = stream_extract_content(tail) {
            buffer.push_str(&content);
        }
    }

    if !buffer.is_empty() {
        // 替换 `\[` 和 `\]` 及相关换行符
        let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
        // 替换 \( 和 \) 及相关换行符
        let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();

        buffer = re_block.replace_all(&buffer, "$$").to_string();
        buffer = re_inline.replace_all(&buffer, "$").to_string();

        // 触发事件处理剩余数据
        app.emit("modle-streaming-respond-output", &buffer)
            .map_err(|e| e.to_string())?;
        info!(
            "get_answer_from_kimi_stream: Emitting remaining buffered data: {}",
            buffer
        );
        info!("get_answer_from_kimi_stream: emit event modle-streaming-respond-output");
    }

    Ok(())
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
        "messages": [{"role": "user", "content": question}],
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

    // 流式输出数据
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                carry.push_str(&String::from_utf8_lossy(&data));

                for line in drain_complete_lines(&mut carry) {
                    if let Some(content) = stream_extract_content(&line) {
                        buffer.push_str(&content);
                    }
                }

                if buffer.is_empty() {
                    continue;
                }

                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }

                let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
                let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();
                buffer = re_block.replace_all(&buffer, "$").to_string();
                buffer = re_inline.replace_all(&buffer, "$").to_string();

                app.emit("modle-streaming-respond-output", &buffer)
                    .map_err(|e| e.to_string())?;

                buffer.clear();
            }
            Err(e) => {
                return Err(format!("Error while reading stream: {}", e));
            }
        }
    }

    // 流结束后，处理剩余的缓冲区数据
    if !buffer.is_empty() {
        // 替换 `\[` 和 `\]` 及相关换行符
        let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
        // 替换 \( 和 \) 及相关换行符
        let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();

        buffer = re_block.replace_all(&buffer, "$$").to_string();
        buffer = re_inline.replace_all(&buffer, "$").to_string();

        // 触发事件处理剩余数据
        app.emit("modle-streaming-respond-output", &buffer)
            .map_err(|e| e.to_string())?;
        debug!(
            "get_answer_from_local_qwen_stream: Emitting remaining buffered data: {}",
            buffer
        );
        debug!("get_answer_from_local_qwen_stream: emit event modle-streaming-respond-output");
    }

    Ok(())
}

pub async fn get_answer_from_local_deepseek_stream(
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
        "model": "deepseek-r1:8b",
        "messages": [{"role": "user", "content": question}],
        "stream": true,  // 启用流式传输
        "session_id": session_id,
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
        .post(url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!(
            "Error calling DeepSeek API: {:?}",
            response.status()
        ));
    }

    let mut stream = response.bytes_stream();
    let mut carry = String::new();
    let mut buffer = String::new();
    let mut last_emitted = String::new(); // 新增：记录上次发送的内容

    // 流式输出数据
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                carry.push_str(&String::from_utf8_lossy(&data));

                for line in drain_complete_lines(&mut carry) {
                    if let Some(content) = stream_extract_content(&line) {
                        buffer.push_str(&content);
                    }
                }

                debug!(
                    "get_answer_from_local_deepseek_stream: origin buffer: {:?}",
                    buffer
                );

                if buffer.is_empty() {
                    continue;
                }

                // 跳过特定字符的情况保持不变
                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }

                debug!(
                    "get_answer_from_local_deepseek_stream: buffer: {:?}",
                    buffer
                );

                // 替换 `\[` 和 `\]` 及相关换行符
                let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
                // 替换 \( 和 \) 及相关换行符
                let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();
                // 对 content 进行替换
                if !buffer.is_empty() {
                    buffer = re_block.replace_all(&buffer, "$").to_string();
                    buffer = re_inline.replace_all(&buffer, "$").to_string();
                }

                // 新增：只有内容变化时才发送
                if buffer != last_emitted && !buffer.is_empty() {
                    app.emit("modle-streaming-respond-output", &buffer)
                        .map_err(|e| e.to_string())?;
                    last_emitted = buffer.clone(); // 更新最后发送的内容
                    debug!(
                        "get_answer_from_local_deepseek_stream: Streaming data: {:?}",
                        buffer
                    );
                }

                buffer.clear();
            }
            Err(e) => {
                return Err(format!("Error while reading stream: {}", e));
            }
        }
    }

    // 流结束后，处理剩余的缓冲区数据
    if !buffer.is_empty() {
        // 替换 `\[` 和 `\]` 及相关换行符
        let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
        // 替换 \( 和 \) 及相关换行符
        let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();

        buffer = re_block.replace_all(&buffer, "$$").to_string();
        buffer = re_inline.replace_all(&buffer, "$").to_string();

        if buffer != last_emitted && !buffer.is_empty() {
            // 触发事件处理剩余数据
            app.emit("modle-streaming-respond-output", &buffer)
                .map_err(|e| e.to_string())?;
            debug!(
                "get_answer_from_local_deepseek_stream: Emitting remaining buffered data: {}",
                buffer
            );
        }

        debug!("get_answer_from_local_deepseek_stream: emit event modle-streaming-respond-output");
    }

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
        "messages": [{"role": "user", "content": question}],
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
        return Err(format!("Error calling Local Qwen API: {:?}", response.status()));
    }

    let data = response.text().await.map_err(|e| e.to_string())?;
    let answer_content = collect_content_from_local_deepseek(&data).await?;

    Ok(ChatResponse {
        answer: normalize_math_symbols(answer_content),
        sessionid: request_body["session_id"].as_str().unwrap_or("").to_string(),
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
    pub content: Value,
}

#[tauri::command]
pub async fn export_chat_session(request: ExportChatSessionRequest) -> Result<String, String> {
    let root = ensure_chat_record_root()?;

    let model = sanitize_filename_component(
        request
            .model_name
            .as_deref()
            .unwrap_or("chat"),
    );
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
    pub content: Value,
}

#[tauri::command]
pub async fn save_chat_session(request: SaveChatSessionRequest) -> Result<String, String> {
    let root = ensure_chat_record_root()?;
    let sessions_dir = root.join("sessions");
    if !sessions_dir.exists() {
        fs::create_dir_all(&sessions_dir).map_err(|e| e.to_string())?;
    }

    let model = sanitize_filename_component(
        request
            .model_name
            .as_deref()
            .unwrap_or("chat"),
    );

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
    let provider = provider.trim().to_lowercase();
    format!(
        r#"(() => {{
  try {{
    const PROVIDER = {provider:?};
    const ENDPOINT = 'http://127.0.0.1:8888/api/web-model-chat';

    if (window.__GUAXI_WEB_CAPTURE_INSTALLED__) return;
    window.__GUAXI_WEB_CAPTURE_INSTALLED__ = true;

    function getSessionId() {{
      const k = `guaxi_web_session_${{PROVIDER}}`;
      let v = null;
      try {{ v = localStorage.getItem(k); }} catch (e) {{}}
      if (!v) {{
        v = `${{Date.now()}}-${{Math.random().toString(16).slice(2)}}`;
        try {{ localStorage.setItem(k, v); }} catch (e) {{}}
      }}
      return v;
    }}

    function textOf(el) {{
      if (!el) return '';
      const t = (el.innerText || el.textContent || '').trim();
      return t;
    }}

    function collectChatgptMessages() {{
      const nodes = Array.from(document.querySelectorAll('[data-message-author-role]'));
      const out = [];
      for (const n of nodes) {{
        const role = (n.getAttribute('data-message-author-role') || '').toLowerCase();
        const content = textOf(n);
        if (!content) continue;
        out.push({{ role: role === 'user' ? 'user' : 'assistant', content }});
      }}
      return out;
    }}

    function collectGenericMessages() {{
      const candidates = Array.from(document.querySelectorAll('main, [role="main"], body'));
      const root = candidates.find(Boolean) || document.body;
      const blocks = Array.from(root.querySelectorAll('div, p, li'));
      const out = [];
      for (const b of blocks) {{
        const content = textOf(b);
        if (!content) continue;
        if (content.length < 6) continue;
        if (content.length > 2000) continue;
        out.push({{ role: 'assistant', content }});
      }}
      return out.slice(-20);
    }}

    function collectMessages() {{
      if (PROVIDER === 'chatgpt') {{
        const msgs = collectChatgptMessages();
        return msgs.length ? msgs : collectGenericMessages();
      }}
      return collectGenericMessages();
    }}

    let lastSig = '';
    let timer = null;

    async function push() {{
      try {{
        const messages = collectMessages();
        const sig = JSON.stringify(messages.slice(-6));
        if (!messages.length || sig === lastSig) return;
        lastSig = sig;

        await fetch(ENDPOINT, {{
          method: 'POST',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify({{
            provider: PROVIDER,
            session_id: getSessionId(),
            messages
          }})
        }});
      }} catch (e) {{}}
    }}

    const observer = new MutationObserver(() => {{
      if (timer) return;
      timer = setTimeout(() => {{
        timer = null;
        push();
      }}, 800);
    }});

    observer.observe(document.documentElement, {{ childList: true, subtree: true, characterData: true }});

    setInterval(push, 3500);
    push();
  }} catch (e) {{}}
}})();"#
    )
}

#[tauri::command]
pub async fn open_web_model_window(app: tauri::AppHandle, provider: String) -> Result<(), String> {
    let provider_key = provider.trim().to_lowercase();
    let url = web_model_url(&provider_key).ok_or("Unsupported provider".to_string())?;

    let label = format!("web-model-{}", provider_key);
    if let Some(w) = app.get_webview_window(&label) {
        w.show().map_err(|e| format!("显示窗口失败: {}", e))?;
        w.set_focus().ok();
        return Ok(());
    }

    let external = Url::parse(url).map_err(|e| e.to_string())?;
    let init_script = web_model_capture_init_script(&provider_key);

    let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(external))
        .title(url)
        .inner_size(1100.0, 780.0)
        .min_inner_size(820.0, 560.0)
        .initialization_script(init_script.as_str())
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
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
            open_web_model_window(app.clone(), "yuanbao".to_string()).await?;
            app.emit(
                "modle-streaming-respond-output",
                "该模型为网页模式，已打开腾讯元宝（请在新窗口内对话）。",
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        }
        ChatModel::CHATGPT_WEB => {
            open_web_model_window(app.clone(), "chatgpt".to_string()).await?;
            app.emit(
                "modle-streaming-respond-output",
                "该模型为网页模式，已打开 ChatGPT 网页（请在新窗口内对话）。",
            )
            .map_err(|e| e.to_string())?;
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
            answer: "该模型为网页模式，请调用 open_web_model_window(provider='yuanbao') 进行对话。".to_string(),
            sessionid: if request.sessionid.is_empty() {
                generate_session_id()
            } else {
                request.sessionid
            },
        }),
        ChatModel::CHATGPT_WEB => Ok(ChatResponse {
            answer: "该模型为网页模式，请调用 open_web_model_window(provider='chatgpt') 进行对话。".to_string(),
            sessionid: if request.sessionid.is_empty() {
                generate_session_id()
            } else {
                request.sessionid
            },
        }),
    }
}
