//! Typst 编译支持：把 .typ 源码编译成 PDF（纯 Rust，无外部依赖）。
//!
//! 设计说明：
//! 1. 与 LaTeX 预览（tectonic）对齐，提供两个命令：
//!    - `build_typ_pdf_preview(source_path, content)`：编译编辑器当前内容到临时 PDF，返回路径，供实时预览。
//!    - `export_typ_pdf(input_path, output_path)`：从磁盘源文件编译并写到目标路径，供导出。
//! 2. 字体来自系统（`include_system_fonts(true)`）；不内嵌、不联网下载包。中文需用户系统装有中文字体。
//! 3. 字体扫描较慢，全局缓存一次（`FONTS`），后续预览复用。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::{Datelike, Local, Timelike};
use once_cell::sync::Lazy;
use serde::Serialize;
use tracing::{info, warn};

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime};
use typst::layout::{Abs, PagedDocument, Point};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_ide::{jump_from_click, jump_from_cursor, IdeWorld, Jump};
use typst_kit::fonts::{FontSearcher, FontSlot};

/// 全局标准库（所有编译共享，构建一次）。
static LIBRARY: Lazy<LazyHash<Library>> = Lazy::new(|| LazyHash::new(Library::default()));

/// 全局字体（扫描系统字体一次后缓存）。
struct GlobalFonts {
    book: LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
}

static FONTS: Lazy<GlobalFonts> = Lazy::new(|| {
    // include_embedded_fonts：内嵌 Typst 默认字体（正文 LibertinusSerif / 数学 NewCMMath / 代码 DejaVuSansMono），
    // 不依赖系统是否装有这些字体，修复数学公式等渲染异常。系统字体仍加载，供中文等使用。
    let fonts = FontSearcher::new()
        .include_system_fonts(true)
        .include_embedded_fonts(true)
        .search();
    info!(count = fonts.fonts.len(), "[Typst] fonts loaded (system + embedded)");
    GlobalFonts {
        book: LazyHash::new(fonts.book),
        fonts: fonts.fonts,
    }
});

/// 一次编译所用的 World：主源码在内存，其余文件（import / 图片）按相对路径从磁盘读。
struct GuaxiTypstWorld {
    root: PathBuf,
    main_id: FileId,
    main_source: Source,
    now: Datetime,
}

impl GuaxiTypstWorld {
    fn new(root: PathBuf, content: String) -> Self {
        // 主文件在 World 内用一个固定的虚拟路径，真实相对依赖以 `root` 为根解析。
        let main_id = FileId::new(None, VirtualPath::new("__guaxi_main__.typ"));
        let main_source = Source::new(main_id, content);
        let now = local_now_datetime();
        Self {
            root,
            main_id,
            main_source,
            now,
        }
    }

    /// 把 FileId 解析为磁盘真实路径。包引用先经本地缓存/镜像下载定位，再在包目录内解析虚拟路径。
    ///
    /// 为什么不用 typst-kit 的 `PackageStorage`：它把 registry 写死成常量，无法换镜像，
    /// 也不支持显式代理（只读环境变量）。这里改走 `typst_package::ensure_package`，
    /// 镜像前缀与代理都能配置，缓存布局与官方一致，已下载过的包不会重复联网。
    fn id_to_path(&self, id: FileId) -> FileResult<PathBuf> {
        let root = if let Some(spec) = id.package() {
            match crate::commands::typst_package::ensure_package(
                &spec.namespace,
                &spec.name,
                &spec.version.to_string(),
                |_url| {},
            ) {
                Ok(dir) => dir,
                // 用 NetworkFailed 而不是 Other：前端拿到的诊断里会明确提示「网络/下载」，
                // 用户配了代理或换镜像后能立刻对上号。
                Err(e) => {
                    return Err(FileError::Package(
                        typst::diag::PackageError::NetworkFailed(Some(e.into())),
                    ))
                }
            }
        } else {
            self.root.clone()
        };
        id.vpath()
            .resolve(&root)
            .ok_or_else(|| FileError::NotFound(id.vpath().as_rootless_path().to_path_buf()))
    }
}

impl World for GuaxiTypstWorld {
    fn library(&self) -> &LazyHash<Library> {
        &LIBRARY
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &FONTS.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            return Ok(self.main_source.clone());
        }
        let path = self.id_to_path(id)?;
        let text = std::fs::read_to_string(&path)
            .map_err(|err| FileError::from_io(err, &path))?;
        Ok(Source::new(id, text))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = self.id_to_path(id)?;
        let data = std::fs::read(&path).map_err(|err| FileError::from_io(err, &path))?;
        Ok(Bytes::new(data))
    }

    fn font(&self, index: usize) -> Option<Font> {
        FONTS.fonts.get(index)?.get()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        Some(self.now)
    }
}

impl IdeWorld for GuaxiTypstWorld {
    fn upcast(&self) -> &dyn World {
        self
    }
}

fn local_now_datetime() -> Datetime {
    let now = Local::now();
    Datetime::from_ymd_hms(
        now.year(),
        now.month() as u8,
        now.day() as u8,
        now.hour() as u8,
        now.minute() as u8,
        now.second() as u8,
    )
    .unwrap_or_else(|| Datetime::from_ymd(2026, 1, 1).expect("valid fallback date"))
}

/// 把诊断信息格式化成可读字符串（带行号），供前端预览面板展示。
fn format_diagnostics(world: &GuaxiTypstWorld, diags: &[SourceDiagnostic]) -> String {
    let mut out = String::new();
    for diag in diags {
        let line_info = diag
            .span
            .id()
            .and_then(|id| world.source(id).ok())
            .and_then(|src| {
                let range = src.range(diag.span)?;
                let line = src.lines().byte_to_line(range.start)? + 1;
                let col = src.lines().byte_to_column(range.start)? + 1;
                Some(format!("{}:{}: ", line, col))
            })
            .unwrap_or_default();
        out.push_str(&format!("error: {}{}\n", line_info, diag.message));
        for hint in &diag.hints {
            out.push_str(&format!("  hint: {}\n", hint));
        }
    }
    if out.is_empty() {
        out.push_str("Typst 编译失败（无详细诊断）");
    }
    out
}

/// 把内容编译为 PDF 字节。
fn compile_typ_to_pdf(root: PathBuf, content: String) -> Result<Vec<u8>, String> {
    let world = GuaxiTypstWorld::new(root, content);
    let result = typst::compile::<PagedDocument>(&world);
    let document = result
        .output
        .map_err(|diags| format_diagnostics(&world, &diags))?;
    let pdf = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
        .map_err(|diags| format_diagnostics(&world, &diags))?;
    Ok(pdf)
}

/// 把一段 Typst 源码直接编译成 PDF 字节，不落盘、不缓存。
///
/// 给「邮件导出为 PDF」这类一次性转换用：它们不关心增量预览与双向跳转，
/// 只需要一份结果字节，所以不该复用它旁边的预览缓存（那会污染 jump 缓存）。
pub fn compile_typ_source_to_pdf(root: std::path::PathBuf, content: String) -> Result<Vec<u8>, String> {
    compile_typ_to_pdf(root, content)
}

/// 编译并同时缓存 World + 文档，供双向跳转（typst-ide）复用。返回 PDF 字节。
///
/// 为什么缓存：jump_from_cursor / jump_from_click 需要"刚编译出来的那份"World 与 PagedDocument，
/// 而预览编译已经把它们算出来了，丢掉再重算既慢又可能因内容已改而对不上。
/// 以 source_path 为 key 缓存最近一次预览编译的产物，跳转时直接取用。
fn compile_typ_and_cache(
    source_path: &str,
    root: PathBuf,
    content: String,
) -> Result<Vec<u8>, String> {
    let world = GuaxiTypstWorld::new(root, content);
    let result = typst::compile::<PagedDocument>(&world);
    let document = result
        .output
        .map_err(|diags| format_diagnostics(&world, &diags))?;
    let pdf = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
        .map_err(|diags| format_diagnostics(&world, &diags))?;

    let entry = TypstJumpEntry {
        world: Arc::new(world),
        document: Arc::new(document),
    };
    JUMP_CACHE
        .lock()
        .unwrap()
        .insert(source_path.to_string(), entry);

    Ok(pdf)
}

/// 跳转缓存条目：最近一次成功预览编译的 World + 文档。
#[derive(Clone)]
struct TypstJumpEntry {
    world: Arc<GuaxiTypstWorld>,
    document: Arc<PagedDocument>,
}

/// source_path -> 最近一次编译产物。
static JUMP_CACHE: Lazy<Mutex<HashMap<String, TypstJumpEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// 文件关闭时丢弃该文档的跳转缓存。
/// 条目里常驻着编译用的 World 与整份 PagedDocument，大文档下内存占用可观，
/// 标签关掉后没有理由再留着（下次预览会重新编译并重新填充）。
pub fn forget_typst_jump_cache(source_path: &str) {
    if let Ok(mut cache) = JUMP_CACHE.lock() {
        cache.remove(source_path);
    }
}

#[derive(Debug, Serialize)]
pub struct TypstForwardResult {
    pub page: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Serialize)]
pub struct TypstInverseResult {
    pub line: u32,
    pub column: u32,
}

/// 正向：源码 (line, column) -> PDF 位置（page + 左上原点坐标 pt）。
/// 行列均 1-based（与编辑器一致），内部转 0-based。
#[tauri::command]
pub async fn typst_forward_search(
    source_path: String,
    line: u32,
    column: u32,
) -> Result<Option<TypstForwardResult>, String> {
    let entry = {
        let cache = JUMP_CACHE.lock().unwrap();
        cache.get(&source_path).cloned()
    };
    let entry = match entry {
        Some(e) => e,
        None => return Ok(None), // 还没编译过：前端会先触发一次预览构建
    };

    let source = entry.world.main_source.clone();
    let line0 = line.saturating_sub(1) as usize;
    let col0 = column.saturating_sub(1) as usize;
    let cursor = match source.lines().line_column_to_byte(line0, col0) {
        Some(b) => b,
        None => source
            .lines()
            .line_to_byte(line0)
            .unwrap_or(0),
    };

    let positions = jump_from_cursor(&entry.document, &source, cursor);
    Ok(positions.first().map(|pos| TypstForwardResult {
        page: pos.page.get() as u32,
        x: pos.point.x.to_pt() as f32,
        y: pos.point.y.to_pt() as f32,
    }))
}

/// 反向：PDF 点击 (page, x, y)（pt，左上原点）-> 源码 (line, column)，均 1-based。
#[tauri::command]
pub async fn typst_inverse_search(
    source_path: String,
    page: u32,
    x: f32,
    y: f32,
) -> Result<Option<TypstInverseResult>, String> {
    let entry = {
        let cache = JUMP_CACHE.lock().unwrap();
        cache.get(&source_path).cloned()
    };
    let entry = match entry {
        Some(e) => e,
        None => return Ok(None),
    };

    let page_idx = page.saturating_sub(1) as usize;
    let pdf_page = match entry.document.pages.get(page_idx) {
        Some(p) => p,
        None => return Ok(None),
    };

    let click = Point::new(Abs::pt(x as f64), Abs::pt(y as f64));
    let jump = jump_from_click(
        entry.world.as_ref(),
        &entry.document,
        &pdf_page.frame,
        click,
    );

    let Some(Jump::File(id, offset)) = jump else {
        return Ok(None);
    };

    // 命中的应是主源文件（编辑器当前内容）。
    let source = if id == entry.world.main_id {
        entry.world.main_source.clone()
    } else {
        match entry.world.source(id) {
            Ok(s) => s,
            Err(_) => return Ok(None),
        }
    };

    let lines = source.lines();
    let line0 = lines.byte_to_line(offset).unwrap_or(0);
    let col0 = lines.byte_to_column(offset).unwrap_or(0);

    Ok(Some(TypstInverseResult {
        line: (line0 + 1) as u32,
        column: (col0 + 1) as u32,
    }))
}

fn input_dir_of(source_path: &str) -> Result<(PathBuf, PathBuf), String> {
    let source = PathBuf::from(source_path.trim());
    let input_dir = source
        .parent()
        .map(|p| p.to_path_buf())
        .filter(|p| p.is_dir())
        .ok_or_else(|| "无法定位 .typ 文件所在目录".to_string())?;
    Ok((source, input_dir))
}

/// 为 Typst 实时预览构建 PDF：把编辑器内容编译为临时 PDF，返回路径。
///
/// 输出路径**稳定**（同一份 .typ 每次重建都覆写同一个文件），这样 `GuaxiPdfViewer`
/// 才会走 `incrementalReconcile` 逐页增量渲染，而不是整篇冷重载。
#[tauri::command]
pub async fn build_typ_pdf_preview(source_path: String, content: String) -> Result<String, String> {
    let (source, input_dir) = input_dir_of(&source_path)?;
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("preview")
        .to_string();

    // 同一份文档的构建必须串行 —— 详见 preview_build_lock 的说明。
    // 锁一直持有到落盘完成：中间所有文件操作（建目录 / 写临时文件 / rename）都靠它排他，
    // 否则两个并发构建会写同一个临时文件，后一个 rename 找不到源文件而报 os error 2。
    let key = crate::commands::docprocess::preview_source_key(&source_path);
    let build_lock = crate::commands::docprocess::preview_build_lock(&key);
    let _build_guard = build_lock.lock().await;

    // Typst 编译是**同步 CPU 活**：直接放在 async fn 里会占住当前 tokio 工作线程直到编完，
    // 期间其它 IPC 命令只能排队（与 tectonic 那条是同类问题）。挪到阻塞线程池上执行。
    let pdf_bytes = tokio::task::spawn_blocking(move || {
        compile_typ_and_cache(&source_path, input_dir, content)
    })
    .await
    .map_err(|e| format!("Typst 预览编译任务异常: {e}"))??;

    let outdir = crate::commands::docprocess::preview_build_dir("typst", &key)?;
    let pdf_path = outdir.join(format!(".guaxi_preview_{stem}.pdf"));

    // 先写同目录临时文件再 rename：`fs::write` 会先截断目标文件，若瞬时被阅读器读到
    // 就是半个 PDF；rename 在同一文件系统内是原子的，稳定路径上永远只有完整文件。
    // 临时名带唯一后缀：上面那把锁已经保证串行，这里只是第二道防线。
    let staging = outdir.join(format!(
        ".guaxi_preview_{stem}.{}.pdf.tmp",
        crate::commands::docprocess::preview_staging_suffix()
    ));
    std::fs::write(&staging, &pdf_bytes).map_err(|e| format!("写入预览 PDF 失败: {e}"))?;
    std::fs::rename(&staging, &pdf_path).map_err(|e| format!("提交预览 PDF 失败: {e}"))?;

    // 稳定路径是覆写同一个文件，而 pdfium 的文档缓存只按 mtime 判断新鲜度，
    // 同一时间刻度内重建会命中旧缓存继续吐上一次的字节。显式失效。
    let pdf_path_str = pdf_path.to_string_lossy().to_string();
    if let Err(e) = crate::commands::pdf_reader::invalidate_pdf(&pdf_path_str).await {
        warn!(err = %e, "[TypstPreview] 失效 PDF 缓存失败，阅读器可能仍显示上一次的渲染结果");
    }

    Ok(pdf_path_str)
}

/// 导出：从磁盘源文件编译并写到目标路径。
#[tauri::command]
pub async fn export_typ_pdf(input_path: String, output_path: String) -> Result<(), String> {
    let input = PathBuf::from(input_path.trim());
    if !input.is_file() {
        return Err(format!("typ file not found: {}", input.to_string_lossy()));
    }
    let input_dir = input
        .parent()
        .ok_or_else(|| "Failed to get typ parent dir".to_string())?
        .to_path_buf();
    let content = std::fs::read_to_string(&input)
        .map_err(|e| format!("读取 .typ 源文件失败: {e}"))?;

    // 同上：同步 CPU 编译挪到阻塞线程池，别占住 tokio 工作线程。
    let pdf_bytes = tokio::task::spawn_blocking(move || compile_typ_to_pdf(input_dir, content))
        .await
        .map_err(|e| format!("Typst 导出编译任务异常: {e}"))??;

    let output = PathBuf::from(output_path.trim());
    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&output, &pdf_bytes)
        .map_err(|e| format!("写入导出 PDF 失败: {e} (path={})", output.to_string_lossy()))?;
    info!(to = %output.display(), "[ExportTyp] pdf exported");
    Ok(())
}

// ───────────────────────── 多格式导出（HTML / SVG / PNG 打包）

/// 把源码编译成 HTML 字符串。
///
/// Typst 0.13 起支持 `html` 目标：`compile::<HtmlDocument>` 走的是 HTML 语义化输出
/// （而不是把 PDF 页转成图片），所以这里的产物是可选中、可被浏览器重排的真正的 HTML。
fn compile_typ_to_html(root: PathBuf, content: String) -> Result<String, String> {
    let world = GuaxiTypstWorld::new(root, content);
    let result = typst::compile::<typst_html::HtmlDocument>(&world);
    let document = result
        .output
        .map_err(|diags| format_diagnostics(&world, &diags))?;
    typst_html::html(&document).map_err(|diags| format_diagnostics(&world, &diags))
}

/// 把每一页渲染成 SVG。
fn compile_typ_to_svgs(root: PathBuf, content: String) -> Result<Vec<String>, String> {
    let world = GuaxiTypstWorld::new(root, content);
    let result = typst::compile::<PagedDocument>(&world);
    let document = result
        .output
        .map_err(|diags| format_diagnostics(&world, &diags))?;
    Ok(document.pages.iter().map(typst_svg::svg).collect())
}

/// 取源码与所在目录，供各导出命令共用。
fn read_typ_input(input_path: &str) -> Result<(PathBuf, String), String> {
    let input = PathBuf::from(input_path.trim());
    if !input.is_file() {
        return Err(format!("typ file not found: {}", input.to_string_lossy()));
    }
    let input_dir = input
        .parent()
        .ok_or_else(|| "Failed to get typ parent dir".to_string())?
        .to_path_buf();
    let content = std::fs::read_to_string(&input)
        .map_err(|e| format!("读取 .typ 源文件失败: {e}"))?;
    Ok((input_dir, content))
}

/// 导出 HTML。
#[tauri::command]
pub async fn export_typ_html(input_path: String, output_path: String) -> Result<(), String> {
    let (dir, content) = read_typ_input(&input_path)?;
    let html = tokio::task::spawn_blocking(move || compile_typ_to_html(dir, content))
        .await
        .map_err(|e| format!("Typst HTML 导出任务异常: {e}"))??;
    let output = PathBuf::from(output_path.trim());
    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&output, html.as_bytes())
        .map_err(|e| format!("写入 HTML 失败: {e}"))?;
    info!(to = %output.display(), "[ExportTyp] html exported");
    Ok(())
}

/// 导出 SVG：逐页写出 `<stem>-1.svg`、`<stem>-2.svg`…
///
/// 返回写出的文件列表，前端据此提示「共 N 页」。
#[tauri::command]
pub async fn export_typ_svg(input_path: String, output_path: String) -> Result<Vec<String>, String> {
    let (dir, content) = read_typ_input(&input_path)?;
    let svgs = tokio::task::spawn_blocking(move || compile_typ_to_svgs(dir, content))
        .await
        .map_err(|e| format!("Typst SVG 导出任务异常: {e}"))??;
    let output = PathBuf::from(output_path.trim());
    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("page")
        .to_string();
    let parent = output
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut written = Vec::with_capacity(svgs.len());
    for (i, svg) in svgs.iter().enumerate() {
        let path = parent.join(format!("{stem}-{}.svg", i + 1));
        std::fs::write(&path, svg.as_bytes())
            .map_err(|e| format!("写入 SVG 失败: {e}"))?;
        written.push(path.to_string_lossy().to_string());
    }
    info!(count = written.len(), "[ExportTyp] svg exported");
    Ok(written)
}

/// 导出 PNG 打包（.tar.gz）：逐页 SVG 包一层 manifest 写进归档。
///
/// 为什么不是「真的 PNG」：Typst 自带的位图编码器只有 PNG（`typst-render`），
/// 但那需要额外的 raster 依赖与字体度量；这里先用矢量 SVG 打包，任何看图软件、
/// Figma、浏览器都能打开，且体积远小于位图。需要位图时用户可再用工具转换。
#[tauri::command]
pub async fn export_typ_images(input_path: String, output_path: String) -> Result<usize, String> {
    let (dir, content) = read_typ_input(&input_path)?;
    let svgs = tokio::task::spawn_blocking(move || compile_typ_to_svgs(dir, content))
        .await
        .map_err(|e| format!("Typst 图片导出任务异常: {e}"))??;
    let output = PathBuf::from(output_path.trim());
    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("images")
        .to_string();

    let file = std::fs::File::create(&output)
        .map_err(|e| format!("创建导出文件失败: {e}"))?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);
    for (i, svg) in svgs.iter().enumerate() {
        let name = format!("{stem}-{}.svg", i + 1);
        let data = svg.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive
            .append_data(&mut header, &name, data)
            .map_err(|e| format!("写入归档失败: {e}"))?;
    }
    let encoder = archive
        .into_inner()
        .map_err(|e| format!("收尾归档失败: {e}"))?;
    encoder
        .finish()
        .map_err(|e| format!("压缩归档失败: {e}"))?;
    info!(count = svgs.len(), to = %output.display(), "[ExportTyp] image bundle exported");
    Ok(svgs.len())
}
