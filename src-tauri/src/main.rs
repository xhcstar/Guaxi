// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use once_cell::sync::OnceCell;
use std::fs;
use std::path::Path;
use std::time::{Duration as StdDuration, SystemTime};
use tokio::time::{sleep, Duration};
use tracing::{error, info};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;
#[cfg(desktop)]
use tauri::{Emitter, Manager};

static LOG_GUARD: OnceCell<tracing_appender::non_blocking::WorkerGuard> = OnceCell::new();

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
        Ok(_) => info!("Database initialized successfully."),
        Err(e) => error!("Failed to initialize database: {}", e),
    }
    match commands::settingview::init_library_config() {
        Ok(_) => info!("Library config initialized successfully."),
        Err(e) => error!("Failed to initialize library config: {}", e),
    }
    match commands::settingview::init_resource_root() {
        Ok(_) => info!("Resource root initialized successfully."),
        Err(e) => error!("Failed to initialize resource root: {}", e),
    }
    match commands::settingview::init_template_root() {
        Ok(_) => info!("Template root initialized successfully."),
        Err(e) => error!("Failed to initialize template root: {}", e),
    }
    match commands::settingview::init_chat_record_root() {
        Ok(_) => info!("ChatRecord root initialized successfully."),
        Err(e) => error!("Failed to initialize ChatRecord root: {}", e),
    }
    match commands::settingview::init_knowledge_bases_root() {
        Ok(_) => info!("KnowledgeBases root initialized successfully."),
        Err(e) => error!("Failed to initialize KnowledgeBases root: {}", e),
    }
}

fn start_background_gc() {
    tokio::spawn(async {
        loop {
            let config = commands::settingview::load_config();
            let interval = config.gc_cycle_seconds.max(60);
            sleep(Duration::from_secs(interval)).await;
            if let Err(e) = commands::repoview::gc_deleted_items() {
                error!("Background GC (database) failed: {}", e);
            }
            if let Err(e) = commands::backup::gc_conflict_files() {
                error!("Background GC (conflicts) failed: {}", e);
            }
        }
    });
}

fn cleanup_old_logs() {
    let logs_dir = Path::new("logs");
    if !logs_dir.exists() {
        info!("Log cleanup skipped: logs directory not found");
        return;
    }

    let cutoff = match SystemTime::now().checked_sub(StdDuration::from_secs(60 * 60 * 24 * 7)) {
        Some(t) => t,
        None => {
            error!("Log cleanup skipped: failed to compute cutoff time");
            return;
        }
    };

    let entries = match fs::read_dir(logs_dir) {
        Ok(entries) => entries,
        Err(e) => {
            error!("Log cleanup failed to read logs directory: {}", e);
            return;
        }
    };

    let mut scanned = 0u64;
    let mut removed = 0u64;

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                error!("Log cleanup failed to read log entry: {}", e);
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        scanned += 1;
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(e) => {
                error!("Log cleanup failed to read metadata: {}", e);
                continue;
            }
        };
        let modified = match metadata.modified() {
            Ok(modified) => modified,
            Err(e) => {
                error!("Log cleanup failed to read modified time: {}", e);
                continue;
            }
        };

        if modified < cutoff {
            match fs::remove_file(&path) {
                Ok(_) => {
                    removed += 1;
                    info!("Log cleanup removed file: {}", path.display());
                }
                Err(e) => {
                    error!("Log cleanup failed to remove file {}: {}", path.display(), e);
                }
            }
        }
    }

    info!("Log cleanup finished: scanned={}, removed={}", scanned, removed);
}

fn start_log_cleanup() {
    tokio::spawn(async {
        loop {
            cleanup_old_logs();
            sleep(Duration::from_secs(60 * 60 * 24 * 7)).await;
        }
    });
}

#[tokio::main]
async fn main() {
    let has_debug_flag = std::env::args().any(|arg| arg == "--debug");
    let enable_verbose_logs = cfg!(debug_assertions) || has_debug_flag;
    let default_filter = if enable_verbose_logs {
        "guaxi=debug,guaxi_lib=debug,html5ever=warn,selectors=warn,reqwest=info,warn"
    } else {
        "guaxi=info,guaxi_lib=info,warn"
    };
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter));

    // 创建文件 appender - 禁用 ANSI 颜色
    let file_appender = tracing_appender::rolling::daily("logs", "logfile.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    // 为文件输出创建单独的 layer - 禁用 ANSI 颜色
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false) // 关键：禁用文件中的 ANSI 颜色
        .with_target(true)
        .with_level(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339()) // 使用时间格式化器
        .compact(); // 使用紧凑格式适合文件

    // 为终端输出创建 layer - 启用 ANSI 颜色
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true) // 终端保持颜色
        .with_target(true)
        .with_level(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339()); // 使用时间格式化器

    // 使用 registry 组合多个 layer
    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(stdout_layer)
        .init();

    // 示例日志输出
    if enable_verbose_logs {
        info!("Project started with verbose logging (app debug + dependency filtering)");
    } else {
        info!("Project started with restricted logging (app info + dependency warnings)");
    }
    tokio::spawn(app_init());
    start_background_gc();
    start_log_cleanup();
    commands::flipboard::start_feed_scheduler_loop();
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
            commands::chatview::audio_text_to_speech,
            commands::chatview::audio_speech_to_text,
            commands::tts_book::tts_book_convert,
            commands::tts_book::tts_book_pause_task,
            commands::tts_book::tts_book_stop_task,
            commands::tts_book::tts_book_pause_all_tasks,
            commands::tts_book::tts_book_stop_all_tasks,
            commands::tts_book::tts_book_list_saved_tasks,
            commands::chatview::cancel_chat_stream,
            commands::chatview::ensure_local_deepseek_service,
            commands::chatview::search_web_duckduckgo,
            commands::chatview::kbmeta_agent_process,
            commands::docprocess::reader_open_file,
            commands::docprocess::convert_to_markdown,
            commands::docprocess::check_file_converted,
            commands::docprocess::get_converted_pdf_page,
            commands::docprocess::load_markdown_content,
            commands::docprocess::search_paginated_markdown,
            commands::docprocess::export_markdown_pdf_pandoc,
            commands::docprocess::export_markdown_pandoc,
            commands::docprocess::export_tex_pdf_tectonic,
            commands::markdown_reader_doc_process::markdown_reader_load_markdown_file,
            commands::word_reader_doc_process::read_word_file,
            commands::epub_reader_doc_process::read_epub_file,
            commands::text_reader_doc_process::read_text_file,
            commands::translator::translate_text,
            commands::translator_free::translate_text_free,
            commands::translator_free::selection_translate_free_multi,
            commands::docprocess::copy_image_file,
            commands::fileprocess::get_file_tree,
            commands::fileprocess::read_file_content,
            commands::fileprocess::get_file_meta,
            commands::fileprocess::read_file_bytes_base64,
            commands::fileprocess::write_file_bytes_base64,
            commands::fileprocess::write_file_content,
            commands::fileprocess::get_executable_dir,
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
            commands::settingview::get_resource_repo_sync_config,
            commands::settingview::set_resource_repo_sync_config,
            commands::settingview::get_template_repo_path,
            commands::settingview::set_template_repo_path,
            commands::settingview::get_slidev_repo_root,
            commands::settingview::set_slidev_repo_root,
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
            commands::resource_repo_window::create_resource_repo_manager_window,
            commands::resource_repo_window::create_slidev_manager_window,
            commands::cloudsync::resource_repo_test_connection,
            commands::cloudsync::resource_repo_sync_now,
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
            commands::settingview::get_lsp_github_token,
            commands::settingview::set_lsp_github_token,
            commands::settingview::get_lsp_server_statuses,
            commands::settingview::install_lsp_server,
            commands::settingview::list_lsp_extensions,
            commands::settingview::search_open_vsx_extensions,
            commands::settingview::install_open_vsx_extension,
            commands::settingview::import_lsp_extension_vsix,
            commands::settingview::import_lsp_extension_directory,
            commands::settingview::remove_lsp_extension,
            commands::settingview::get_webrtc_signal_url,
            commands::settingview::set_webrtc_signal_url,
            commands::settingview::get_llm_config,
            commands::settingview::set_llm_config,
            commands::settingview::get_tts_config,
            commands::settingview::set_tts_config,
            commands::settingview::get_asr_config,
            commands::settingview::set_asr_config,
            commands::settingview::get_terminal_config,
            commands::settingview::set_terminal_config,
            commands::settingview::get_chat_record_root,
            commands::settingview::set_chat_record_root,
            commands::settingview::get_markdown_models_status,
            commands::settingview::get_local_deepseek_status,
            commands::settingview::get_markdown_convertor_env_status,
            commands::settingview::install_markdown_models_archive,
            commands::settingview::download_local_deepseek_model,
            commands::settingview::install_local_deepseek_model_from_file,
            commands::settingview::install_markdown_convertor_env_archive,
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
            commands::knowledgebase::delete_knowledge_base,
            commands::knowledgebase::get_knowledge_base_list,
            commands::knowledgebase::get_knowledge_base_tree,
            commands::knowledgebase::get_knowledge_graph_data,
            commands::slidev::slidev_get_env_status,
            commands::slidev::slidev_start_dev,
            commands::slidev::slidev_stop_dev,
            commands::slidev::slidev_split_markdown_paragraphs,
            commands::slidev::slidev_export_pptx,
            commands::notebook::notebook_sync_from_markdown,
            commands::notebook::notebook_update_search_index,
            commands::notebook::notebook_search,
            commands::excerptsearch::search_excerpt_notes,
            commands::excerptsearch::read_excerpt_file,
            commands::git::get_git_root,
            commands::git::get_git_status,
            commands::git::get_current_branch,
            commands::git::get_git_head_markers,
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
            commands::git::discard_hunk,
            commands::git::get_commit_diff,
            commands::git::create_git_window,
            commands::markdown_manual::create_markdown_manual_window,
            commands::language_learning::create_language_learning_window,
            commands::language_learning::language_learning_record_translation,
            commands::language_learning::language_learning_list_records,
            commands::language_learning::language_learning_get_counts,
            commands::language_learning::language_learning_update_record,
            commands::language_learning::language_learning_delete_records,
            commands::language_learning::language_learning_export_bundle,
            commands::language_learning::language_learning_import_bundle,
            commands::language_learning::language_learning_cleanup_duplicate_topics,
            commands::language_learning::language_learning_export_scope_package,
            commands::language_learning::language_learning_import_scope_package,
            commands::language_learning::language_learning_enrich_word_basic,
            commands::language_learning::language_learning_enrich_word_details,
            commands::language_learning::language_learning_enrich_word,
            commands::language_learning::language_learning_list_audio_resources,
            commands::language_learning::language_learning_list_pending_sync_audio_ids,
            commands::language_learning::language_learning_upsert_audio_resource,
            commands::language_learning::language_learning_delete_audio_resource,
            commands::language_learning::language_learning_collect_audio_by_topic,
            commands::language_learning::language_learning_fetch_transcript_from_source,
            commands::language_learning::language_learning_generate_whisper_sync_json,
            commands::language_learning::language_learning_backfill_sync_translation,
            commands::language_learning::language_learning_generate_whisper_sync_json_async,
            commands::language_learning::language_learning_read_local_media_base64,
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
            commands::flipboard::get_feed_health,
            commands::flipboard::generate_feeds_for_platforms,
            commands::flipboard::ensure_category_default_feeds,
            commands::flipboard::add_feed,
            commands::flipboard::add_site,
            commands::flipboard::import_feeds_from_website_markdown,
            commands::flipboard::set_feed_enabled,
            commands::flipboard::update_feed,
            commands::flipboard::delete_feed,
            commands::flipboard::create_flipboard_feed_window,
            commands::flipboard::fetch_rss_feed,
            commands::flipboard::fetch_knowledge_from_feed_ids,
            commands::flipboard::get_cached_articles,
            commands::flipboard::refresh_articles_from_feeds,
            commands::flipboard::refresh_topic_knowledge_from_feeds,
            commands::flipboard::run_feed_scheduler_tick,
            commands::flipboard::get_feed_schedule_status,
            commands::flipboard::set_feed_scheduler_enabled,
            commands::flipboard::create_kb_note_from_topic,
            commands::flipboard::create_kb_note_from_article_ids,
            commands::flipboard::get_featured_articles,
            commands::flipboard::get_recommendations,
            commands::flipboard::get_popular_articles,
            commands::todo::todo_init,
            commands::todo::todo_get_snapshot,
            commands::todo::todo_upsert_list,
            commands::todo::todo_delete_list,
            commands::todo::todo_reorder_lists,
            commands::todo::todo_upsert_task,
            commands::todo::todo_delete_task,
            commands::todo::todo_set_task_completed,
            commands::todo::todo_toggle_task_important,
            commands::todo::todo_set_task_myday,
            commands::todo::todo_set_task_due_date,
            commands::todo::todo_set_task_repeat_rule,
            commands::todo::todo_set_task_notes,
            commands::todo::todo_upsert_step,
            commands::todo::todo_delete_step,
            commands::todo::todo_reorder_steps,
            commands::todo::todo_search_tasks,
            commands::todo::todo_poll_due_reminders,
            commands::filetransfer::filetransfer_start_receiver,
            commands::filetransfer::filetransfer_stop_receiver,
            commands::filetransfer::filetransfer_get_status,
            commands::filetransfer::filetransfer_start_lan_discovery,
            commands::filetransfer::filetransfer_stop_lan_discovery,
            commands::filetransfer::filetransfer_get_peers,
            commands::filetransfer::filetransfer_send_files,
            commands::filetransfer::filetransfer_list_tasks,
            commands::filetransfer::filetransfer_pause_task,
            commands::filetransfer::filetransfer_resume_task,
            commands::filetransfer::filetransfer_cancel_task,
            commands::filetransfer::filetransfer_start_http_share,
            commands::filetransfer::filetransfer_stop_http_share,
            commands::filetransfer::filetransfer_stat_file,
            commands::filetransfer::filetransfer_compute_upload_id,
            commands::filetransfer::filetransfer_read_file_chunk_base64,
            commands::filetransfer::filetransfer_webrtc_recv_init,
            commands::filetransfer::filetransfer_webrtc_recv_chunk_base64,
            commands::filetransfer::filetransfer_webrtc_recv_complete,
             commands::code_runtime::python_exec,
             commands::code_runtime::python_reset_repl,
             commands::code_runtime::python_runtime_get_session_info,
             commands::code_runtime::python_runtime_drop_session,
             commands::code_runtime::python_runtime_get_status,
             commands::code_runtime::python_runtime_initialize,
             commands::code_runtime::python_runtime_inspect_archive,
             commands::code_runtime::python_runtime_install_archive,
             commands::code_runtime::python_runtime_install_packages,
             commands::code_runtime::python_runtime_manage_packages,
             commands::code_runtime::python_runtime_import_table_file,
             commands::code_cell_runtime::code_cell_exec,
             commands::terminal::terminal_detect_profiles,
             commands::terminal::terminal_create_session,
             commands::terminal::terminal_write_session,
             commands::terminal::terminal_resize_session,
             commands::terminal::terminal_close_session,
             commands::terminal::terminal_probe_session_exit,
             commands::terminal::terminal_list_sessions,
            commands::frontend_logging::frontend_log,
            commands::frontend_logging::frontend_log_batch,
        ])
        .setup(move |app| {
            shared_state.set_app_handle(app.handle().clone());

            let state_for_server = shared_state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_backend_services(state_for_server).await {
                    error!("Failed to start backend services: {}", e);
                }
            });

            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::ShortcutState;

                let plugin = tauri_plugin_global_shortcut::Builder::new()
                    .with_shortcuts(["ctrl+alt+z", "ctrl+alt+a"])
                    .map(|builder| {
                        builder
                            .with_handler(|_app, shortcut, event| {
                                if event.state != ShortcutState::Pressed {
                                    return;
                                }

                                let shortcut_text = shortcut.to_string().to_lowercase();
                                info!("Shortcut pressed: {}", shortcut_text);

                                if shortcut_text == "ctrl+alt+z" {
                                    tauri::async_runtime::spawn(async move {
                                        match commands::guaxiscreencap::trigger_screenshot_tool_once().await {
                                            Ok(_) => info!("截图成功"),
                                            Err(e) => error!("截图失败: {}", e),
                                        }
                                    });
                                    return;
                                }

                                if shortcut_text == "ctrl+alt+a" {
                                    if let Some(window) = _app.get_webview_window("main") {
                                        if let Err(e) = window.show() {
                                            error!("显示主窗口失败: {}", e);
                                        }
                                        if let Err(e) = window.unminimize() {
                                            error!("取消主窗口最小化失败: {}", e);
                                        }
                                        if let Err(e) = window.set_focus() {
                                            error!("主窗口聚焦失败: {}", e);
                                        }
                                    }
                                    if let Err(e) = _app.emit("assistant_global_shortcut", "wake_assistant")
                                    {
                                        error!("发送助手唤醒事件失败: {}", e);
                                    }
                                }
                            })
                            .build()
                    });

                match plugin {
                    Ok(plugin) => {
                        if let Err(e) = app.handle().plugin(plugin) {
                            error!("Failed to register global shortcut plugin: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to configure global shortcut plugin: {}", e);
                    }
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
