//! 内嵌 PDF 阅读器后端：把"需要性能"的部分（解析、栅格化、文本/坐标提取、全文搜索）放在 Rust。
//!
//! 前端（GuaxiPdfViewer.vue / GuaxiLatexPreview.vue）负责连续滚动的虚拟化、文本层叠加、
//! 搜索高亮的"展示"，真正重的活全部走这里的命令：
//!   - `pdf_open` / `pdf_session_open`  打开 PDF，返回总页数 + 每页尺寸（pt），供前端撑出占位做虚拟滚动。
//!   - `pdf_render_page`  按目标 DPI 栅格化单页 -> base64 PNG（缩放时真重渲染，而非 CSS 拉伸，保证清晰）。
//!   - `pdf_page_text`    提取单页"段"级文本及其 bbox（pt，左上原点）。
//!   - `pdf_page_chars`   提取单页"字符"级文本框（pt，左上原点）-> 前端按行分组 + scaleX 铺透明文本层。
//!   - `pdf_outline`      提取书签大纲（标题/层级/目标页）。
//!   - `pdf_locate_point` 给定页内点，返回包裹该点所在整行文本的 bbox（用于 typst 正向定位只有点的情况）。
//!   - `pdf_page_render_hash` 每页内容指纹（便宜、不栅格化）-> 前端做增量渲染 diff。
//!   - `pdf_search`       全文搜索，返回每个命中所在页与矩形（pt，左上原点）。
//!
//! 坐标约定：本模块对外统一用"左上角为原点、单位 pt（1/72 inch）"的坐标系，
//! 与前端 CSS（top-left）以及 SyncTeX 对齐；pdfium 内部用左下原点，这里做翻转。
//!
//! 线程模型：pdfium 既非线程安全、又要求所有调用来自同一线程，且 `PdfDocument` 借用 `Pdfium`
//! （`!Send`/`!Sync`）。因此这里用一个**专属 pdfium 工作线程（actor）**：进程内唯一 `Pdfium`
//! 经 `Box::leak` 提升为 `&'static`，工作线程独占持有它和一个按 `path#mtime` 命中的文档 LRU 缓存。
//! 各 Tauri 命令把 `Send` 闭包投递到该线程、用 tokio oneshot 取回结果。这样既消除了"每次命令重开
//! 整份文件"的开销（5000 页时的主要瓶颈），又满足 pdfium 的线程亲和性，且不需要 `unsafe`。

use base64::engine::general_purpose;
use base64::Engine as _;
use image::ImageFormat;
use pdfium_render::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::SystemTime;
use tracing::{error, info};

/// pdfium 动态库位置：与 docprocess.rs 保持一致（debug 在仓库 libs，release 在可执行目录 libs）。
const PDFIUM_LIB_PATH: &str = if cfg!(debug_assertions) {
    "../../libs"
} else {
    "./libs"
};

/// 文档缓存上限：常见场景同时打开 1~2 份（阅读器 + 预览），3 份足够，超出按 LRU 淘汰。
const DOC_CACHE_CAP: usize = 3;

// ───────────────────────── pdfium 工作线程（actor）─────────────────────────

/// 投递给工作线程的任务：在工作线程上拿到可变的 `PdfWorkerState` 后执行。
type PdfJob = Box<dyn FnOnce(&mut PdfWorkerState) + Send>;

static PDF_WORKER: OnceLock<Mutex<mpsc::Sender<PdfJob>>> = OnceLock::new();

struct CacheEntry {
    key: String,
    mtime: SystemTime,
    doc: PdfDocument<'static>,
}

/// 工作线程独占的状态：唯一的 pdfium 实例（leak 成 'static）+ 文档 LRU。
struct PdfWorkerState {
    pdfium: Option<&'static Pdfium>,
    cache: Vec<CacheEntry>,
}

impl PdfWorkerState {
    /// 拿到（必要时按 mtime 重载并缓存）指定路径的文档，交给闭包使用。命中页移到 LRU 队首。
    fn with_doc<R>(
        &mut self,
        path: &str,
        f: impl FnOnce(&PdfDocument<'static>) -> Result<R, String>,
    ) -> Result<R, String> {
        let pdfium = self
            .pdfium
            .ok_or_else(|| "PDFium 初始化失败：未能加载动态库".to_string())?;

        let mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        if let Some(pos) = self.cache.iter().position(|e| e.key == path) {
            if self.cache[pos].mtime == mtime {
                if pos != 0 {
                    let entry = self.cache.remove(pos);
                    self.cache.insert(0, entry);
                }
                return f(&self.cache[0].doc);
            }
            // mtime 变了：丢弃旧文档，下面重载。
            self.cache.remove(pos);
        }

        let doc = pdfium
            .load_pdf_from_file(path, None)
            .map_err(|e| format!("PDF 加载失败: {e}"))?;
        self.cache.insert(
            0,
            CacheEntry {
                key: path.to_string(),
                mtime,
                doc,
            },
        );
        if self.cache.len() > DOC_CACHE_CAP {
            self.cache.truncate(DOC_CACHE_CAP);
        }
        f(&self.cache[0].doc)
    }

    fn invalidate(&mut self, path: &str) {
        self.cache.retain(|e| e.key != path);
    }
}

fn init_worker() -> Mutex<mpsc::Sender<PdfJob>> {
    let (tx, rx) = mpsc::channel::<PdfJob>();
    std::thread::Builder::new()
        .name("pdfium-worker".into())
        .spawn(move || {
            // 绑定 pdfium 一次；leak 成 'static，整个进程生命周期内复用。
            let pdfium: Option<&'static Pdfium> = match Pdfium::bind_to_library(
                Pdfium::pdfium_platform_library_name_at_path(PDFIUM_LIB_PATH),
            ) {
                Ok(bindings) => Some(&*Box::leak(Box::new(Pdfium::new(bindings)))),
                Err(e) => {
                    error!(error = %e, "pdfium 绑定失败，PDF 命令将全部返回错误");
                    None
                }
            };
            let mut state = PdfWorkerState {
                pdfium,
                cache: Vec::new(),
            };
            while let Ok(job) = rx.recv() {
                job(&mut state);
            }
        })
        .expect("无法启动 pdfium 工作线程");
    Mutex::new(tx)
}

/// 在 pdfium 工作线程上执行闭包并异步取回结果。
async fn run_pdf<R, F>(f: F) -> Result<R, String>
where
    R: Send + 'static,
    F: FnOnce(&mut PdfWorkerState) -> Result<R, String> + Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<R, String>>();
    let job: PdfJob = Box::new(move |state| {
        let _ = tx.send(f(state));
    });
    {
        let sender = PDF_WORKER
            .get_or_init(init_worker)
            .lock()
            .map_err(|_| "PDF worker 锁失败".to_string())?;
        sender
            .send(job)
            .map_err(|_| "PDF worker 线程不可用".to_string())?;
    }
    rx.await.map_err(|_| "PDF worker 无响应".to_string())?
}

// ───────────────────────── pdf_open / pdf_session_open ─────────────────────────

#[derive(Debug, Serialize)]
pub struct PdfPageSize {
    /// 页宽（pt）
    pub w_pt: f32,
    /// 页高（pt）
    pub h_pt: f32,
}

#[derive(Debug, Serialize)]
pub struct PdfOpenResponse {
    pub total_pages: usize,
    pub pages: Vec<PdfPageSize>,
}

fn read_page_sizes(document: &PdfDocument) -> PdfOpenResponse {
    let mut pages = Vec::new();
    for page in document.pages().iter() {
        pages.push(PdfPageSize {
            w_pt: page.width().value,
            h_pt: page.height().value,
        });
    }
    let total_pages = pages.len();
    PdfOpenResponse { total_pages, pages }
}

/// 打开 PDF：返回总页数和每页尺寸（pt）。前端据此撑出每页占位，做连续滚动的虚拟化。
#[tauri::command]
pub async fn pdf_open(path: String) -> Result<PdfOpenResponse, String> {
    run_pdf(move |state| {
        let resp = state.with_doc(&path, |doc| Ok(read_page_sizes(doc)))?;
        info!(path = %path, total_pages = resp.total_pages, "pdf_open");
        Ok(resp)
    })
    .await
}

/// 预热缓存版的 `pdf_open`：语义/返回形状与 `pdf_open` 一致，前端打开文档时调用以提前载入。
#[tauri::command]
pub async fn pdf_session_open(path: String) -> Result<PdfOpenResponse, String> {
    pdf_open(path).await
}

/// 关闭会话：从缓存中剔除该文档，释放内存。
#[tauri::command]
pub async fn pdf_session_close(path: String) -> Result<(), String> {
    run_pdf(move |state| {
        state.invalidate(&path);
        Ok(())
    })
    .await
}

// ─────────────────────── pdf_render_page ───────────────────────

#[derive(Debug, Deserialize)]
pub struct PdfRenderPageRequest {
    pub path: String,
    /// 1-based 页码
    pub page: usize,
    /// 目标渲染 DPI（默认 144 ≈ 2x；缩放时由前端按 zoom 提升，后端真重渲染）。
    #[serde(default)]
    pub dpi: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct PdfRenderPageResponse {
    pub page: usize,
    /// 渲染位图像素宽高（前端按此设 <img> 的 CSS 尺寸 = px / (dpi/72) 得到 pt 显示尺寸）。
    pub width_px: u32,
    pub height_px: u32,
    /// 实际使用的 dpi（回传，方便前端换算 pt<->px）。
    pub dpi: f32,
    pub image_data: String,
}

/// 把指定页按目标 DPI 栅格化为 PNG（base64）。pdfium 正确渲染 CJK 等内嵌字体。
#[tauri::command]
pub async fn pdf_render_page(
    request: PdfRenderPageRequest,
) -> Result<PdfRenderPageResponse, String> {
    let dpi = request.dpi.unwrap_or(144.0).clamp(36.0, 600.0);
    let path = request.path.clone();
    let page_num = request.page;

    run_pdf(move |state| {
        state.with_doc(&path, |document| {
            let total = document.pages().len() as usize;
            if page_num < 1 || page_num > total {
                return Err(format!("页码 {} 超出范围(总页数: {})", page_num, total));
            }

            let page = document
                .pages()
                .get((page_num - 1) as u16)
                .map_err(|_| format!("获取页面 {} 失败", page_num))?;

            // 目标像素宽 = 页宽(pt) * dpi / 72
            let scale = dpi / 72.0;
            let target_w = (page.width().value * scale).round().max(1.0) as i32;
            let target_h = (page.height().value * scale).round().max(1.0) as i32;

            let config = PdfRenderConfig::new()
                .set_target_width(target_w)
                .set_target_height(target_h);

            let bitmap = page
                .render_with_config(&config)
                .map_err(|e| format!("页面渲染失败: {e}"))?;

            let dyn_image = bitmap.as_image();
            let width_px = dyn_image.width();
            let height_px = dyn_image.height();

            let mut bytes = Vec::new();
            dyn_image
                .into_rgb8()
                .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
                .map_err(|e| format!("图片编码失败: {e}"))?;
            let image_data = general_purpose::STANDARD.encode(&bytes);

            Ok(PdfRenderPageResponse {
                page: page_num,
                width_px,
                height_px,
                dpi,
                image_data,
            })
        })
    })
    .await
}

// ──────────────────────── pdf_page_text（段级）────────────────────────

/// 文本片段：text + 左上原点 bbox（pt）。
#[derive(Debug, Serialize)]
pub struct PdfTextSegment {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Serialize)]
pub struct PdfPageTextResponse {
    pub page: usize,
    pub w_pt: f32,
    pub h_pt: f32,
    pub segments: Vec<PdfTextSegment>,
}

/// 提取单页文本，按"段"（pdfium segment）给出文字与其 bbox（pt，左上原点）。
#[tauri::command]
pub async fn pdf_page_text(path: String, page: usize) -> Result<PdfPageTextResponse, String> {
    run_pdf(move |state| {
        state.with_doc(&path, |document| {
            let total = document.pages().len() as usize;
            if page < 1 || page > total {
                return Err(format!("页码 {} 超出范围(总页数: {})", page, total));
            }

            let pdf_page = document
                .pages()
                .get((page - 1) as u16)
                .map_err(|_| format!("获取页面 {} 失败", page))?;

            let page_h = pdf_page.height().value;
            let page_w = pdf_page.width().value;

            let mut segments = Vec::new();
            if let Ok(text_page) = pdf_page.text() {
                for segment in text_page.segments().iter() {
                    let s = segment.text();
                    if s.trim().is_empty() {
                        continue;
                    }
                    let rect = segment.bounds();
                    // pdfium：左下原点，bottom < top。转左上原点：y_top = page_h - top。
                    let left = rect.left().value;
                    let right = rect.right().value;
                    let top = rect.top().value;
                    let bottom = rect.bottom().value;
                    segments.push(PdfTextSegment {
                        text: s,
                        x: left,
                        y: page_h - top,
                        w: (right - left).max(0.0),
                        h: (top - bottom).max(0.0),
                    });
                }
            }

            Ok(PdfPageTextResponse {
                page,
                w_pt: page_w,
                h_pt: page_h,
                segments,
            })
        })
    })
    .await
}

// ──────────────────────── pdf_page_chars（字符级）────────────────────────

/// 单个字符：unicode + 左上原点 tight bbox（pt）+ 字号（pt）。前端据此按行分组铺文本层。
#[derive(Debug, Serialize)]
pub struct PdfChar {
    pub c: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub font_size: f32,
}

#[derive(Debug, Serialize)]
pub struct PdfPageCharsResponse {
    pub page: usize,
    pub w_pt: f32,
    pub h_pt: f32,
    pub chars: Vec<PdfChar>,
}

/// 提取单页"字符级"文本框。空白/控制字符不返回（前端按水平间距还原空格）。
#[tauri::command]
pub async fn pdf_page_chars(path: String, page: usize) -> Result<PdfPageCharsResponse, String> {
    run_pdf(move |state| {
        state.with_doc(&path, |document| {
            let total = document.pages().len() as usize;
            if page < 1 || page > total {
                return Err(format!("页码 {} 超出范围(总页数: {})", page, total));
            }

            let pdf_page = document
                .pages()
                .get((page - 1) as u16)
                .map_err(|_| format!("获取页面 {} 失败", page))?;

            let page_h = pdf_page.height().value;
            let page_w = pdf_page.width().value;

            let mut chars = Vec::new();
            if let Ok(text_page) = pdf_page.text() {
                for ch in text_page.chars().iter() {
                    let c = match ch.unicode_char() {
                        Some(c) if !c.is_whitespace() && !c.is_control() => c,
                        _ => continue,
                    };
                    let rect = match ch.tight_bounds() {
                        Ok(r) => r,
                        Err(_) => continue,
                    };
                    let left = rect.left().value;
                    let right = rect.right().value;
                    let top = rect.top().value;
                    let bottom = rect.bottom().value;
                    let w = (right - left).max(0.0);
                    let h = (top - bottom).max(0.0);
                    if w <= 0.0 || h <= 0.0 {
                        continue;
                    }
                    chars.push(PdfChar {
                        c: c.to_string(),
                        x: left,
                        y: page_h - top,
                        w,
                        h,
                        font_size: ch.unscaled_font_size().value,
                    });
                }
            }

            Ok(PdfPageCharsResponse {
                page,
                w_pt: page_w,
                h_pt: page_h,
                chars,
            })
        })
    })
    .await
}

// ──────────────────────── pdf_outline（书签大纲）────────────────────────

#[derive(Debug, Serialize)]
pub struct PdfOutlineItem {
    /// 1-based 层级（顶层 = 1）。
    pub level: u32,
    pub title: String,
    /// 目标 1-based 页码；解析不到则为 0。
    pub page: usize,
    /// 目标 y（pt，左上原点）；当前实现暂为 0（跳到页顶）。
    pub y_pt: f32,
}

fn walk_bookmark(bm: &PdfBookmark, level: u32, out: &mut Vec<PdfOutlineItem>) {
    if let Some(title) = bm.title() {
        let page = bm
            .destination()
            .and_then(|d| d.page_index().ok())
            .map(|i| i as usize + 1)
            .unwrap_or(0);
        out.push(PdfOutlineItem {
            level,
            title,
            page,
            y_pt: 0.0,
        });
    }
    let mut child = bm.first_child();
    while let Some(c) = child {
        walk_bookmark(&c, level + 1, out);
        child = c.next_sibling();
    }
}

/// 提取书签大纲：层级 + 标题 + 目标页。无书签返回空表。
#[tauri::command]
pub async fn pdf_outline(path: String) -> Result<Vec<PdfOutlineItem>, String> {
    run_pdf(move |state| {
        state.with_doc(&path, |document| {
            let mut out = Vec::new();
            // root() 返回首个顶层书签；其余顶层书签是它的 next_sibling。
            let mut node = document.bookmarks().root();
            while let Some(n) = node {
                walk_bookmark(&n, 1, &mut out);
                node = n.next_sibling();
            }
            Ok(out)
        })
    })
    .await
}

// ──────────────────────── pdf_locate_point ────────────────────────

#[derive(Debug, Serialize)]
pub struct PdfLocateResponse {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub found: bool,
}

/// 给定页内点（pt，左上原点），返回包裹该点所在整行文本的 bbox。
/// 用于 typst 正向定位只返回点、需要给出完整高亮框的场景，以及字/词级定位。
#[tauri::command]
pub async fn pdf_locate_point(
    path: String,
    page: usize,
    x: f32,
    y: f32,
) -> Result<PdfLocateResponse, String> {
    run_pdf(move |state| {
        state.with_doc(&path, |document| {
            let total = document.pages().len() as usize;
            if page < 1 || page > total {
                return Err(format!("页码 {} 超出范围(总页数: {})", page, total));
            }
            let pdf_page = document
                .pages()
                .get((page - 1) as u16)
                .map_err(|_| format!("获取页面 {} 失败", page))?;
            let page_h = pdf_page.height().value;

            // 收集字框（左上原点 pt）。
            let mut boxes: Vec<(f32, f32, f32, f32)> = Vec::new();
            if let Ok(text_page) = pdf_page.text() {
                for ch in text_page.chars().iter() {
                    if let Ok(r) = ch.tight_bounds() {
                        let left = r.left().value;
                        let right = r.right().value;
                        let top = r.top().value;
                        let bottom = r.bottom().value;
                        let w = (right - left).max(0.0);
                        let h = (top - bottom).max(0.0);
                        if w > 0.0 && h > 0.0 {
                            boxes.push((left, page_h - top, w, h));
                        }
                    }
                }
            }

            if boxes.is_empty() {
                return Ok(PdfLocateResponse {
                    page,
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                    found: false,
                });
            }

            // 命中：优先包含该点的字框；否则取中心最近者。
            let mut hit = boxes.iter().cloned().find(|(bx, by, bw, bh)| {
                x >= bx - 1.0 && x <= bx + bw + 1.0 && y >= by - 1.0 && y <= by + bh + 1.0
            });
            if hit.is_none() {
                let mut best = f32::MAX;
                for b in &boxes {
                    let cx = b.0 + b.2 / 2.0;
                    let cy = b.1 + b.3 / 2.0;
                    let d = (cx - x).powi(2) + (cy - y).powi(2);
                    if d < best {
                        best = d;
                        hit = Some(*b);
                    }
                }
            }

            let (hx, hy, hw, hh) = hit.unwrap();
            // 扩展到整行：垂直中心与命中字接近的字框并入。
            let hcy = hy + hh / 2.0;
            let tol = (hh * 0.6).max(2.0);
            let mut minx = hx;
            let mut maxx = hx + hw;
            let mut miny = hy;
            let mut maxy = hy + hh;
            for b in &boxes {
                let cy = b.1 + b.3 / 2.0;
                if (cy - hcy).abs() <= tol {
                    minx = minx.min(b.0);
                    maxx = maxx.max(b.0 + b.2);
                    miny = miny.min(b.1);
                    maxy = maxy.max(b.1 + b.3);
                }
            }

            Ok(PdfLocateResponse {
                page,
                x: minx,
                y: miny,
                w: (maxx - minx).max(0.0),
                h: (maxy - miny).max(0.0),
                found: true,
            })
        })
    })
    .await
}

// ──────────────────────── pdf_page_render_hash（增量渲染指纹）────────────────────────

/// 每页内容指纹：hash(页尺寸 + 各字符 unicode + 紧框几何)。便宜、不栅格化。
/// 注：纯图形（无文本）改动检测不到；但对 latex/typst 这类以文本为主的增量重建足够，
/// 且偏"敏感"——误判为变化只是多渲一次，误判为未变才会留旧内容。
#[tauri::command]
pub async fn pdf_page_render_hash(path: String, page: usize) -> Result<String, String> {
    run_pdf(move |state| {
        state.with_doc(&path, |document| {
            let total = document.pages().len() as usize;
            if page < 1 || page > total {
                return Err(format!("页码 {} 超出范围(总页数: {})", page, total));
            }
            let pdf_page = document
                .pages()
                .get((page - 1) as u16)
                .map_err(|_| format!("获取页面 {} 失败", page))?;

            let mut hasher = Sha256::new();
            hasher.update(pdf_page.width().value.to_bits().to_le_bytes());
            hasher.update(pdf_page.height().value.to_bits().to_le_bytes());
            if let Ok(text_page) = pdf_page.text() {
                for ch in text_page.chars().iter() {
                    if let Some(c) = ch.unicode_char() {
                        let mut buf = [0u8; 4];
                        hasher.update(c.encode_utf8(&mut buf).as_bytes());
                    }
                    if let Ok(r) = ch.tight_bounds() {
                        for v in [
                            r.left().value,
                            r.top().value,
                            r.right().value,
                            r.bottom().value,
                        ] {
                            hasher.update(((v * 100.0).round() as i32).to_le_bytes());
                        }
                    }
                }
            }
            Ok(format!("{:x}", hasher.finalize()))
        })
    })
    .await
}

// ───────────────────────── pdf_search ─────────────────────────

#[derive(Debug, Serialize)]
pub struct PdfSearchRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Serialize)]
pub struct PdfSearchHit {
    pub page: usize,
    /// 命中片段在页内的矩形（pt，左上原点）。一个命中可能跨多行 -> 多个矩形。
    pub rects: Vec<PdfSearchRect>,
    /// 命中处上下文片段（便于在结果列表显示）。
    pub snippet: String,
}

#[derive(Debug, Serialize)]
pub struct PdfSearchResponse {
    pub query: String,
    pub total: usize,
    pub hits: Vec<PdfSearchHit>,
}

/// 全文搜索：返回每个命中所在页 + 命中矩形（pt，左上原点）+ 片段。大小写不敏感。
#[tauri::command]
pub async fn pdf_search(path: String, query: String) -> Result<PdfSearchResponse, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(PdfSearchResponse {
            query: q,
            total: 0,
            hits: vec![],
        });
    }

    run_pdf(move |state| {
        state.with_doc(&path, |document| {
            let mut hits = Vec::new();
            let options = PdfSearchOptions::new();

            for (idx, page) in document.pages().iter().enumerate() {
                let page_h = page.height().value;
                let text_page = match page.text() {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                let search = match text_page.search(&q, &options) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                // 逐个命中：find_next() 返回一组 segments（命中可能跨行）。
                while let Some(result_segments) = search.find_next() {
                    let mut rects = Vec::new();
                    let mut snippet = String::new();
                    for seg in result_segments.iter() {
                        let r = seg.bounds();
                        let left = r.left().value;
                        let right = r.right().value;
                        let top = r.top().value;
                        let bottom = r.bottom().value;
                        rects.push(PdfSearchRect {
                            x: left,
                            y: page_h - top,
                            w: (right - left).max(0.0),
                            h: (top - bottom).max(0.0),
                        });
                        snippet.push_str(&seg.text());
                    }
                    if rects.is_empty() {
                        continue;
                    }
                    hits.push(PdfSearchHit {
                        page: idx + 1,
                        rects,
                        snippet,
                    });
                }
            }

            Ok(PdfSearchResponse {
                query: q.clone(),
                total: hits.len(),
                hits,
            })
        })
    })
    .await
}

// ──────────────────────── 高亮批注（写回 .pdf）────────────────────────
//
// 持久化策略（用户选定"写回 .pdf 文件"）：用 pdfium 原生 Highlight 批注 + quadpoints 写入页面，
// 外部阅读器也能看到；同时把 {id, note, rects, color} 作为 JSON 元数据存进批注的 Contents 字段
// （带哨兵前缀），读回时无损还原 —— 既拿到稳定 id 做幂等删除，又拿到精确矩形给前端画覆盖层。

/// 批注 Contents 中高亮元数据的哨兵前缀。
const HL_MARKER: &str = "\u{1}GXH1:";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HighlightRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Deserialize)]
pub struct AddHighlightRequest {
    pub path: String,
    pub page: usize,
    pub rects: Vec<HighlightRect>,
    /// RGBA（0-255）；缺省给半透明黄。
    #[serde(default)]
    pub color: Option<[u8; 4]>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AddHighlightResponse {
    pub annot_id: String,
}

#[derive(Debug, Serialize)]
pub struct HighlightInfo {
    pub annot_id: String,
    pub page: usize,
    pub rects: Vec<HighlightRect>,
    pub color: [u8; 4],
    pub note: String,
}

const DEFAULT_HL_COLOR: [u8; 4] = [255, 235, 59, 90];

fn new_annot_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::UNIX_EPOCH;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("h{:x}{:x}", nanos, c)
}

fn build_marker(id: &str, note: &str, rects: &[HighlightRect], color: [u8; 4]) -> String {
    let meta = serde_json::json!({
        "v": 1, "id": id, "note": note, "rects": rects, "color": color,
    });
    format!("{}{}", HL_MARKER, meta)
}

fn parse_marker(contents: &str) -> Option<(String, String, Vec<HighlightRect>, [u8; 4])> {
    let idx = contents.find(HL_MARKER)?;
    let json = &contents[idx + HL_MARKER.len()..];
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let id = v.get("id")?.as_str()?.to_string();
    let note = v
        .get("note")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let rects = v
        .get("rects")
        .cloned()
        .and_then(|r| serde_json::from_value(r).ok())
        .unwrap_or_default();
    let color = v
        .get("color")
        .cloned()
        .and_then(|c| serde_json::from_value(c).ok())
        .unwrap_or(DEFAULT_HL_COLOR);
    Some((id, note, rects, color))
}

/// 把"左上原点 pt"的矩形转成 pdfium 页坐标（左下原点）下的 quadpoints。
fn rect_to_quad(rc: &HighlightRect, page_h: f32) -> PdfQuadPoints {
    let left = rc.x;
    let right = rc.x + rc.w;
    let top = page_h - rc.y;
    let bottom = page_h - (rc.y + rc.h);
    PdfQuadPoints::new_from_values(left, bottom, right, bottom, right, top, left, top)
}

/// 在指定页创建高亮批注并写回 .pdf；返回稳定 annot_id（供后续删除/定位）。
#[tauri::command]
pub async fn pdf_add_highlight(
    request: AddHighlightRequest,
) -> Result<AddHighlightResponse, String> {
    let AddHighlightRequest {
        path,
        page,
        rects,
        color,
        note,
    } = request;
    let color = color.unwrap_or(DEFAULT_HL_COLOR);
    let note = note.unwrap_or_default();

    run_pdf(move |state| {
        let annot_id = new_annot_id();
        state.with_doc(&path, |document| {
            let total = document.pages().len() as usize;
            if page < 1 || page > total {
                return Err(format!("页码 {} 超出范围(总页数: {})", page, total));
            }
            let mut pdf_page = document
                .pages()
                .get((page - 1) as u16)
                .map_err(|_| format!("获取页面 {} 失败", page))?;
            let page_h = pdf_page.height().value;
            {
                let mut hl = pdf_page
                    .annotations_mut()
                    .create_highlight_annotation()
                    .map_err(|e| format!("创建高亮失败: {e}"))?;
                let [r, g, b, a] = color;
                hl.set_fill_color(PdfColor::new(r, g, b, a))
                    .map_err(|e| format!("设置高亮颜色失败: {e}"))?;
                hl.set_contents(&build_marker(&annot_id, &note, &rects, color))
                    .map_err(|e| format!("写入高亮元数据失败: {e}"))?;
                for rc in &rects {
                    hl.attachment_points_mut()
                        .create_attachment_point_at_end(rect_to_quad(rc, page_h))
                        .map_err(|e| format!("设置高亮区域失败: {e}"))?;
                }
            }
            document
                .save_to_file(&path)
                .map_err(|e| format!("保存 PDF 失败: {e}"))?;
            Ok(())
        })?;
        // 文件已被改写，缓存 mtime 失效。
        state.invalidate(&path);
        Ok(AddHighlightResponse { annot_id })
    })
    .await
}

/// 删除指定页上 id 匹配的高亮并写回 .pdf。
#[tauri::command]
pub async fn pdf_delete_highlight(
    path: String,
    page: usize,
    annot_id: String,
) -> Result<(), String> {
    run_pdf(move |state| {
        state.with_doc(&path, |document| {
            let total = document.pages().len() as usize;
            if page < 1 || page > total {
                return Err(format!("页码 {} 超出范围(总页数: {})", page, total));
            }
            let mut pdf_page = document
                .pages()
                .get((page - 1) as u16)
                .map_err(|_| format!("获取页面 {} 失败", page))?;

            // 先找到匹配项的索引（只读扫描，借用在块内结束）。
            let mut target_idx: Option<usize> = None;
            {
                let anns = pdf_page.annotations();
                for i in 0..anns.len() {
                    if let Ok(a) = anns.get(i) {
                        if let Some(c) = a.contents() {
                            if let Some((id, ..)) = parse_marker(&c) {
                                if id == annot_id {
                                    target_idx = Some(i);
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            if let Some(i) = target_idx {
                // 取出该批注（temp 借用随表达式结束），交给 delete_annotation 消费。
                let a = pdf_page
                    .annotations()
                    .get(i)
                    .map_err(|_| "获取待删高亮失败".to_string())?;
                pdf_page
                    .annotations_mut()
                    .delete_annotation(a)
                    .map_err(|e| format!("删除高亮失败: {e}"))?;
                document
                    .save_to_file(&path)
                    .map_err(|e| format!("保存 PDF 失败: {e}"))?;
            }
            Ok(())
        })?;
        state.invalidate(&path);
        Ok(())
    })
    .await
}

/// 读取指定页所有"本应用创建的"高亮（带哨兵元数据），供前端画覆盖层。
#[tauri::command]
pub async fn pdf_page_highlights(path: String, page: usize) -> Result<Vec<HighlightInfo>, String> {
    run_pdf(move |state| {
        state.with_doc(&path, |document| {
            let total = document.pages().len() as usize;
            if page < 1 || page > total {
                return Err(format!("页码 {} 超出范围(总页数: {})", page, total));
            }
            let pdf_page = document
                .pages()
                .get((page - 1) as u16)
                .map_err(|_| format!("获取页面 {} 失败", page))?;

            let mut out = Vec::new();
            let anns = pdf_page.annotations();
            for i in 0..anns.len() {
                if let Ok(a) = anns.get(i) {
                    if a.as_highlight_annotation().is_none() {
                        continue;
                    }
                    if let Some(c) = a.contents() {
                        if let Some((id, note, rects, color)) = parse_marker(&c) {
                            out.push(HighlightInfo {
                                annot_id: id,
                                page,
                                rects,
                                color,
                                note,
                            });
                        }
                    }
                }
            }
            Ok(out)
        })
    })
    .await
}

/// 保存文档（默认覆盖原路径，可另存到 dest_path）。
#[tauri::command]
pub async fn pdf_save(path: String, dest_path: Option<String>) -> Result<(), String> {
    run_pdf(move |state| {
        let target = dest_path.clone().unwrap_or_else(|| path.clone());
        state.with_doc(&path, |document| {
            document
                .save_to_file(&target)
                .map_err(|e| format!("保存 PDF 失败: {e}"))
        })?;
        state.invalidate(&path);
        Ok(())
    })
    .await
}
