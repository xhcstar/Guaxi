// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tracing::info;
use tracing_subscriber;
use tracing_subscriber::fmt::format::FmtSpan;
// use tauri::command;

mod commands;

#[tokio::main]
async fn main() {
    // 初始化日志订阅器，输出到终端和文件
    // let file_appender = tracing_appender::rolling::daily("logs", "logfile.log");
    // let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO) // 设置最大日志级别
        .with_span_events(FmtSpan::CLOSE) // 关闭 span 结束事件
        // .json()  // 输出 JSON 格式（可选）
        .with_writer(std::io::stdout) // 输出到终端
        // .with_writer(non_blocking)  // 输出到文件
        .init(); // 初始化

    // 示例日志输出
    info!("Project started!");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::chatview::get_chat_answer,
            commands::chatview::get_chat_answer_stream,
            commands::docprocess::openfile,
            commands::fileprocess::get_file_tree,
            commands::fileprocess::read_file_content,
            commands::fileprocess::write_file_content,
            commands::fileprocess::create_directory,
            commands::fileprocess::create_file,
            commands::fileprocess::delete_path,
            commands::fileprocess::rename_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
