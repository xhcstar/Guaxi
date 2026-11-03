//! 资料库增量扫描：把新入库 / 新转换的文档以「自有材料」感知入库。
//!
//! 为什么需要它：知识库的语义通道读的是 `kg_index`（需要建索引/生成向量），
//! 而资料库里刚导入、刚转换完的文档在索引更新之前对大脑是隐形的。这里每天扫一遍
//! `Libraries/<kb>/<book>/{markdown,md}/`，按 mtime 水位增量把新文件以状态 `linked`
//! 的感知入库——它们不进收件箱，但会参与打分、锚定与召回。
//!
//! 约束：
//! - 只处理 markdown（转换产物与原稿），不碰二进制与 notes/；
//! - 每次最多 20 个文件，避免首次运行把整库灌进来；
//! - 水位写进 `cog_settings.library_digest_cursor`（epoch 秒），文件更新后 mtime 变大才会重扫。
//! - 正文截断到 6 万字：感知是"记住有这份材料"，不是替代原文。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::json;
use tracing::{debug, info};

use crate::cognition::percept::{self, PerceptInput};
use crate::cognition::store;

const SCAN_BATCH: usize = 20;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MIN_FILE_CHARS: usize = 200;
const MAX_TEXT_CHARS: usize = 60_000;
/// 首次运行只回看最近 7 天：老资料应该由知识库索引负责，不该在第一次启动时全量入库。
const FIRST_RUN_DAYS: i64 = 7;
const CURSOR_KEY: &str = "library_digest_cursor";

#[derive(Debug, Default, Clone)]
pub struct DigestReport {
    pub scanned: usize,
    pub ingested: usize,
    pub duplicates: usize,
    pub skipped: usize,
    pub cursor: i64,
}

fn mtime_secs(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false)
}

/// 从 `Libraries/<kb>/<book>/<type>/<file>` 里取出库名与书名。
fn kb_and_book(libs: &Path, file: &Path) -> (String, String) {
    let mut kb = String::new();
    let mut book = String::new();
    if let Ok(rel) = file.strip_prefix(libs) {
        let parts: Vec<&str> = rel
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();
        if let Some(p) = parts.first() {
            kb = (*p).to_string();
        }
        if let Some(p) = parts.get(1) {
            book = (*p).to_string();
        }
    }
    (kb, book)
}

/// 收集水位之后的候选文件（未排序）。
fn collect_candidates(libs: &Path, floor: i64) -> Vec<(i64, PathBuf)> {
    let mut out = Vec::new();
    let Ok(kb_dirs) = fs::read_dir(libs) else {
        return out;
    };
    for kb_entry in kb_dirs.flatten() {
        let kb_dir = kb_entry.path();
        if !kb_dir.is_dir() {
            continue;
        }
        let Ok(book_dirs) = fs::read_dir(&kb_dir) else {
            continue;
        };
        for book_entry in book_dirs.flatten() {
            let book_dir = book_entry.path();
            if !book_dir.is_dir() {
                continue;
            }
            // 转换产物在 markdown/；少数旧库用 md/，两种都扫。
            for sub in ["markdown", "md"] {
                let dir = book_dir.join(sub);
                if !dir.is_dir() {
                    continue;
                }
                let Ok(files) = fs::read_dir(&dir) else {
                    continue;
                };
                for file_entry in files.flatten() {
                    let path = file_entry.path();
                    if !path.is_file() || !is_markdown(&path) {
                        continue;
                    }
                    let Ok(meta) = file_entry.metadata() else {
                        continue;
                    };
                    let mtime = mtime_secs(&meta);
                    if mtime > floor {
                        out.push((mtime, path));
                    }
                }
            }
        }
    }
    out
}

/// 每日维护调用一次（同步）。失败只影响本次扫描，不向上抛给维护循环之外的任何地方。
pub fn run() -> Result<DigestReport, String> {
    let root = crate::commands::settingview::get_libraries_root();
    if root.trim().is_empty() {
        return Ok(DigestReport::default());
    }
    let libs = Path::new(&root).join("Libraries");
    if !libs.is_dir() {
        return Ok(DigestReport::default());
    }

    let cursor = store::get_i64(CURSOR_KEY, 0);
    let floor = if cursor > 0 {
        cursor
    } else {
        chrono::Local::now().timestamp() - FIRST_RUN_DAYS * 86_400
    };

    let mut candidates = collect_candidates(&libs, floor);
    if candidates.is_empty() {
        return Ok(DigestReport {
            cursor: cursor.max(floor),
            ..Default::default()
        });
    }
    candidates.sort_by_key(|(mtime, _)| *mtime);

    let mut report = DigestReport::default();
    let mut newest = cursor.max(floor);
    for (mtime, path) in candidates.into_iter().take(SCAN_BATCH) {
        report.scanned += 1;
        newest = newest.max(mtime);
        let Ok(meta) = fs::metadata(&path) else {
            report.skipped += 1;
            continue;
        };
        if meta.len() > MAX_FILE_BYTES {
            report.skipped += 1;
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            report.skipped += 1;
            continue;
        };
        if text.chars().count() < MIN_FILE_CHARS {
            report.skipped += 1;
            continue;
        }
        let (kb, book) = kb_and_book(&libs, &path);
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| book.clone());
        let uri = path.to_string_lossy().to_string();
        let input = PerceptInput {
            source_kind: "library".to_string(),
            source_id: uri.clone(),
            uri: uri.clone(),
            title,
            text: text.chars().take(MAX_TEXT_CHARS).collect(),
            kb_name: kb.clone(),
            trust: Some(0.9),
            meta: Some(json!({ "kb": kb, "book": book, "mtime": mtime, "path": uri })),
        };
        match percept::ingest_owned(input, "linked") {
            Ok(r) => {
                if r.duplicate {
                    report.duplicates += 1;
                } else {
                    report.ingested += 1;
                }
            }
            Err(e) => {
                report.skipped += 1;
                debug!(
                    "[cognition] 资料库摘要入库跳过 path={}: {}",
                    path.display(),
                    e
                );
            }
        }
    }

    if newest > cursor {
        store::set_setting(CURSOR_KEY, &newest.to_string())?;
    }
    report.cursor = newest;
    if report.ingested > 0 || report.scanned > 0 {
        info!(
            "[cognition] 资料库增量扫描：扫描 {}，入库 {}，重复 {}，跳过 {}",
            report.scanned, report.ingested, report.duplicates, report.skipped
        );
    }
    Ok(report)
}
