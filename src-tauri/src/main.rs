// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tracing::info;
use tracing_subscriber::fmt::format::FmtSpan;
use tokio::time::{sleep, Duration};

mod commands;
mod guaxiservices;
mod state;

use commands::file_watcher::FileWatcher;
use guaxiservices::server::start_backend_services;

use state::SharedState;

async fn app_init() {
    commands::settingview::load_config();
    info!("Config loaded");
    match commands::repoview::setup_db() {
        Ok(_) => println!("Database initialized successfully."),
        Err(e) => eprintln!("Failed to initialize database: {}", e),
    }
    match commands::settingview::init_library_config() {
        Ok(_) => println!("Library config initialized successfully."),
        Err(e) => eprintln!("Failed to initialize library config: {}", e),
    }
    match commands::settingview::init_resource_root() {
        Ok(_) => println!("Resource root initialized successfully."),
        Err(e) => eprintln!("Failed to initialize resource root: {}", e),
    }
    match commands::settingview::init_template_root() {
        Ok(_) => println!("Template root initialized successfully."),
        Err(e) => eprintln!("Failed to initialize template root: {}", e),
    }
    match commands::settingview::init_chat_record_root() {
        Ok(_) => println!("ChatRecord root initialized successfully."),
        Err(e) => eprintln!("Failed to initialize ChatRecord root: {}", e),
    }
    match commands::settingview::init_knowledge_bases_root() {
        Ok(_) => println!("KnowledgeBases root initialized successfully."),
        Err(e) => eprintln!("Failed to initialize KnowledgeBases root: {}", e),
    }
}

fn start_background_gc() {
    tokio::spawn(async {
        loop {
            let config = commands::settingview::load_config();
            let interval = config.gc_cycle_seconds.max(60);
            sleep(Duration::from_secs(interval)).await;
            if let Err(e) = commands::repoview::gc_deleted_items() {
                eprintln!("Background GC (database) failed: {}", e);
            }
            if let Err(e) = commands::backup::gc_conflict_files() {
                eprintln!("Background GC (conflicts) failed: {}", e);
            }
        }
    });
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
    tokio::spawn(app_init());
    start_background_gc();
    let file_watcher = FileWatcher::new();

    let shared_state = SharedState::new();

    tauri::Builder::default()
        // .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(file_watcher)
        .manage(shared_state.clone())
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
            commands::docprocess::export_markdown_pdf_pandoc,
            commands::docprocess::export_markdown_pandoc,
            commands::markdown_reader_doc_process::markdown_reader_load_markdown_file,
            commands::word_reader_doc_process::read_word_file,
            commands::epub_reader_doc_process::read_epub_file,
            commands::text_reader_doc_process::read_text_file,
            commands::translator::translate_text,
            commands::translator_free::translate_text_free,
            commands::docprocess::copy_image_file,
            commands::fileprocess::get_file_tree,
            commands::fileprocess::read_file_content,
            commands::fileprocess::write_file_content,
            commands::fileprocess::create_directory,
            commands::fileprocess::create_file,
            commands::fileprocess::delete_path,
            commands::fileprocess::rename_path,
            commands::fileprocess::move_path,
            commands::fileprocess::create_file_with_content,
            commands::fileprocess::rename_directory,
            commands::life_album::life_album_import_dir,
            commands::life_album::life_album_import_files,
            commands::life_album::life_album_scan_dir,
            commands::life_album::life_album_enrich_files,
            commands::life_album::life_album_reverse_geocode,
            commands::life_album::life_album_get_internal_root_dir,
            commands::settingview::get_life_album_album_dir,
            commands::settingview::set_life_album_album_dir,
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
            commands::settingview::get_image_bed_root,
            commands::settingview::set_image_bed_root,
            commands::settingview::get_template_repo_path,
            commands::settingview::set_template_repo_path,
            commands::settingview::get_knowledge_bases_remote_config,
            commands::settingview::set_knowledge_bases_remote_config,
            commands::settingview::get_knowledge_bases_root,
            commands::settingview::set_knowledge_bases_root,
            commands::kbmeta::create_knowledge_base_manager_window,
            commands::kbmeta::kbmeta_sync_all,
            commands::kbmeta::kbmeta_read_kb_meta,
            commands::kbmeta::kbmeta_save_kb_meta,
            commands::kbmeta::kbmeta_read_file_meta,
            commands::kbmeta::kbmeta_save_file_meta,
            commands::templateview::create_template_manager_window,
            commands::templateview::open_application_window,
            commands::templateview::emit_template_insert,
            commands::templateview::list_templates,
            commands::templateview::read_template,
            commands::templateview::save_template,
            commands::templateview::rename_template,
            commands::templateview::delete_template,
            commands::templateview::import_template_from_file,
            commands::settingview::get_gc_cycle_seconds,
            commands::settingview::set_gc_cycle_seconds,
            commands::settingview::get_lsp_syntax_highlight_enabled,
            commands::settingview::set_lsp_syntax_highlight_enabled,
            commands::settingview::get_llm_config,
            commands::settingview::set_llm_config,
            commands::settingview::get_chat_record_root,
            commands::settingview::set_chat_record_root,
            commands::chatview::save_chat_session,
            commands::chatview::export_chat_session,
            commands::chatview::open_web_model_window,
            commands::chatview::prepare_web_model_window,
            commands::chatview::web_model_send_message,
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
            commands::repoview::get_knowledge_repos,
            commands::repoview::gc_deleted_items,
            commands::repoview::add_resource,
            commands::repoview::get_resources_by_owner,
            commands::backup::backup_database,
            commands::backup::recover_database,
            commands::backup::import_items,
            commands::backup::export_items,
            commands::repoview::repofileproc::get_file_info,
            commands::repoview::repofileproc::add_repo_import_file,
            commands::repoview::repofileproc::add_web_clipper_import_file,
            commands::repoview::repofileproc::open_in_file_explorer,
            commands::repoview::repofileproc::delete_imported_all_file,
            commands::guaxireader::create_reader_window,
            commands::guaxireader::save_notes_to_file,
            commands::guaxireader::save_notes_to_file_no_append,
            commands::guaxireader::load_notes_file,
            commands::guaxireader::update_notes_file,
            commands::guaxireader::save_svg_file,
            commands::guaxireader::load_existing_notes,
            commands::guaxiscreencap::take_screenshot,
            commands::guaxiscreencap::trigger_screenshot_tool_once,
            commands::guaxiscreencap::take_screenshot_markdown,
            commands::guaxiscreencap::create_screenshot_dir,
            commands::searcher::fd_search,
            commands::searcher::rg_search,
            commands::searcher::cancel_search,
            commands::knowledgebase::create_knowledge_base,
            commands::knowledgebase::get_knowledge_base_list,
            commands::knowledgebase::get_knowledge_base_tree,
            commands::knowledgebase::get_knowledge_graph_data,
            commands::excerptsearch::search_excerpt_notes,
            commands::excerptsearch::read_excerpt_file,
            commands::git::get_git_root,
            commands::git::get_git_status,
            commands::git::get_current_branch,
            commands::git::list_branches,
            commands::git::git_commit,
            commands::git::git_push,
            commands::git::git_pull,
            commands::git::checkout_branch,
            commands::git::create_branch,
            commands::git::git_read_file,
            commands::git::get_git_log,
            commands::git::stage_file,
            commands::git::stage_all,
            commands::git::unstage_file,
            commands::git::unstage_all,
            commands::git::discard_file,
            commands::git::discard_all,
            commands::git::get_commit_diff,
            commands::git::create_git_window,
            commands::git::get_file_content,
            commands::git::get_file_diff,
            commands::git::get_commit_file_diff,
            commands::git::get_commit_files,
            commands::serverview::init_backend_services,
            commands::markdowncheck::markdown_format,
            commands::markdowncheck::markdown_batch_format,
            commands::markdowncheck::get_default_format_options,
            commands::markdowncheck::validate_markdown,
            commands::markdowncheck::markdown_clean,
            commands::markdowncheck::markdown_stats,
            commands::flipboard::get_articles,
            commands::flipboard::get_article_detail,
            commands::flipboard::search_articles,
            commands::flipboard::get_categories,
            commands::flipboard::get_feeds,
            commands::flipboard::add_feed,
            commands::flipboard::fetch_rss_feed,
            commands::flipboard::get_featured_articles,
            commands::flipboard::get_recommendations,
            commands::flipboard::get_popular_articles,
        ])
        .setup(move |app| {
            shared_state.set_app_handle(app.handle().clone());

            let state_for_server = shared_state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_backend_services(state_for_server).await {
                    eprintln!("Failed to start backend services: {}", e);
                }
            });

            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::ShortcutState;

                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_shortcuts(["ctrl+alt+z"])?
                        .with_handler(|_app, shortcut, event| {
                            if event.state == ShortcutState::Pressed {
                                info!("Shortcut pressed: {:?}", shortcut);
                                // 创建异步任务处理截图
                                tauri::async_runtime::spawn(async move {
                                    match commands::guaxiscreencap::trigger_screenshot_tool_once().await
                                    {
                                        Ok(_) => info!("截图成功"),
                                        Err(e) => eprintln!("截图失败: {}", e),
                                    }
                                });
                            }
                        })
                        .build(),
                )?;
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
