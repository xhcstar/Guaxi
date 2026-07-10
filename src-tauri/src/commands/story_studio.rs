//! Guaxi AI 影像工坊的本地工程与 Blender 执行桥。
//!
//! 设计原则：
//! 1. Markdown 可以来自任意目录，知识库只作为文件选择器的默认起点。
//! 2. 大模型只生成经过校验的 Story IR，绝不把模型返回值当作 Python 执行。
//! 3. Blender 进程仅运行仓库内受控的 headless_runner.py，并通过固定参数读取 Story IR。
//! 4. 每个任务都有 job_id、结构化事件和取消标记，便于独立窗口展示进度与日志。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};
use uuid::Uuid;

const TARGET_BLENDER_VERSION: &str = "5.2 LTS";
const TARGET_BLENDER_MAJOR: u32 = 5;
const TARGET_BLENDER_MINOR: u32 = 2;
const MAX_MARKDOWN_BYTES: u64 = 16 * 1024 * 1024;
const JOB_EVENT_NAME: &str = "story-studio-job";

/// 只在内存中保存轻量取消标记；真正的 Child 由专属等待线程持有，避免跨线程锁死 stdout。
static CANCEL_FLAGS: LazyLock<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownSource {
    pub path: String,
    pub file_name: String,
    pub content: String,
    pub byte_length: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlenderStatus {
    pub installed: bool,
    pub compatible: bool,
    pub target_version: String,
    pub executable: String,
    pub detected_version: String,
    pub is_prerelease: bool,
    pub plugin_directory: String,
    pub runner_path: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub project_name: String,
    pub source_path: String,
    pub source_content: String,
    pub story_ir: Value,
    #[serde(default)]
    pub output_root: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryProject {
    pub project_id: String,
    pub project_name: String,
    pub project_dir: String,
    pub source_copy_path: String,
    pub story_ir_path: String,
    pub blend_path: String,
    pub preview_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedStoryProject {
    pub project: StoryProject,
    pub source_path: String,
    pub source_content: String,
    pub story_ir: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaToolsStatus {
    pub ffmpeg_available: bool,
    pub ffmpeg_path: String,
    pub ffmpeg_version: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPreviewRequest {
    pub project_dir: String,
    #[serde(default)]
    pub blender_path: String,
    #[serde(default)]
    pub ffmpeg_path: String,
    #[serde(default = "default_render_animation")]
    pub render_animation: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeCommandRequest {
    #[serde(default = "default_bridge_port")]
    pub port: u16,
    pub token: String,
    pub payload: Value,
}

fn default_bridge_port() -> u16 {
    19_152
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StoryJobEvent {
    job_id: String,
    status: String,
    stage: String,
    progress: u8,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview_path: Option<String>,
}

fn default_render_animation() -> bool {
    true
}

/// Windows 后台任务不弹出额外控制台窗口；其他平台保持默认行为。
#[cfg(target_os = "windows")]
fn apply_hidden_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn apply_hidden_window(_command: &mut Command) {}

fn normalized_markdown_extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn validate_markdown_path(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("Markdown 文件不存在：{}", path.display()));
    }
    let extension = normalized_markdown_extension(path);
    if extension != "md" && extension != "markdown" && extension != "mdown" {
        return Err("仅支持 .md、.markdown 或 .mdown 文稿".to_string());
    }
    let size = fs::metadata(path)
        .map_err(|e| format!("读取文稿元数据失败：{e}"))?
        .len();
    if size > MAX_MARKDOWN_BYTES {
        return Err(format!(
            "文稿超过 {} MiB 限制，请先拆分为章节或场景",
            MAX_MARKDOWN_BYTES / 1024 / 1024
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn story_studio_read_markdown(path: String) -> Result<MarkdownSource, String> {
    let path_buf = PathBuf::from(path.trim());
    validate_markdown_path(&path_buf)?;

    let mut file = fs::File::open(&path_buf).map_err(|e| format!("打开文稿失败：{e}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("读取文稿失败：{e}"))?;
    let byte_length = bytes.len();
    let mut content = String::from_utf8(bytes).map_err(|_| "文稿不是有效的 UTF-8 编码".to_string())?;
    // 兼容 Windows 编辑器可能写入的 UTF-8 BOM，避免其进入第一个 Markdown 标题。
    if content.starts_with('\u{feff}') {
        content.remove(0);
    }

    info!(
        path = %path_buf.display(),
        byte_length,
        "[StoryStudio] markdown source loaded"
    );
    Ok(MarkdownSource {
        path: path_buf.to_string_lossy().to_string(),
        file_name: path_buf
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("未命名文稿.md")
            .to_string(),
        content,
        byte_length,
    })
}

/// 开发环境直接使用仓库根目录插件；打包环境允许通过环境变量或可执行文件同级目录覆盖。
fn resolve_plugin_directory() -> PathBuf {
    if let Ok(path) = env::var("GUAXI_BLENDER_ADDON_DIR") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return candidate;
        }
    }

    if let Ok(executable) = env::current_exe() {
        if let Some(parent) = executable.parent() {
            let candidate = parent.join("guaxi_blender_addon");
            if candidate.exists() {
                return candidate;
            }
        }
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .join("guaxi_blender_addon")
}

fn parse_blender_version(output: &str) -> Option<(u32, u32, u32)> {
    let first_line = output.lines().find(|line| line.contains("Blender"))?;
    let raw_version = first_line
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))?;
    let mut parts = raw_version
        .trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.')
        .split('.')
        .filter_map(|part| part.parse::<u32>().ok());
    Some((parts.next()?, parts.next()?, parts.next().unwrap_or(0)))
}

fn run_blender_version(executable: &Path) -> Result<(String, bool, bool), String> {
    let mut command = Command::new(executable);
    command.arg("--version");
    apply_hidden_window(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("无法执行 Blender：{e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let combined = if stdout.is_empty() { stderr } else { stdout };
    let version = parse_blender_version(&combined)
        .ok_or_else(|| format!("无法解析 Blender 版本：{combined}"))?;
    let compatible = version.0 == TARGET_BLENDER_MAJOR && version.1 == TARGET_BLENDER_MINOR;
    let lowered = combined.to_ascii_lowercase();
    let prerelease = lowered.contains("alpha")
        || lowered.contains("beta")
        || lowered.contains("release candidate")
        || lowered.contains("rc");
    Ok((combined.lines().next().unwrap_or_default().to_string(), compatible, prerelease))
}

fn blender_candidates(explicit_path: Option<&str>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = explicit_path.filter(|value| !value.trim().is_empty()) {
        candidates.push(PathBuf::from(path.trim()));
    }
    if let Ok(path) = env::var("GUAXI_BLENDER_PATH") {
        if !path.trim().is_empty() {
            candidates.push(PathBuf::from(path.trim()));
        }
    }
    #[cfg(target_os = "windows")]
    {
        candidates.push(PathBuf::from(
            r"C:\Program Files\Blender Foundation\Blender 5.2\blender.exe",
        ));
        candidates.push(PathBuf::from(
            r"C:\Program Files\Blender Foundation\Blender\blender.exe",
        ));
    }
    #[cfg(target_os = "macos")]
    candidates.push(PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender"));
    #[cfg(not(target_os = "windows"))]
    candidates.push(PathBuf::from("blender"));
    #[cfg(target_os = "windows")]
    candidates.push(PathBuf::from("blender.exe"));
    candidates
}

fn resolve_blender(explicit_path: Option<&str>) -> Result<(PathBuf, String, bool, bool), String> {
    let mut last_error = String::new();
    for candidate in blender_candidates(explicit_path) {
        // 纯程序名由 PATH 解析；包含目录的路径则提前过滤不存在项。
        if candidate.components().count() > 1 && !candidate.is_file() {
            continue;
        }
        match run_blender_version(&candidate) {
            Ok((version, compatible, prerelease)) => {
                return Ok((candidate, version, compatible, prerelease));
            }
            Err(err) => last_error = err,
        }
    }
    if last_error.is_empty() {
        Err("未检测到 Blender，请安装 Blender 5.2 LTS 或手动选择 blender 可执行文件".to_string())
    } else {
        Err(last_error)
    }
}

#[tauri::command]
pub fn story_studio_get_blender_status(blender_path: Option<String>) -> BlenderStatus {
    let plugin_directory = resolve_plugin_directory();
    let runner_path = plugin_directory
        .join("guaxi_blender_bridge")
        .join("headless_runner.py");
    let mut warnings = Vec::new();
    if !runner_path.is_file() {
        warnings.push(format!("未找到受控执行器：{}", runner_path.display()));
    }

    match resolve_blender(blender_path.as_deref()) {
        Ok((executable, detected_version, compatible, is_prerelease)) => {
            if !compatible {
                warnings.push(format!(
                    "当前为 {detected_version}，本工作流锁定 {TARGET_BLENDER_VERSION}"
                ));
            }
            if is_prerelease {
                warnings.push("检测到 Blender 5.2 预发布版本，仅建议用于开发验证".to_string());
            }
            info!(
                executable = %executable.display(),
                version = %detected_version,
                compatible,
                prerelease = is_prerelease,
                "[StoryStudio] blender status detected"
            );
            BlenderStatus {
                installed: true,
                compatible: compatible && runner_path.is_file(),
                target_version: TARGET_BLENDER_VERSION.to_string(),
                executable: executable.to_string_lossy().to_string(),
                detected_version,
                is_prerelease,
                plugin_directory: plugin_directory.to_string_lossy().to_string(),
                runner_path: runner_path.to_string_lossy().to_string(),
                warnings,
            }
        }
        Err(err) => {
            warnings.push(err);
            BlenderStatus {
                installed: false,
                compatible: false,
                target_version: TARGET_BLENDER_VERSION.to_string(),
                executable: String::new(),
                detected_version: String::new(),
                is_prerelease: false,
                plugin_directory: plugin_directory.to_string_lossy().to_string(),
                runner_path: runner_path.to_string_lossy().to_string(),
                warnings,
            }
        }
    }
}

fn sanitize_project_name(raw: &str) -> String {
    let mut result = String::new();
    for ch in raw.trim().chars() {
        if !matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            && !ch.is_control()
        {
            result.push(ch);
        }
        if result.chars().count() >= 60 {
            break;
        }
    }
    let trimmed = result.trim().trim_end_matches('.').to_string();
    if trimmed.is_empty() {
        "Guaxi影像项目".to_string()
    } else {
        trimmed
    }
}

fn create_project_directories(project_dir: &Path) -> Result<(), String> {
    for relative in [
        "source",
        "story",
        "assets/characters",
        "assets/environments",
        "assets/props",
        "audio/dialogue",
        "audio/music",
        "audio/sfx",
        "blender",
        "renders/preview",
        "renders/final",
        "exports",
        "cache",
        "logs",
    ] {
        fs::create_dir_all(project_dir.join(relative))
            .map_err(|e| format!("创建项目目录 {relative} 失败：{e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn story_studio_create_project(request: CreateProjectRequest) -> Result<StoryProject, String> {
    if request.source_content.trim().is_empty() {
        return Err("文稿内容为空，无法创建影像项目".to_string());
    }
    let (story_ir, validation_warnings) =
        crate::commands::story_ir::validate_and_normalize_story_ir(request.story_ir)?;
    for warning_message in &validation_warnings {
        warn!(warning = %warning_message, "[StoryStudio] project Story IR validation warning");
    }

    let project_name = sanitize_project_name(&request.project_name);
    let project_id = Uuid::new_v4().to_string();
    let output_root = if request.output_root.trim().is_empty() {
        let source = PathBuf::from(request.source_path.trim());
        source
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("GuaxiFilmProjects")
    } else {
        PathBuf::from(request.output_root.trim())
    };
    let project_dir = output_root.join(format!("{}-{}", project_name, &project_id[..8]));
    create_project_directories(&project_dir)?;

    let source_copy_path = project_dir.join("source").join("source.md");
    let story_ir_path = project_dir.join("story").join("story_ir.json");
    let blend_path = project_dir.join("blender").join("preview.blend");
    let preview_path = project_dir.join("renders").join("preview").join("preview.mp4");

    fs::write(&source_copy_path, request.source_content.as_bytes())
        .map_err(|e| format!("写入 UTF-8 文稿副本失败：{e}"))?;
    let pretty_ir = serde_json::to_vec_pretty(&story_ir)
        .map_err(|e| format!("序列化 Story IR 失败：{e}"))?;
    fs::write(&story_ir_path, pretty_ir).map_err(|e| format!("写入 Story IR 失败：{e}"))?;

    let manifest = json!({
        "schemaVersion": "1.0",
        "projectId": project_id,
        "projectName": project_name,
        "targetBlenderVersion": TARGET_BLENDER_VERSION,
        "source": {
            "originalPath": request.source_path,
            "copyPath": source_copy_path,
        },
        "storyIrPath": story_ir_path,
        "workflow": [
            "source", "research", "story-ir", "storyboard", "assets", "layout",
            "animation", "lighting", "audio", "render", "edit", "publish"
        ]
    });
    fs::write(
        project_dir.join("guaxi-film-project.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("写入项目清单失败：{e}"))?;

    info!(
        project_id = %project_id,
        project_dir = %project_dir.display(),
        "[StoryStudio] project created"
    );
    Ok(StoryProject {
        project_id,
        project_name,
        project_dir: project_dir.to_string_lossy().to_string(),
        source_copy_path: source_copy_path.to_string_lossy().to_string(),
        story_ir_path: story_ir_path.to_string_lossy().to_string(),
        blend_path: blend_path.to_string_lossy().to_string(),
        preview_path: preview_path.to_string_lossy().to_string(),
    })
}

/// 打开已经创建的制作项目。只读取项目清单声明的标准文件，不扫描或执行项目内脚本。
#[tauri::command]
pub fn story_studio_open_project(path: String) -> Result<OpenedStoryProject, String> {
    let requested = PathBuf::from(path.trim());
    let manifest_path = if requested.is_dir() {
        requested.join("guaxi-film-project.json")
    } else {
        requested
    };
    if manifest_path.file_name().and_then(|value| value.to_str()) != Some("guaxi-film-project.json") {
        return Err("请选择 guaxi-film-project.json 或其所在项目目录".to_string());
    }
    if !manifest_path.is_file() {
        return Err(format!("项目清单不存在：{}", manifest_path.display()));
    }
    let project_dir = manifest_path
        .parent()
        .ok_or_else(|| "项目清单没有父目录".to_string())?
        .to_path_buf();
    let manifest_bytes = fs::read(&manifest_path).map_err(|e| format!("读取项目清单失败：{e}"))?;
    let manifest: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|e| format!("项目清单 JSON 无效：{e}"))?;
    if manifest.get("schemaVersion").and_then(Value::as_str) != Some("1.0") {
        return Err("当前只支持 schemaVersion=1.0 的 Guaxi 影像项目".to_string());
    }

    let source_copy_path = project_dir.join("source").join("source.md");
    let story_ir_path = project_dir.join("story").join("story_ir.json");
    if !source_copy_path.is_file() || !story_ir_path.is_file() {
        return Err("项目缺少 source/source.md 或 story/story_ir.json".to_string());
    }
    let source_content = fs::read_to_string(&source_copy_path)
        .map_err(|e| format!("读取 UTF-8 文稿副本失败：{e}"))?
        .trim_start_matches('\u{feff}')
        .to_string();
    let story_ir: Value = serde_json::from_slice(
        &fs::read(&story_ir_path).map_err(|e| format!("读取 Story IR 失败：{e}"))?,
    )
    .map_err(|e| format!("Story IR JSON 无效：{e}"))?;
    let (story_ir, validation_warnings) =
        crate::commands::story_ir::validate_and_normalize_story_ir(story_ir)?;
    for warning_message in &validation_warnings {
        warn!(warning = %warning_message, "[StoryStudio] opened Story IR validation warning");
    }

    let project_id = manifest
        .get("projectId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let project_name = manifest
        .get("projectName")
        .and_then(Value::as_str)
        .unwrap_or("未命名影像项目")
        .to_string();
    let source_path = manifest
        .pointer("/source/originalPath")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let blend_path = project_dir.join("blender").join("preview.blend");
    let preview_path = project_dir.join("renders").join("preview").join("preview.mp4");
    let project = StoryProject {
        project_id,
        project_name,
        project_dir: project_dir.to_string_lossy().to_string(),
        source_copy_path: source_copy_path.to_string_lossy().to_string(),
        story_ir_path: story_ir_path.to_string_lossy().to_string(),
        blend_path: blend_path.to_string_lossy().to_string(),
        preview_path: preview_path.to_string_lossy().to_string(),
    };
    info!(
        project_id = %project.project_id,
        project_dir = %project.project_dir,
        "[StoryStudio] existing project opened"
    );
    Ok(OpenedStoryProject {
        project,
        source_path,
        source_content,
        story_ir,
    })
}

fn ffmpeg_candidates(explicit_path: Option<&str>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = explicit_path.filter(|value| !value.trim().is_empty()) {
        candidates.push(PathBuf::from(path.trim()));
    }
    if let Ok(path) = env::var("GUAXI_FFMPEG_PATH") {
        if !path.trim().is_empty() {
            candidates.push(PathBuf::from(path.trim()));
        }
    }
    #[cfg(target_os = "windows")]
    candidates.push(PathBuf::from("ffmpeg.exe"));
    #[cfg(not(target_os = "windows"))]
    candidates.push(PathBuf::from("ffmpeg"));
    candidates
}

fn resolve_ffmpeg(explicit_path: Option<&str>) -> Result<(PathBuf, String), String> {
    let mut last_error = String::new();
    for candidate in ffmpeg_candidates(explicit_path) {
        if candidate.components().count() > 1 && !candidate.is_file() {
            continue;
        }
        let mut command = Command::new(&candidate);
        command.arg("-version");
        apply_hidden_window(&mut command);
        match command.output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("ffmpeg")
                    .to_string();
                return Ok((candidate, version));
            }
            Ok(output) => {
                last_error = String::from_utf8_lossy(&output.stderr).trim().to_string();
            }
            Err(err) => last_error = err.to_string(),
        }
    }
    if last_error.is_empty() {
        Err("未检测到独立 FFmpeg；Blender 会保留 PNG 序列，但暂不生成 MP4".to_string())
    } else {
        Err(format!("FFmpeg 不可用：{last_error}"))
    }
}

#[tauri::command]
pub fn story_studio_get_media_tools_status(ffmpeg_path: Option<String>) -> MediaToolsStatus {
    match resolve_ffmpeg(ffmpeg_path.as_deref()) {
        Ok((path, version)) => {
            info!(path = %path.display(), version = %version, "[StoryStudio] ffmpeg detected");
            MediaToolsStatus {
                ffmpeg_available: true,
                ffmpeg_path: path.to_string_lossy().to_string(),
                ffmpeg_version: version,
                warnings: Vec::new(),
            }
        }
        Err(err) => MediaToolsStatus {
            ffmpeg_available: false,
            ffmpeg_path: String::new(),
            ffmpeg_version: String::new(),
            warnings: vec![err],
        },
    }
}

fn emit_job(app: &AppHandle, payload: StoryJobEvent) {
    if let Err(err) = app.emit(JOB_EVENT_NAME, payload.clone()) {
        warn!(job_id = %payload.job_id, error = %err, "[StoryStudio] emit job event failed");
    }
}

fn forward_blender_output<R: Read + Send + 'static>(
    reader: R,
    app: AppHandle,
    job_id: String,
    stream_name: &'static str,
) {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(raw) = trimmed.strip_prefix("GUAXI_PROGRESS:") {
                match serde_json::from_str::<Value>(raw.trim()) {
                    Ok(value) => {
                        let progress = value
                            .get("progress")
                            .and_then(Value::as_u64)
                            .unwrap_or(0)
                            .min(100) as u8;
                        let stage = value
                            .get("stage")
                            .and_then(Value::as_str)
                            .unwrap_or("blender")
                            .to_string();
                        let message = value
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("Blender 正在处理")
                            .to_string();
                        emit_job(
                            &app,
                            StoryJobEvent {
                                job_id: job_id.clone(),
                                status: "running".to_string(),
                                stage,
                                progress,
                                message,
                                project_dir: None,
                                preview_path: None,
                            },
                        );
                    }
                    Err(err) => warn!(
                        job_id = %job_id,
                        error = %err,
                        "[StoryStudio] invalid blender progress payload"
                    ),
                }
            } else if stream_name == "stderr" {
                warn!(job_id = %job_id, line = %trimmed, "[StoryStudio][Blender]");
            } else {
                info!(job_id = %job_id, line = %trimmed, "[StoryStudio][Blender]");
            }
        }
    });
}

#[tauri::command]
pub fn story_studio_start_preview(
    app: AppHandle,
    request: StartPreviewRequest,
) -> Result<String, String> {
    let project_dir = PathBuf::from(request.project_dir.trim());
    let story_ir_path = project_dir.join("story").join("story_ir.json");
    if !story_ir_path.is_file() {
        return Err(format!("项目缺少 Story IR：{}", story_ir_path.display()));
    }

    let explicit = request.blender_path.trim();
    let (blender, version, compatible, prerelease) = resolve_blender(
        if explicit.is_empty() { None } else { Some(explicit) },
    )?;
    if !compatible {
        return Err(format!(
            "检测到 {version}，当前执行器只允许 Blender {TARGET_BLENDER_MAJOR}.{TARGET_BLENDER_MINOR}.x"
        ));
    }

    let runner_path = resolve_plugin_directory()
        .join("guaxi_blender_bridge")
        .join("headless_runner.py");
    if !runner_path.is_file() {
        return Err(format!("找不到 Blender 受控执行器：{}", runner_path.display()));
    }

    let mode = if request.render_animation { "preview" } else { "project" };
    let mut command = Command::new(&blender);
    command
        .arg("--background")
        .arg("--factory-startup")
        .arg("--python-exit-code")
        .arg("17")
        .arg("--python")
        .arg(&runner_path)
        .arg("--")
        .arg("--project-dir")
        .arg(&project_dir)
        .arg("--story-ir")
        .arg(&story_ir_path)
        .arg("--mode")
        .arg(mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if !request.ffmpeg_path.trim().is_empty() {
        // 只给受控 runner 传入单个编码器路径，不修改进程级全局环境。
        command.env("GUAXI_FFMPEG_PATH", request.ffmpeg_path.trim());
    }
    apply_hidden_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| format!("启动 Blender 失败：{e}"))?;
    let job_id = Uuid::new_v4().to_string();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    CANCEL_FLAGS
        .lock()
        .map_err(|_| "影像任务状态锁已损坏".to_string())?
        .insert(job_id.clone(), cancel_flag.clone());

    if let Some(stdout) = child.stdout.take() {
        forward_blender_output(stdout, app.clone(), job_id.clone(), "stdout");
    }
    if let Some(stderr) = child.stderr.take() {
        forward_blender_output(stderr, app.clone(), job_id.clone(), "stderr");
    }

    let wait_job_id = job_id.clone();
    let wait_project_dir = project_dir.clone();
    let preview_path = project_dir.join("renders").join("preview").join("preview.mp4");
    info!(
        job_id = %job_id,
        blender = %blender.display(),
        version = %version,
        prerelease,
        project_dir = %project_dir.display(),
        mode,
        "[StoryStudio] blender preview started"
    );
    emit_job(
        &app,
        StoryJobEvent {
            job_id: job_id.clone(),
            status: "running".to_string(),
            stage: "launch".to_string(),
            progress: 1,
            message: if prerelease {
                "已启动 Blender 5.2 预发布版开发任务".to_string()
            } else {
                "已启动 Blender 5.2 LTS 任务".to_string()
            },
            project_dir: Some(project_dir.to_string_lossy().to_string()),
            preview_path: None,
        },
    );

    thread::spawn(move || {
        let final_event = loop {
            if cancel_flag.load(Ordering::SeqCst) {
                match child.kill() {
                    Ok(()) => info!(job_id = %wait_job_id, "[StoryStudio] blender job cancelled"),
                    Err(err) => warn!(job_id = %wait_job_id, error = %err, "[StoryStudio] kill blender failed"),
                }
                let _ = child.wait();
                break StoryJobEvent {
                    job_id: wait_job_id.clone(),
                    status: "cancelled".to_string(),
                    stage: "cancelled".to_string(),
                    progress: 0,
                    message: "任务已取消".to_string(),
                    project_dir: Some(wait_project_dir.to_string_lossy().to_string()),
                    preview_path: None,
                };
            }

            match child.try_wait() {
                Ok(Some(status)) if status.success() => {
                    let generated_preview = preview_path.is_file();
                    info!(
                        job_id = %wait_job_id,
                        generated_preview,
                        "[StoryStudio] blender job completed"
                    );
                    break StoryJobEvent {
                        job_id: wait_job_id.clone(),
                        status: "completed".to_string(),
                        stage: "completed".to_string(),
                        progress: 100,
                        message: if generated_preview {
                            "Blender 预演视频生成完成".to_string()
                        } else {
                            "Blender 预演工程生成完成".to_string()
                        },
                        project_dir: Some(wait_project_dir.to_string_lossy().to_string()),
                        preview_path: generated_preview
                            .then(|| preview_path.to_string_lossy().to_string()),
                    };
                }
                Ok(Some(status)) => {
                    error!(job_id = %wait_job_id, exit_status = %status, "[StoryStudio] blender job failed");
                    break StoryJobEvent {
                        job_id: wait_job_id.clone(),
                        status: "failed".to_string(),
                        stage: "failed".to_string(),
                        progress: 0,
                        message: format!("Blender 任务失败，退出状态：{status}"),
                        project_dir: Some(wait_project_dir.to_string_lossy().to_string()),
                        preview_path: None,
                    };
                }
                Ok(None) => thread::sleep(Duration::from_millis(200)),
                Err(err) => {
                    error!(job_id = %wait_job_id, error = %err, "[StoryStudio] wait blender failed");
                    break StoryJobEvent {
                        job_id: wait_job_id.clone(),
                        status: "failed".to_string(),
                        stage: "failed".to_string(),
                        progress: 0,
                        message: format!("等待 Blender 进程失败：{err}"),
                        project_dir: Some(wait_project_dir.to_string_lossy().to_string()),
                        preview_path: None,
                    };
                }
            }
        };

        emit_job(&app, final_event);
        if let Ok(mut jobs) = CANCEL_FLAGS.lock() {
            jobs.remove(&wait_job_id);
        }
    });

    Ok(job_id)
}

#[tauri::command]
pub fn story_studio_cancel_job(job_id: String) -> Result<(), String> {
    let jobs = CANCEL_FLAGS
        .lock()
        .map_err(|_| "影像任务状态锁已损坏".to_string())?;
    let flag = jobs
        .get(job_id.trim())
        .ok_or_else(|| "任务不存在或已经结束".to_string())?;
    flag.store(true, Ordering::SeqCst);
    info!(job_id = %job_id, "[StoryStudio] cancel requested");
    Ok(())
}

/// 探测交互式 Blender 插件。URL 固定为 loopback，不能由前端传入任意主机。
#[tauri::command]
pub async fn story_studio_probe_blender_bridge(port: Option<u16>) -> Result<Value, String> {
    let port = port.unwrap_or_else(default_bridge_port);
    if port < 1024 {
        return Err("Bridge 端口必须大于等于 1024".to_string());
    }
    let url = format!("http://127.0.0.1:{port}/health");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| format!("创建 Bridge 客户端失败：{e}"))?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("无法连接 Blender Bridge：{e}"))?;
    if !response.status().is_success() {
        return Err(format!("Blender Bridge 返回状态 {}", response.status()));
    }
    let payload = response
        .json::<Value>()
        .await
        .map_err(|e| format!("解析 Bridge 状态失败：{e}"))?;
    info!(port, "[StoryStudio] interactive blender bridge detected");
    Ok(payload)
}

/// 向交互式插件发送受控协议。令牌只用于 Authorization，不写日志、不写项目文件。
#[tauri::command]
pub async fn story_studio_send_blender_commands(
    request: BridgeCommandRequest,
) -> Result<Value, String> {
    if request.port < 1024 {
        return Err("Bridge 端口必须大于等于 1024".to_string());
    }
    if request.token.trim().is_empty() {
        return Err("Blender Bridge 访问令牌不能为空".to_string());
    }
    let operation_count = request
        .payload
        .get("operations")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if operation_count == 0 || operation_count > 100 {
        return Err("Bridge 请求必须包含 1 至 100 个受控操作".to_string());
    }

    let url = format!("http://127.0.0.1:{}/v1/commands", request.port);
    let client = reqwest::Client::builder()
        // 构建场景或保存工程可能较慢；真正的渲染仍建议使用后台任务队列。
        .timeout(Duration::from_secs(130))
        .build()
        .map_err(|e| format!("创建 Bridge 客户端失败：{e}"))?;
    let response = client
        .post(&url)
        .bearer_auth(request.token.trim())
        .json(&request.payload)
        .send()
        .await
        .map_err(|e| format!("调用 Blender Bridge 失败：{e}"))?;
    let status = response.status();
    let payload = response
        .json::<Value>()
        .await
        .map_err(|e| format!("解析 Bridge 响应失败：{e}"))?;
    if !status.is_success() || payload.get("ok").and_then(Value::as_bool) != Some(true) {
        let reason = payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        return Err(format!("Blender Bridge 拒绝请求：{reason}"));
    }
    info!(
        port = request.port,
        operation_count,
        "[StoryStudio] interactive blender commands completed"
    );
    Ok(payload)
}
