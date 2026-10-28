// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use once_cell::sync::OnceCell;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration as StdDuration, SystemTime};
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;
#[cfg(desktop)]
use tauri::{Emitter, Manager};

static LOG_GUARD: OnceCell<tracing_appender::non_blocking::WorkerGuard> = OnceCell::new();

mod cognition;
mod commands;
mod guaxiservices;
mod state;

use commands::file_watcher::FileWatcher;

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

static BACKGROUND_SHUTDOWN_DONE: AtomicBool = AtomicBool::new(false);

/// 关闭软件时统一回收所有由本程序拉起的后台子进程。
///
/// 设计说明：
/// 1. 这些进程都是“按需常驻”的：智能助手（agent server / python）、本地大模型
///    （llama.cpp 的 deepseek / qwen / embedding 三个 server）、探索者模式的 python
///    引擎、Marker 文档转换服务、LSP server、内置终端、slidev dev、无头 Chrome。
///    它们不会随主窗口关闭而退出，必须在这里显式收掉，否则会残留在任务管理器里。
/// 2. 顺序上先收“有句柄的”，最后用托管 PID 表兜底那些拿不到句柄的（LSP、转换任务、Chrome）。
/// 3. 每一步都不返回错误、不 panic：退出阶段任何单点失败都不应该卡住关闭流程。
/// 4. 用一次性标记保护，避免 Exit 事件重复触发时重复执行。
fn shutdown_background_processes(reason: &str) {
    if BACKGROUND_SHUTDOWN_DONE.swap(true, Ordering::SeqCst) {
        return;
    }

    info!(reason, "Shutting down background child processes");

    commands::agent_server::stop_agent_server();
    commands::rsshub_server::stop();
    commands::codex_agent::stop_codex_agent();
    commands::codex_app_server::stop();
    commands::chatview::shutdown_local_llm_servers();
    commands::embedding::shutdown_embedding_server();
    commands::translate_local::shutdown_local_translate_server();
    commands::code_runtime::shutdown_all_python_workers();
    commands::docprocess::shutdown_marker_server();
    commands::slidev::shutdown_dev_server();
    commands::terminal::shutdown_all_sessions();
    commands::ssh_terminal::shutdown_all_sessions();
    commands::gdb_mi::shutdown_all_sessions();
    commands::remote_terminal_tools::shutdown_all_collectors();
    commands::serial_telnet::shutdown_all_serial();
    commands::serial_telnet::shutdown_all_telnet();
    commands::net_conn::shutdown_all_servers();
    commands::net_capture::shutdown_all_captures();
    // 认知层：WAL 下每次写入即提交，不需要刷盘；这里只把用户没看的自动采集内容删掉，
    // 别让它们跨越一次重启继续占着收件箱。同步执行但不做 VACUUM，所以不会拖住退出。
    cognition::shutdown();

    let leftovers = guaxiservices::process_shutdown::shutdown_registered(reason);
    info!(
        reason,
        leftovers, "Background child processes shutdown finished"
    );
}

fn main() {
    // MCP stdio 模式：codex 把主程序自己当作 MCP 服务器拉起来时走这里。
    //
    // 必须在 `#[tokio::main]` **之外**分叉：桥接用的是 `reqwest::blocking`，
    // 它内部会建一个阻塞运行时；若此时已有 tokio 运行时就绪（`#[tokio::main]`
    // 已经建好），退出时会 panic「Cannot drop a runtime in a context where
    // blocking is not allowed」，进程直接死，codex 那侧只看到握手失败。
    // 这条路径在安装包里是**唯一**可用的（不额外分发 guaxi_mcp.exe），
    // 所以它必须是普通同步入口。
    if std::env::args().any(|arg| arg == "--mcp-server") {
        cognition::mcp_bridge::run_stdio_bridge();
        return;
    }
    app_main()
}

#[tokio::main]
async fn app_main() {
    let has_debug_flag = std::env::args().any(|arg| arg == "--debug");
    let enable_verbose_logs = cfg!(debug_assertions) || has_debug_flag;
    // 必须在任何窗口创建之前声明 DPI 感知：否则 Windows 会把坐标虚拟化成
    // 96 DPI 视角，而 GDI 截图拿到的是真实像素，高缩放下模型算出的点击位置会错位。
    commands::computer::init_dpi_awareness();
    // iroh 的中继重连与网络探测在国内环境会大量打 WARN（relay 断连、net_report 等），
    // 但属于正常瞬态行为，统一降到 error 级别避免刷屏；真正致命错误仍可见。可用 RUST_LOG 覆盖。
    let iroh_noise_filter =
        "iroh::net_report=error,iroh::socket::transports::relay=error,iroh_relay=warn,iroh::magicsock=warn,chromiumoxide::handler=error,chromiumoxide::conn=error,chromiumoxide::browser=error";
    let default_filter = if enable_verbose_logs {
        format!("guaxi=debug,guaxi_lib=debug,html5ever=warn,selectors=warn,reqwest=info,{iroh_noise_filter},warn")
    } else {
        format!("guaxi=info,guaxi_lib=info,{iroh_noise_filter},warn")
    };
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&default_filter));

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
    // 认知层：感知打分、审议、简报、日常维护四条循环。受 cognition_enabled 总开关控制。
    cognition::start_background_loops();
    let file_watcher = FileWatcher::new();

    let shared_state = SharedState::new();

    tauri::Builder::default()
        // .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(file_watcher)
        .manage(shared_state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::file_watcher::start_file_watching,
            commands::file_watcher::stop_file_watching,
            commands::remote_terminal::log_import_file,
            commands::remote_terminal::log_list_frames,
            commands::remote_terminal::log_read_lines,
            commands::remote_terminal::log_close_session,
            commands::remote_terminal::log_frame_metrics,
            commands::remote_terminal::log_metric_series,
            commands::remote_terminal::log_leak_ranking,
            commands::remote_terminal::log_snapshot_diff,
            commands::remote_terminal::log_save_profile,
            commands::remote_terminal::log_load_profile,
            commands::remote_terminal::log_builtin_profiles,
            commands::remote_terminal::log_reparse,
            commands::remote_terminal::log_save_parse_profile,
            commands::remote_terminal::log_load_parse_profile,
            commands::remote_terminal::log_search,
            commands::remote_terminal::log_cluster_templates,
            commands::remote_terminal::log_detect_format,
            commands::remote_terminal::log_extract_regex,
            commands::remote_terminal::log_aggregate,
            commands::ssh_terminal::ssh_list_profiles,
            commands::ssh_terminal::ssh_save_profile,
            commands::ssh_terminal::ssh_delete_profile,
            commands::ssh_terminal::ssh_set_secret,
            commands::ssh_terminal::ssh_delete_secret,
            commands::ssh_terminal::ssh_has_secret,
            commands::ssh_terminal::ssh_create_session,
            commands::ssh_terminal::ssh_write_session,
            commands::ssh_terminal::ssh_resize_session,
            commands::ssh_terminal::ssh_close_session,
            commands::ssh_terminal::ssh_list_recordings,
            commands::gdb_mi::gdb_mi_start,
            commands::gdb_mi::gdb_mi_submit,
            commands::gdb_mi::gdb_mi_await,
            commands::gdb_mi::gdb_mi_state,
            commands::gdb_mi::gdb_mi_command,
            commands::gdb_mi::gdb_mi_raw,
            commands::gdb_mi::gdb_mi_close,
            commands::gdb_mi::gdb_save_macros,
            commands::gdb_mi::gdb_load_macros,
            commands::remote_terminal_tools::ssh_exec_capture,
            commands::remote_terminal_tools::ssh_read_text_file,
            commands::remote_terminal_tools::gdb_coredump_analyze,
            commands::remote_terminal_tools::ssh_start_periodic,
            commands::remote_terminal_tools::ssh_stop_periodic,
            commands::remote_terminal_tools::binary_analyze,
            commands::remote_terminal_tools::crypto_inspect,
            commands::remote_tool::remote_tool_exec,
            commands::remote_tool::remote_tool_upload,
            commands::remote_tool::remote_tool_download,
            commands::remote_tool::remote_mount_status,
            commands::remote_tool::remote_mount,
            commands::remote_tool::remote_mounts,
            commands::remote_tool::remote_unmount,
            commands::crypto_remote::crypto_remote_gen_key,
            commands::crypto_remote::crypto_remote_create_ca,
            commands::crypto_remote::crypto_remote_gen_csr,
            commands::crypto_remote::crypto_remote_issue_cert,
            commands::crypto_remote::crypto_remote_issue_sm2_dual,
            commands::crypto_remote::crypto_remote_verify_chain,
            commands::crypto_remote::crypto_remote_sign,
            commands::crypto_remote::crypto_remote_verify_sign,
            commands::crypto_remote::crypto_remote_algorithms,
            commands::net_toolbox::net_parse_packet,
            commands::net_toolbox::net_build_packet,
            commands::net_conn::net_tcp_send,
            commands::net_conn::net_udp_send,
            commands::net_conn::net_dns_query,
            commands::net_conn::net_tcp_listen,
            commands::net_conn::net_udp_listen,
            commands::net_conn::net_server_stop,
            commands::net_conn::net_tls_inspect,
            commands::net_conn::net_http_request,
            commands::net_conn::net_tls_send,
            commands::net_conn::net_http_listen,
            commands::net_capture::net_pcap_status,
            commands::net_capture::net_list_interfaces,
            commands::net_capture::net_capture_start,
            commands::net_capture::net_capture_stop,
            commands::net_capture::net_send_raw,
            commands::sftp_fs::sftp_connect,
            commands::sftp_fs::sftp_list,
            commands::sftp_fs::sftp_canonicalize,
            commands::sftp_fs::sftp_download,
            commands::sftp_fs::sftp_upload,
            commands::sftp_fs::sftp_mkdir,
            commands::sftp_fs::sftp_remove,
            commands::sftp_fs::sftp_rename,
            commands::sftp_fs::sftp_close,
            commands::crypto_lab::crypto_load_bytes,
            commands::crypto_lab::crypto_gen_key,
            commands::crypto_lab::crypto_gen_csr,
            commands::crypto_lab::crypto_issue_cert,
            commands::crypto_lab::crypto_sign_csr,
            commands::crypto_lab::crypto_verify_cert,
            commands::crypto_lab::crypto_pfx_parse,
            commands::crypto_lab::crypto_pfx_build,
            commands::crypto_lab::crypto_sym,
            commands::crypto_lab::crypto_rsa_crypt,
            commands::crypto_lab::crypto_sign,
            commands::crypto_lab::crypto_verify,
            commands::crypto_lab::crypto_hash,
            commands::crypto_lab::crypto_ca_list,
            commands::crypto_lab::crypto_ca_create,
            commands::crypto_lab::crypto_ca_import,
            commands::crypto_lab::crypto_ca_import_pem,
            commands::crypto_lab::crypto_leaf_import_pem,
            commands::crypto_lab::crypto_ca_delete,
            commands::crypto_lab::crypto_ca_export,
            commands::crypto_lab::crypto_issue_from_ca,
            commands::crypto_lab::crypto_leaf_list,
            commands::crypto_lab::crypto_leaf_delete,
            commands::crypto_lab::crypto_leaf_export,
            commands::crypto_lab::crypto_ca_revoke,
            commands::crypto_lab::crypto_ca_gen_crl,
            commands::crypto_lab::crypto_asset_list,
            commands::crypto_lab::crypto_asset_save,
            commands::crypto_lab::crypto_asset_get,
            commands::crypto_lab::crypto_asset_delete,
            commands::crypto_lab::crypto_pem_details,
            commands::cmd_dispatch::cmd_dispatch_plan,
            commands::cmd_dispatch::cmd_dispatch_parse,
            commands::cmd_dispatch::cmd_dispatch_to_yaml,
            commands::cmd_dispatch::cmd_dispatch_run,
            commands::cmd_dispatch::cmd_dispatch_cancel,
            commands::cmd_dispatch::cmd_strategy_list,
            commands::cmd_dispatch::cmd_strategy_save,
            commands::cmd_dispatch::cmd_strategy_load,
            commands::cmd_dispatch::cmd_strategy_delete,
            commands::cmd_dispatch::cmd_dispatch_stage_pem,
            commands::cmd_dispatch::cmd_dispatch_stage_config,
            commands::cmd_dispatch::cmd_open_in_os,
            commands::cmd_dispatch::cmd_reveal_in_os,
            commands::cmd_catalog::cmd_catalog_list,
            commands::cmd_catalog::cmd_scenario_list,
            commands::cmd_catalog::cmd_scenario_render,
            commands::source_locate::cmd_source_locate,
            commands::source_locate::cmd_source_read,
            commands::source_locate::ssh_list_dirs,
            commands::ssh_terminal::ssh_set_enable_secret,
            commands::ssh_terminal::ssh_delete_enable_secret,
            commands::ssh_terminal::ssh_has_enable_secret,
            commands::serial_telnet::serial_list_ports,
            commands::serial_telnet::serial_open,
            commands::serial_telnet::serial_write,
            commands::serial_telnet::serial_close,
            commands::serial_telnet::telnet_open,
            commands::serial_telnet::telnet_write,
            commands::serial_telnet::telnet_close,
            commands::serial_telnet::console_list_sessions,
            commands::ocr::ocr_recognize,
            commands::ocr::screenshot_ocr,
            commands::chatview::get_chat_answer,
            commands::chatview::get_chat_answer_stream,
            commands::chatview::agent_chat_stream,
            commands::chatview::agent_tool_decision,
            commands::codex_agent::codex_agent_status,
            // Agent 工作台：codex app-server 引擎
            commands::codex_app_server::agent_engine_status,
            commands::codex_app_server::agent_engine_ready,
            commands::codex_app_server::agent_list_models,
            commands::codex_app_server::agent_start_thread,
            commands::codex_app_server::agent_resume_thread,
            commands::codex_app_server::agent_read_thread,
            commands::codex_app_server::agent_list_threads,
            commands::codex_app_server::agent_start_turn,
            commands::codex_app_server::agent_steer_turn,
            commands::codex_app_server::agent_interrupt_turn,
            commands::codex_app_server::agent_resolve_approval,
            commands::codex_app_server::agent_rename_thread,
            commands::codex_app_server::agent_archive_thread,
            commands::codex_app_server::agent_unarchive_thread,
            commands::codex_app_server::agent_delete_thread,
            commands::codex_app_server::agent_fork_thread,
            commands::codex_app_server::agent_compact_thread,
            commands::codex_app_server::agent_review_start,
            commands::codex_app_server::agent_section_list,
            commands::codex_app_server::agent_section_create,
            commands::codex_app_server::agent_section_update,
            commands::codex_app_server::agent_section_delete,
            commands::codex_app_server::agent_thread_move_section,
            commands::codex_app_server::agent_list_permission_profiles,
            commands::codex_app_server::agent_list_mcp_servers,
            commands::codex_app_server::agent_reload_mcp_servers,
            commands::codex_app_server::agent_list_skills,
            commands::codex_app_server::agent_fuzzy_file_search,
            commands::codex_app_server::agent_read_config,
            commands::codex_app_server::agent_git_status,
            commands::codex_app_server::agent_commit_changes,
            commands::codex_app_server::agent_restart_engine,
            commands::cc_switch::detect_cc_switch_config,
            commands::chatview::audio_text_to_speech,
            commands::chatview::audio_speech_to_text,
            commands::tts_book::tts_book_convert,
            commands::tts_book::tts_book_pause_task,
            commands::tts_book::tts_book_stop_task,
            commands::tts_book::tts_book_pause_all_tasks,
            commands::tts_book::tts_book_stop_all_tasks,
            commands::tts_book::tts_book_list_saved_tasks,
            commands::tts_book::tts_book_list_records,
            commands::tts_book::tts_book_rename_record,
            commands::tts_book::tts_book_delete_record,
            commands::tts_book::tts_book_scan_records,
            commands::readaloud::readaloud_prepare,
            commands::readaloud::readaloud_prefetch,
            commands::readaloud::readaloud_stop,
            commands::readaloud::readaloud_drop,
            commands::chatview::cancel_chat_stream,
            commands::chatview::ensure_local_deepseek_service,
            commands::chatview::ensure_local_qwen_service,
            commands::chatview::search_web_duckduckgo,
            commands::chatview::kbmeta_agent_process,
            commands::chatview::doc_agent_process,
            commands::chatview::story_studio_compile_markdown,
            commands::chatview::python_codegen_from_prompt,
            commands::editor_plugin::list_editor_plugins,
            commands::editor_plugin::get_editor_plugins_dir,
            commands::editor_plugin::list_all_plugins,
            commands::editor_plugin::set_plugin_enabled,
            commands::editor_plugin::delete_plugin,
            commands::editor_plugin::create_plugin,
            commands::editor_plugin::read_plugin_entry,
            commands::editor_plugin::write_plugin_entry,
            commands::editor_plugin::export_plugin_to_zip,
            commands::editor_plugin::import_plugin_from_zip,
            commands::story_studio::story_studio_read_markdown,
            commands::story_studio::story_studio_get_blender_status,
            commands::story_studio::story_studio_create_project,
            commands::story_studio::story_studio_open_project,
            commands::story_studio::story_studio_open_managed_directory,
            commands::story_studio::story_studio_get_media_tools_status,
            commands::story_studio::story_studio_start_preview,
            commands::story_studio::story_studio_cancel_job,
            commands::story_studio::story_studio_probe_blender_bridge,
            commands::story_studio::story_studio_send_blender_commands,
            commands::docprocess::reader_open_file,
            commands::docprocess::convert_to_markdown,
            commands::docprocess::check_file_converted,
            commands::convert::commands::enqueue_conversion,
            commands::convert::commands::reconvert_imported_file,
            commands::convert::commands::list_conversion_tasks,
            commands::convert::commands::get_conversion_task,
            commands::convert::commands::retry_conversion_task,
            commands::convert::commands::cancel_conversion_task,
            commands::convert::commands::delete_conversion_task,
            commands::convert::commands::clear_finished_conversion_tasks,
            commands::convert::commands::resume_pending_conversions,
            commands::convert::commands::conversion_engine_status,
            commands::convert::commands::ensure_fast_engine_ready,
            commands::convert::commands::ensure_marker_engine_ready,
            commands::convert::commands::ensure_marker_models_ready,
            commands::import_suggest::suggest_import_metadata,
            commands::convert::commands::convert_to_markdown_queued,
            commands::docprocess::get_converted_pdf_page,
            commands::docprocess::load_markdown_content,
            commands::docprocess::search_paginated_markdown,
            commands::docprocess::export_rendered_html_pdf,
            commands::docprocess::export_markdown_pdf_pandoc,
            commands::docprocess::export_markdown_pandoc,
            commands::docprocess::convert_file_to_markdown_pandoc,
            commands::docprocess::export_tex_pdf_tectonic,
            commands::docprocess::build_tex_pdf_preview,
            commands::docprocess::clear_preview_build,
            commands::preview_render_cache::preview_render_cache_get,
            commands::preview_render_cache::preview_render_cache_put,
            commands::preview_render_cache::preview_render_cache_stats,
            commands::preview_render_cache::preview_render_cache_clear,
            guaxiservices::markdown_browser::remember_browser_preview_path,
            commands::service_control::service_list,
            commands::service_control::service_start,
            commands::service_control::service_stop,
            commands::service_control::service_restart,
            commands::service_control::service_set_autostart,
            commands::typst_compile::build_typ_pdf_preview,
            commands::typst_compile::export_typ_pdf,
            commands::typst_compile::typst_forward_search,
            commands::typst_compile::typst_inverse_search,
            commands::pdf_reader::pdf_open,
            commands::pdf_reader::pdf_session_open,
            commands::pdf_reader::pdf_session_close,
            commands::pdf_reader::pdf_render_page,
            commands::pdf_reader::pdf_page_text,
            commands::pdf_reader::pdf_page_chars,
            commands::pdf_reader::pdf_outline,
            commands::pdf_reader::pdf_page_links,
            commands::pdf_reader::pdf_locate_point,
            commands::pdf_reader::pdf_page_render_hash,
            commands::pdf_reader::pdf_search,
            commands::pdf_reader::pdf_add_highlight,
            commands::pdf_reader::pdf_delete_highlight,
            commands::pdf_reader::pdf_page_highlights,
            commands::pdf_reader::pdf_list_highlights,
            commands::pdf_reader::pdf_save,
            // 文档阅读器与「打开文件」分流（纯新增命令，不影响既有打开/转换链路）
            commands::doc_reader::sniff_file_kind,
            commands::doc_reader::read_document_for_reader,
            commands::doc_reader::read_spreadsheet,
            commands::doc_reader::convert_document_to_markdown,
            commands::doc_reader_window::create_doc_reader_window,
            commands::doc_reader_window::close_doc_reader_window,
            commands::craft_app::open_in_craft_app,
            commands::craft_app::probe_craft_apps,
            commands::craft_cli::craft_cli_info,
            commands::craft_cli::craft_cli_export_pdf,
            commands::craft_cli::craft_cli_text,
            commands::craft_cli::probe_craft_clis,
            commands::reader_state::reader_get_state,
            commands::reader_state::reader_save_state,
            commands::reader_state::reader_list_bookmarks,
            commands::reader_state::reader_add_bookmark,
            commands::reader_state::reader_delete_bookmark,
            commands::reader_state::reader_list_annotations,
            commands::reader_state::reader_add_annotation,
            commands::reader_state::reader_update_annotation,
            commands::reader_state::reader_delete_annotation,
            commands::reader_state::reader_get_prefs,
            commands::reader_state::reader_save_prefs,
            commands::synctex::synctex_forward,
            commands::synctex::synctex_inverse,
            commands::taskmanager::task_get_config,
            commands::taskmanager::task_set_config,
            commands::taskmanager::task_get_git_config,
            commands::taskmanager::task_set_git_config,
            commands::taskmanager::task_ensure_dirs,
            commands::taskmanager::task_scan_and_index,
            commands::taskmanager::task_query,
            commands::taskmanager::task_query_fresh,
            commands::taskmanager::task_update_item,
            commands::taskmanager::task_import,
            commands::taskmanager::task_export_report,
            commands::taskmanager::task_read_file,
            commands::taskmanager::task_write_file,
            commands::taskmanager::task_write_report_file,
            commands::taskmanager::task_validate_content,
            commands::taskmanager::task_start_watching,
            commands::task_manager_window::create_task_manager_window,
            commands::markdown_reader_doc_process::markdown_reader_load_markdown_file,
            commands::word_reader_doc_process::read_word_file,
            commands::epub_reader_doc_process::read_epub_file,
            commands::text_reader_doc_process::read_text_file,
            commands::translator::translate_text,
            commands::translator_free::translate_text_free,
            commands::translator_free::selection_translate_free_multi,
            commands::translator_free::translate_pool_status,
            commands::translate_local::translate_local_status,
            commands::translate_local::translate_local_prepare,
            commands::translate_local::translate_local_set_model,
            commands::translate_local::translate_local_set_enabled,
            commands::translate_local::translate_local_stop,
            commands::translate_local::translate_local_texts,
            commands::translate_local::translate_local_download_model,
            commands::translate_local::translate_local_import_model,
            commands::translate_cache::translate_cache_stats,
            commands::translate_cache::translate_cache_clear,
            commands::docprocess::copy_image_file,
            commands::fileprocess::get_file_tree,
            commands::fileprocess::fuzzy_search_files,
            commands::fileprocess::read_file_content,
            commands::fileprocess::resolve_resource_path,
            commands::fileprocess::get_file_meta,
            commands::fileprocess::read_file_bytes_base64,
            commands::fileprocess::write_file_bytes_base64,
            commands::fileprocess::write_file_content,
            commands::neovim::neovim_open_path,
            commands::fileprocess::get_executable_dir,
            commands::fileprocess::create_directory,
            commands::fileprocess::create_file,
            commands::fileprocess::delete_path,
            commands::fileprocess::rename_path,
            commands::fileprocess::move_path,
            commands::fileprocess::copy_path,
            commands::fileprocess::create_file_with_content,
            commands::fileprocess::rename_directory,
            commands::resource_repo::resource_repo_scan,
            commands::resource_repo::resource_repo_thumbnail,
            commands::resource_repo::resource_repo_usage_index,
            commands::life_album::life_album_import_dir,
            commands::life_album::life_album_import_files,
            commands::life_album::life_album_scan_dir,
            commands::life_album::life_album_enrich_files,
            commands::life_album::life_album_reverse_geocode,
            commands::life_album::life_album_reverse_geocode_batch,
            commands::life_album::life_album_clear_geocode_cache,
            commands::life_album::life_album_get_internal_root_dir,
            commands::life_album::life_album_save_video_poster,
            commands::life_album_viewer_window::open_life_album_viewer_window,
            commands::life_album_viewer_window::life_album_viewer_take_request,
            commands::settingview::get_life_album_album_dir,
            commands::settingview::set_life_album_album_dir,
            commands::settingview::get_life_album_amap_key_cmd,
            commands::settingview::set_life_album_amap_key,
            commands::settingview::get_active_repo_path,
            commands::settingview::set_active_repo_path,
            commands::settingview::get_server_config,
            commands::settingview::set_server_config,
            commands::settingview::get_database_config,
            commands::settingview::set_database_config,
            commands::settingview::get_appearance_config,
            commands::settingview::set_appearance_config,
            commands::settingview::get_film_studio_config,
            commands::settingview::set_film_studio_config,
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
            commands::templateview::create_template_manager_window,
            commands::resource_repo_window::create_resource_repo_manager_window,
            commands::resource_repo_window::create_slidev_manager_window,
            commands::cloudsync::resource_repo_test_connection,
            commands::cloudsync::resource_repo_sync_now,
            commands::p2psync::p2p_sync_status,
            commands::p2psync::p2p_sync_test_connection,
            commands::p2psync::p2p_sync_create_ticket,
            commands::p2psync::p2p_sync_join_ticket,
            commands::p2psync::p2p_sync_list_peers,
            commands::p2psync::p2p_federation_set_enabled,
            commands::p2psync::p2p_federation_status,
            commands::p2psync::p2p_sync_probe_peers,
            commands::p2psync::p2p_sync_remove_peer,
            commands::p2psync::p2p_sync_resource_repo_now,
            commands::p2psync::p2p_sync_verify_resource_repo,
            commands::p2psync::p2p_sync_knowledge_base_now,
            commands::p2psync::p2p_sync_verify_knowledge_base,
            commands::p2psync::p2p_sync_libraries_root_now,
            commands::p2psync::p2p_sync_verify_libraries_root,
            commands::p2psync::p2p_sync_template_now,
            commands::p2psync::p2p_sync_verify_template,
            commands::p2psync::p2p_sync_slidev_now,
            commands::p2psync::p2p_sync_verify_slidev,
            commands::p2psync::p2p_sync_custom_now,
            commands::p2psync::p2p_sync_verify_custom,
            commands::p2psync::p2p_sync_cancel,
            commands::templateview::open_application_window,

            // 邮箱客户端（应用页「邮件」卡片）
            commands::mail::mail_list_accounts,
            // 通讯录（联系人 / 通讯组 / vCard 导入导出）
            commands::mail::contacts::mail_list_contacts,
            commands::mail::contacts::mail_get_contact,
            commands::mail::contacts::mail_address_cards,
            commands::mail::contacts::mail_save_address_contact,
            commands::mail::contacts::mail_contact_by_email,
            commands::mail::contacts::mail_save_contact,
            commands::mail::contacts::mail_delete_contact,
            commands::mail::contacts::mail_contact_stats,
            commands::mail::contacts::mail_contact_suggest,
            commands::mail::contacts::mail_contact_messages,
            commands::mail::contacts::mail_list_contact_groups,
            commands::mail::contacts::mail_save_contact_group,
            commands::mail::contacts::mail_delete_contact_group,
            commands::mail::contacts::mail_expand_contact_group,
            commands::mail::contacts::mail_import_contacts,
            commands::mail::contacts::mail_export_contacts,
            commands::mail::contacts::mail_contacts_rebuild_from_mail,
            commands::mail::mail_save_account,
            commands::mail::mail_delete_account,
            commands::mail::mail_has_secret,
            commands::mail::mail_delete_secret,
            commands::mail::mail_test_account,
            commands::mail::mail_list_folders,
            commands::mail::mail_refresh_folders,
            commands::mail::mail_unread_counts,
            commands::mail::mail_create_folder,
            commands::mail::mail_delete_folder,
            commands::mail::mail_set_folder_pinned,
            commands::mail::mail_junk_messages,
            commands::mail::mail_unmark_junk,
            commands::mail::mail_batch_flags,
            commands::mail::mail_batch_delete,
            commands::mail::mail_storage_info,
            commands::mail::mail_migrate_storage,
            commands::mail::mail_backup,
            commands::mail::mail_restore,
            commands::mail::mail_open_origin,
            commands::mail::mail_export_message,
            commands::mail::mail_save_to_notes,
            commands::mail::mail_to_task,
            commands::mail::mail_stash_draft,
            commands::mail::mail_list_stashed_drafts,
            commands::mail::mail_delete_stashed_draft,
            commands::mail::mail_save_as_eml,
            commands::mail::mail_save_as_html,
            commands::mail::mail_get_encryption_enabled,
            commands::mail::mail_set_encryption_enabled,
            commands::mail::mail_sync_folder,
            commands::mail::mail_sync_all,
            commands::mail::mail_list_messages,
            commands::mail::mail_search,
            commands::mail::mail_suggest,
            commands::mail::mail_labels,
            commands::mail::mail_thread,
            commands::mail::mail_get_message,
            commands::mail::mail_set_seen,
            commands::mail::mail_set_flagged,
            commands::mail::mail_set_labels,
            commands::mail::mail_move_messages,
            commands::mail::mail_delete_messages,
            commands::mail::mail_save_attachment,
            commands::mail::mail_open_attachment,
            commands::mail::mail_send,
            commands::mail::mail_queue_send,
            commands::mail::mail_list_outbox,
            commands::mail::mail_cancel_outbox,
            commands::mail::mail_retry_outbox,
            commands::mail::mail_flush_outbox,
            commands::mail::mail_save_draft,
            commands::mail::mail_discard_draft,
            commands::office_app::probe_office_suites,
            commands::office_app::open_in_office,
            commands::office_app::open_with_system_default,
            commands::mail::mail_list_rules,
            commands::mail::mail_save_rule,
            commands::mail::mail_delete_rule,
            commands::mail::mail_get_settings,
            commands::mail::mail_set_settings,
            commands::mail::mail_theme_tokens,
            commands::mail::mail_builtin_themes,
            commands::mail::mail_oauth_start,
            commands::mail::mail_oauth_poll,
            commands::mail::mail_oauth_complete,
            commands::mail::mail_oauth_sign_out,
            commands::mail::mail_oauth_status,
            commands::mail::mail_offline_list,
            commands::mail::mail_offline_count,
            commands::mail::mail_offline_replay,
            commands::mail::mail_connection_status,
            commands::mail::mail_start_idle,
            commands::mail::mail_stop_idle,
            commands::mail::mail_list_delegate_folders,
            commands::mail::mail_list_templates,
            commands::mail::mail_save_template,
            commands::mail::mail_delete_template,
            commands::mail::mail_list_sender_rules,
            commands::mail::mail_save_sender_rule,
            commands::mail::mail_delete_sender_rule,
            commands::mail::mail_check_attachments,
            commands::mail::mail_is_dangerous_attachment,
            commands::mail::mail_sieve_preview,
            commands::mail::mail_sieve_sync,
            commands::mail::mail_smime_status,
            commands::mail::mail_smime_import,
            commands::mail::mail_smime_save_peer,
            commands::mail::mail_smime_delete_peer,
            commands::mail::mail_smime_peer_status,
            commands::mail::mail_smime_delete,
            commands::mail::mail_pgp_keys,
            commands::mail::mail_pgp_import_key,
            commands::mail::mail_pgp_import_file,
            commands::mail::mail_test_notification,
            commands::mail::mail_export_diagnostics,
            commands::mail::mail_reset_connections,
            commands::mail::mail_clear_cache,
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
            commands::settingview::get_code_theme,
            commands::settingview::set_code_theme,
            commands::settingview::get_markdown_format_options,
            commands::settingview::set_markdown_format_options,
            commands::settingview::get_editor_display_config,
            commands::settingview::set_editor_display_config,
            commands::settingview::list_system_fonts,
            commands::settingview::get_python_codegen_assist_enabled,
            commands::settingview::set_python_codegen_assist_enabled,
            commands::settingview::get_lsp_github_token,
            commands::settingview::set_lsp_github_token,
            commands::settingview::list_lsp_extensions,
            commands::settingview::search_open_vsx_extensions,
            commands::settingview::search_vscode_marketplace_extensions,
            commands::settingview::install_open_vsx_extension,
            commands::settingview::install_vscode_marketplace_extension,
            commands::settingview::install_catalog_lsp_extension,
            commands::settingview::import_lsp_extension_vsix,
            commands::settingview::import_lsp_extension_directory,
            commands::settingview::remove_lsp_extension,
            commands::settingview::get_lsp_extension_task,
            commands::settingview::list_lsp_extension_tasks,
            commands::settingview::start_install_open_vsx_extension_task,
            commands::settingview::start_install_vscode_marketplace_extension_task,
            commands::settingview::start_install_catalog_lsp_extension_task,
            commands::settingview::start_import_lsp_extension_vsix_task,
            commands::settingview::start_import_lsp_extension_directory_task,
            commands::settingview::start_remove_lsp_extension_task,
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
            commands::settingview::get_marker_ocr_mode,
            commands::settingview::set_marker_ocr_mode,
            commands::settingview::get_marker_reading_order,
            commands::settingview::set_marker_reading_order,
            commands::settingview::get_markdown_models_status,
            commands::settingview::get_local_deepseek_status,
            commands::settingview::get_local_qwen_status,
            commands::settingview::install_markdown_models_archive,
            commands::settingview::download_local_deepseek_model,
            commands::settingview::install_local_deepseek_model_from_file,
            commands::settingview::download_local_qwen_model,
            commands::settingview::install_local_qwen_model_from_file,
            commands::chatview::save_chat_session,
            commands::chatview::export_chat_session,
            commands::chatview::open_web_model_window,
            commands::chatview::prepare_web_model_window,
            commands::chatview::web_model_send_message,
            commands::repoview::get_imported_files,
            commands::repoview::find_duplicate_imported_files,
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
            commands::guaxireader::open_repo_reader,
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
            commands::file_index::file_index_ensure,
            commands::file_index::file_index_search,
            commands::file_index::file_index_status,
            commands::file_index::file_index_release,
            commands::fulltext::fulltext_ensure,
            commands::fulltext::fulltext_search,
            commands::fulltext::fulltext_status,
            commands::fulltext::fulltext_release,
            commands::diffcompare::diff_files,
            commands::diffcompare::diff_texts,
            commands::diffcompare::diff_buffer_vs_file,
            commands::diffcompare::diff_dirs,
            commands::diffcompare::diff_dirs_page,
            commands::diffcompare::diff_dirs_children,
            commands::diffcompare::diff_dirs_cancel,
            commands::diffcompare::diff_dirs_close,
            commands::diffcompare::diff_dirs_summary,
            commands::diffcompare::diff_dirs_entry_paths,
            commands::diffcompare::diff_read_text,
            commands::diffcompare::diff_write_text,
            commands::diff_compare_window::open_diff_compare_window,
            commands::diff_compare_window::diff_compare_take_request,
            commands::diff_compare_window::diff_probe_paths,
            commands::mail_compose_window::mail_open_compose_window,
            commands::mail_compose_window::mail_take_compose_request,
            commands::mail_compose_window::mail_drop_compose_request,
            commands::mail_compose_window::mail_compose_window_ready,
            commands::mail_compose_window::mail_compose_closed,
            commands::mail_compose_window::mail_close_compose_window,
            commands::mail_compose_window::mail_compose_windows,
            commands::knowledgebase::create_knowledge_base,
            commands::knowledgebase::delete_knowledge_base,
            commands::knowledgebase::get_knowledge_base_list,
            commands::knowledgebase::get_knowledge_base_tree,
            commands::knowledgebase::get_knowledge_graph_data,
            commands::kgraph::kg_index_status,
            commands::kgraph::kg_build_index,
            commands::kgraph::kg_query_graph,
            commands::kgraph::kg_node_neighbors,
            commands::kgraph::kg_shortest_path,
            commands::kgraph::kg_stats,
            commands::kgraph::kg_set_embedding_model,
            commands::embedding::kg_ensure_embedding_service,
            commands::embedding::kg_embedding_status,
            commands::embedding::kg_download_embedding_model,
            commands::kgraph::kg_embed_kb,
            commands::kgraph::kg_semantic_search,
            commands::kgraph::kg_suggest_relations,
            commands::kgraph::kg_suggest_tags,
            commands::kgraph::kg_confirm_relation,
            commands::kgraph::kg_add_tag,
            commands::kgraph::kg_remove_tag,
            commands::kgraph::kg_remove_relation,
            commands::kgraph::kg_export_manual,
            commands::kgraph::kg_import_manual,
            commands::kgraph::kg_write_manual_snapshot,
            commands::kgraph::kg_set_manual_sync,
            commands::kgraph::kg_get_manual_sync,
            commands::kgraph::kg_kb_summaries,
            commands::kgraph::kg_all_tags,
            commands::kgraph::kg_files_by_tags,
            commands::kg_llm::kg_extract_concepts,
            commands::kgraph::kg_detect_communities,
            commands::kgraph::kg_communities,
            commands::kgraph::kg_save_layout,
            commands::kgraph::kg_load_layout,
            commands::kgraph::kg_compute_layout,
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
            commands::git::auto_commit_cellmeta_changes,
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
            commands::git::stage_hunk,
            commands::git::unstage_hunk,
            commands::git::get_commit_diff,
            commands::git::create_git_window,
            commands::git::get_git_sync_state,
            commands::git::git_fetch,
            commands::git::merge_branch,
            commands::git::abort_merge,
            commands::git::abort_git_operation,
            commands::git::rename_branch,
            commands::git::delete_branch,
            commands::git::undo_last_commit,
            commands::git::git_cherry_pick,
            commands::git::git_revert_commit,
            commands::git::git_checkout_commit,
            commands::git::git_reset_to_commit,
            commands::git::git_create_tag,
            commands::git::git_stash_push,
            commands::git::git_stash_list,
            commands::git::git_stash_apply,
            commands::git::git_stash_drop,
            commands::git::get_merge_conflict,
            commands::git::resolve_conflict,
            commands::markdown_manual::create_markdown_manual_window,
            commands::markdown_manual::create_syntax_manual_window,
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
            commands::git::get_git_file_diff,
            commands::git::get_file_diff,
            commands::git::get_file_diff_hunks,
            commands::git::get_commit_file_diff,
            commands::git::get_commit_files,
            commands::serverview::init_backend_services,
            commands::markdowncheck::markdown_format,
            commands::markdowncheck::markdown_batch_format,
            commands::markdowncheck::get_default_format_options,
            commands::markdowncheck::validate_markdown,
            commands::markdowncheck::markdown_clean,
            commands::markdowncheck::markdown_stats,
            commands::syntax_highlight::syntax_highlight_code,
            commands::syntax_highlight::syntax_highlight_tokens,
            commands::syntax_highlight::syntax_highlight_tokens_batch,
            commands::syntax_highlight::syntax_highlight_batch,
            commands::syntax_highlight::syntax_supported_languages,
            commands::syntax_highlight::syntax_analyze_document,
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
            commands::flipboard::start_site_crawl,
            commands::flipboard::get_crawl_progress,
            commands::flipboard::reset_crawl_queue,
            commands::flipboard::ensure_chromium_ready,
            commands::flipboard::open_login_session,
            commands::rsshub_server::rsshub_refresh_cookies,
            commands::rsshub_server::rsshub_base_url,
            commands::rsshub_server::rsshub_radar_detect,
            commands::library_site::library_site_local_url,
            commands::library_site::open_library_site_window,
            commands::library_site::library_site_start_share,
            commands::library_site::library_site_stop_share,
            commands::library_site::library_site_share_status,
            commands::library_site::library_site_qr_svg,
            commands::library_site::library_site_file_share_link,
            commands::flipboard::open_article_in_browser,
            commands::flipboard::purge_unbookmarked_articles,
            commands::flipboard::delete_feeds,
            commands::flipboard::set_feeds_enabled,
            commands::flipboard::stop_feed_task,
            commands::flipboard::stop_all_feed_tasks,
            commands::flipboard::get_pending_logins,
            commands::flipboard::dismiss_pending_login,
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
            commands::music::local::music_import_files,
            commands::music::local::music_import_dir,
            commands::music::local::music_local_list,
            commands::music::local::music_local_remove,
            commands::music::local::music_local_clear,
            commands::music::netease::music_netease_search,
            commands::music::netease::music_netease_song_url,
            commands::music::netease::music_login_netease,
            commands::music::netease::music_netease_account,
            commands::music::netease::music_netease_my_playlists,
            commands::music::netease::music_netease_recommend_playlists,
            commands::music::netease::music_netease_playlist_tracks,
            commands::music::qq::music_login_qq,
            commands::music::qq::music_qq_account,
            commands::music::qq::music_qq_my_playlists,
            commands::music::qq::music_qq_playlist_tracks,
            commands::music::kugou::music_login_kugou,
            commands::music::kugou::music_kugou_account,
            commands::music::kugou::music_kugou_my_playlists,
            commands::music::kugou::music_kugou_playlist_tracks,
            commands::music::resolve::music_resolve_unm,
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
             commands::code_runtime::python_runtime_prewarm_session,
             commands::code_runtime::python_runtime_cancel_request,
             commands::code_runtime::python_reset_repl,
             commands::code_runtime::python_runtime_get_session_info,
             commands::code_runtime::python_runtime_drop_session,
             commands::code_runtime::python_runtime_get_status,
             commands::code_runtime::python_runtime_list_downloadable_versions,
             commands::code_runtime::python_runtime_initialize,
             commands::code_runtime::python_runtime_inspect_archive,
             commands::code_runtime::python_runtime_install_archive,
             commands::code_runtime::python_runtime_install_packages,
              commands::code_runtime::python_runtime_manage_packages,
             commands::code_runtime::python_runtime_upgrade_uv,
             commands::code_runtime::python_runtime_import_table_file,
             commands::code_runtime::python_runtime_get_table_rows,
             commands::code_runtime::python_runtime_export_table_result,
             commands::notebook_lab::notebook_data_lab_rpc,
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
            // ── 认知层（感知 / 记忆 / 召回 / 信念 / 主动 / 简报 / 工具面 / 进化闩）
            cognition::commands::cognition_status,
            cognition::commands::cognition_set_enabled,
            cognition::commands::cognition_set_setting,
            cognition::commands::cognition_percept_list,
            cognition::commands::cognition_percept_ingest,
            cognition::commands::cognition_percept_set_state,
            cognition::commands::cognition_percept_get,
            cognition::commands::cognition_score_percepts,
            cognition::commands::cognition_recall,
            cognition::commands::cognition_nudge_list,
            cognition::commands::cognition_nudge_decide,
            cognition::commands::cognition_deliberate_now,
            cognition::commands::cognition_review_feedback,
            cognition::commands::cognition_policy_train,
            cognition::commands::cognition_policy_reset,
            cognition::commands::cognition_brief_get,
            cognition::commands::cognition_brief_generate,
            cognition::commands::cognition_brief_list,
            cognition::commands::cognition_brief_mark_opened,
            cognition::commands::cognition_profile_get,
            cognition::commands::cognition_profile_save,
            cognition::commands::cognition_interest_upsert,
            cognition::commands::cognition_interest_remove,
            cognition::commands::cognition_belief_list,
            cognition::commands::cognition_belief_extract,
            cognition::commands::cognition_belief_set_status,
            cognition::commands::cognition_belief_remove,
            cognition::commands::cognition_belief_evidence,
            cognition::commands::cognition_detect_contradictions,
            cognition::commands::cognition_episode_recent,
            cognition::commands::cognition_episode_steps,
            cognition::commands::cognition_episode_citations,
            cognition::commands::cognition_citation_clicked,
            cognition::commands::cognition_tools_list,
            cognition::commands::cognition_tool_set_tier,
            cognition::commands::cognition_tool_call,
            cognition::commands::cognition_tool_audit,
            cognition::commands::cognition_mcp_info,
            cognition::commands::cognition_eval_mine,
            cognition::commands::cognition_eval_run,
            cognition::commands::cognition_eval_tune,
            cognition::commands::cognition_eval_runs,
            cognition::commands::cognition_recall_weights_rollback,
            cognition::commands::cognition_maintenance,
            cognition::commands::cognition_chat_finish,
            cognition::commands::cognition_sources,
            cognition::commands::cognition_set_source_enabled,
            cognition::commands::cognition_capture,
            cognition::commands::cognition_nudge_get,
            cognition::commands::cognition_channel_stats,
            cognition::commands::cognition_purge_percepts,
            commands::resource_hunter::rh_search,
            commands::resource_hunter::rh_enqueue,
            commands::resource_hunter::rh_enqueue_batch,
            commands::resource_hunter::rh_list_tasks,
            commands::resource_hunter::rh_pause,
            commands::resource_hunter::rh_resume,
            commands::resource_hunter::rh_cancel,
            commands::resource_hunter::rh_list_resources,
            commands::resource_hunter::rh_get_proxy,
            commands::resource_hunter::rh_set_proxy,
            commands::resource_hunter::rh_open_path,
            commands::resource_hunter::rh_list_engines,
            commands::resource_hunter::rh_list_sources,
            commands::resource_hunter::rh_get_sources_off,
            commands::resource_hunter::rh_set_sources_off,
            commands::resource_hunter::rh_delete_resource,
            commands::resource_hunter::rh_update_tags,
            commands::resource_hunter::rh_all_tags,
            commands::resource_hunter::rh_reveal,
            commands::resource_hunter::rh_library_search,
            commands::resource_hunter::rh_get_download_dir,
            commands::resource_hunter::rh_set_download_dir,
            commands::resource_hunter::rh_get_speed_limit,
            commands::resource_hunter::rh_set_speed_limit,
            commands::resource_hunter::rh_get_api_keys,
            commands::resource_hunter::rh_set_api_keys,
            commands::resource_hunter::rh_get_searxng,
            commands::resource_hunter::rh_set_searxng,
            commands::resource_hunter::rh_fetch_thumb,
            commands::resource_hunter::rh_export_library,
            commands::resource_hunter::rh_import_library,
        ])
        .setup(move |app| {
            shared_state.set_app_handle(app.handle().clone());
            // 知识图谱索引变更广播需要在非命令上下文(文件 CRUD 钩子)里 emit,这里注入全局句柄。
            commands::kgraph::init_app_handle(app.handle().clone());
            // 认知层的后台循环同样要在非命令上下文里广播提醒与简报。
            cognition::init_app_handle(app.handle().clone());

            // 启动持久化文档转换队列（含断点续跑）
            commands::convert::ConversionQueue::init(app.handle().clone());
            // 后台自动装配快速转换引擎（下载托管 Python + 依赖），无需用户手动构建。
            commands::convert::provision::spawn_startup_provision();
            // 后台预下载本地翻译小模型（免费接口全挂时的兜底），只下模型不起进程。
            commands::translate_local::spawn_startup_model_provision();

            // 本地后端服务：按登记表启动（带退避重试，并尊重用户在「设置 → 服务器」里的停用偏好）。
            // 以前是一次性 spawn、失败只打一行日志 —— 而 8888 挂掉会让 Markdown 的本地图片
            // 全部渲染不出来（图片路径改写与 /markdown/api/images 都在这条服务上）。
            // 端口暂时被占（TIME_WAIT / 上一实例收尾）时重试几次即可恢复；重试仍失败则放弃，
            // 之后可在设置页手动启动。详见 service_control::autostart_services 的说明。
            commands::service_control::autostart_services(shared_state.clone());

            tauri::async_runtime::spawn(async {
                commands::p2psync::p2p_autostart_if_enabled().await;
            });

            // 启动 Agent Server（Python 进程）。尊重「设置 → 服务器」里的停用偏好 ——
            // 停用是持久化的，用户关掉之后重启应用不该自己又起来。
            if !commands::service_control::autostart_enabled("agent-server") {
                info!("Agent server 已被用户停用，跳过启动");
            } else {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = commands::agent_server::ensure_agent_server_started(&app_handle).await {
                        warn!("Agent server 启动失败（不影响主功能）: {}", e);
                    }
                });
            }

            // 启动 RSSHub sidecar（Node，复用 SlidevEnv 的 node.exe）。仅在已构建进包、且未被
            // 用户停用时启动；未打包则 is_available() 为 false，直接跳过——聚合功能自动降级。
            if !commands::service_control::autostart_enabled("rsshub") {
                info!("RSSHub 已被用户停用，跳过启动");
            } else {
                tauri::async_runtime::spawn(async {
                    if commands::rsshub_server::is_available() {
                        if let Err(e) = commands::rsshub_server::ensure_started().await {
                            warn!("RSSHub 启动失败（聚合功能降级，不影响主功能）: {}", e);
                        }
                    } else {
                        info!("RSSHub 未打包，跳过启动（聚合功能不启用 RSSHub 源）");
                    }
                });
            }

            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

                // 设计说明（两个坑，缺一个都会表现为“快捷键按了没反应”）：
                // 1. 这里以前用 `shortcut.to_string().to_lowercase()` 去和 "ctrl+alt+z" 比较。
                //    global-hotkey 的 Display 实际输出是 "control+alt+KeyZ"（修饰键写作 control，
                //    主键是 Code 的名字），字符串永远对不上，两个全局快捷键的处理分支都是死代码。
                //    改用 Shortcut::matches 按 Modifiers + Code 比较，不再依赖文本格式。
                // 2. 全局快捷键是 OS 级注册，按下后事件不会再送进 webview。此前把“唤醒助手”注册在
                //    ctrl+alt+x 上，正好和 Header 里“截图并插入”的 Ctrl+Alt+X 撞车，于是前端那个
                //    handler 永远收不到按键。助手在应用内本来就有 Ctrl+Alt+A，这里把全局唤醒也
                //    统一到 ctrl+alt+a（同一动作、同一按键），把 ctrl+alt+x 让回给前端截图。
                const GLOBAL_MODS: Modifiers = Modifiers::CONTROL.union(Modifiers::ALT);

                let plugin = tauri_plugin_global_shortcut::Builder::new()
                    .with_shortcuts(["ctrl+alt+z", "ctrl+alt+a"])
                    .map(|builder| {
                        builder
                            .with_handler(|_app, shortcut, event| {
                                if event.state != ShortcutState::Pressed {
                                    return;
                                }

                                info!("Shortcut pressed: {}", shortcut.into_string());

                                if shortcut.matches(GLOBAL_MODS, Code::KeyZ) {
                                    tauri::async_runtime::spawn(async move {
                                        match commands::guaxiscreencap::trigger_screenshot_tool_once().await {
                                            Ok(_) => info!("截图成功"),
                                            Err(e) => error!("截图失败: {}", e),
                                        }
                                    });
                                    return;
                                }

                                if shortcut.matches(GLOBAL_MODS, Code::KeyA) {
                                    // 用户当时在不在本应用里。App 在后台 / 最小化时按下这个键，
                                    // 用户要的是「把助手唤出来」，这时前端必须只展开、不能再 toggle 一次收起；
                                    // 只有窗口本来就在眼前（可见 + 最小化否 + 有焦点），才把它当成开/关切换。
                                    let mut was_revealed = false;
                                    if let Some(window) = _app.get_webview_window("main") {
                                        let visible = window.is_visible().unwrap_or(false);
                                        let minimized = window.is_minimized().unwrap_or(false);
                                        let focused = window.is_focused().unwrap_or(false);
                                        was_revealed = !visible || minimized || !focused;

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
                                    let payload = if was_revealed {
                                        "reveal_assistant"
                                    } else {
                                        "toggle_assistant"
                                    };
                                    if let Err(e) = _app.emit("assistant_global_shortcut", payload)
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        // 设计说明（关键 · 对应“关闭软件后 llama.cpp / python 等后台进程残留”）：
        // 以前收尾代码写在 `.run(...)` 之后，但 tao 的事件循环永不返回（内部直接
        // `process::exit`），那段代码从未执行过，所有后台子进程都被留在了系统里。
        // `RunEvent::Exit` 是进程真正退出前最后一个能跑用户代码的时机，收尾必须放这里。
        .run(|_app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                shutdown_background_processes("run-event-exit");
            }
        });
}
