//! 暴露给前端的转换任务命令。

use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::AppHandle;
use tracing::info;
use uuid::Uuid;

use super::engine;
use super::queue::ConversionQueue;
use super::task_store::{self, ConversionTask, NewConversionTask};

/// 入队请求。engine 取值：auto | fast | marker | mineru（默认 auto）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueRequest {
    pub input_path: String,
    pub output_dir: String,
    #[serde(default)]
    pub imported_file_id: Option<i64>,
    #[serde(default = "default_engine")]
    pub engine: String,
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub max_attempts: Option<i64>,
}

fn default_engine() -> String {
    "auto".to_string()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueResponse {
    pub task_id: i64,
    pub reused: bool,
    pub task: ConversionTask,
}

fn file_name_of(input_path: &str) -> String {
    Path::new(input_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("未知文件")
        .to_string()
}

/// 入队一个转换任务；若同一输入已有活动任务则复用，避免重复转换。
#[tauri::command]
pub async fn enqueue_conversion(request: EnqueueRequest) -> Result<EnqueueResponse, String> {
    info!(
        input = request.input_path.as_str(),
        engine = request.engine.as_str(),
        "enqueue_conversion"
    );

    // 复用活动任务
    if let Some(existing) = task_store::find_active_by_input(&request.input_path)? {
        return Ok(EnqueueResponse {
            task_id: existing.id,
            reused: true,
            task: existing,
        });
    }

    let requested_engine =
        if engine::Engine::from_str(&request.engine).is_some() || request.engine == "auto" {
            request.engine.clone()
        } else {
            "auto".to_string()
        };

    let stem = Path::new(&request.input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let expected_md = Path::new(&request.output_dir)
        .join(stem)
        .join(format!("{}.md", stem))
        .to_string_lossy()
        .to_string();

    let new_task = NewConversionTask {
        uuid: Uuid::new_v4().to_string(),
        imported_file_id: request.imported_file_id,
        file_name: file_name_of(&request.input_path),
        input_path: request.input_path.clone(),
        output_dir: request.output_dir.clone(),
        expected_md_path: Some(expected_md),
        requested_engine,
        max_attempts: request.max_attempts.unwrap_or(3).clamp(1, 10),
        priority: request.priority.unwrap_or(0),
    };

    let id = task_store::insert_task(&new_task)?;
    let task = task_store::get_task(id)?
        .ok_or_else(|| "任务插入后读取失败".to_string())?;

    if let Some(q) = ConversionQueue::get() {
        q.wake();
    }

    Ok(EnqueueResponse {
        task_id: id,
        reused: false,
        task,
    })
}

/// 重新转换一个已导入文件（用于中断/失败后重启、或缺少 Markdown 的历史条目）。
///
/// 稳定性关键：源文件用的是导入时**拷贝进 Libraries 的副本**（不依赖用户原始外部路径，
/// 原文件被移动/删除也不影响），路径与 add_repo_import_file 的落盘规则一致：
///   输入: {librariesRoot}/Libraries/{library}/{stem}/{ext}/{file_name}
///   输出: {librariesRoot}/Libraries/{library}/{stem}/markdown
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconvertRequest {
    pub library: String,
    /// 原始文件名（含扩展名），对应 imported_files.original_filename。
    pub file_name: String,
    #[serde(default)]
    pub imported_file_id: Option<i64>,
    #[serde(default = "default_engine")]
    pub engine: String,
}

#[tauri::command]
pub async fn reconvert_imported_file(request: ReconvertRequest) -> Result<EnqueueResponse, String> {
    use std::path::PathBuf;

    let libraries_root = crate::commands::settingview::get_libraries_root();
    let file_name = &request.file_name;

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "无法解析文件名".to_string())?
        .to_string();
    // 扩展名映射与导入时一致：doc/docx -> word。
    let ext = Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| match e {
            "doc" | "docx" => "word",
            other => other,
        })
        .unwrap_or("");

    let file_dir = PathBuf::from(&libraries_root)
        .join("Libraries")
        .join(&request.library)
        .join(&stem);
    let source_path = if ext.is_empty() {
        file_dir.join(file_name)
    } else {
        file_dir.join(ext).join(file_name)
    };
    let output_dir = file_dir.join("markdown");

    if !source_path.exists() {
        return Err(format!(
            "找不到已导入的源文件，无法重新转换: {}",
            source_path.to_string_lossy()
        ));
    }
    let _ = std::fs::create_dir_all(&output_dir);

    info!(
        library = request.library.as_str(),
        file_name = file_name.as_str(),
        source = source_path.to_string_lossy().as_ref(),
        engine = request.engine.as_str(),
        "reconvert_imported_file"
    );

    enqueue_conversion(EnqueueRequest {
        input_path: source_path.to_string_lossy().to_string(),
        output_dir: output_dir.to_string_lossy().to_string(),
        imported_file_id: request.imported_file_id,
        engine: request.engine,
        priority: Some(1),
        max_attempts: None,
    })
    .await
}

/// 列出任务（可选状态过滤）。
#[tauri::command]
pub async fn list_conversion_tasks(
    status: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<ConversionTask>, String> {
    task_store::list_tasks(status.as_deref(), limit.unwrap_or(200))
}

#[tauri::command]
pub async fn get_conversion_task(task_id: i64) -> Result<Option<ConversionTask>, String> {
    task_store::get_task(task_id)
}

/// 重试失败/取消的任务。
#[tauri::command]
pub async fn retry_conversion_task(task_id: i64) -> Result<bool, String> {
    let changed = task_store::reset_for_retry(task_id)?;
    if changed {
        if let Some(q) = ConversionQueue::get() {
            q.wake();
        }
    }
    Ok(changed)
}

/// 取消任务（运行中会终止子进程）。
#[tauri::command]
pub async fn cancel_conversion_task(task_id: i64) -> Result<bool, String> {
    task_store::mark_canceled(task_id)
}

/// 删除单个任务条目（用于清除单条失败/已结束记录）。
#[tauri::command]
pub async fn delete_conversion_task(task_id: i64) -> Result<bool, String> {
    task_store::delete_task(task_id)
}

/// 清除所有已结束（成功/失败/取消）的任务条目，返回删除数量。
#[tauri::command]
pub async fn clear_finished_conversion_tasks() -> Result<usize, String> {
    task_store::clear_finished()
}

/// 手动触发一次中断任务恢复（正常由启动流程自动完成）。
#[tauri::command]
pub async fn resume_pending_conversions() -> Result<usize, String> {
    let n = task_store::requeue_interrupted()?;
    if let Some(q) = ConversionQueue::get() {
        q.wake();
    }
    Ok(n)
}

/// 查询各转换引擎装配/可用状态（前端据此显示“引擎就绪/装配中/不可用”）。
#[tauri::command]
pub async fn conversion_engine_status() -> Result<super::provision::ProvisionStatus, String> {
    Ok(super::provision::current_status())
}

/// 手动触发快速引擎装配（幂等）。用于前端“重试装配”按钮或首次导入前主动准备。
#[tauri::command]
pub async fn ensure_fast_engine_ready() -> Result<super::provision::ProvisionStatus, String> {
    tokio::task::spawn_blocking(super::provision::ensure_fast_engine_blocking)
        .await
        .map_err(|e| format!("装配任务失败: {e}"))??;
    Ok(super::provision::current_status())
}

/// 向后兼容包装：旧前端仍可调用 convert_to_markdown 风格的阻塞式转换。
/// 这里改为入队 + 轮询等待完成，保持“await 到转换结束”的调用语义。
#[tauri::command]
pub async fn convert_to_markdown_queued(
    input_path: String,
    output_dir: String,
    engine: Option<String>,
    _app: AppHandle,
) -> Result<(), String> {
    let resp = enqueue_conversion(EnqueueRequest {
        input_path,
        output_dir,
        imported_file_id: None,
        engine: engine.unwrap_or_else(default_engine),
        priority: Some(1),
        max_attempts: None,
    })
    .await?;

    let task_id = resp.task_id;
    // 轮询任务终态
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        match task_store::get_task(task_id)? {
            Some(t) => match t.status.as_str() {
                task_store::STATUS_SUCCEEDED => return Ok(()),
                task_store::STATUS_FAILED => {
                    return Err(t.error.unwrap_or_else(|| "转换失败".to_string()))
                }
                task_store::STATUS_CANCELED => return Err("转换已取消".to_string()),
                _ => continue,
            },
            None => return Err("任务丢失".to_string()),
        }
    }
}
