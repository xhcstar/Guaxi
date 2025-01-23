// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::command;
use reqwest::{Client, Error};
use futures_util::StreamExt;

#[derive(Deserialize)]
struct ChatRequest {
    modelid: i32,
    question: String,
    topicid: i32,
}

#[derive(Serialize)]
struct ChatResponse {
    answer: String,
}

#[derive(Deserialize)]
struct DeepSeekMessage {
    content: String,
}

#[derive(Deserialize)]
struct DeepSeekChoice {
    message: DeepSeekMessage,
}

#[derive(Deserialize)]
struct DeepSeekResponse {
    choices: Vec<DeepSeekChoice>,
}

// 调用 DeepSeek API 获取回答
async fn get_answer_from_deepseek(question: &str) -> Result<String, Error> {
    let api_key = "REDACTED_DEEPSEEK_API_KEY";
    let url = "https://api.deepseek.com/chat/completions";

    let request_body = json!({
        "model": "deepseek-chat",
        "messages": [{"role": "user", "content": question}],
    });

    let client = Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        eprintln!("Request failed: {:?}", response.status());
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

    let data = String::from_utf8(full_data).expect("Failed to convert bytes to string");

    let chat_response: DeepSeekResponse = serde_json::from_str(&data).expect("Failed to parse JSON");;

    Ok(chat_response
        .choices
        .get(0)
        .map(|choice| choice.message.content.clone())
        .unwrap_or_else(|| "No answer received.".to_string()))
}

#[command]
async fn get_chat_answer(request: ChatRequest) -> Result<ChatResponse, String> {
    match get_answer_from_deepseek(&request.question).await {
        Ok(answer) => Ok(ChatResponse { answer }),
        Err(err) => Err(format!("Error calling DeepSeek API: {}", err)),
    }
}

#[tokio::main]
async fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_chat_answer])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
