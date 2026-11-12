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
use typst::layout::{Abs, Point};
// 0.15 起 PagedDocument 由 typst_layout 导出，typst 只是重新导出模块。
use typst_layout::PagedDocument;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::introspection::PagedPosition;
use std::num::NonZeroUsize;
use typst::{Library, LibraryExt, World, WorldExt};
use typst_ide::{jump_from_click, jump_from_cursor, IdeWorld, Jump};
use typst_kit::fonts::FontStore;

/// 全局标准库（所有编译共享，构建一次）。
///
/// 0.15 把 HTML 导出收进了「实验特性」开关：不启用 `Feature::Html` 时，
/// 编译成 `HtmlDocument` 会直接报 "html export is only available when `--features html`"。
/// 我们的导出菜单依赖 HTML，所以在这里显式启用——这与 Typst CLI 传 `--features html` 等价。
/// Bundle 不启用：图集/打包导出还没接。
static LIBRARY: Lazy<LazyHash<Library>> = Lazy::new(|| {
    let features: typst::Features = std::iter::once(typst::Feature::Html).collect();
    LazyHash::new(Library::builder().with_features(features).build())
});

/// 全局字体（扫描系统字体一次后缓存）。
///
/// 0.15 的 `typst-kit` 用 `FontStore` 统一管理字体与 font book，
/// 不再暴露 `FontSearcher` / `FontSlot`，因此这里直接持有 `FontStore`：
/// `book()` 给 `World::book`，`font(i)` 给 `World::font`。
struct GlobalFonts {
    store: FontStore,
}

static FONTS: Lazy<GlobalFonts> = Lazy::new(|| {
    // include_embedded_fonts：内嵌 Typst 默认字体（正文 LibertinusSerif / 数学 NewCMMath / 代码 DejaVuSansMono），
    // 不依赖系统是否装有这些字体，修复数学公式等渲染异常。系统字体仍加载，供中文等使用。
    let mut store = FontStore::new();
    store.extend(typst_kit::fonts::embedded());
    store.extend(typst_kit::fonts::system());
    info!("[Typst] fonts loaded (system + embedded)");
    GlobalFonts { store }
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
        // 0.15 起 FileId 由 RootedPath 构造（项目根 + 虚拟路径），不再接受 Option<Spec>。
        let vpath = VirtualPath::new("__guaxi_main__.typ").expect("固定的主文件名必然合法");
        let main_id = RootedPath::new(VirtualRoot::Project, vpath).intern();
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
    /// 0.15 的 `typst-kit` 已经原生支持镜像地址与自定义缓存目录
    /// （`UniversePackages::with_url` / `SystemPackages::from_parts`），
    /// 所以这里直接用上游的 `SystemPackages::obtain`，行为和官方 CLI 完全一致：
    /// 先查数据目录、再查缓存目录，都没有才按配置的 registry 下载并缓存。
    fn id_to_path(&self, id: FileId) -> FileResult<PathBuf> {
        // 0.15：`RootedPath::package()` 已弃用，改看 `root()`；它直接给出
        // Project / Package(spec) 两种根，比「取 spec 再判断」更直白。
        let root = match id.root() {
            VirtualRoot::Package(spec) => {
                let storage = crate::commands::typst_package::package_storage();
                storage
                    .obtain(spec)
                    .map_err(FileError::Package)?
                    .path()
                    .to_path_buf()
            }
            VirtualRoot::Project => self.root.clone(),
        };
        // 0.15：`VirtualPath::resolve` → `realize`。失败时保留真实的 RealizeError
        // （FileError 有专门的 `Realize` 变体，诊断里能看出是转义出根目录还是路径非法），
        // 比一律折叠成 NotFound 更有助于排查。
        id.vpath().realize(&root).map_err(FileError::from)
    }
}

impl World for GuaxiTypstWorld {
    fn library(&self) -> &LazyHash<Library> {
        &LIBRARY
    }

    fn book(&self) -> &LazyHash<FontBook> {
        FONTS.store.book()
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
        FONTS.store.font(index)
    }

    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
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
            .and_then(|id| {
                let src = world.source(id).ok()?;
                // 0.15 的 Source::range 需要 SpanNumber + SubRange；WorldExt::range(span)
                // 是对应的便捷入口，语义与 0.14 的 src.range(span) 一致。
                let range = world.range(diag.span)?;
                let line = src.lines().byte_to_line(range.start)? + 1;
                let col = src.lines().byte_to_column(range.start)? + 1;
                Some(format!("{}:{}: ", line, col))
            })
            .unwrap_or_default();
        // 0.15 起 hints 是 Spanned<EcoString>（附带 span），message 仍是普通 EcoString。
        out.push_str(&format!("error: {}{}\n", line_info, diag.message));
        for hint in &diag.hints {
            out.push_str(&format!("  hint: {}\n", hint.v));
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
        // 存裸文档而不是 Arc：0.15 的 JumpFromDocument 只为 PagedDocument 本身实现，
        // 包在 Arc 里就不再满足 trait bound。
        document,
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
    document: PagedDocument,
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
    // 0.15 的 jump_from_click 直接收「页 + 页内坐标」组成的 PagedPosition，
    // 不再需要自己取 frame 再传点。
    if entry.document.pages().get(page_idx).is_none() {
        return Ok(None);
    }
    let Some(page_no) = NonZeroUsize::new(page_idx + 1) else {
        return Ok(None);
    };
    let position = PagedPosition {
        page: page_no,
        point: Point::new(Abs::pt(x as f64), Abs::pt(y as f64)),
    };
    let jump = jump_from_click(entry.world.as_ref(), &entry.document, &position);

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
    // 0.15 起 `html` 需要显式 options；默认值不美化输出（体积更小）。
    typst_html::html(&document, &typst_html::HtmlOptions::default())
        .map_err(|diags| format_diagnostics(&world, &diags))
}

/// 把每一页渲染成 SVG。
fn compile_typ_to_svgs(root: PathBuf, content: String) -> Result<Vec<String>, String> {
    let world = GuaxiTypstWorld::new(root, content);
    let result = typst::compile::<PagedDocument>(&world);
    let document = result
        .output
        .map_err(|diags| format_diagnostics(&world, &diags))?;
    // 0.15 起 `svg` 需要显式 options；默认值与 0.14 行为一致（按页面尺寸、不美化）。
    let opts = typst_svg::SvgOptions::default();
    Ok(document.pages().iter().map(|page| typst_svg::svg(page, &opts)).collect())
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

#[cfg(test)]
mod tests {
    use super::*;

    // 这些测试真实跑一遍编译，用来验证 0.14 → 0.15 的 API 迁移没有改变运行期行为。
    // 字体扫描较慢（首次几秒），但这是唯一能确认「排版/数学/代码块仍正常」的方式。

    const SAMPLE: &str = r#"#set page(width: 12cm, height: 8cm)
= 回归测试
这是一个段落，用于确认字体与版面正常。

$ sum_(i=1)^n i = (n(n+1))/2 $

```python
print("hello")
```
"#;

    #[test]
    fn pdf_export_produces_valid_pdf() {
        let root = std::env::temp_dir();
        let bytes = compile_typ_to_pdf(root, SAMPLE.to_string())
            .expect("Typst 编译应当成功");
        assert!(!bytes.is_empty(), "PDF 不应为空");
        assert_eq!(&bytes[..5], b"%PDF-", "产物应是合法 PDF");
    }

    #[test]
    fn html_export_produces_html_document() {
        let root = std::env::temp_dir();
        let html = compile_typ_to_html(root, SAMPLE.to_string())
            .expect("HTML 导出应当成功");
        assert!(html.len() > 100, "HTML 不应为空");
        // 0.15 的 html 目标输出完整文档，应包含 doctype
        let head = html.trim_start().to_ascii_lowercase();
        assert!(head.starts_with("<!doctype html"), "应输出完整 HTML 文档，实际开头: {}",
            &html[..html.len().min(60)]);
    }

    #[test]
    fn svg_export_produces_one_svg_per_page() {
        let root = std::env::temp_dir();
        let svgs = compile_typ_to_svgs(root, SAMPLE.to_string())
            .expect("SVG 导出应当成功");
        assert_eq!(svgs.len(), 1, "单页文档应只产出一个 SVG");
        assert!(svgs[0].contains("<svg"), "产物应是 SVG");
    }

    #[test]
    fn jump_cache_roundtrip_supports_bidirectional_search() {
        // 双向跳转是 0.15 里改动最大的一部分（jump_from_click 改收 PagedPosition、
        // JumpFromDocument 只为裸 PagedDocument 实现）。这里跑一遍完整往返：
        // 编译并缓存 -> 正向（源码行 -> 页内坐标）-> 反向（页内坐标 -> 源码行）。
        let root = std::env::temp_dir();
        let key = "guaxi-test-jump";
        let bytes = compile_typ_and_cache(key, root, SAMPLE.to_string())
            .expect("编译应成功");
        assert!(!bytes.is_empty());

        let entry = {
            let cache = JUMP_CACHE.lock().unwrap();
            cache.get(key).cloned().expect("应已写入跳转缓存")
        };
        assert_eq!(entry.document.pages().len(), 1, "单页文档");

        // 正向：取正文段落的那一行（SAMPLE 里「这是一个段落…」），应能定位到第 1 页
        let source = entry.world.main_source.clone();
        let para_line = SAMPLE
            .lines()
            .position(|l| l.contains("这是一个段落"))
            .expect("样例里应有该段落");
        let line_byte = source.lines().line_to_byte(para_line).expect("该行应存在");
        let positions = jump_from_cursor(&entry.document, &source, line_byte);
        assert!(!positions.is_empty(), "正文行应能在文档中找到对应位置");
        let pos = &positions[0];
        assert_eq!(pos.page.get(), 1, "应落在第 1 页");

        // 反向：用正向得到的位置再点回去，应回到同一段源码附近
        let jump = jump_from_click(entry.world.as_ref(), &entry.document, pos);
        assert!(jump.is_some(), "反向跳转应能命中源码位置");

        forget_typst_jump_cache(key);
    }

    #[test]
    fn package_storage_resolves_cached_package_without_network() {
        // 包解析改走 0.15 的 SystemPackages 后，最容易坏的是「镜像/缓存目录没传进去」。
        // 这里不联网：把手写的包放进自定义缓存目录，断言能按 @preview 定位到它。
        let root = std::env::temp_dir().join("guaxi-typst-pkg-test");
        let _ = std::fs::remove_dir_all(&root);
        let pkg_root = root.join("preview").join("guaxitest").join("1.0.0");
        std::fs::create_dir_all(&pkg_root).expect("建包目录");
        std::fs::write(
            pkg_root.join("typst.toml"),
            "[package]
name = \"guaxitest\"
version = \"1.0.0\"
entrypoint = \"lib.typ\"
",
        )
        .expect("写 manifest");
        std::fs::write(pkg_root.join("lib.typ"), "#let hi = [hello]
").expect("写入口");

        // 直接验证缓存布局解析（不触碰进程级配置，避免污染其它测试）。
        let resolved =
            crate::commands::typst_package::package_dir(&root, "preview", "guaxitest", "1.0.0");
        assert!(resolved.join("typst.toml").is_file(), "缓存布局应能定位到 manifest");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn diagnostics_include_line_numbers() {
        let root = std::env::temp_dir();
        // 故意引用不存在的变量，确认错误信息带行号（0.15 的诊断结构变了）
        let bad = "#set page(width: 10cm)\n#let x = (\n这里有语法错误\n";
        let err = compile_typ_to_pdf(root, bad.to_string()).unwrap_err();
        assert!(err.contains("error"), "错误信息应包含 error，实际: {err}");
        assert!(
            err.chars().any(|c| c.is_ascii_digit()),
            "错误信息应包含位置信息，实际: {err}"
        );
    }
}
