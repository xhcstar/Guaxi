use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tracing::{info, warn};

use base64::engine::general_purpose;
use base64::Engine as _;

use crate::commands::kgraph;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileNode {
    id: String,
    name: String,
    is_directory: bool,
    children: Option<Vec<FileNode>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMeta {
    path: String,
    mtime_ms: Option<u64>,
    file_size: u64,
    is_directory: bool,
}

// 辅助函数：规范化路径分隔符
fn normalize_path(path: &str) -> PathBuf {
    use std::path::Component;

    let mut s = path.trim().replace('\\', "/");

    if s.starts_with('/') && s.len() >= 4 {
        let bytes = s.as_bytes();
        if bytes[2] == b':' && bytes[3] == b'/' {
            s = s[1..].to_string();
        }
    }

    let mut out = PathBuf::new();
    let mut anchored = false;
    let mut depth = 0usize;

    for comp in Path::new(&s).components() {
        match comp {
            Component::Prefix(p) => {
                out.push(p.as_os_str());
                anchored = true;
                depth = 0;
            }
            Component::RootDir => {
                out.push(Path::new("/"));
                anchored = true;
                depth = 0;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if depth > 0 {
                    out.pop();
                    depth -= 1;
                } else if !anchored {
                    out.push("..");
                }
            }
            Component::Normal(part) => {
                out.push(part);
                depth += 1;
            }
        }
    }

    out
}

fn exact_child_name_exists(path: &Path) -> Result<bool, String> {
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    let Some(file_name) = path.file_name() else {
        return Ok(false);
    };
    if !parent.exists() {
        return Ok(false);
    }

    for entry in fs::read_dir(parent).map_err(|e| format!("读取父目录失败: {}", e))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        if entry.file_name() == file_name {
            return Ok(true);
        }
    }

    Ok(false)
}

fn case_insensitive_child_name_exists(path: &Path) -> Result<bool, String> {
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    let Some(file_name) = path.file_name() else {
        return Ok(false);
    };
    if !parent.exists() {
        return Ok(false);
    }

    let wanted = file_name.to_string_lossy().to_lowercase();
    for entry in fs::read_dir(parent).map_err(|e| format!("读取父目录失败: {}", e))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        if entry.file_name().to_string_lossy().to_lowercase() == wanted {
            return Ok(true);
        }
    }

    Ok(false)
}

fn is_case_only_create_conflict(path: &Path, err: &std::io::Error) -> bool {
    err.kind() == ErrorKind::AlreadyExists
        && !exact_child_name_exists(path).unwrap_or(false)
        && case_insensitive_child_name_exists(path).unwrap_or(false)
}

// 设计说明：前端已经允许 README.md/readme.md 这类大小写不同的名称同时进入创建流程，
// 但 Windows 默认目录通常仍是大小写不敏感的。这里仅在确认是“仅大小写不同”的冲突时，
// 尝试为父目录开启大小写敏感，再重试一次真正的文件系统创建，避免普通同名误触发目录属性修改。
#[cfg(target_os = "windows")]
fn enable_case_sensitive_parent_for_create(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法获取父目录：{}", path.display()))?;

    info!(
        parent = %parent.display(),
        target = %path.display(),
        "try enable Windows per-directory case sensitivity before create"
    );

    let output = std::process::Command::new("fsutil.exe")
        .arg("file")
        .arg("setCaseSensitiveInfo")
        .arg(parent)
        .arg("enable")
        .output()
        .map_err(|e| format!("调用 fsutil 开启目录大小写敏感失败: {}", e))?;

    if output.status.success() {
        info!(
            parent = %parent.display(),
            target = %path.display(),
            "Windows per-directory case sensitivity enabled"
        );
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    warn!(
        parent = %parent.display(),
        target = %path.display(),
        status = ?output.status.code(),
        stdout = %stdout,
        stderr = %stderr,
        "enable Windows per-directory case sensitivity failed"
    );

    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("fsutil 退出码 {:?}", output.status.code())
    };

    Err(format!(
        "当前目录所在文件系统未启用大小写敏感，Guaxi 已尝试为父目录开启但失败：{}",
        detail
    ))
}

#[cfg(not(target_os = "windows"))]
fn enable_case_sensitive_parent_for_create(path: &Path) -> Result<(), String> {
    Err(format!(
        "当前文件系统拒绝创建仅大小写不同的同名项：{}",
        path.display()
    ))
}

fn case_sensitive_create_conflict_message(path: &Path, err: &std::io::Error) -> String {
    if err.kind() == ErrorKind::AlreadyExists {
        if exact_child_name_exists(path).unwrap_or(false) {
            return format!("创建失败：{} 已存在", path.display());
        }
        if case_insensitive_child_name_exists(path).unwrap_or(false) {
            return format!(
                "创建失败：当前目录所在文件系统未启用大小写敏感，无法创建仅大小写不同的同名项：{}",
                path.display()
            );
        }
        format!(
            "创建失败：{} 已存在，或当前文件系统拒绝创建该路径",
            path.display()
        )
    } else {
        err.to_string()
    }
}

fn create_directory_case_aware(path: &Path) -> Result<(), String> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(first_err) if is_case_only_create_conflict(path, &first_err) => {
            enable_case_sensitive_parent_for_create(path)?;
            fs::create_dir(path).map_err(|retry_err| {
                format!(
                    "创建失败：已尝试开启父目录大小写敏感，但仍无法创建 {}：{}",
                    path.display(),
                    retry_err
                )
            })
        }
        Err(err) => Err(case_sensitive_create_conflict_message(path, &err)),
    }
}

fn create_file_case_aware(path: &Path) -> Result<fs::File, String> {
    let open_new = || OpenOptions::new().write(true).create_new(true).open(path);

    match open_new() {
        Ok(file) => Ok(file),
        Err(first_err) if is_case_only_create_conflict(path, &first_err) => {
            enable_case_sensitive_parent_for_create(path)?;
            open_new().map_err(|retry_err| {
                format!(
                    "创建失败：已尝试开启父目录大小写敏感，但仍无法创建 {}：{}",
                    path.display(),
                    retry_err
                )
            })
        }
        Err(err) => Err(case_sensitive_create_conflict_message(path, &err)),
    }
}

#[tauri::command]
pub fn get_file_tree(path: String) -> Result<Vec<FileNode>, String> {
    let path = normalize_path(&path);
    let dir = Path::new(&path);

    // info!("get_file_tree input: {}", raw_path);
    // info!("get_file_tree normalized: {}", dir.display());
    // info!("get_file_tree normalized(debug): {:?}", dir);

    if !dir.is_dir() {
        warn!("get_file_tree: not a directory: {}", dir.display());
        return Err("路径不是目录".to_string());
    }

    let mut nodes = Vec::new();
    for entry in fs::read_dir(dir)
        .map_err(|e| format!("read_dir failed: {} ({})", e, dir.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let is_directory = path.is_dir();

        let node = FileNode {
            id: path.to_string_lossy().to_string(),
            name: path.file_name().unwrap().to_string_lossy().to_string(),
            is_directory,
            children: if is_directory { Some(Vec::new()) } else { None },
        };

        nodes.push(node);
    }

    Ok(nodes)
}

#[tauri::command]
pub fn read_file_content(path: String) -> Result<String, String> {
    let path = normalize_path(&path);
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// 把资源库相对路径解析成绝对路径（资源库根目录由配置决定，前端不必知道它）。
///
/// 图表数据文件的引用顺序是「文档目录 → 资源库」，资源库这一层走这里：
/// 前端只知道 `charts/sales.csv` 这样的库内相对路径，换机器后只要资源库同步到位就能取到，
/// 不会因为原机器上的绝对路径不存在而渲染失败。
#[tauri::command]
pub fn resolve_resource_path(relative_path: String) -> Result<String, String> {
    let rel = relative_path.trim().replace('\\', "/");
    if rel.is_empty() {
        return Err("资源库相对路径为空".to_string());
    }
    // 越权防护：库内相对路径不允许跳出根目录。
    if rel.split('/').any(|seg| seg == "..") || rel.starts_with('/') {
        return Err(format!("资源库相对路径非法: {}", relative_path));
    }

    let root = crate::commands::settingview::get_image_bed_root();
    let root = root.trim();
    if root.is_empty() {
        return Err("资源库根目录未配置".to_string());
    }
    Ok(format!(
        "{}/{}",
        root.trim_end_matches(['/', '\\']).replace('\\', "/"),
        rel
    ))
}

#[tauri::command]
pub fn get_file_meta(path: String) -> Result<FileMeta, String> {
    let normalized = normalize_path(&path);
    let metadata = fs::metadata(&normalized).map_err(|e| e.to_string())?;
    let mtime_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64);

    let meta = FileMeta {
        path: normalized.to_string_lossy().to_string(),
        mtime_ms,
        file_size: metadata.len(),
        is_directory: metadata.is_dir(),
    };

    info!(
        path = %meta.path,
        mtime_ms = ?meta.mtime_ms,
        file_size = meta.file_size,
        is_directory = meta.is_directory,
        "[fileprocess] get_file_meta"
    );

    Ok(meta)
}

#[tauri::command]
pub fn read_file_bytes_base64(path: String) -> Result<String, String> {
    let path = normalize_path(&path);
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    Ok(general_purpose::STANDARD.encode(bytes))
}

#[tauri::command]
pub fn write_file_bytes_base64(path: String, base64: String) -> Result<(), String> {
    let path = normalize_path(&path);
    let bytes = general_purpose::STANDARD
        .decode(base64.as_bytes())
        .map_err(|e| e.to_string())?;
    // 设计说明：
    // 1. 前端会把二进制内容直接写到资源库中的分层日期目录，这些父目录不一定已经存在。
    // 2. 这里统一在后端兜底创建父目录，避免不同前端调用方各自重复处理并遗漏。
    // 3. 同时补充日志，便于排查资源写回、导出文件等链路中的路径问题。
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            info!(
                parent = %parent.display(),
                "[fileprocess] ensured parent directory for binary write"
            );
        }
    }
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    info!(
        path = %path.display(),
        "[fileprocess] wrote binary file from base64 payload"
    );
    Ok(())
}

#[tauri::command]
pub fn write_file_content(path: String, content: String) -> Result<(), String> {
    let path = normalize_path(&path);
    fs::write(&path, &content).map_err(|e| e.to_string())?;

    if let Err(e) = kgraph::after_write_file(&path, &content) {
        warn!(error = %e, path = %path.display(), "kbmeta after_write_file failed");
    }

    Ok(())
}

#[tauri::command]
pub fn get_executable_dir() -> Result<String, String> {
    let exe_path = env::current_exe().map_err(|e| format!("获取可执行文件路径失败: {}", e))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取可执行文件所在目录".to_string())?
        .to_path_buf();

    info!(
        exe_path = %exe_path.display(),
        exe_dir = %exe_dir.display(),
        "get_executable_dir"
    );

    Ok(exe_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn create_directory(path: String) -> Result<(), String> {
    let path = normalize_path(&path);
    if exact_child_name_exists(&path)? {
        return Err(format!("创建目录失败：{} 已存在", path.display()));
    }

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建父目录失败: {}", e))?;
        }
    }

    create_directory_case_aware(&path)?;
    info!(path = %path.display(), "create_directory success");

    if let Err(e) = kgraph::after_create_path(&path, true, None) {
        warn!(error = %e, path = %path.display(), "kbmeta after_create_path failed");
    }

    Ok(())
}

#[tauri::command]
pub fn create_file(path: String) -> Result<(), String> {
    let path = normalize_path(&path);

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }

    if exact_child_name_exists(&path)? {
        return Err(format!("创建文件失败：{} 已存在", path.display()));
    }

    create_file_case_aware(&path).map(|_| ())?;
    info!(path = %path.display(), "create_file success");

    if let Err(e) = kgraph::after_create_path(&path, false, None) {
        warn!(error = %e, path = %path.display(), "kbmeta after_create_path failed");
    }

    Ok(())
}

#[tauri::command]
pub fn delete_path(path: String) -> Result<(), String> {
    let path = normalize_path(&path);
    let is_dir = fs::metadata(&path).map_err(|e| e.to_string())?.is_dir();

    if is_dir {
        fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
    } else {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }

    if let Err(e) = kgraph::after_delete_path(&path, is_dir) {
        warn!(error = %e, path = %path.display(), "kbmeta after_delete_path failed");
    }

    Ok(())
}

#[tauri::command]
pub fn move_path(source_path: String, destination_path: String) -> Result<(), String> {
    let source_path = normalize_path(&source_path);
    let destination_path = normalize_path(&destination_path);

    info!("move_path: {} -> {}", source_path.display(), destination_path.display());
    std::fs::rename(&source_path, &destination_path).map_err(|e| e.to_string())?;

    if let Err(e) = kgraph::after_move_path(&source_path, &destination_path) {
        warn!(error = %e, src = %source_path.display(), dst = %destination_path.display(), "kbmeta after_move_path failed");
    }

    Ok(())
}

#[tauri::command]
pub async fn create_file_with_content(
    path: String,
    content: Option<String>, // 改为接受可选参数
) -> Result<(), String> {
    let content = content.unwrap_or_default(); // 处理null情况

    let path = normalize_path(&path.to_string());

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }
    }

    if exact_child_name_exists(&path)? {
        return Err(format!("创建文件失败：{} 已存在", path.display()));
    }

    let mut file = create_file_case_aware(&path)?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("写入文件失败: {}", e))?;
    info!(path = %path.display(), bytes = content.len(), "create_file_with_content success");

    if let Err(e) = kgraph::after_create_path(&path, false, Some(&content)) {
        warn!(error = %e, path = %path.display(), "kbmeta after_create_path failed");
    }

    Ok(())
}

// 辅助函数：检查是否是有效目录
fn is_valid_directory(path: &Path) -> bool {
    path.exists() && path.is_dir()
}

#[tauri::command]
pub fn copy_path(source_path: String, destination_path: String) -> Result<(), String> {
    let source_path = normalize_path(&source_path);
    let destination_path = normalize_path(&destination_path);
    if let Some(parent) = destination_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
        }
    }
    let meta = fs::metadata(&source_path).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        copy_dir_recursive(&source_path, &destination_path)?;
    } else {
        fs::copy(&source_path, &destination_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn rename_directory(old_path: String, new_path: String) -> Result<(), String> {
    let old_path = normalize_path(&old_path);
    let new_path = normalize_path(&new_path);

    info!(
        "Attempting to rename directory: {:?} -> {:?}",
        old_path, new_path
    );

    // 1. 验证源目录
    if !is_valid_directory(&old_path) {
        warn!(
            "Source directory does not exist or is not a directory: {:?}",
            old_path
        );
        return Err("源目录不存在或不是目录".to_string());
    }

    // 2. 检查目标路径是否已存在
    if new_path.exists() {
        warn!("Target path already exists: {:?}", new_path);
        return Err("目标路径已存在".to_string());
    }

    // 3. 确保父目录存在
    if let Some(parent) = new_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建父目录失败: {}", e))?;
        }
    }

    // 4. 执行重命名
    fs::rename(&old_path, &new_path).map_err(|e| format!("重命名目录失败: {}", e))?;

    if let Err(e) = kgraph::after_move_path(&old_path, &new_path) {
        warn!(error = %e, src = %old_path.display(), dst = %new_path.display(), "kbmeta after_move_path failed");
    }

    info!(
        "Directory renamed successfully: {:?} -> {:?}",
        old_path, new_path
    );
    Ok(())
}

#[tauri::command]
pub fn rename_path(old_path: String, new_path: String) -> Result<(), String> {
    let old_path = normalize_path(&old_path);
    let new_path = normalize_path(&new_path);

    info!(
        "Attempting to rename path: {:?} -> {:?}",
        old_path, new_path
    );

    // 1. 验证源路径是否存在
    if !old_path.exists() {
        warn!("Source path does not exist: {:?}", old_path);
        return Err("源路径不存在".to_string());
    }

    // 2. 检查目标路径是否已存在
    if new_path.exists() {
        warn!("Target path already exists: {:?}", new_path);
        return Err("目标路径已存在".to_string());
    }

    // 3. 确保父目录存在
    if let Some(parent) = new_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("创建父目录失败: {}", e))?;
        }
    }

    // 4. 执行重命名
    fs::rename(&old_path, &new_path).map_err(|e| format!("重命名失败: {}", e))?;

    if let Err(e) = kgraph::after_move_path(&old_path, &new_path) {
        warn!(error = %e, src = %old_path.display(), dst = %new_path.display(), "kbmeta after_move_path failed");
    }

    info!(
        "Path renamed successfully: {:?} -> {:?}",
        old_path, new_path
    );
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct FuzzyFileHit {
    /// 文件名（不含目录）
    pub name: String,
    /// 文件绝对路径
    pub path: String,
    /// 相对所属知识库根的路径
    pub relative: String,
    /// 所属知识库根路径
    pub repo: String,
}

/// 遍历时跳过的噪声目录名（除了所有以 `.` 开头的隐藏目录）。
fn is_noise_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules" | "target" | "dist" | "build" | "__pycache__" | "venv" | ".venv"
    )
}

/// 在若干知识库根目录下递归模糊搜索文件名，用于智能助手输入框的 `@` 引用。
///
/// - `query` 为空：返回各根目录下的前若干文件（按名称排序）。
/// - `query` 非空：用 SkimMatcherV2 对「文件名」打分，并对「相对路径」做弱匹配兜底，按分数降序返回。
///
/// 跳过隐藏目录（名字以 `.` 开头）与常见噪声目录，仅返回文件。
#[tauri::command]
pub fn fuzzy_search_files(
    query: String,
    roots: Vec<String>,
    limit: Option<usize>,
) -> Result<Vec<FuzzyFileHit>, String> {
    use fuzzy_matcher::skim::SkimMatcherV2;
    use fuzzy_matcher::FuzzyMatcher;
    use walkdir::WalkDir;

    let limit = limit.unwrap_or(40).clamp(1, 200);
    let query = query.trim();
    let matcher = SkimMatcherV2::default();

    // 去重根目录（规范化后），避免同一库重复遍历。
    let mut seen_roots: Vec<PathBuf> = Vec::new();
    for raw in &roots {
        if raw.trim().is_empty() {
            continue;
        }
        let root = normalize_path(raw);
        if !root.is_dir() {
            continue;
        }
        if seen_roots.iter().any(|r| r == &root) {
            continue;
        }
        seen_roots.push(root);
    }

    // (score, hit)。query 为空时 score 统一为 0，最终按名称排序。
    let mut scored: Vec<(i64, FuzzyFileHit)> = Vec::new();
    // 单次搜索遍历的文件上限，防止超大库卡顿。
    const MAX_SCAN_FILES: usize = 20000;
    let mut scanned = 0usize;

    'roots: for root in &seen_roots {
        let root_str = root.to_string_lossy().to_string();
        let walker = WalkDir::new(root).into_iter().filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // 始终允许根自身；对子项过滤隐藏/噪声目录。
            if e.depth() == 0 {
                return true;
            }
            if e.file_type().is_dir() {
                return !name.starts_with('.') && !is_noise_dir(&name);
            }
            true
        });

        for entry in walker.filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            scanned += 1;
            if scanned > MAX_SCAN_FILES {
                break 'roots;
            }

            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            // 跳过隐藏文件。
            if name.starts_with('.') {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| name.clone());

            let score = if query.is_empty() {
                0
            } else if let Some(s) = matcher.fuzzy_match(&name, query) {
                s + 50 // 文件名命中加权
            } else if let Some(s) = matcher.fuzzy_match(&relative, query) {
                s
            } else {
                continue;
            };

            scored.push((
                score,
                FuzzyFileHit {
                    name,
                    path: path.to_string_lossy().to_string(),
                    relative,
                    repo: root_str.clone(),
                },
            ));
        }
    }

    if query.is_empty() {
        scored.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));
    } else {
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
    }

    Ok(scored.into_iter().take(limit).map(|(_, h)| h).collect())
}
