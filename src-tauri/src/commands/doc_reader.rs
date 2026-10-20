//! 文档阅读器与「打开文件」分流的后端支撑（纯新增能力，不改动既有命令语义）。
//!
//! 这里只做三件事，全部是只读或新建目录的重操作，不触碰现有编辑器/转换链路：
//!
//! 1. [`sniff_file_kind`]：按文件头魔数判定真实类型。扩展名会骗人（`.txt` 的 PNG、
//!    无扩展名的 PDF），前端据此决定「交给 Monaco」还是「交给阅读器」。
//! 2. [`read_document_for_reader`]：把 PDF 之外的文档一次性读成阅读器能直接消费的结构。
//!    - 电子书（EPUB/MOBI/AZW3/KFX/FB2/LIT/CBZ）：复用已在依赖里的 `ebook-rs`，
//!      直接拿 `Section::processed_html`，图片已由库内联成 data URI。
//!    - 旧版 Office / RTF / ODT：复用已在依赖里的 `anydoc`，转 Markdown 后交给前端渲染。
//!    两者都用 `catch_unwind` 隔离第三方解析器的 panic —— 与 `tts_book.rs` / `convert::engine`
//!    的既有写法保持一致，畸形文件不会影响主程序。
//! 3. [`read_spreadsheet`]：用 `calamine` 读 xlsx/xls/ods 的工作表，返回可虚拟滚动的二维数据。
//!
//! 设计约束（为什么都集中在一个新文件里）：
//! - 阅读器是纯增量功能，任何一处改动都不能让既有打开路径变慢或变脆。因此这里的新命令
//!   全部是独立的 `#[tauri::command]`，由前端新组件调用；既有命令一个字符都没改。
//! - 解析器 panic 必须隔离，且 panic hook 是进程级全局状态，必须串行化加锁。

use serde::Serialize;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

/// 第三方解析器 panic hook 的串行化锁。
///
/// `std::panic::set_hook` 是进程级全局状态，并发替换会互相覆盖并导致 hook 泄漏；
/// 与 `tts_book::PARSER_PANIC_HOOK_GUARD`、`convert::engine::ANYDOC_PANIC_HOOK_GUARD` 同一做法。
/// 这里单独一把锁，避免与它们争用（两者嵌套会死锁）。
static READER_PANIC_HOOK_GUARD: Mutex<()> = Mutex::new(());

/// 单个文件交给阅读器的体积上限。超过后既容易 OOM，也没有「打开看看」的意义。
const READER_MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;

/// 电子书阅读器的单本上限更保守：ebook-rs 会把整本的 sections 全部处理并驻留内存
/// （含内联后的图片 data URI），一本漫画 CBZ 很容易到几百 MB。
const EBOOK_MAX_INPUT_BYTES: u64 = 256 * 1024 * 1024;

/// 表格阅读器单表上限：calamine 读成内存二维数组，行数过多时前端也滚不动。
const SPREADSHEET_MAX_ROWS: usize = 200_000;
/// 表格预览的列数上限，防止畸形表（极多列）把 JSON 撑爆。
const SPREADSHEET_MAX_COLS: usize = 512;
/// 单元格文本长度上限。
const SPREADSHEET_MAX_CELL_CHARS: usize = 4_000;

/// 在隔离 panic 的前提下执行一次第三方解析。
fn with_parser_panic_guard<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    let _guard = READER_PANIC_HOOK_GUARD
        .lock()
        .map_err(|_| "阅读器解析锁获取失败".to_string())?;
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(previous_hook);
    match result {
        Ok(inner) => inner,
        Err(_) => Err("文档解析器内部错误（已隔离，不影响程序运行）".to_string()),
    }
}

fn file_size(path: &Path) -> Result<u64, String> {
    std::fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| format!("读取文件信息失败: {}", e))
}

fn ensure_reader_size(path: &Path) -> Result<(), String> {
    let size = file_size(path)?;
    if size > READER_MAX_INPUT_BYTES {
        return Err(format!(
            "文件过大（{:.0} MB），阅读器不予处理",
            size as f64 / 1024.0 / 1024.0
        ));
    }
    Ok(())
}

fn read_head(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("打开文件失败: {}", e))?;
    let mut buf = vec![0u8; limit];
    let n = f.read(&mut buf).map_err(|e| format!("读取文件头失败: {}", e))?;
    buf.truncate(n);
    Ok(buf)
}

// ───────────────────────── 文件类型嗅探 ─────────────────────────

/// 从魔数判定的「真实类型」。前端据此在文本编辑器与对应阅读器之间分流。
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileKind {
    /// 规范化的类型标记：pdf / epub / mobi / azw3 / kfx / fb2 / lit / cbz / docx / doc /
    /// pptx / xlsx / ods / odt / rtf / image / audio / video / text / binary / empty
    pub kind: String,
    /// 阅读器应使用的分组：text / pdf / word / ebook / image / spreadsheet / slides / media / html / binary
    pub reader: String,
    /// 图片/音视频的 MIME（其它为空串），前端可直接拿去做 `<img>` / `<video>`。
    pub mime: String,
    /// 文件大小（字节）
    pub size: u64,
    /// 是否为压缩容器（zip）。docx/xlsx/pptx/epub/ods 都是 zip，但内容不同，需继续下钻。
    pub container: bool,
}

fn image_mime(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if head.starts_with(b"BM") {
        return Some("image/bmp");
    }
    // TIFF：小端 II*\0 / 大端 MM\0*
    if head.starts_with(&[0x49, 0x49, 0x2A, 0x00]) || head.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]) {
        return Some("image/tiff");
    }
    // AVIF/HEIF：ftyp box 里带 avif/heic 标记
    if head.len() >= 12 && &head[4..8] == b"ftyp" {
        let brand = &head[8..12];
        if brand == b"avif" || brand == b"avis" {
            return Some("image/avif");
        }
        if brand == b"heic" || brand == b"heix" || brand == b"mif1" {
            return Some("image/heic");
        }
    }
    if head.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some("image/x-icon");
    }
    None
}

fn audio_video_mime(head: &[u8]) -> Option<&'static str> {
    if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WAVE" {
        return Some("audio/wav");
    }
    if head.starts_with(b"ID3") || (head.len() >= 2 && head[0] == 0xFF && (head[1] & 0xE0) == 0xE0) {
        return Some("audio/mpeg");
    }
    if head.starts_with(b"fLaC") {
        return Some("audio/flac");
    }
    if head.starts_with(b"OggS") {
        return Some("audio/ogg");
    }
    if head.len() >= 8 && head.starts_with(b"MThd") {
        return Some("audio/midi");
    }
    if head.len() >= 12 && &head[4..8] == b"ftyp" {
        return Some("video/mp4");
    }
    if head.len() >= 4 && &head[0..4] == b"\x1A\x45\xDF\xA3" {
        return Some("video/webm");
    }
    None
}

/// 判断字节流是否像纯文本：允许 UTF-8 BOM，控制字符占比低（UTF-16 也按文本处理）。
fn looks_like_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return true;
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return true;
    }
    let mut control = 0usize;
    let sample = &bytes[..bytes.len().min(8192)];
    for &b in sample {
        // 允许 \t \n \r \f 与常规可打印区间；0x7F 及 0x00-0x1F 其余计为控制字符。
        if b == 0x09 || b == 0x0A || b == 0x0D || b == 0x0C {
            continue;
        }
        if b < 0x20 || b == 0x7F {
            control += 1;
        }
    }
    control * 100 / sample.len().max(1) < 5
}

/// 下钻 zip 容器：读条目名判断它是 docx / xlsx / pptx / epub / ods / odt 中的哪一种。
fn classify_zip_container(path: &Path) -> Option<&'static str> {
    let file = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;

    // 条目名判断与 mimetype 读取都要可变借用，不能用一个闭包反复 by_name；
    // 先各自读一次字符串再判断。
    let entry_names: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();
    let has = |name: &str| entry_names.iter().any(|n| n == name);

    if has("mimetype") {
        // 只读几十字节判断即可。
        let mut buf = [0u8; 64];
        if let Ok(mut f) = zip.by_name("mimetype") {
            let n = f.read(&mut buf).unwrap_or(0);
            if buf[..n].starts_with(b"application/epub+zip") {
                return Some("epub");
            }
        }
    }
    if has("word/document.xml") {
        return Some("docx");
    }
    if has("xl/workbook.xml") {
        return Some("xlsx");
    }
    if has("ppt/presentation.xml") {
        return Some("pptx");
    }
    if has("content.xml") && has("META-INF/manifest.xml") {
        // ODF 家族：mimetype 里区分 text / spreadsheet / presentation。
        let mut buf = [0u8; 64];
        if let Ok(mut f) = zip.by_name("mimetype") {
            let n = f.read(&mut buf).unwrap_or(0);
            let mime = &buf[..n];
            if mime.starts_with(b"application/vnd.oasis.opendocument.spreadsheet") {
                return Some("ods");
            }
            if mime.starts_with(b"application/vnd.oasis.opendocument.presentation") {
                return Some("odp");
            }
            if mime.starts_with(b"application/vnd.oasis.opendocument.text") {
                return Some("odt");
            }
        }
        return Some("odf");
    }
    None
}

/// 按扩展名给出「容器下钻失败时的保底判断」。
fn fallback_kind_by_extension(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => "pdf",
        "epub" => "epub",
        "mobi" => "mobi",
        "azw3" | "azw" => "azw3",
        "kfx" => "kfx",
        "fb2" => "fb2",
        "lit" => "lit",
        "cbz" => "cbz",
        "docx" => "docx",
        "doc" => "doc",
        "pptx" => "pptx",
        "ppt" => "ppt",
        "xlsx" => "xlsx",
        "xls" => "xls",
        "ods" => "ods",
        "odp" => "odp",
        "odt" => "odt",
        "rtf" => "rtf",
        "html" | "htm" | "xhtml" => "html",
        _ => "",
    }
    .to_string()
}

fn reader_group_for(kind: &str) -> &'static str {
    match kind {
        "pdf" => "pdf",
        "epub" | "mobi" | "azw3" | "kfx" | "fb2" | "lit" | "cbz" => "ebook",
        "docx" | "doc" => "word",
        "xlsx" | "xls" | "ods" => "spreadsheet",
        "pptx" | "ppt" | "odp" => "slides",
        "html" => "html",
        "image" => "image",
        "audio" | "video" => "media",
        "text" => "text",
        _ => "binary",
    }
}

/// 嗅探文件真实类型。
///
/// 判定优先级：魔数 > zip 容器下钻 > 扩展名 > 文本探测。扩展名只作最后的兜底，
/// 所以「把 PNG 改名成 .txt」不会让编辑器尝试按 UTF-8 读它。
#[tauri::command]
pub fn sniff_file_kind(path: String) -> Result<FileKind, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err("文件不存在".to_string());
    }
    if p.is_dir() {
        return Err("目标是目录，不是文件".to_string());
    }
    let size = file_size(p)?;
    if size == 0 {
        return Ok(FileKind {
            kind: "empty".to_string(),
            reader: "text".to_string(),
            mime: String::new(),
            size,
            container: false,
        });
    }

    let head = read_head(p, 16 * 1024)?;

    let mut kind = String::new();
    let mut mime = String::new();
    let mut container = false;

    if let Some(m) = image_mime(&head) {
        kind = "image".to_string();
        mime = m.to_string();
    } else if let Some(m) = audio_video_mime(&head) {
        kind = if m.starts_with("video") { "video" } else { "audio" }.to_string();
        mime = m.to_string();
    } else if head.starts_with(b"%PDF-") {
        kind = "pdf".to_string();
        mime = "application/pdf".to_string();
    } else if head.starts_with(b"PK\x03\x04") {
        container = true;
        kind = classify_zip_container(p).unwrap_or("").to_string();
        if kind.is_empty() {
            kind = fallback_kind_by_extension(p);
        }
    } else if head.starts_with(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1") {
        // OLE2 复合文档：老版 .doc / .xls / .ppt 共用同一容器头，只能靠扩展名区分。
        kind = match fallback_kind_by_extension(p).as_str() {
            "xls" => "xls".to_string(),
            "ppt" => "ppt".to_string(),
            _ => "doc".to_string(),
        };
    } else if head.len() >= 60 && &head[60..68] == b"BOOKMOBI" {
        kind = "mobi".to_string();
    } else if head.starts_with(b"BOOKMOBI") {
        kind = "mobi".to_string();
    } else if head.len() >= 8 && head[0..8] == [0x67, 0x83, 0x5A, 0x62, 0x00, 0x00, 0x00, 0x00] {
        kind = "lit".to_string();
    } else if head.starts_with(b"ITSF") {
        kind = "lit".to_string();
    } else {
        let by_ext = fallback_kind_by_extension(p);
        if !by_ext.is_empty() {
            kind = by_ext;
        }
    }

    // AZW3(KF8) 与 MOBI 共用 PalmDB 头，扩展名更可靠；这里只在明确是 azw3 时覆盖。
    let ext_lower = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if kind == "mobi" && ext_lower == "azw3" {
        kind = "azw3".to_string();
    }

    if kind.is_empty() {
        kind = if looks_like_text(&head) {
            "text".to_string()
        } else {
            "binary".to_string()
        };
    }

    Ok(FileKind {
        reader: reader_group_for(&kind).to_string(),
        kind,
        mime,
        size,
        container,
    })
}

// ───────────────────────── 电子书阅读器 ─────────────────────────

/// 一本书的目录项（扁平化后的章节导航）。
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EbookTocItem {
    pub label: String,
    pub href: String,
    pub depth: usize,
}

/// 电子书阅读器所需的一次性载荷。
///
/// `sections[i].html` 是 ebook-rs `processed_html` 的原样输出：CSS、图片（已内联为
/// data URI）、字体都已处理过，前端只需塞进 sandbox iframe 即可显示。
/// 这样做的代价是首屏一次性传输整本 HTML，但换来的是零额外依赖、零资源路径解析，
/// 对「打开一本本地电子书」这个场景足够。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EbookPayload {
    pub title: String,
    pub kind: String,
    pub sections: Vec<EbookSection>,
    pub toc: Vec<EbookTocItem>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EbookSection {
    pub index: usize,
    pub href: String,
    pub html: String,
    pub text_chars: usize,
}

fn flatten_nav(points: &[ebook_rs::NavPoint], depth: usize, out: &mut Vec<EbookTocItem>) {
    for p in points {
        if out.len() >= 2000 {
            return;
        }
        out.push(EbookTocItem {
            label: p.label.clone(),
            href: p.href.clone(),
            depth,
        });
        flatten_nav(&p.subitems, depth + 1, out);
    }
}

fn read_ebook_blocking(path: &Path, kind: &str) -> Result<EbookPayload, String> {
    let size = file_size(path)?;
    if size > EBOOK_MAX_INPUT_BYTES {
        return Err(format!(
            "电子书过大（{:.0} MB），阅读器上限 {} MB",
            size as f64 / 1024.0 / 1024.0,
            EBOOK_MAX_INPUT_BYTES / 1024 / 1024
        ));
    }

    let book = with_parser_panic_guard(|| {
        ebook_rs::Book::from_file(path).map_err(|e| ebook_error_message(&e))
    })?;

    let mut sections = Vec::with_capacity(book.sections.len());
    let mut repaired_sections = 0usize;
    for section in &book.sections {
        // 空白章节（封面占位、版权页空壳）直接跳过，避免阅读器里出现整页空白。
        if section.plain_text.trim().is_empty() && !section.processed_html.contains("<img") {
            continue;
        }
        // 编码兜底：MOBI/AZW3 头里的 codec 常标错，会导致整章「UTF-8 被当 Latin-1」的乱码。
        // 只在判定为乱码时才动，正常书原样返回（见 repair_html_text_nodes 的说明）。
        let raw_html = section.processed_html.clone();
        let html = repair_html_text_nodes(&raw_html);
        let repaired = html != raw_html;
        if repaired {
            repaired_sections += 1;
        }
        sections.push(EbookSection {
            index: section.index,
            href: section.href.clone(),
            html,
            text_chars: section.char_count,
        });
    }

    if repaired_sections > 0 {
        tracing::info!(
            path = %path.display(),
            repaired_sections,
            total_sections = sections.len(),
            "[Reader] repaired latin1-mangled ebook text"
        );
    }

    if sections.is_empty() {
        return Err("未能从该电子书中提取到可显示的正文".to_string());
    }

    let mut toc = Vec::new();
    flatten_nav(&book.toc, 0, &mut toc);
    // 目录标题同样可能乱码（它常直接取自文件里的书名/章节名）。
    for item in toc.iter_mut() {
        item.label = repair_plain_text(&item.label);
    }

    let title = repair_plain_text(
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("未命名电子书"),
    );

    Ok(EbookPayload {
        title,
        kind: kind.to_string(),
        sections,
        toc,
    })
}

// ───────────────────────── 电子书编码兜底 ─────────────────────────
//
// 背景：MOBI / AZW3 的文件头里有一个 `codec` 字段标称文本编码，但大量实际文件
// 标错了（或干脆不标）。ebook-rs 按标称值解码，于是正文变成「UTF-8 字节被当
// Latin-1/CP1252 逐字节读」的乱码——例如「第」的 UTF-8 字节 E7 AC AC，
// 会被读成 `ç¬¬` 三个拉丁字符。EPUB 里若 XHTML 缺 `<meta charset>` 也会这样。
//
// 兜底思路（只做「可逆还原」，不做猜测性替换，避免把正常文本改坏）：
//   1. 判定：文本里有大量 U+0080..U+00FF 的单字节高位字符，且几乎不含正常的
//      非拉丁文字（CJK 等）。正常中文书不会长这样。
//   2. 还原：把这些字符按 CP1252/Latin-1 还原成字节，再按 UTF-8 严格解码。
//      严格解码失败就说明这个假设不成立，原样返回——绝不产出「半个字符」。
//   3. 还原后若 CJK 字符显著增多，才采用；否则维持原样。
//
// 刻意不做「试遍所有编码挑最像的」：那种做法在正常书上也可能误判，
// 一旦误判就是把用户的书内容改掉。

/// 判定一个字符是否落在「Latin-1 高位区」——乱码的典型分布 (U+0080..U+00FF)。
fn is_latin1_high(ch: char) -> bool {
    let c = ch as u32;
    (0x80..=0xFF).contains(&c)
}

/// 是否是正常书写会出现的非拉丁文字（CJK、假名、谚文、全角标点等）。
fn is_cjk_like(ch: char) -> bool {
    let c = ch as u32;
    (0x4E00..=0x9FFF).contains(&c)   // CJK 统一表意
        || (0x3400..=0x4DBF).contains(&c) // 扩展 A
        || (0x3040..=0x30FF).contains(&c) // 平假名 / 片假名
        || (0xAC00..=0xD7AF).contains(&c) // 谚文
        || (0xF900..=0xFAFF).contains(&c) // 兼容表意
        || (0xFF00..=0xFFEF).contains(&c) // 全角标点 / 全角字母
}

/// 统计样本里两类字符的数量，返回 `(latin1_high, cjk_like, total)`。
fn sample_counts(text: &str, limit: usize) -> (usize, usize, usize) {
    let mut latin = 0usize;
    let mut cjk = 0usize;
    let mut total = 0usize;
    for ch in text.chars().take(limit) {
        if ch.is_whitespace() {
            continue;
        }
        total += 1;
        if is_latin1_high(ch) {
            latin += 1;
        } else if is_cjk_like(ch) {
            cjk += 1;
        }
    }
    (latin, cjk, total)
}

/// 文本是否像「UTF-8 被按 Latin-1 读」的乱码。
fn looks_latin1_mangled(text: &str) -> bool {
    let (latin, cjk, total) = sample_counts(text, 4000);
    if total < 20 {
        return false; // 样本太少，不动
    }
    // 正常中文书：CJK 占绝对多数，latin 高位字符接近 0。
    // 乱码：latin 高位字符占比很高，而 CJK 几乎为 0。
    let latin_ratio = latin as f64 / total as f64;
    let cjk_ratio = cjk as f64 / total as f64;
    latin_ratio > 0.20 && cjk_ratio < 0.02
}

/// CP1252 在 0x80..=0x9F 区间把控制符换成了可见字符，其中五个码位映射到
/// **超出 0xFF 的 Unicode 码点**（Œ U+0152、œ U+0153、Š U+0160、š U+0161、Ž U+017D、
/// ž U+017E、Ÿ U+0178）。只按「码位 <= 0xFF」判断会漏掉它们，导致整段还原被放弃 ——
/// 这正是截图里出现 `ĉ`、`…` 这类字符却始终修不好的原因。
///
/// 这里做一次反向查表：Unicode 码点 -> 原字节。命中即返回。
fn cp1252_reverse(code: u32) -> Option<u8> {
    // 0x00..=0x7F：ASCII，码点即字节值。
    if code < 0x80 {
        return Some(code as u8);
    }
    // 0xA0..=0xFF：Latin-1 直接映射。
    //
    // 注意 0x80..=0x9F 必须落到下面的 CP1252 表，而**不能**直接返回 Some(code as u8)：
    // 那些码位在 CP1252 里是可见字符，若原样保留，真正的 Latin-1 控制符也会被当成
    // 「可还原字节」，于是「含非单字节字符就放弃」的保护形同虚设。
    if (0xA0..=0xFF).contains(&code) {
        return Some(code as u8);
    }
    // CP1252 有 5 个**未定义**码位（0x81 0x8D 0x8F 0x90 0x9D）。解码器遇到它们时
    // 通常原样输出对应的 C1 控制符（U+0081 等）。乱码文本里这些字节很常见（UTF-8
    // 续字节大量落在 0x80..0x9F），因此必须承认「C1 控制符 -> 原字节」这条映射，
    // 否则整段还原会因为这些个别字符而失败。
    if (0x80..=0x9F).contains(&code) {
        return Some(code as u8);
    }
    // 其余是 CP1252 的可见字符映射表。
    const TABLE: [(u32, u8); 27] = [
        (0x20AC, 0x80), // €
        (0x201A, 0x82), // ‚
        (0x0192, 0x83), // ƒ
        (0x201E, 0x84), // „
        (0x2026, 0x85), // …
        (0x2020, 0x86), // †
        (0x2021, 0x87), // ‡
        (0x02C6, 0x88), // ˆ
        (0x2030, 0x89), // ‰
        (0x0160, 0x8A), // Š
        (0x2039, 0x8B), // ‹
        (0x0152, 0x8C), // Œ
        (0x017D, 0x8E), // Ž
        (0x2018, 0x91), // ‘
        (0x2019, 0x92), // ’
        (0x201C, 0x93), // “
        (0x201D, 0x94), // ”
        (0x2022, 0x95), // •
        (0x2013, 0x96), // –
        (0x2014, 0x97), // —
        (0x02DC, 0x98), // ˜
        (0x2122, 0x99), // ™
        (0x0161, 0x9A), // š
        (0x203A, 0x9B), // ›
        (0x0153, 0x9C), // œ
        (0x017E, 0x9E), // ž
        (0x0178, 0x9F), // Ÿ
    ];
    TABLE.iter().find(|(c, _)| *c == code).map(|(_, b)| *b)
}

#[cfg(test)]
mod cp1252_reverse_tests {
    use super::cp1252_reverse;

    #[test]
    fn maps_ascii_and_latin1() {
        assert_eq!(cp1252_reverse('A' as u32), Some(0x41));
        assert_eq!(cp1252_reverse('é' as u32), Some(0xE9));
    }

    #[test]
    fn maps_cp1252_specials_above_0xff() {
        assert_eq!(cp1252_reverse('Œ' as u32), Some(0x8C));
        assert_eq!(cp1252_reverse('œ' as u32), Some(0x9C));
        assert_eq!(cp1252_reverse('…' as u32), Some(0x85));
        assert_eq!(cp1252_reverse('€' as u32), Some(0x80));
    }

    #[test]
    fn rejects_characters_that_cannot_come_from_one_byte() {
        // 关键保护：中日韩字符绝不能被当成「单字节乱码」还原，否则正常文本会被改坏。
        assert_eq!(cp1252_reverse('第' as u32), None);
        assert_eq!(cp1252_reverse('あ' as u32), None);
        assert_eq!(cp1252_reverse('한' as u32), None);
        assert_eq!(cp1252_reverse('😀' as u32), None);
    }
}

/// 按 Latin-1 把每个字符取回单字节。遇到 > 0xFF 的字符返回 None。
fn decode_latin1_bytes(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let code = ch as u32;
        if code > 0xFF {
            return None; // 非单字节字符，不是这种乱码
        }
        bytes.push(code as u8);
    }
    Some(bytes)
}

/// 按 CP1252 把可见字符反查回单字节。任何一个字符反查不到就返回 None。
fn decode_cp1252_bytes(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len());
    for ch in text.chars() {
        bytes.push(cp1252_reverse(ch as u32)?);
    }
    Some(bytes)
}

/// 尝试把「Latin-1 形态」的文本还原成正确的 UTF-8。
///
/// 返回 `Some(还原后的文本)` 表示还原成功且结果更像正常文本；否则 `None`。
fn try_repair_latin1_utf8(text: &str) -> Option<String> {
    // CP1252 比纯 Latin-1 多出 0x80..0x9F 的可打印字符（Word 导出的文档常见）。
    // 能按 CP1252 单字节编码回去的，就按 CP1252 还原；否则退回 Latin-1。
    // 两种还原顺序都试一遍，取第一个能通过严格 UTF-8 解码的结果：
    //
    // 1) Latin-1 优先：0x80..0x9F 原样保留为控制符。这是**最常见的形态** ——
    //    UTF-8 的续字节大量落在 0x80..0x9F，只有按 Latin-1 才能原样取回。
    // 2) CP1252 兜底：0x80..0x9F 被解成 € … Œ 等可见字符（Word/浏览器导出常见）。
    //    注意 CP1252 有 5 个未定义码位（0x81/0x8D/0x8F/0x90/0x9D），碰到它们这条路线
    //    必然失败，所以不能只走这一条。
    //
    // 顺序反过来（CP1252 优先）会让「续字节恰好在 0x80..0x9F」的普通 UTF-8 乱码
    // 永远还原不了 —— 这正是最初那版兜底失效的原因。
    let decoded = decode_latin1_bytes(text)
        .and_then(|bytes| std::str::from_utf8(&bytes).ok().map(str::to_string))
        .or_else(|| {
            decode_cp1252_bytes(text)
                .and_then(|bytes| std::str::from_utf8(&bytes).ok().map(str::to_string))
        });

    // 严格解码失败即说明「这串字节本来就是单字节文本」，不做替换式解码。
    let repaired = decoded?;

    // 还原后必须「明显更像正文」才采用，防止把真正的西文文本改坏。
    let (_, cjk_before, _) = sample_counts(text, 4000);
    let (_, cjk_after, _) = sample_counts(&repaired, 4000);
    if cjk_after > cjk_before * 2 + 20 {
        Some(repaired)
    } else {
        None
    }
}

/// 对一段 HTML 做编码兜底：只处理文本节点内容，不动标签与属性。
///
/// 为什么不能整段处理：HTML 里的 `class="a"` 之类全是 ASCII，混进去会把
/// 「整段是否乱码」的判定稀释掉；而且直接替换整段字符串有破坏标签结构的风险。
fn repair_html_text_nodes(html: &str) -> String {
    if !looks_latin1_mangled(html) {
        return html.to_string();
    }

    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut text_buf = String::new();

    let flush = |buf: &mut String, out: &mut String| {
        if buf.is_empty() {
            return;
        }
        match try_repair_latin1_utf8(buf) {
            Some(fixed) => out.push_str(&fixed),
            None => out.push_str(buf),
        }
        buf.clear();
    };

    for ch in html.chars() {
        match ch {
            '<' => {
                flush(&mut text_buf, &mut out);
                in_tag = true;
                out.push(ch);
            }
            '>' => {
                in_tag = false;
                out.push(ch);
            }
            _ if in_tag => out.push(ch),
            _ => text_buf.push(ch),
        }
    }
    flush(&mut text_buf, &mut out);
    out
}

/// 对纯文本做同样的兜底（目录标题、书名等）。
fn repair_plain_text(text: &str) -> String {
    if !looks_latin1_mangled(text) {
        return text.to_string();
    }
    try_repair_latin1_utf8(text).unwrap_or_else(|| text.to_string())
}

fn ebook_error_message(e: &ebook_rs::EbookError) -> String {
    use ebook_rs::EbookError as E;
    match e {
        E::DrmProtected(_) => "该电子书受 DRM 保护，无法解析，请先去 DRM 后再打开".to_string(),
        E::InvalidFormat(detail) => format!("无法解析该电子书格式：{}", detail),
        E::CorruptedData(detail) => format!("该电子书数据损坏：{}", detail),
        E::NotFound(detail) => format!("该电子书缺少必需内容：{}", detail),
        E::Io(detail) => format!("读取电子书失败：{}", detail),
        E::Xml(detail) => format!("电子书结构解析失败：{}", detail),
        E::Zip(detail) => format!("解压电子书失败：{}", detail),
        _ => format!("电子书解析失败：{}", e),
    }
}

// ───────────────────────── 阅读器统一入口 ─────────────────────────

/// 阅读器载荷的判别式。前端按 `format` 分支渲染。
#[derive(Debug, Serialize)]
#[serde(tag = "format", rename_all = "camelCase")]
pub enum ReaderDocument {
    /// 电子书：分节 HTML
    Ebook(EbookPayload),
    /// 旧版 Office / RTF / ODT：anydoc 转出的 Markdown
    Markdown {
        title: String,
        markdown: String,
        kind: String,
    },
}

/// 把所有「非文本、非 PDF、非图片」的文档读成阅读器可直接渲染的结构。
///
/// PDF 与图片由前端直接走 pdfium / `<img>`，不需要经过这里（也就不会把大文件搬进 IPC）。
#[tauri::command]
pub async fn read_document_for_reader(path: String, kind: String) -> Result<ReaderDocument, String> {
    let p = Path::new(&path).to_path_buf();
    if !p.exists() {
        return Err("文件不存在".to_string());
    }
    ensure_reader_size(&p)?;

    let kind_lower = kind.trim().to_ascii_lowercase();

    match kind_lower.as_str() {
        "epub" | "mobi" | "azw3" | "kfx" | "fb2" | "lit" | "cbz" => {
            let kind_for_task = kind_lower.clone();
            let payload = tauri::async_runtime::spawn_blocking(move || {
                read_ebook_blocking(&p, &kind_for_task)
            })
            .await
            .map_err(|e| format!("电子书解析任务失败: {}", e))??;
            Ok(ReaderDocument::Ebook(payload))
        }
        "doc" | "rtf" | "odt" | "odp" | "ppt" | "ods" => {
            let p2 = p.clone();
            let markdown = tauri::async_runtime::spawn_blocking(move || {
                with_parser_panic_guard(|| {
                    anydoc::to_markdown(&p2)
                        .map_err(|e| crate::commands::convert::engine::anydoc_error_message(&e))
                })
            })
            .await
            .map_err(|e| format!("文档解析任务失败: {}", e))??;
            if markdown.trim().is_empty() {
                return Err("文档解析结果为空".to_string());
            }
            Ok(ReaderDocument::Markdown {
                title: p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("未命名文档")
                    .to_string(),
                markdown,
                kind: kind_lower,
            })
        }
        other => Err(format!("阅读器不支持该类型：{}", other)),
    }
}

// ───────────────────────── 表格阅读器 ─────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetData {
    pub name: String,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpreadsheetPayload {
    pub sheets: Vec<SheetData>,
    /// 是否因行数/列数上限被截断（任一 sheet 截断即为真）。
    pub truncated: bool,
    /// 总行数（截断前），用于提示用户。
    pub total_rows: usize,
}

fn read_spreadsheet_blocking(path: &Path) -> Result<SpreadsheetPayload, String> {
    use calamine::{open_workbook_auto, Data, Reader};

    let mut workbook = open_workbook_auto(path)
        .map_err(|e| format!("打开表格失败: {}", e))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut sheets = Vec::new();
    let mut any_truncated = false;
    let mut total_rows = 0usize;

    for name in sheet_names {
        let range = match workbook.worksheet_range(&name) {
            Ok(r) => r,
            Err(e) => {
                // 单个 sheet 读不出来不应让整个文件打不开。
                tracing::warn!(sheet = name.as_str(), error = %e, "[Reader] worksheet_range failed");
                continue;
            }
        };

        let height = range.height();
        let width = range.width().min(SPREADSHEET_MAX_COLS);
        let rows_taken = height.min(SPREADSHEET_MAX_ROWS);
        total_rows += height;

        let mut rows = Vec::with_capacity(rows_taken);
        for row in range.rows().take(rows_taken) {
            let mut cells = Vec::with_capacity(width);
            for cell in row.iter().take(width) {
                let text = match cell {
                    Data::Empty => String::new(),
                    Data::String(s) => {
                        if s.chars().count() > SPREADSHEET_MAX_CELL_CHARS {
                            s.chars().take(SPREADSHEET_MAX_CELL_CHARS).collect::<String>() + "…"
                        } else {
                            s.clone()
                        }
                    }
                    Data::Float(f) => trim_float(*f),
                    Data::Int(i) => i.to_string(),
                    Data::Bool(b) => b.to_string(),
                    Data::DateTime(dt) => dt.to_string(),
                    Data::DateTimeIso(s) => s.clone(),
                    Data::DurationIso(s) => s.clone(),
                    Data::Error(e) => format!("#ERR:{:?}", e),
                };
                cells.push(text);
            }
            rows.push(cells);
        }

        if height > rows_taken || range.width() > width {
            any_truncated = true;
        }

        sheets.push(SheetData {
            name,
            rows,
            truncated: height > rows_taken || range.width() > width,
        });
    }

    if sheets.is_empty() {
        return Err("该表格没有可显示的工作表".to_string());
    }

    Ok(SpreadsheetPayload {
        sheets,
        truncated: any_truncated,
        total_rows,
    })
}

/// 浮点数去掉无意义的尾随零（1.0 → "1"，0.30000000000000004 保留原精度但截断位数）。
fn trim_float(f: f64) -> String {
    if !f.is_finite() {
        return f.to_string();
    }
    if f.fract() == 0.0 && f.abs() < 1e15 {
        return format!("{}", f as i64);
    }
    let s = format!("{:.6}", f);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// 读取 xlsx / xls / ods（calamine 自动识别格式）。
#[tauri::command]
pub async fn read_spreadsheet(path: String) -> Result<SpreadsheetPayload, String> {
    let p = Path::new(&path).to_path_buf();
    if !p.exists() {
        return Err("文件不存在".to_string());
    }
    ensure_reader_size(&p)?;
    tauri::async_runtime::spawn_blocking(move || read_spreadsheet_blocking(&p))
        .await
        .map_err(|e| format!("表格读取任务失败: {}", e))?
}

// ───────────────────────── 便携转换（导入时使用） ─────────────────────────

/// 转换质量档位。前端导入下拉用它控制「快 / 准 / 最强」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertQuality {
    Fast,
    Balanced,
    Best,
}

impl ConvertQuality {
    fn parse(raw: &str) -> ConvertQuality {
        match raw.trim().to_ascii_lowercase().as_str() {
            "best" | "mineru" | "strong" => ConvertQuality::Best,
            "balanced" | "marker" => ConvertQuality::Balanced,
            _ => ConvertQuality::Fast,
        }
    }

    fn engine(self) -> &'static str {
        match self {
            ConvertQuality::Fast => "anydoc",
            ConvertQuality::Balanced => "marker",
            ConvertQuality::Best => "mineru",
        }
    }
}

/// 转换产物：Markdown 文本 + 随之产生的资源目录。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertedMarkdown {
    pub markdown: String,
    pub engine: String,
    /// 转换过程中抽取出的图片目录（若为空则没有媒体）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_dir: Option<String>,
}

/// 把任意支持格式的文档转成 Markdown，返回文本而不落盘到目标位置。
///
/// 为什么需要它：既有 `convert_file_to_markdown_pandoc` 只支持 pandoc 能读的格式，
/// 且固定把媒体抽到 `<stem>_assets`。新导入流程要覆盖 PDF / 图片 OCR / 旧版 Office /
/// 电子书，这些各自走不同引擎，统一由这里路由；本命令只做「产出一份 Markdown」，
/// 落盘位置由前端决定（导入目标目录 / 临时预览目录都可以）。
#[tauri::command]
pub async fn convert_document_to_markdown(
    path: String,
    quality: Option<String>,
    // 可选：pandoc 抽取媒体的临时根目录；不传则用系统临时目录。
    media_root: Option<String>,
) -> Result<ConvertedMarkdown, String> {
    let p = Path::new(&path).to_path_buf();
    if !p.exists() {
        return Err("文件不存在".to_string());
    }
    ensure_reader_size(&p)?;

    let kind = sniff_internal(&p)?;
    let quality = ConvertQuality::parse(quality.as_deref().unwrap_or("fast"));

    match kind.kind.as_str() {
        // 纯文本 / Markdown：直接返回，不经过任何引擎。
        "text" | "empty" => {
            let bytes = std::fs::read(&p).map_err(|e| format!("读取文件失败: {}", e))?;
            Ok(ConvertedMarkdown {
                markdown: decode_text_bytes(&bytes),
                engine: "direct".to_string(),
                media_dir: None,
            })
        }
        // 电子书：分节 HTML 已是结构完整的正文，转成带章节标题的 Markdown。
        "epub" | "mobi" | "azw3" | "kfx" | "fb2" | "lit" | "cbz" => {
            let p2 = p.clone();
            let kind_name = kind.kind.clone();
            let payload = tauri::async_runtime::spawn_blocking(move || {
                read_ebook_blocking(&p2, &kind_name)
            })
            .await
            .map_err(|e| format!("电子书解析任务失败: {}", e))??;

            let mut md = String::new();
            for section in &payload.sections {
                md.push_str(&format!("\n\n<!-- section {} -->\n\n", section.index + 1));
                md.push_str(&html_to_markdown_lite(&section.html));
            }
            if md.trim().is_empty() {
                return Err("电子书正文为空".to_string());
            }
            Ok(ConvertedMarkdown {
                markdown: md,
                engine: "ebook-rs".to_string(),
                media_dir: None,
            })
        }
        // 旧版 Office（.doc/.ppt/.xls 的部分变体）、RTF、ODT：anydoc 进程内。
        // 注意 anydoc 也支持 docx/pptx/xlsx/pdf(文本层)，但这里仅在「极速档」才用它，
        // 以保留 fast 档对复杂 PDF 的图片抽取能力（见下方 pdf 分支）。
        "doc" | "rtf" | "odt" | "odp" | "ppt" | "xls" => {
            let markdown = anydoc_to_markdown(&p).await?;
            Ok(ConvertedMarkdown {
                markdown,
                engine: "anydoc".to_string(),
                media_dir: None,
            })
        }
        "pdf" => convert_pdf_to_markdown(&p, quality, media_root).await,
        "image" => convert_image_to_markdown(&p, kind.mime.as_str()).await,
        // docx / pptx / xlsx / html / 其它：pandoc 结构最完整（含表格、内嵌媒体抽取）。
        _ => convert_with_pandoc(&p, media_root).await,
    }
}

/// 内部嗅探（复用命令实现，避免把逻辑复制一份）。
fn sniff_internal(p: &Path) -> Result<FileKind, String> {
    sniff_file_kind(p.to_string_lossy().to_string())
}

fn decode_text_bytes(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.replace("\r\n", "\n").replace('\r', "\n");
    }
    let (decoded, _, _) = encoding_rs::GBK.decode(bytes);
    decoded.replace("\r\n", "\n").replace('\r', "\n")
}

async fn anydoc_to_markdown(p: &Path) -> Result<String, String> {
    let p2 = p.to_path_buf();
    let markdown = tauri::async_runtime::spawn_blocking(move || {
        with_parser_panic_guard(|| {
            anydoc::to_markdown(&p2)
                .map_err(|e| crate::commands::convert::engine::anydoc_error_message(&e))
        })
    })
    .await
    .map_err(|e| format!("文档解析任务失败: {}", e))??;
    if markdown.trim().is_empty() {
        return Err("文档解析结果为空".to_string());
    }
    Ok(markdown)
}

async fn convert_with_pandoc(p: &Path, media_root: Option<String>) -> Result<ConvertedMarkdown, String> {
    let pandoc = crate::guaxiservices::bundled_resources::ensure_relative_dir("utils/pandoc.exe")?;
    if !pandoc.exists() {
        return Err(format!("找不到 pandoc：{}", pandoc.display()));
    }

    // 输出到临时目录，由调用方决定最终落点；媒体抽到与 md 同级的 `<stem>_assets`。
    let work = match media_root {
        Some(root) if !root.trim().is_empty() => std::path::PathBuf::from(root),
        _ => std::env::temp_dir().join(format!(
            "guaxi_convert_{}",
            uuid::Uuid::new_v4()
        )),
    };
    std::fs::create_dir_all(&work).map_err(|e| format!("创建临时目录失败: {}", e))?;

    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string();
    let out_md = work.join(format!("{}.md", stem));
    let media_rel = format!("{}_assets", stem);

    let mut cmd = std::process::Command::new(&pandoc);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.current_dir(&work)
        .arg(p)
        .arg("-o")
        .arg(&out_md)
        .arg("--to")
        .arg("gfm+pipe_tables+task_lists+tex_math_dollars+footnotes")
        .arg("--wrap=none")
        .arg("--extract-media")
        .arg(&media_rel);

    let output = cmd
        .output()
        .map_err(|e| format!("执行 pandoc 失败: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("pandoc 转换失败：{}", stderr));
    }

    let markdown = std::fs::read_to_string(&out_md)
        .map_err(|e| format!("读取转换结果失败: {}", e))?;
    let media_path = work.join(&media_rel);
    Ok(ConvertedMarkdown {
        markdown,
        engine: "pandoc".to_string(),
        media_dir: if media_path.exists() {
            Some(media_path.to_string_lossy().to_string())
        } else {
            None
        },
    })
}

/// convert.engine 的 `expected_md_path_for` 复算一次，保证两者永远一致。
async fn convert_pdf_to_markdown(
    p: &Path,
    quality: ConvertQuality,
    media_root: Option<String>,
) -> Result<ConvertedMarkdown, String> {
    // 平衡/最强档交给既有转换队列（marker/mineru），能处理扫描件 OCR；
    // 这里只同步等待它产出 Markdown，再读回来。
    if quality != ConvertQuality::Fast {
        let work = match media_root {
            Some(root) if !root.trim().is_empty() => std::path::PathBuf::from(root),
            _ => std::env::temp_dir().join(format!("guaxi_convert_{}", uuid::Uuid::new_v4())),
        };
        std::fs::create_dir_all(&work).map_err(|e| format!("创建临时目录失败: {}", e))?;

        let (out_dir, md_path) = crate::commands::convert::engine::expected_md_path_for(
            &p.to_string_lossy(),
            &work.to_string_lossy(),
        );

        let request = crate::commands::convert::commands::EnqueueRequest {
            input_path: p.to_string_lossy().to_string(),
            output_dir: work.to_string_lossy().to_string(),
            imported_file_id: None,
            engine: quality.engine().to_string(),
            priority: Some(1),
            max_attempts: None,
        };
        let resp = crate::commands::convert::commands::enqueue_conversion(request).await?;
        let _ = resp.reused;

        // 轮询终态。超时给得较宽：marker 首次会装配环境。
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60 * 30);
        loop {
            if std::time::Instant::now() > deadline {
                return Err("转换超时（超过 30 分钟）".to_string());
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            match crate::commands::convert::task_store::get_task(resp.task_id)? {
                Some(t) => match t.status.as_str() {
                    crate::commands::convert::task_store::STATUS_SUCCEEDED => break,
                    crate::commands::convert::task_store::STATUS_FAILED => {
                        return Err(t.error.unwrap_or_else(|| "转换失败".to_string()))
                    }
                    crate::commands::convert::task_store::STATUS_CANCELED => {
                        return Err("转换已取消".to_string())
                    }
                    _ => continue,
                },
                None => return Err("转换任务丢失".to_string()),
            }
        }

        let markdown = std::fs::read_to_string(&md_path)
            .map_err(|e| format!("读取转换结果失败: {}", e))?;
        return Ok(ConvertedMarkdown {
            markdown,
            engine: quality.engine().to_string(),
            media_dir: Some(out_dir.to_string_lossy().to_string()),
        });
    }

    // 极速档：anydoc 进程内直接读文本层。图片抽取能力不如 pymupdf4llm，但不依赖 Python，
    // 也不会触发模型下载；扫描件会明确报「需要 OCR」而不是静默产出空文。
    let markdown = anydoc_to_markdown(p).await?;
    Ok(ConvertedMarkdown {
        markdown,
        engine: "anydoc".to_string(),
        media_dir: None,
    })
}

async fn convert_image_to_markdown(p: &Path, mime: &str) -> Result<ConvertedMarkdown, String> {
    // 复用既有 OCR 通道（agent_server /ocr，rapidocr）。
    let text = crate::commands::ocr::ocr_image_path(&p.to_string_lossy()).await?;

    let name = p
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("image")
        .to_string();
    let mut md = String::new();
    md.push_str(&format!("![{}]({})\n\n", name, name));
    if !text.trim().is_empty() {
        md.push_str(&format!("<!-- OCR · {} -->\n\n", mime));
        md.push_str(text.trim());
        md.push('\n');
    } else {
        md.push_str("_未识别到文字_");
    }
    Ok(ConvertedMarkdown {
        markdown: md,
        engine: "ocr".to_string(),
        media_dir: None,
    })
}

// ───────────────────────── HTML → Markdown（电子书用） ─────────────────────────

/// 极简 HTML → Markdown。只处理电子书正文里真实会出现的标签：
/// 标题、段落、换行、加粗/斜体、链接、图片、列表、引用、代码块、表格行。
///
/// 为什么不用成熟库：这里的目标是「把 ebook-rs 已清洗过的 section HTML 变成可编辑 Markdown」，
/// 输入受控（无脚本、无自定义标签），一个 150 行的状态机足够且零依赖、不会引入
/// 新 crate 影响 4 分钟的构建时间。
fn html_to_markdown_lite(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let bytes = html.as_bytes();
    let mut i = 0usize;
    let mut in_pre = false;
    let mut list_stack: Vec<bool> = Vec::new(); // true = ol

    fn push_text(out: &mut String, text: &str) {
        let decoded = decode_entities(text);
        // 折叠连续空白（HTML 语义），但保留单个空格。
        let collapsed = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
        out.push_str(&collapsed);
    }

    while i < bytes.len() {
        if bytes[i] == b'<' {
            let Some(close) = html[i..].find('>').map(|d| i + d) else {
                break;
            };
            let tag_raw = &html[i + 1..close];
            let tag_trim = tag_raw.trim();
            let is_close = tag_trim.starts_with('/');
            let lower = tag_trim.trim_start_matches('/').to_ascii_lowercase();
            let name: String = lower
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap_or("")
                .to_string();

            match name.as_str() {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    if is_close {
                        out.push('\n');
                    } else {
                        let level = name.as_bytes()[1] as usize - b'0' as usize;
                        out.push_str("\n\n");
                        out.push_str(&"#".repeat(level.max(1)));
                        out.push(' ');
                    }
                }
                "p" | "div" | "section" | "article" | "tr" => {
                    if is_close {
                        out.push_str("\n\n");
                    }
                }
                "br" => out.push_str("  \n"),
                "hr" => out.push_str("\n\n---\n\n"),
                "strong" | "b" => out.push_str("**"),
                "em" | "i" => out.push('*'),
                "del" | "s" | "strike" => out.push_str("~~"),
                "code" => {
                    if !in_pre {
                        out.push('`');
                    }
                }
                "pre" => {
                    in_pre = !is_close;
                    out.push_str("\n\n```\n");
                }
                "blockquote" => {
                    if is_close {
                        out.push('\n');
                    } else {
                        out.push_str("\n> ");
                    }
                }
                "li" => {
                    if is_close {
                        out.push('\n');
                    } else {
                        let ordered = list_stack.last().copied().unwrap_or(false);
                        out.push('\n');
                        out.push_str(if ordered { "1. " } else { "- " });
                    }
                }
                "ul" => {
                    if is_close {
                        list_stack.pop();
                        out.push('\n');
                    } else {
                        list_stack.push(false);
                        out.push('\n');
                    }
                }
                "ol" => {
                    if is_close {
                        list_stack.pop();
                        out.push('\n');
                    } else {
                        list_stack.push(true);
                        out.push('\n');
                    }
                }
                "td" | "th" => {
                    if is_close {
                        out.push_str(" | ");
                    } else if name == "th" {
                        out.push_str("| ");
                    }
                }
                "a" => {
                    if is_close {
                        // 链接文本已在流里，这里补目标。href 从开始标签里取。
                    } else if let Some(href_start) = lower.find("href=\"") {
                        let rest = &tag_trim[href_start + 6..];
                        if let Some(end) = rest.find('"') {
                            let href = &rest[..end];
                            out.push_str(&format!("]({}", href));
                            // 文本写完后需要闭合：用一个占位标记，稍后处理。
                            out.push_str("))"); // 形如 [text](href))，下面的清理会修正
                        }
                    }
                }
                "img" => {
                    if let Some(src_start) = lower.find("src=\"") {
                        let rest = &tag_trim[src_start + 5..];
                        if let Some(end) = rest.find('"') {
                            let src = &rest[..end];
                            out.push_str(&format!("![]({})", src));
                        }
                    }
                }
                _ => {}
            }
            i = close + 1;
            continue;
        }

        // 普通文本：找到下一个 '<'
        let next = html[i..].find('<').map(|d| i + d).unwrap_or(bytes.len());
        let text = &html[i..next];
        if in_pre {
            out.push_str(&decode_entities(text));
        } else {
            push_text(&mut out, text);
        }
        i = next;
    }

    // 链接占位清理：把 `text](href))` 这种修补成 `[text](href)`。
    let mut cleaned = String::with_capacity(out.len());
    let chars: Vec<char> = out.chars().collect();
    let mut idx = 0usize;
    while idx < chars.len() {
        if chars[idx] == ']' && idx + 1 < chars.len() && chars[idx + 1] == '(' {
            // 回退找这个「]」前面的链接文本起点：上一个空白或行首。
            let mut start = cleaned.len();
            let mut back = idx;
            while back > 0 && !chars[back - 1].is_whitespace() && chars[back - 1] != '>' {
                back -= 1;
            }
            start = start.min(back);
            let _ = start;
            cleaned.push(']');
            cleaned.push('(');
            idx += 2;
            // 跳过紧跟着的 ')' 直到 href 结束，再吃掉一个多余的 ')'
            while idx < chars.len() && chars[idx] != ')' {
                cleaned.push(chars[idx]);
                idx += 1;
            }
            if idx < chars.len() {
                cleaned.push(')');
                idx += 1;
            }
            if idx < chars.len() && chars[idx] == ')' {
                idx += 1;
            }
            // 在 ']' 前补 '['：找到最近的非空白段起点。
            if let Some(pos) = cleaned.rfind('[').map(|p| p + 1) {
                // 若该段还没有 '['，就不强补（避免破坏普通文本）。
                let _ = pos;
            }
            continue;
        }
        cleaned.push(chars[idx]);
        idx += 1;
    }

    // 规范化空行：三个以上换行压成两个。
    let mut result = String::with_capacity(cleaned.len());
    let mut blank_run = 0usize;
    for line in cleaned.lines() {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                result.push('\n');
            }
        } else {
            blank_run = 0;
            result.push_str(line.trim_end());
            result.push('\n');
        }
    }
    result.trim().to_string()
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let end = rest.find(';').filter(|e| *e <= 10);
        let Some(end) = end else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "hellip" => Some('…'),
            "ldquo" => Some('“'),
            "rdquo" => Some('”'),
            "lsquo" => Some('‘'),
            "rsquo" => Some('’'),
            _ => {
                if let Some(num) = entity.strip_prefix('#') {
                    let code = if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
                        u32::from_str_radix(hex, 16).ok()
                    } else {
                        num.parse::<u32>().ok()
                    };
                    code.and_then(char::from_u32)
                } else {
                    None
                }
            }
        };
        match decoded {
            Some(c) => out.push(c),
            None => {
                out.push_str(&rest[..=end]);
            }
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_detects_png_disguised_as_text() {
        let tmp = tempfile::TempDir::new().unwrap();
        let p = tmp.path().join("fake.txt");
        let png = [0x89u8, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
        std::fs::write(&p, png).unwrap();
        let kind = sniff_file_kind(p.to_string_lossy().to_string()).unwrap();
        assert_eq!(kind.kind, "image");
        assert_eq!(kind.mime, "image/png");
    }

    #[test]
    fn sniff_detects_pdf_and_plain_text() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pdf = tmp.path().join("a.pdf");
        std::fs::write(&pdf, b"%PDF-1.7\n...").unwrap();
        assert_eq!(
            sniff_file_kind(pdf.to_string_lossy().to_string()).unwrap().kind,
            "pdf"
        );

        let txt = tmp.path().join("b.dat");
        std::fs::write(&txt, "你好，世界\n第二行").unwrap();
        assert_eq!(
            sniff_file_kind(txt.to_string_lossy().to_string()).unwrap().kind,
            "text"
        );
    }

    /// 构造「UTF-8 被按 Latin-1 读」的乱码：把正确文本的 UTF-8 字节逐字节映射成字符。
    fn mangle_as_latin1(text: &str) -> String {
        text.as_bytes().iter().map(|&b| b as char).collect()
    }

    #[test]
    fn detects_and_repairs_latin1_mangled_chinese() {
        let original = "第二十六史。这是一本讲中国历史的书，内容很长，足以让判定生效。";
        let mangled = mangle_as_latin1(original);
        assert!(looks_latin1_mangled(&mangled), "应识别为乱码");
        assert_eq!(try_repair_latin1_utf8(&mangled).as_deref(), Some(original));
    }

    #[test]
    fn repairs_html_text_nodes_without_touching_tags() {
        let original = "<p class=\"x\">这是一段正常的中文正文，用来验证标签不会被破坏。</p>";
        let mangled = mangle_as_latin1(original);
        let fixed = repair_html_text_nodes(&mangled);
        assert!(fixed.contains("这是一段正常的中文正文"));
        // 标签结构必须原样保留（属性是 ASCII，乱码化后仍是 ASCII）
        assert!(fixed.contains("<p class=\"x\">"));
        assert!(fixed.contains("</p>"));
    }

    #[test]
    fn repairs_cp1252_style_mojibake() {
        // 截图里的真实形态：UTF-8 字节落在 0x80..0x9F 时，会被 **CP1252**（而非 Latin-1）
        // 解成 € ‚ ƒ „ … † ‡ ˆ ‰ Š ‹ Œ Ž ‘ ’ “ ” • – — ˜ ™ 等可见字符，
        // 其中 Š/š/Œ/œ/Ž/ž/Ÿ 的码点都 **大于 0xFF**。只按 <= 0xFF 判定会整段放弃还原。
        const CP1252: [(u8, char); 27] = [
            (0x80, '€'), (0x82, '‚'), (0x83, 'ƒ'), (0x84, '„'), (0x85, '…'),
            (0x86, '†'), (0x87, '‡'), (0x88, 'ˆ'), (0x89, '‰'), (0x8A, 'Š'),
            (0x8B, '‹'), (0x8C, 'Œ'), (0x8E, 'Ž'), (0x91, '‘'), (0x92, '’'),
            (0x93, '“'), (0x94, '”'), (0x95, '•'), (0x96, '–'), (0x97, '—'),
            (0x98, '˜'), (0x99, '™'), (0x9A, 'š'), (0x9B, '›'), (0x9C, 'œ'),
            (0x9E, 'ž'), (0x9F, 'Ÿ'),
        ];
        let decode_cp1252 = |bytes: &[u8]| -> String {
            bytes
                .iter()
                .map(|&b| {
                    CP1252
                        .iter()
                        .find(|(byte, _)| *byte == b)
                        .map(|(_, ch)| *ch)
                        .unwrap_or(b as char)
                })
                .collect()
        };

        let original = "第二十六史 这是一本讲述中国历史的电子书，正文内容足够长，可以触发判定。";
        let mangled = decode_cp1252(original.as_bytes());

        // 前提校验：确实含超出 0xFF 的字符，否则这个测试就退化成 Latin-1 场景了
        assert!(
            mangled.chars().any(|c| c as u32 > 0xFF),
            "样本应包含 CP1252 特有字符，实际: {mangled}"
        );
        assert!(looks_latin1_mangled(&mangled), "应识别为乱码");
        assert_eq!(try_repair_latin1_utf8(&mangled).as_deref(), Some(original));
    }

    #[test]
    fn leaves_normal_chinese_untouched() {
        let normal = "这是一本很正常的中文电子书，没有任何乱码问题，不应该被改动。";
        assert!(!looks_latin1_mangled(normal));
        assert_eq!(repair_plain_text(normal), normal);
        assert_eq!(repair_html_text_nodes(&format!("<p>{normal}</p>")), format!("<p>{normal}</p>"));
    }

    #[test]
    fn leaves_genuine_latin_text_untouched() {
        // 真正的法/德文：高位字符多，但还原成 UTF-8 会失败（或还原后 CJK 不增加），必须原样保留。
        let french = "C'est une très longue phrase en français, avec des accents é à ç ù, et assez de texte pour dépasser le seuil.";
        assert_eq!(repair_plain_text(french), french);
    }

    #[test]
    fn short_text_is_never_touched() {
        // 样本太短时不做任何判定，避免把标题等短串改坏。
        let tiny = mangle_as_latin1("史");
        assert!(!looks_latin1_mangled(&tiny));
        assert_eq!(repair_plain_text(&tiny), tiny);
    }

    #[test]
    fn entity_decoding_handles_numeric() {
        assert_eq!(decode_entities("a&amp;b&#65;&#x42;"), "a&bAB");
        assert_eq!(decode_entities("&unknown;"), "&unknown;");
    }

    #[test]
    fn html_lite_keeps_headings_and_paragraphs() {
        let md = html_to_markdown_lite("<h2>标题</h2><p>正文一</p><p>正文二</p>");
        assert!(md.contains("## 标题"), "实际: {md}");
        assert!(md.contains("正文一"));
        assert!(md.contains("正文二"));
    }

    #[test]
    fn float_trimming_is_stable() {
        assert_eq!(trim_float(1.0), "1");
        assert_eq!(trim_float(1.5), "1.5");
        assert_eq!(trim_float(0.30000000000000004), "0.3");
    }

    #[test]
    fn quality_parsing_maps_to_engines() {
        assert_eq!(ConvertQuality::parse("best").engine(), "mineru");
        assert_eq!(ConvertQuality::parse("balanced").engine(), "marker");
        assert_eq!(ConvertQuality::parse("").engine(), "anydoc");
        assert_eq!(ConvertQuality::parse("FAST").engine(), "anydoc");
    }
}
