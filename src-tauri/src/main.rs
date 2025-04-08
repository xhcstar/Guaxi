// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tracing::info;
use tracing_subscriber;
use tracing_subscriber::fmt::format::FmtSpan;

mod commands;
mod guaxiservices;

use commands::file_watcher::FileWatcher;
use guaxiservices::server::start_backend_services;

async fn app_init() {
    // 在启动时加载配置文件
    let config = commands::settingview::load_config();
    info!("Loaded config: {:?}", config);
    match commands::repoview::setup_db() {
        Ok(_) => println!("Database initialized successfully."),
        Err(e) => eprintln!("Failed to initialize database: {}", e),
    }
    match commands::settingview::init_library_config() {
        Ok(_) => println!("Library config initialized successfully."),
        Err(e) => eprintln!("Failed to initialize library config: {}", e),
    }
}

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
    app_init().await;
    let file_watcher = FileWatcher::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(file_watcher)
        .invoke_handler(tauri::generate_handler![
            commands::file_watcher::start_file_watching,
            commands::file_watcher::stop_file_watching,
            commands::chatview::get_chat_answer,
            commands::chatview::get_chat_answer_stream,
            commands::docprocess::reader_open_file,
            commands::docprocess::convert_to_markdown,
            commands::docprocess::check_file_converted,
            commands::docprocess::get_converted_pdf_page,
            commands::docprocess::load_markdown_content,
            commands::fileprocess::get_file_tree,
            commands::fileprocess::read_file_content,
            commands::fileprocess::write_file_content,
            commands::fileprocess::create_directory,
            commands::fileprocess::create_file,
            commands::fileprocess::delete_path,
            commands::fileprocess::rename_path,
            commands::fileprocess::move_path,
            commands::settingview::get_active_repo_path,
            commands::settingview::set_active_repo_path,
            commands::settingview::get_server_config,
            commands::settingview::set_server_config,
            commands::settingview::get_database_config,
            commands::settingview::set_database_config,
            commands::settingview::get_appearance_config,
            commands::settingview::set_appearance_config,
            commands::settingview::set_repopathlist,
            commands::settingview::get_repopathlist,
            commands::settingview::get_libraries_repo_config,
            commands::settingview::set_libraries_repo_config,
            commands::settingview::get_libraries_root,
            commands::settingview::set_libraries_root,
            commands::repoview::get_imported_files,
            commands::repoview::add_imported_file,
            commands::repoview::delete_imported_file,
            commands::repoview::update_imported_file,
            commands::repoview::get_notes,
            commands::repoview::add_note,
            commands::repoview::delete_note,
            commands::repoview::update_note,
            commands::repoview::search_notes_tags,
            commands::repoview::search_imported_files_tags,
            commands::repoview::search_imported_files,
            commands::repoview::search_libraries,
            commands::repoview::search_imported_file_libraries,
            commands::repoview::repofileproc::get_file_info,
            commands::repoview::repofileproc::add_repo_import_file,
            commands::gauxireader::create_reader_window,
        ])
        .setup(|_app| {
            // 直接启动服务而不使用spawn
            tauri::async_runtime::spawn(async {
                if let Err(e) = start_backend_services().await {
                    eprintln!("Failed to start backend services: {}", e);
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
