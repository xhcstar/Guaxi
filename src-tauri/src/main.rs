// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::command;
use reqwest::{Client, Error};
use futures_util::StreamExt;
use regex::Regex;

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
    println!("Data: {}", data);

    // let test_content = r#"{
    //     "choices": [
    //         {
    //             "message": {
    //                 "content": "麦克斯韦方程组（Maxwell's Equations）是描述电磁场行为的一组基本方程，由19世纪物理学家詹姆斯·克拉克·麦克斯韦（James Clerk Maxwell）提出。这组方程统一了电学和磁学，并预言了电磁波的存在，为现代电磁学奠定了基础。麦克斯韦方程组包括四个方程，通常以微分形式或积分形式表示。\n\n### 麦克斯韦方程组的微分形式\n1. **高斯定律（Gauss's Law for Electricity）**  \n   \\[\n   \\nabla \\cdot \\mathbf{E} = \\frac{\\rho}{\\varepsilon_0}\n   \\]  \n   描述电场 \\(\\mathbf{E}\\) 与电荷密度 \\(\\rho\\) 的关系，表明电场是由电荷产生的。\n\n2. **高斯磁定律（Gauss's Law for Magnetism）**  \n   \\[\n   \\nabla \\cdot \\mathbf{B} = 0\n   \\]  \n   描述磁场 \\(\\mathbf{B}\\) 的性质，表明磁场是无源的，即不存在磁单极子。\n\n3. **法拉第电磁感应定律（Faraday's Law of Induction）**  \n   \\[\n   \\nabla \\times \\mathbf{E} = -\\frac{\\partial \\mathbf{B}}{\\partial t}\n   \\]  \n   描述变化的磁场如何产生电场。\n\n4. **安培-麦克斯韦定律（Ampère's Law with Maxwell's Addition）**  \n   \\[\n   \\nabla \\times \\mathbf{B} = \\mu_0 \\mathbf{J} + \\mu_0 \\varepsilon_0 \\frac{\\partial \\mathbf{E}}{\\partial t}\n   \\]  \n   描述电流 \\(\\mathbf{J}\\) 和变化的电场如何产生磁场。\n\n### 麦克斯韦方程组的积分形式\n1. **高斯定律**  \n   \\[\n   \\oint_{\\partial V} \\mathbf{E} \\cdot d\\mathbf{A} = \\frac{Q_{\\text{enc}}}{\\varepsilon_0}\n   \\]  \n   电场通过闭合曲面的通量等于该曲面内包围的总电荷除以真空电容率 \\(\\varepsilon_0\\)。\n\n2. **高斯磁定律**  \n   \\[\n   \\oint_{\\partial V} \\mathbf{B} \\cdot d\\mathbf{A} = 0\n   \\]  \n   磁场通过任何闭合曲面的通量为零。\n\n3. **法拉第电磁感应定律**  \n   \\[\n   \\oint_{\\partial S} \\mathbf{E} \\cdot d\\mathbf{l} = -\\frac{d}{dt} \\int_S \\mathbf{B} \\cdot d\\mathbf{A}\n   \\]  \n   电场沿闭合路径的环流等于通过该路径包围曲面的磁通量的变化率的负值。\n\n4. **安培-麦克斯韦定律**  \n   \\[\n   \\oint_{\\partial S} \\mathbf{B} \\cdot d\\mathbf{l} = \\mu_0 I_{\\text{enc}} + \\mu_0 \\varepsilon_0 \\frac{d}{dt} \\int_S \\mathbf{E} \\cdot d\\mathbf{A}\n   \\]  \n   磁场沿闭合路径的环流等于通过该路径包围曲面的电流加上电场通量的变化率。\n\n..."
    //             }
    //         }
    //     ]
    // }"#;
    
    let mut chat_response: DeepSeekResponse = serde_json::from_str(&data).expect("Failed to parse JSON");
    // let mut chat_response: DeepSeekResponse = serde_json::from_str(&test_content).expect("Failed to parse JSON");

    // 替换 `\[` 和 `\]` 及相关换行符
    let re_block = Regex::new(r"\\\[\s*\n|\n\s*\\\]").unwrap();
    // 替换 \( 和 \) 及相关换行符
    let re_inline = Regex::new(r"\\\(\s*|\s*\\\)").unwrap();

    // 对 content 进行替换
    if let Some(message) = chat_response.choices.get_mut(0).map(|choice| &mut choice.message) {
        message.content = re_block.replace_all(&message.content, "$$").to_string();
        message.content = re_inline.replace_all(&message.content, "$").to_string();
    }

    println!("Answer: {}", chat_response.choices[0].message.content);

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
