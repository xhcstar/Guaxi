use futures_util::{StreamExt};
use regex::Regex;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::Utc;
use rand::Rng;
use std::convert::TryFrom;
use tracing::{error, info, debug};
use tauri::{AppHandle, Emitter};
// use std::error::Error;


enum ChatModel {
    CHATGPT,
    DEEPSEEK,
    QWEN,
    KIMI,
    DOUBAO,
    LOCALQWEN,
    LOCALDEEPSEEK,
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
        // 解析每一行的 JSON
        let response: LocalDeepSeekResponse = serde_json::from_str(line).map_err(|e| e.to_string())?;
        
        // 提取 content 字段并拼接
        full_answer.push_str(&response.message.content);
    }

    // 返回完整的回答
    Ok(full_answer)
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
pub async fn get_answer_from_deepseek(question: &str, sessionid: &str) -> Result<ChatResponse, String> {
    let api_key = "REDACTED_DEEPSEEK_API_KEY";
    let url = "https://api.deepseek.com/chat/completions";
    let session_id;

    info!("get_answer_from_deepseek: Received request: {:?} {}", question, sessionid);

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

    info!("get_answer_from_deepseek: request_body: {:?} ", request_body);

    let client = Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        error!("get_answer_from_deepseek: response error: {:?} ", response.status());
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

    info!("get_chat_answer: chat_response {} ", chat_response.choices[0].message.content);

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

    info!("Answer replaced for math symble: {}", chat_response.choices[0].message.content);

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

pub async fn get_answer_from_chatgpt(question: &str, sessionid: &str) -> Result<ChatResponse, String> {
    info!("get_answer_from_chatgpt: Received request: {} {}", question, sessionid);
    Err("Function not implemented yet".to_string())
}

pub async fn get_answer_from_kimi(question: &str, sessionid: &str) -> Result<ChatResponse, String> {
    info!("get_answer_from_kimi: Received request: {} {}", question, sessionid);
    Err("Function not implemented yet".to_string())
}

pub async fn get_answer_from_qwen(question: &str, sessionid: &str) -> Result<ChatResponse, String> {
    info!("get_answer_from_qwen: Received request: {} {}", question, sessionid);
    Err("Function not implemented yet".to_string())
}

pub async fn get_answer_from_doubao(question: &str, sessionid: &str) -> Result<ChatResponse, String> {
    info!("get_answer_from_doubao: Received request: {} {}", question, sessionid);
    Err("Function not implemented yet".to_string())
}

pub async fn get_answer_from_local_deepseek(question: &str, sessionid: &str) -> Result<ChatResponse, String> {
    let url = "http://localhost:11434/api/chat";
    let session_id;

    info!("get_answer_from_local_deepseek: Received request: {} {}", question, sessionid);

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

    info!("get_answer_from_local_deepseek: request_body: {:?} ", request_body);

    // 禁用系统代理
    let client = ClientBuilder::new()
    .no_proxy()  // 禁用代理
    .build()
    .map_err(|e| e.to_string())?;

    let response = client
        .post(url)
        .json(&request_body)
        .send()
        .await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        error!("get_answer_from_local_deepseek: response error: {:?} ", response.status());
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
    debug!("get_answer_from_local_deepseek: full_data: {:?} ", full_data);

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

pub async fn get_answer_from_chatgpt_stream(app: AppHandle, question: &str, sessionid: &str) -> Result<(), String> {
    Ok(())
}
pub async fn get_answer_from_deepseek_stream(app: AppHandle, question: &str, sessionid: &str) -> Result<(), String> {
    let api_key = "REDACTED_DEEPSEEK_API_KEY";
    let url = "https://api.deepseek.com/chat/completions";
    let session_id;

    info!("get_answer_from_deepseek_stream: Received request: {:?} {}", question, sessionid);

    if sessionid.is_empty() {
        session_id = generate_session_id();
        info!("get_answer_from_deepseek_stream: sessionid is empty, generating a new one.");
    } else {
        session_id = sessionid.to_string();
        info!("get_answer_from_deepseek_stream: session_id: {}", session_id);
    }

    let request_body = json!({
        "model": "deepseek-chat",
        "messages": [{"role": "user", "content": question}],
        "stream": true,
    });

    info!("get_answer_from_deepseek_stream: request_body: {:?} ", request_body);

    let client = Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Error calling DeepSeek API: {:?}", response.status()));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    // 流式输出数据
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                let chunk_str = String::from_utf8_lossy(&data).to_string();
                let mut answer_content = collect_content_from_local_deepseek(&chunk_str).await?;

                buffer.push_str(&answer_content);
                debug!("get_answer_from_local_deepseek_stream: origin buffer: {:?}", buffer);
                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }
                debug!("get_answer_from_local_deepseek_stream: buffer: {:?}", buffer);
                // 替换 `\[` 和 `\]` 及相关换行符
                let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
                // 替换 \( 和 \) 及相关换行符
                let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();
                // 对 content 进行替换
                if !buffer.is_empty() {
                    buffer = re_block.replace_all(&buffer, "$$").to_string();
                    buffer = re_inline.replace_all(&buffer, "$").to_string();
                }

                app.emit("modle-streaming-respond-output", &buffer).map_err(|e| e.to_string())?;
                info!("get_answer_from_local_deepseek_stream: Streaming data: {:?}", buffer);
                debug!("get_answer_from_local_deepseek_stream: emit event modle-streaming-respond-output");

                buffer.clear();
            },
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
        app.emit("modle-streaming-respond-output", &buffer).map_err(|e| e.to_string())?;
        debug!("get_answer_from_local_deepseek_stream: Emitting remaining buffered data: {}", buffer);
        debug!("get_answer_from_local_deepseek_stream: emit event modle-streaming-respond-output");
    }

    Ok(())
}

pub async fn get_answer_from_kimi_stream(app: AppHandle, question: &str, sessionid: &str) -> Result<(), String> {
    let api_key = "REDACTED_MOONSHOT_API_KEY";
    let url = "https://api.moonshot.cn/v1/chat/completions";
    let session_id;

    info!("get_answer_from_kimi_stream: Received request: {:?} {}", question, sessionid);

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

    info!("get_answer_from_kimi_stream: request_body: {:?} ", request_body);

    let client = Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await.map_err(|e| e.to_string())?;

    info!("Construct response");

    if !response.status().is_success() {
        info!("Error calling kimi API:");
        return Err(format!("Error calling kimi API: {:?}", response.status()));
    }

    let mut stream = response.bytes_stream();
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
        
                info!("get_chat_answer: chat_response {} ", answer_content.choices[0].message.content);
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

                app.emit("modle-streaming-respond-output", &buffer).map_err(|e| e.to_string())?;
                info!("get_answer_from_kimi_stream: Streaming data: {:?}", buffer);
                info!("get_answer_from_kimi_stream: emit event modle-streaming-respond-output");

                buffer.clear();
            },
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
        app.emit("modle-streaming-respond-output", &buffer).map_err(|e| e.to_string())?;
        info!("get_answer_from_kimi_stream: Emitting remaining buffered data: {}", buffer);
        info!("get_answer_from_kimi_stream: emit event modle-streaming-respond-output");
    }

    Ok(())
}

pub async fn get_answer_from_doubao_stream(app: AppHandle, question: &str, sessionid: &str) -> Result<(), String> {
    Ok(())
}

pub async fn get_answer_from_qwen_stream(app: AppHandle, question: &str, sessionid: &str) -> Result<(), String> {
    Ok(())
}

pub async fn get_answer_from_local_qwen_stream(app: AppHandle, question: &str, sessionid: &str) -> Result<(), String> {
    let url = "http://localhost:11434/api/chat";
    let session_id = if sessionid.is_empty() {
        generate_session_id()
    } else {
        sessionid.to_string()
    };

    let request_body = json!({
        "model": "qwen2.5:latest",
        "messages": [{"role": "user", "content": question}],
        "stream": true,  // 启用流式传输
    });

    info!("get_answer_from_local_qwen_stream: request_body: {:?}", request_body);

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
        return Err(format!("Error calling DeepSeek API: {:?}", response.status()));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    // 流式输出数据
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                let chunk_str = String::from_utf8_lossy(&data).to_string();
                let mut answer_content = collect_content_from_local_deepseek(&chunk_str).await?;

                buffer.push_str(&answer_content);
                debug!("get_answer_from_local_qwen_stream: origin buffer: {:?}", buffer);
                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }
                debug!("get_answer_from_local_qwen_stream: buffer: {:?}", buffer);
                // 替换 `\[` 和 `\]` 及相关换行符
                let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
                // 替换 \( 和 \) 及相关换行符
                let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();
                // 对 content 进行替换
                if !buffer.is_empty() {
                    buffer = re_block.replace_all(&buffer, "$$").to_string();
                    buffer = re_inline.replace_all(&buffer, "$").to_string();
                }

                // info!("Answer replaced for math symble: {}", buffer);
                app.emit("modle-streaming-respond-output", &buffer).map_err(|e| e.to_string())?;
                debug!("get_answer_from_local_qwen_stream: Streaming data: {:?}", buffer);
                debug!("get_answer_from_local_deepseek_stream: emit event modle-streaming-respond-output");

                buffer.clear();
            },
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
        app.emit("modle-streaming-respond-output", &buffer).map_err(|e| e.to_string())?;
        debug!("get_answer_from_local_qwen_stream: Emitting remaining buffered data: {}", buffer);
        debug!("get_answer_from_local_qwen_stream: emit event modle-streaming-respond-output");
    }

    Ok(())
}

pub async fn get_answer_from_local_deepseek_stream(app: AppHandle, question: &str, sessionid: &str) -> Result<(), String> {
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
    });

    info!("get_answer_from_local_deepseek_stream: request_body: {:?}", request_body);

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
        return Err(format!("Error calling DeepSeek API: {:?}", response.status()));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    // 流式输出数据
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                let chunk_str = String::from_utf8_lossy(&data).to_string();
                let mut answer_content = collect_content_from_local_deepseek(&chunk_str).await?;

                buffer.push_str(&answer_content);
                debug!("get_answer_from_local_deepseek_stream: origin buffer: {:?}", buffer);
                if buffer.ends_with('\\') || buffer.ends_with(' ') || buffer.ends_with('\n') {
                    continue;
                }
                debug!("get_answer_from_local_deepseek_stream: buffer: {:?}", buffer);
                // 替换 `\[` 和 `\]` 及相关换行符
                let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
                // 替换 \( 和 \) 及相关换行符
                let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();
                // 对 content 进行替换
                if !buffer.is_empty() {
                    buffer = re_block.replace_all(&buffer, "$$").to_string();
                    buffer = re_inline.replace_all(&buffer, "$").to_string();
                }

                app.emit("modle-streaming-respond-output", &buffer).map_err(|e| e.to_string())?;
                info!("get_answer_from_local_deepseek_stream: Streaming data: {:?}", buffer);
                debug!("get_answer_from_local_deepseek_stream: emit event modle-streaming-respond-output");

                buffer.clear();
            },
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
        app.emit("modle-streaming-respond-output", &buffer).map_err(|e| e.to_string())?;
        debug!("get_answer_from_local_deepseek_stream: Emitting remaining buffered data: {}", buffer);
        debug!("get_answer_from_local_deepseek_stream: emit event modle-streaming-respond-output");
    }

    Ok(())
}

pub async fn get_answer_from_local_qwen(question: &str, sessionid: &str) -> Result<ChatResponse, String> {
    info!("get_answer_from_local_qwen: Received request: {} {}", question, sessionid);
    Err("Function not implemented yet".to_string())
}

#[tauri::command]
pub async fn get_chat_answer_stream(app: AppHandle, request: ChatRequest) -> Result<(), String> {
    info!("Enter get_chat_answer_stream");
    let model = ChatModel::try_from(request.modelid)
    .map_err(|e| format!("Invalid model ID: {}", e))?;
    info!("Received request: model_id:{} question:{} session_id:{}", request.modelid, request.question, request.sessionid);

    match model {
        ChatModel::CHATGPT => match get_answer_from_chatgpt_stream(app, &request.question, &request.sessionid).await {
            Ok(()) => {
                info!("get_chat_answer_stream CHATGPT: respond stream successfully");
                Ok(())
            },
            Err(err) => Err(format!("Error calling ChatGPT API: {}", err)),
        },
        ChatModel::DEEPSEEK => match get_answer_from_deepseek_stream(app, &request.question, &request.sessionid).await {
            Ok(()) => {
                info!("get_chat_answer_stream DEEPSEEK: respond stream successfully");
                Ok(())
            },
            Err(err) => Err(format!("Error calling DeepSeek API: {}", err)),
        },
        ChatModel::KIMI => match get_answer_from_kimi_stream(app, &request.question, &request.sessionid).await {
            Ok(()) => {
                info!("get_chat_answer_stream KIMI: respond stream successfully");
                Ok(())
            },
            Err(err) => Err(format!("Error calling Kimi API: {}", err)),
        },
        ChatModel::QWEN => match get_answer_from_qwen_stream(app, &request.question, &request.sessionid).await {
            Ok(()) => {
                info!("get_chat_answer_stream QWEN: respond stream successfully");
                Ok(())
            },
            Err(err) => Err(format!("Error calling Qwen API: {}", err)),
        },
        ChatModel::DOUBAO => match get_answer_from_doubao_stream(app, &request.question, &request.sessionid).await {
            Ok(()) => {
                info!("get_chat_answer_stream DOUBAO: respond stream successfully");
                Ok(())
            },
            Err(err) => Err(format!("Error calling Doubao API: {}", err)),
        },
        ChatModel::LOCALQWEN => match get_answer_from_local_qwen_stream(app, &request.question, &request.sessionid).await {
            Ok(()) => {
                info!("get_chat_answer_stream LOCALQWEN: respond stream successfully");
                Ok(())
            },
            Err(err) => Err(format!("Error calling Local Qwen API: {}", err)),
        },
        ChatModel::LOCALDEEPSEEK => match get_answer_from_local_deepseek_stream(app, &request.question, &request.sessionid).await {
            Ok(()) => {
                info!("get_chat_answer_stream LOCALDEEPSEEK: respond stream successfully");
                Ok(())
            },
            Err(err) => Err(format!("Error calling Local Model API: {}", err)),
        }
    }
}

#[tauri::command]
pub async fn get_chat_answer(request: ChatRequest) -> Result<ChatResponse, String> {
    info!("Enter get_chat_answer");
    let model = ChatModel::try_from(request.modelid)
    .map_err(|e| format!("Invalid model ID: {}", e))?;
    info!("Received request: model_id:{} question:{} session_id:{}", request.modelid, request.question, request.sessionid);

    match model {
        ChatModel::CHATGPT => match get_answer_from_chatgpt(&request.question, &request.sessionid).await {
            Ok(answer) => {
                info!("get_chat_answer CHATGPT: respond ChatResponse  {} {}", answer.answer, answer.sessionid);
                Ok(answer)
            },
            Err(err) => Err(format!("Error calling ChatGPT API: {}", err)),
        },
        ChatModel::DEEPSEEK => match get_answer_from_deepseek(&request.question, &request.sessionid).await {
            Ok(answer) => {
                info!("get_chat_answer DEEPSEEK: respond ChatResponse  {} {}", answer.answer, answer.sessionid);
                Ok(answer)
            },
            Err(err) => Err(format!("Error calling DeepSeek API: {}", err)),
        },
        ChatModel::KIMI => match get_answer_from_kimi(&request.question, &request.sessionid).await {
            Ok(answer) => {
                info!("get_chat_answer KIMI: respond ChatResponse  {} {}", answer.answer, answer.sessionid);
                Ok(answer)
            },
            Err(err) => Err(format!("Error calling Kimi API: {}", err)),
        },
        ChatModel::QWEN => match get_answer_from_qwen(&request.question, &request.sessionid).await {
            Ok(answer) => {
                info!("get_chat_answer QWEN: respond ChatResponse  {} {}", answer.answer, answer.sessionid);
                Ok(answer)
            },
            Err(err) => Err(format!("Error calling Qwen API: {}", err)),
        },
        ChatModel::DOUBAO => match get_answer_from_doubao(&request.question, &request.sessionid).await {
            Ok(answer) => {
                info!("get_chat_answer DOUBAO: respond ChatResponse  {} {}", answer.answer, answer.sessionid);
                Ok(answer)
            },
            Err(err) => Err(format!("Error calling Doubao API: {}", err)),
        },
        ChatModel::LOCALQWEN => match get_answer_from_local_qwen(&request.question, &request.sessionid).await {
            Ok(answer) => {
                info!("get_chat_answer LOCALQWEN: respond ChatResponse  {} {}", answer.answer, answer.sessionid);
                Ok(answer)
            },
            Err(err) => Err(format!("Error calling Local Qwen API: {}", err)),
        },
        ChatModel::LOCALDEEPSEEK => match get_answer_from_local_deepseek(&request.question, &request.sessionid).await {
            Ok(answer) => {
                info!("get_chat_answer LOCALDEEPSEEK: respond ChatResponse  {} {}", answer.answer, answer.sessionid);
                Ok(answer)
            },
            Err(err) => Err(format!("Error calling Local Model API: {}", err)),
        }
    }
}