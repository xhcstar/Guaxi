//! 资源库（图床 / 图表数据）的索引、缩略图与引用反查。
//!
//! 为什么单独加这几个命令，而不是继续让前端一层层 `get_file_tree`：
//! - `get_file_tree` 一次只给一层，且不带大小与修改时间。管理器要做「按大小排序 /
//!   最近修改 / 类型统计 / 全局搜索」，这些信息必须在一次扫描里拿到；
//! - 网格视图要缩略图。直接让 `<img>` 加载原图，一屏十几张 4K 截图就会把窗口拖慢；
//! - 资源本身不记录「被谁引用」，而 `resources` 表里已经有 owner_file_path。把这张表
//!   读出来，预览侧才能回答「这张图还能不能删」。
//!
//! 三个命令都是只读的：扫描不写库、缩略图只写系统临时目录、引用表只查不改。
//! 既有 `get_file_tree` / `delete_path` / `move_path` 的行为一个字节都没动。

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;
use tracing::{info, warn};
use walkdir::WalkDir;

use crate::commands::settingview::get_image_bed_root;

/// 单次扫描的条目上限。资源库通常几千项；超过这个量说明用户把别的目录塞进来了，
/// 与其卡死窗口，不如截断并让界面明确告诉用户。
const MAX_SCAN_ENTRIES: usize = 60_000;

/// 这里只认图片的缩略图；其它类型由前端用类型图标表达，不做额外解码。
const THUMBNAIL_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "jfif", "webp", "gif", "bmp", "ico", "avif", "tif", "tiff",
];

#[derive(Debug, Serialize)]
pub struct ResourceRepoEntry {
    /// 相对资源库根目录的路径，统一用 `/` 分隔。
    pub relative_path: String,
    /// 相对父目录（根目录下为空串）。
    pub parent: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct ResourceRepoScan {
    pub root: String,
    pub entries: Vec<ResourceRepoEntry>,
    pub truncated: bool,
    /// 扫描耗时，界面状态栏直接显示，便于用户判断资源库规模。
    pub elapsed_ms: u128,
    /// 是否包含隐藏项（. 开头的目录）。
    pub include_hidden: bool,
}

#[derive(Debug, Serialize)]
pub struct ResourceRepoThumb {
    pub path: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize)]
pub struct ResourceUsageRow {
    pub relative_path: String,
    pub file_name: String,
    pub resource_type: String,
    pub owner_file_path: String,
    pub created_at: String,
}

fn strip_unc(path: PathBuf) -> PathBuf {
    // Windows 的 canonicalize 会返回 `\\?\D:\...`。这个前缀只在 Rust 侧有效，
    // 交给前端后 asset 协议、URL 编码和显示都会出问题，所以统一剥掉再往下传。
    #[cfg(target_os = "windows")]
    {
        let text = path.to_string_lossy().to_string();
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

fn repo_root() -> Result<PathBuf, String> {
    let raw = get_image_bed_root();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("资源库根目录未配置".to_string());
    }
    let path = PathBuf::from(trimmed);
    let canon = path
        .canonicalize()
        .map_err(|e| format!("资源库根目录不可用: {}", e))?;
    let canon = strip_unc(canon);
    if !canon.is_dir() {
        return Err("资源库根目录不是目录".to_string());
    }
    Ok(canon)
}

/// 把库内相对路径解析为绝对路径，并保证结果仍在根目录内。
/// 组件里所有接受 `relative_path` 的命令都走这里，越权路径统一拒绝。
fn resolve_in_root(root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let rel = relative_path.trim().replace('\\', "/");
    if rel.is_empty() {
        return Ok(root.to_path_buf());
    }
    let candidate = Path::new(&rel);
    for comp in candidate.components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {}
            _ => return Err(format!("资源库相对路径非法: {}", relative_path)),
        }
    }
    let joined = root.join(&rel);
    let canon = joined
        .canonicalize()
        .map_err(|e| format!("资源不存在: {} ({})", relative_path, e))?;
    let canon = strip_unc(canon);
    if !canon.starts_with(root) {
        return Err(format!("资源库相对路径越界: {}", relative_path));
    }
    Ok(canon)
}

fn mtime_ms(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn to_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn parent_of(rel: &str) -> String {
    match rel.rfind('/') {
        Some(idx) if idx > 0 => rel[..idx].to_string(),
        _ => String::new(),
    }
}

fn scan_sync(
    include_hidden: bool,
    relative_path: Option<String>,
    single_level: bool,
) -> Result<ResourceRepoScan, String> {
    let started = std::time::Instant::now();
    let root = repo_root()?;

    // 给了相对路径就只扫这一层：删除/新建/重命名后的局部刷新走这里。
    // 只取直接子项，是因为前端已经持有更深层节点，整棵重扫会把它们的展开状态和
    // 已缓存的缩略图上下文全部作废。
    let scope = match relative_path.as_deref() {
        Some(rel) if !rel.trim().is_empty() => resolve_in_root(&root, rel)?,
        _ => root.clone(),
    };
    if !scope.is_dir() {
        return Err("刷新目标不是目录".to_string());
    }
    let single_level_default = normalize_rel(&scope, &root) != "";

    let mut entries = Vec::new();
    let mut truncated = false;

    let mut walker = WalkDir::new(&scope)
        .follow_links(false)
        .min_depth(1)
        .sort_by(|a, b| a.file_name().cmp(b.file_name()));
    if single_level || single_level_default {
        walker = walker.max_depth(1);
    }

    for item in walker.into_iter().filter_entry(|e| {
        if include_hidden {
            return true;
        }
        let name = e.file_name().to_string_lossy();
        !name.starts_with('.')
    }) {
        let entry = match item {
            Ok(e) => e,
            Err(e) => {
                warn!(error = %e, "resource_repo_scan: skip unreadable entry");
                continue;
            }
        };
        if entries.len() >= MAX_SCAN_ENTRIES {
            truncated = true;
            break;
        }
        if entry.depth() == 0 {
            continue;
        }

        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let is_dir = meta.is_dir();
        let relative_path = to_relative(&root, path);
        if relative_path.is_empty() {
            continue;
        }

        entries.push(ResourceRepoEntry {
            parent: parent_of(&relative_path),
            name: entry.file_name().to_string_lossy().to_string(),
            is_dir,
            size: if is_dir { 0 } else { meta.len() },
            mtime_ms: mtime_ms(&meta),
            relative_path,
        });
    }

    let elapsed_ms = started.elapsed().as_millis();
    info!(
        root = %root.display(),
        count = entries.len(),
        truncated,
        include_hidden,
        scope = %scope.display(),
        elapsed_ms,
        "resource_repo_scan"
    );

    Ok(ResourceRepoScan {
        root: root.to_string_lossy().to_string(),
        entries,
        truncated,
        elapsed_ms,
        include_hidden,
    })
}

fn normalize_rel(path: &Path, root: &Path) -> String {
    if path == root {
        String::new()
    } else {
        to_relative(root, path)
    }
}

/// `single_level` 只取目标目录的直接子项：局部刷新（删/建/改名之后）用它，根目录也一样。
#[tauri::command]
pub async fn resource_repo_scan(
    include_hidden: Option<bool>,
    relative_path: Option<String>,
    single_level: Option<bool>,
) -> Result<ResourceRepoScan, String> {
    let include_hidden = include_hidden.unwrap_or(false);
    let single_level = single_level.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || scan_sync(include_hidden, relative_path, single_level))
        .await
        .map_err(|e| format!("扫描资源库失败: {}", e))?
}

fn thumbs_root() -> PathBuf {
    std::env::temp_dir()
        .join(".guaxi-display-artifacts")
        .join("guaxi-resource-thumbs")
}

fn is_thumbable(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    THUMBNAIL_EXTS.contains(&ext.as_str())
}

fn thumbnail_sync(relative_path: &str, max_edge: u32) -> Result<ResourceRepoThumb, String> {
    let root = repo_root()?;
    let abs = resolve_in_root(&root, relative_path)?;
    if !abs.is_file() {
        return Err("不是文件".to_string());
    }
    if !is_thumbable(&abs) {
        return Err("该类型不生成缩略图".to_string());
    }

    let meta = fs::metadata(&abs).map_err(|e| e.to_string())?;
    let key = {
        let mut hasher = Sha256::new();
        hasher.update(abs.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        hasher.update(mtime_ms(&meta).to_le_bytes());
        hasher.update(b"\0");
        hasher.update(max_edge.to_le_bytes());
        format!("{:x}", hasher.finalize())
    };

    let dir = thumbs_root().join(&key[..2]);
    fs::create_dir_all(&dir).map_err(|e| format!("创建缩略图目录失败: {}", e))?;
    let out = dir.join(format!("{}.jpg", key));

    if out.is_file() {
        let (w, h) = image::image_dimensions(&out).unwrap_or((0, 0));
        return Ok(ResourceRepoThumb {
            path: out.to_string_lossy().to_string(),
            width: w,
            height: h,
        });
    }

    // 原图解码失败（损坏、缺解码器、超大）时把错误交给前端，前端会退回加载原图。
    let img = image::ImageReader::open(&abs)
        .map_err(|e| format!("打开图片失败: {}", e))?
        .with_guessed_format()
        .map_err(|e| format!("识别图片格式失败: {}", e))?
        .decode()
        .map_err(|e| format!("解码图片失败: {}", e))?;

    let thumb = img.thumbnail(max_edge, max_edge);
    let (w, h) = (thumb.width(), thumb.height());
    let rgb = thumb.to_rgb8();

    let tmp = dir.join(format!("{}.tmp.jpg", key));
    {
        use image::codecs::jpeg::JpegEncoder;
        use image::ExtendedColorType;
        let mut file = fs::File::create(&tmp).map_err(|e| format!("创建缩略图失败: {}", e))?;
        let mut encoder = JpegEncoder::new_with_quality(&mut file, 82);
        encoder
            .encode(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)
            .map_err(|e| format!("编码缩略图失败: {}", e))?;
    }
    fs::rename(&tmp, &out).map_err(|e| format!("写入缩略图失败: {}", e))?;

    Ok(ResourceRepoThumb {
        path: out.to_string_lossy().to_string(),
        width: w,
        height: h,
    })
}

#[tauri::command]
pub async fn resource_repo_thumbnail(
    relative_path: String,
    max_edge: Option<u32>,
) -> Result<ResourceRepoThumb, String> {
    let max_edge = max_edge.unwrap_or(480).clamp(96, 1024);
    tauri::async_runtime::spawn_blocking(move || thumbnail_sync(&relative_path, max_edge))
        .await
        .map_err(|e| format!("生成缩略图失败: {}", e))?
}

/// 资源 -> 引用文档。表不存在（从没用过导入/图床）时返回空表而不是报错：
/// 管理器启动不该因为一张可选表失败。
#[tauri::command]
pub fn resource_repo_usage_index() -> Result<Vec<ResourceUsageRow>, String> {
    let db_path = match crate::commands::repoview::create_db_file() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "resource_repo_usage_index: db path unavailable");
            return Ok(Vec::new());
        }
    };
    if !db_path.is_file() {
        return Ok(Vec::new());
    }

    let conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "resource_repo_usage_index: open db failed");
            return Ok(Vec::new());
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT relative_path, file_name, resource_type, owner_file_path, created_at
         FROM resources
         ORDER BY created_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "resource_repo_usage_index: prepare failed (table missing?)");
            return Ok(Vec::new());
        }
    };

    let rows = stmt.query_map([], |row| {
        Ok(ResourceUsageRow {
            relative_path: row.get::<_, String>(0)?,
            file_name: row.get::<_, String>(1)?,
            resource_type: row.get::<_, String>(2)?,
            owner_file_path: row.get::<_, String>(3)?,
            created_at: row.get::<_, String>(4)?,
        })
    });

    let mut out = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    if let Ok(rows) = rows {
        for row in rows.flatten() {
            let rel = row.relative_path.replace('\\', "/");
            let key = (rel.clone(), row.owner_file_path.clone());
            // 同一文档多次插入同一资源时会留下重复行，这里按 (资源, 文档) 去重。
            if !seen.insert(key) {
                continue;
            }
            out.push(ResourceUsageRow {
                relative_path: rel,
                ..row
            });
        }
    }

    info!(rows = out.len(), "resource_repo_usage_index");
    Ok(out)
}
