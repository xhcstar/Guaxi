//! 内嵌 PDF 阅读器后端：把"需要性能"的部分（解析、栅格化、文本/坐标提取、全文搜索）放在 Rust。
//!
//! 前端（GuaxiLatexPreview.vue）负责连续滚动的虚拟化、文本层叠加、搜索高亮的"展示"，
//! 真正重的活全部走这里的命令：
//!   - `pdf_open`         打开 PDF，返回总页数 + 每页尺寸（pt），供前端先撑出占位高度做虚拟滚动。
//!   - `pdf_render_page`  按目标 DPI 栅格化单页 -> base64 PNG（缩放时真重渲染，而非 CSS 拉伸，保证清晰）。
//!   - `pdf_page_text`    提取单页文本片段及其 bbox（pt，左上原点）-> 前端铺透明文本层，支持选择/复制。
//!   - `pdf_search`       全文搜索，返回每个命中所在页与矩形（pt，左上原点）-> 前端高亮。
//!
//! 坐标约定：本模块对外统一用"左上角为原点、单位 pt（1/72 inch）"的坐标系，
//! 与前端 CSS（top-left）以及 SyncTeX 对齐；pdfium 内部用左下原点，这里做翻转。

use base64::engine::general_purpose;
use base64::Engine as _;
use image::ImageFormat;
use pdfium_render::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tracing::info;

/// pdfium 动态库位置：与 docprocess.rs 保持一致（debug 在仓库 libs，release 在可执行目录 libs）。
const PDFIUM_LIB_PATH: &str = if cfg!(debug_assertions) {
    "../../libs"
} else {
    "./libs"
};

fn bind_pdfium() -> Result<Pdfium, String> {
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
        PDFIUM_LIB_PATH,
    ))
    .map_err(|e| format!("PDFium 初始化失败: {e}"))?;
    Ok(Pdfium::new(bindings))
}

// ───────────────────────── pdf_open ─────────────────────────

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

/// 打开 PDF：返回总页数和每页尺寸（pt）。前端据此撑出每页占位，做连续滚动的虚拟化。
#[tauri::command]
pub async fn pdf_open(path: String) -> Result<PdfOpenResponse, String> {
    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(&path, None)
        .map_err(|e| format!("PDF 加载失败: {e}"))?;

    let mut pages = Vec::new();
    for page in document.pages().iter() {
        pages.push(PdfPageSize {
            w_pt: page.width().value,
            h_pt: page.height().value,
        });
    }
    let total_pages = pages.len();
    info!(path = %path, total_pages, "pdf_open");
    Ok(PdfOpenResponse { total_pages, pages })
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
pub async fn pdf_render_page(request: PdfRenderPageRequest) -> Result<PdfRenderPageResponse, String> {
    let dpi = request.dpi.unwrap_or(144.0).clamp(36.0, 600.0);
    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(&request.path, None)
        .map_err(|e| format!("PDF 加载失败: {e}"))?;

    let total = document.pages().len() as usize;
    if request.page < 1 || request.page > total {
        return Err(format!("页码 {} 超出范围(总页数: {})", request.page, total));
    }

    let page = document
        .pages()
        .get((request.page - 1) as u16)
        .map_err(|_| format!("获取页面 {} 失败", request.page))?;

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
        page: request.page,
        width_px,
        height_px,
        dpi,
        image_data,
    })
}

// ──────────────────────── pdf_page_text ────────────────────────

/// 文本片段：text + 左上原点 bbox（pt）。前端据此铺透明可选文本层。
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
    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(&path, None)
        .map_err(|e| format!("PDF 加载失败: {e}"))?;

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
            let x = left;
            let y = page_h - top;
            let w = (right - left).max(0.0);
            let h = (top - bottom).max(0.0);
            segments.push(PdfTextSegment {
                text: s,
                x,
                y,
                w,
                h,
            });
        }
    }

    Ok(PdfPageTextResponse {
        page,
        w_pt: page_w,
        h_pt: page_h,
        segments,
    })
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

    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(&path, None)
        .map_err(|e| format!("PDF 加载失败: {e}"))?;

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
        query: q,
        total: hits.len(),
        hits,
    })
}
