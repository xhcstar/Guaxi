//! Feeds 库：把清洗后的文章写成 Markdown 文件，作为**正文唯一真源**。
//!
//! 设计要点（对应设计文档第 7 节）：
//!
//! 1. **Markdown 是正文真源**，SQLite 只存索引与状态。这解决"库体积随抓取无界膨胀"
//!    （实测出现过 128MB 装 120 行、98% 是空闲页）。
//! 2. **目录即结构**：`Feeds/<collection>/<source>/<yyyy-MM>/<date>-<slug>.md`，
//!    可被 kgraph/fulltext/备份直接消费，不需要额外导出。
//! 3. **frontmatter 带质量分与抽取器版本**，让"规则升级后哪些要重算"变成一条查询。
//! 4. **入盘受质量门约束**（设计文档 12.1 的闸一）：低于门限的不产生文件，
//!    只在 SQLite 留一行带拒收原因——这是"不囤积"的第一道闸。
//!
//! 与知识库的关系：`Feeds/` 是知识库根下的一个**顶层目录**，因此天然被
//! `kgraph::kb_name_from_path` 识别为独立知识库，无需注册。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Feeds 库的默认目录名（知识库根下）。
pub const DEFAULT_FEEDS_DIR: &str = "Feeds";

/// 把文章写盘所需的最小数据。
#[derive(Debug, Clone)]
pub struct ArticleDoc<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub url: &'a str,
    pub source: &'a str,
    pub source_id: &'a str,
    pub collection: &'a str,
    pub published_at: &'a str,
    pub fetched_at: &'a str,
    pub author: Option<&'a str>,
    pub lang: Option<&'a str>,
    /// 清洗后的 Markdown 正文
    pub body: &'a str,
    /// 质量分 0–100
    pub quality: u8,
    /// 页面类型（article / unknown …）
    pub page_kind: &'a str,
    /// 抽取器标识与版本
    pub extractor: &'a str,
    pub extractor_version: u32,
    pub tags: &'a [String],
    pub summary: &'a str,
    /// 已本地化的图片相对路径列表（相对文档所在目录）
    pub local_images: Vec<String>,
}

/// 写盘结果。
#[derive(Debug, Clone)]
pub struct SavedDoc {
    /// 绝对路径。写盘时构造，供"在文件管理器中显示"等 UI 功能使用。
    #[allow(dead_code)]
    pub path: PathBuf,
    pub rel_path: String,
    pub bytes: usize,
}

/// Feeds 库设置。独立配置文件，避免污染带加密逻辑的主配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct FeedsSettings {
    /// 是否启用 Markdown 双写。关掉后只保留 SQLite。
    pub markdown_enabled: bool,
    /// 库根目录名（知识库根之下）。默认 "Feeds"。
    pub dir_name: String,
    /// 入盘质量门：低于此分不写文件（闸一）。
    pub min_quality: u8,
    /// 每个 Collection 每日入盘上限（闸二）。0 = 不限。
    pub daily_admit_cap: u32,
    /// 是否本地化图片。
    pub localize_images: bool,
    /// 单图大小上限（字节）。
    pub max_image_bytes: u64,
    /// 单篇图片数量上限。
    pub max_images_per_article: usize,
}

impl Default for FeedsSettings {
    fn default() -> Self {
        FeedsSettings {
            markdown_enabled: true,
            dir_name: DEFAULT_FEEDS_DIR.to_string(),
            // 35 分：实测被拒的促销/索引页在联动页面类型后都低于 25，
            // 正常短文一般 55+，留足余量。
            min_quality: 35,
            daily_admit_cap: 200,
            localize_images: true,
            max_image_bytes: 8 * 1024 * 1024,
            max_images_per_article: 30,
        }
    }
}

impl FeedsSettings {
    /// 从 JSON 读；文件缺失/损坏时回落默认值（不能因为配置坏掉就停止采集）。
    pub fn from_json(raw: &str) -> Self {
        serde_json::from_str(raw).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// 由知识库根 + 设置得出 Feeds 库根目录。
///
/// 刻意接受 `kb_root` 参数而不是自己读配置：便于单测，也避免此处依赖
/// `settingview` 的全局状态。
pub fn feeds_root(kb_root: &Path, settings: &FeedsSettings) -> PathBuf {
    let dir = settings.dir_name.trim();
    let dir = if dir.is_empty() { DEFAULT_FEEDS_DIR } else { dir };
    kb_root.join(dir)
}

/// 生成 Markdown 文档的完整内容（frontmatter + 正文）。
pub fn render_document(doc: &ArticleDoc<'_>) -> String {
    let mut out = String::with_capacity(doc.body.len() + 1024);
    out.push_str("---\n");
    out.push_str(&format!("id: {}\n", yaml_str(doc.id)));
    out.push_str(&format!("title: {}\n", yaml_str(doc.title)));
    out.push_str(&format!("url: {}\n", yaml_str(doc.url)));
    out.push_str(&format!("canonical_url: {}\n", yaml_str(doc.url)));
    out.push_str(&format!("source: {}\n", yaml_str(doc.source)));
    out.push_str(&format!("source_id: {}\n", yaml_str(doc.source_id)));
    out.push_str(&format!("collection: {}\n", yaml_str(doc.collection)));
    if let Some(a) = doc.author.filter(|a| !a.trim().is_empty()) {
        out.push_str(&format!("author: {}\n", yaml_str(a)));
    }
    out.push_str(&format!("published_at: {}\n", yaml_str(doc.published_at)));
    out.push_str(&format!("fetched_at: {}\n", yaml_str(doc.fetched_at)));
    if let Some(l) = doc.lang.filter(|l| !l.trim().is_empty()) {
        out.push_str(&format!("lang: {}\n", yaml_str(l)));
    }
    out.push_str(&format!("quality: {}\n", doc.quality));
    out.push_str(&format!("page_kind: {}\n", yaml_str(doc.page_kind)));
    out.push_str(&format!("extractor: {}\n", yaml_str(doc.extractor)));
    out.push_str(&format!("extractor_version: {}\n", doc.extractor_version));
    if !doc.tags.is_empty() {
        out.push_str("tags:\n");
        for t in doc.tags {
            out.push_str(&format!("  - {}\n", yaml_str(t)));
        }
    }
    if !doc.summary.trim().is_empty() {
        out.push_str(&format!("summary: {}\n", yaml_str(doc.summary)));
    }
    if !doc.local_images.is_empty() {
        out.push_str("images:\n");
        for img in &doc.local_images {
            out.push_str(&format!("  - {}\n", yaml_str(img)));
        }
    }
    out.push_str("---\n\n");
    out.push_str(doc.body.trim());
    out.push('\n');
    out
}

/// YAML 标量转义。
///
/// 只处理会破坏 frontmatter 结构的字符：双引号、反斜杠、换行。
/// 用双引号包裹最省心——比裸标量安全得多（标题里常有 `:` 和 `#`）。
fn yaml_str(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ");
    format!("\"{}\"", escaped)
}

/// 生成文件名用的 slug：保留中日韩与字母数字，其余换成连字符。
///
/// Windows 文件名有长度与字符限制，所以：
/// - 限制总长度 80 字符（留出日期前缀与扩展名）
/// - 去掉 Windows 保留字符 `<>:"/\|?*`
pub fn slugify(title: &str, fallback_id: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in title.chars() {
        let keep = c.is_alphanumeric()
            && !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if keep {
            out.push(c);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
        if out.chars().count() >= 80 {
            break;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        // 标题全是符号（或空）时用 id 兜底，保证文件名稳定可寻。
        let id_tail: String = fallback_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(16)
            .collect();
        if id_tail.is_empty() {
            "untitled".to_string()
        } else {
            id_tail
        }
    } else {
        trimmed
    }
}

/// 由发布时间推出 `<yyyy-MM>` 目录与 `<yyyymmdd>` 前缀。
/// 时间解析失败时回落到"当前时间"以外的一个稳定值，避免文件名每天变。
pub fn date_parts(published_at: &str) -> (String, String) {
    // 支持 RFC3339（2026-11-04T…）与纯日期（2026-11-04）。
    let date = if published_at.len() >= 10 {
        &published_at[..10]
    } else {
        published_at
    };
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() == 3
        && parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
    {
        let month = format!("{}-{}", parts[0], parts[1]);
        let day = format!("{}{}{}", parts[0], parts[1], parts[2]);
        (month, day)
    } else {
        // 未知发布时间：归到 undated 目录，便于之后统一重试/排序。
        ("undated".to_string(), "00000000".to_string())
    }
}

/// 计算文档相对路径：`<collection>/<source>/<yyyy-MM>/<day>-<slug>.md`。
pub fn article_rel_path(doc: &ArticleDoc<'_>) -> String {
    let (month, day) = date_parts(doc.published_at);
    let collection = path_segment(doc.collection, "uncategorized");
    let source = path_segment(doc.source_id_or_name(), "unknown-source");
    let slug = slugify(doc.title, doc.id);
    format!("{}/{}/{}/{}-{}.md", collection, source, month, day, slug)
}

impl<'a> ArticleDoc<'a> {
    /// 源目录名：优先 source_id（稳定、无空格），退化到 source 名称。
    fn source_id_or_name(&self) -> &str {
        if !self.source_id.trim().is_empty() {
            self.source_id
        } else {
            self.source
        }
    }
}

/// 把任意字符串变成安全的单层目录名。
fn path_segment(raw: &str, fallback: &str) -> String {
    let s = slugify(raw, fallback);
    if s.is_empty() {
        fallback.to_string()
    } else {
        s
    }
}

/// 写文档到磁盘。已存在且内容相同则跳过（幂等），内容变化则覆盖。
pub fn write_document(root: &Path, doc: &ArticleDoc<'_>) -> Result<SavedDoc, String> {
    let rel = article_rel_path(doc);
    let abs = root.join(&rel);
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create dir failed: {}", e))?;
    }

    let content = render_document(doc);
    // 幂等：内容没变就不写，避免无谓的 mtime 变动触发 kgraph 重建。
    if let Ok(existing) = fs::read_to_string(&abs) {
        if existing == content {
            return Ok(SavedDoc {
                path: abs,
                rel_path: rel,
                bytes: content.len(),
            });
        }
    }
    fs::write(&abs, &content).map_err(|e| format!("write doc failed: {}", e))?;

    // 通知 kgraph 该知识库需要重建（Feeds 是顶层目录，会被识别为库名）。
    let _ = crate::commands::kgraph::after_create_path(&abs, false, Some(&content));
    let _ = crate::commands::kgraph::after_write_file(&abs, &content);

    Ok(SavedDoc {
        path: abs,
        rel_path: rel,
        bytes: content.len(),
    })
}

/// 删除一篇文档（保留策略回收时用）。返回是否真的删掉了文件。
/// 回收站目录名（库根下）。软删除的文件移到这里，保留一段时间供撤销。
pub const TRASH_DIR: &str = "_trash";

/// 软删除一篇文档：移动到 `_trash/<时间戳>/<原相对路径>`，而不是直接 unlink。
///
/// 为什么要软删：保留策略按"未读 21 天"自动回收，误判是可能的
/// （比如用户出差两周没开应用）。直接 unlink 不可恢复，
/// 而移动一份文件几乎零成本，却能换来 24 小时反悔窗口。
///
/// 返回 `Some(回收站内的相对路径)`，供后续撤销或彻底删除。
pub fn soft_delete_document(
    root: &Path,
    rel_path: &str,
    stamp: &str,
) -> Option<String> {
    if !safe_rel_path(rel_path) {
        return None;
    }
    let abs = root.join(rel_path);
    if !abs.is_file() {
        return None;
    }
    // 保留原目录结构，撤销时能原样放回。
    let trash_rel = format!("{}/{}/{}", TRASH_DIR, stamp, rel_path);
    let trash_abs = root.join(&trash_rel);
    if let Some(parent) = trash_abs.parent() {
        if fs::create_dir_all(parent).is_err() {
            return None;
        }
    }
    if fs::rename(&abs, &trash_abs).is_err() {
        // 跨盘/权限问题时退回"复制+删除"，仍保证可撤销。
        if fs::copy(&abs, &trash_abs).is_err() {
            return None;
        }
        let _ = fs::remove_file(&abs);
    }
    let _ = crate::commands::kgraph::after_delete_path(&abs, false);
    prune_empty_parents(root, abs.parent());
    Some(trash_rel)
}

/// 撤销软删除：把文件从回收站放回原位。
pub fn restore_document(root: &Path, trash_rel: &str) -> Option<String> {
    if !safe_rel_path(trash_rel) || !trash_rel.starts_with(&format!("{}/", TRASH_DIR)) {
        return None;
    }
    let trash_abs = root.join(trash_rel);
    if !trash_abs.is_file() {
        return None;
    }
    // `_trash/<stamp>/<原相对路径>` → 取 `<stamp>/` 之后的部分
    let rest = trash_rel.strip_prefix(&format!("{}/", TRASH_DIR))?;
    let orig_rel = rest.split_once('/').map(|(_, p)| p)?;
    let orig_abs = root.join(orig_rel);
    if let Some(parent) = orig_abs.parent() {
        if fs::create_dir_all(parent).is_err() {
            return None;
        }
    }
    // 原位已有同名文件时不覆盖——那是新抓的同名文章，让用户自己决定。
    if orig_abs.exists() {
        return None;
    }
    if fs::rename(&trash_abs, &orig_abs).is_err() {
        if fs::copy(&trash_abs, &orig_abs).is_err() {
            return None;
        }
        let _ = fs::remove_file(&trash_abs);
    }
    let _ = crate::commands::kgraph::after_create_path(&orig_abs, false, None);
    Some(orig_rel.to_string())
}

/// 彻底删除超过宽限期的回收站内容。返回清理的文件数。
pub fn empty_trash_older_than(root: &Path, cutoff_stamp: &str) -> usize {
    let trash = root.join(TRASH_DIR);
    let Ok(entries) = fs::read_dir(&trash) else {
        return 0;
    };
    let mut removed = 0usize;
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        // 目录名是时间戳（`YYYYMMDD-HHMMSS`），按字典序比较即可。
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name < cutoff_stamp && fs::remove_dir_all(&p).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// 相对路径安全校验：禁止绝对路径、`..` 逃逸。
fn safe_rel_path(rel_path: &str) -> bool {
    !rel_path.is_empty()
        && !rel_path.starts_with('/')
        && !rel_path.starts_with('\\')
        && !rel_path.contains("..")
        && !rel_path.contains(':')
}

/// 直接删除（不经回收站）。
///
/// 当前保留策略走 `soft_delete_document`；这个函数留给"用户确认永久删除"
/// 与"清空回收站"两条路径。保留它是因为软删除之后仍然需要一个不可撤销的出口。
#[allow(dead_code)]
pub fn remove_document(root: &Path, rel_path: &str) -> bool {
    if !safe_rel_path(rel_path) {
        return false;
    }
    let abs = root.join(rel_path);
    let removed = fs::remove_file(&abs).is_ok();
    if removed {
        let _ = crate::commands::kgraph::after_delete_path(&abs, false);
        // 顺带清掉空目录，避免年积月累留下一堆空月份目录。
        prune_empty_parents(root, abs.parent());
    }
    removed
}

fn prune_empty_parents(root: &Path, mut dir: Option<&Path>) {
    while let Some(d) = dir {
        if d == root || !d.starts_with(root) {
            break;
        }
        match fs::read_dir(d) {
            Ok(mut it) => {
                if it.next().is_some() {
                    break; // 非空，停止
                }
            }
            Err(_) => break,
        }
        if fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }
}

/// 图片本地化的结果。
#[derive(Debug, Default, Clone)]
pub struct LocalizedImages {
    /// 已本地化的图片：原始 URL → 相对文档目录的路径
    pub mapped: Vec<(String, String)>,
    /// 下载失败的 URL（保留远程引用，并标记）
    pub failed: Vec<String>,
    /// 因体积/数量上限被跳过的 URL
    pub skipped: Vec<String>,
}

/// 本地化一篇正文里引用的图片。
///
/// 为什么本地化：
/// - 离线可读（release 包离线自足的既有要求）
/// - 源站删图后文档不残缺
/// - 训练语料图文配对完整
///
/// 存储布局：`<feeds_root>/_assets/<sha256 前两位>/<sha256>.<ext>`，内容寻址，
/// 同一张图被多篇文章引用只存一份。文档里用**相对路径**引用，
/// 这样整个 Feeds 目录可以整体移动到别处而不破坏图片链接。
///
/// 目录深度的选取：文档在 `<collection>/<source>/<yyyy-MM>/` 下，
/// 到 `_assets` 是四层，所以相对路径前缀固定为 `../../../../_assets/`。
pub fn localize_images(
    feeds_root: &Path,
    image_urls: &[String],
    settings: &FeedsSettings,
) -> LocalizedImages {
    let mut result = LocalizedImages::default();
    if !settings.localize_images || image_urls.is_empty() {
        return result;
    }

    let max_n = settings.max_images_per_article.max(1);
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (compatible; GuaxiFeeds/1.0)")
        .build()
    {
        Ok(c) => c,
        Err(_) => return result,
    };

    for url in image_urls.iter().take(max_n) {
        let url = url.trim();
        if url.is_empty() {
            continue;
        }
        match download_one(&client, feeds_root, url, settings) {
            Ok(rel) => result.mapped.push((url.to_string(), rel)),
            Err(LocalizeError::TooLarge) | Err(LocalizeError::Unsupported) => {
                result.skipped.push(url.to_string())
            }
            Err(_) => result.failed.push(url.to_string()),
        }
    }
    result
}

enum LocalizeError {
    Network,
    TooLarge,
    Unsupported,
}

fn download_one(
    client: &reqwest::blocking::Client,
    feeds_root: &Path,
    url: &str,
    settings: &FeedsSettings,
) -> Result<String, LocalizeError> {
    // 只下载 http(s)，跳过 data: 与相对地址（相对地址无法脱离原站解析）。
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(LocalizeError::Unsupported);
    }

    let resp = client.get(url).send().map_err(|_| LocalizeError::Network)?;
    if !resp.status().is_success() {
        return Err(LocalizeError::Network);
    }
    if let Some(len) = resp.content_length() {
        if len > settings.max_image_bytes {
            return Err(LocalizeError::TooLarge);
        }
    }
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let ext = image_ext(&content_type).ok_or(LocalizeError::Unsupported)?;

    let bytes = resp.bytes().map_err(|_| LocalizeError::Network)?;
    if bytes.len() as u64 > settings.max_image_bytes {
        return Err(LocalizeError::TooLarge);
    }

    // 内容寻址：同一张图多篇引用只存一份。
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(&bytes);
    let digest = format!("{:x}", h.finalize());
    let rel = format!("_assets/{}/{}.{}", &digest[..2], digest, ext);

    let abs = feeds_root.join(&rel);
    if !abs.exists() {
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).map_err(|_| LocalizeError::Network)?;
        }
        fs::write(&abs, &bytes).map_err(|_| LocalizeError::Network)?;
    }
    Ok(rel)
}

fn image_ext(content_type: &str) -> Option<&'static str> {
    match content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        "image/avif" => Some("avif"),
        "image/bmp" => Some("bmp"),
        _ => None,
    }
}

/// 把正文里的图片 URL 换成相对路径。
///
/// 只替换 Markdown 图片语法 `![alt](url)` 与 HTML `<img src="url">`，
/// 不碰普通链接——正文里的超链接应当保持指向原站。
///
/// 相对前缀固定为 `../../../`，因为文档位于
/// `<collection>/<source>/<yyyy-MM>/<file>.md`（从文件所在目录到库根是 3 层），
/// 而 `_assets` 在库根下。
///
/// `undated` 目录同样是 3 层，所以前缀不变。
pub fn rewrite_image_urls(body: &str, mapped: &[(String, String)]) -> String {
    if mapped.is_empty() {
        return body.to_string();
    }
    const PREFIX: &str = "../../../";

    // 建一张 "远程 URL → 本地相对路径" 表，扫描时一次查。
    let lookup: std::collections::HashMap<&str, String> = mapped
        .iter()
        .map(|(remote, rel)| (remote.as_str(), format!("{}{}", PREFIX, rel)))
        .collect();

    let chars: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len());
    let mut i = 0;

    while i < chars.len() {
        // 情况一：HTML 的 <img src="...">
        if chars[i] == '<' {
            let rest: String = chars[i..].iter().take(5).collect();
            if rest.eq_ignore_ascii_case("<img ") || rest.eq_ignore_ascii_case("<img>") {
                // 找到这个标签的结束位置
                if let Some(end) = chars[i..].iter().position(|c| *c == '>') {
                    let tag: String = chars[i..=i + end].iter().collect();
                    let rewritten = rewrite_img_tag_src(&tag, &lookup);
                    out.push_str(&rewritten);
                    i += end + 1;
                    continue;
                }
            }
        }

        // 情况二：Markdown 图片 ![alt](url)——必须先看到 '!' 再看到 ']('。
        if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            // 找配对的 `](`
            if let Some(bracket) = find_md_image_url_start(&chars, i) {
                let url_start = bracket + 2; // 跳过 `](`
                if let Some(url_end) = chars[url_start..].iter().position(|c| *c == ')') {
                    let url: String = chars[url_start..url_start + url_end].iter().collect();
                    let trimmed = url.trim();
                    // 必须先原样输出 `![alt](`，只替换括号里的 URL。
                    out.extend(&chars[i..url_start]);
                    if let Some(local) = lookup.get(trimmed) {
                        out.push_str(local);
                    } else {
                        out.push_str(&url);
                    }
                    i = url_start + url_end;
                    continue;
                }
            }
        }

        out.push(chars[i]);
        i += 1;
    }
    out
}

/// 从 `![` 开始，找到图片语法里 `](` 的 `]` 位置。
fn find_md_image_url_start(chars: &[char], bang_pos: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = bang_pos + 1; // 指向 '['
    while i < chars.len() {
        match chars[i] {
            '[' => depth += 1,
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    // 必须是 ]( 才算图片 URL
                    if i + 1 < chars.len() && chars[i + 1] == '(' {
                        return Some(i);
                    }
                    return None;
                }
            }
            '\n' => return None, // 图片语法不跨行
            _ => {}
        }
        i += 1;
    }
    None
}

/// 把 `<img ... src="远程">` 里的 src 换成相对路径。
fn rewrite_img_tag_src(
    tag: &str,
    lookup: &std::collections::HashMap<&str, String>,
) -> String {
    let Some(src_start) = tag.find("src=\"") else {
        return tag.to_string();
    };
    let value_start = src_start + 5;
    let Some(len) = tag[value_start..].find('"') else {
        return tag.to_string();
    };
    let remote = &tag[value_start..value_start + len];
    let Some(local) = lookup.get(remote.trim()) else {
        return tag.to_string();
    };
    format!(
        "{}{}{}",
        &tag[..value_start],
        local,
        &tag[value_start + len..]
    )
}

/// 保留策略设置（设计文档 7.5 的第三道闸）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct RetentionPolicy {
    /// 未读文章的保留天数（0 = 不清理）。
    ///
    /// 这是"不囤积"的第三道闸，也是最容易被忽略的一条：
    /// 前两道（质量门、每日上限）限制**新增**，这道限制**存量**。
    pub unread_days: u32,
    /// 已读文章的保留天数（0 = 永久）。
    pub read_days: u32,
    /// 软删除宽限期：清理后多少小时内还能撤销。
    pub undo_hours: u32,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        RetentionPolicy {
            unread_days: 21,
            read_days: 180,
            undo_hours: 24,
        }
    }
}

/// 一条待回收的条目。
#[derive(Debug, Clone)]
pub struct PurgeCandidate {
    pub url_norm: String,
    pub doc_path: String,
    /// 为什么被选中（写日志/给用户看）
    pub reason: &'static str,
}

/// 判断哪些条目到期。`now_secs` 是当前 Unix 时间。
///
/// 输入是 `(url_norm, doc_path, first_seen, last_opened)` 的四元组；
/// `last_opened` 为 None 表示从未打开。
pub fn select_purge_candidates(
    items: &[(String, Option<String>, i64, Option<i64>)],
    policy: &RetentionPolicy,
    now_secs: i64,
) -> Vec<PurgeCandidate> {
    let day = 86_400i64;
    let mut out = Vec::new();
    for (url_norm, doc_path, first_seen, last_opened) in items {
        // 没有落盘文件的条目由 SQLite 侧的状态清理负责，这里只管文件。
        let Some(path) = doc_path.as_ref().filter(|p| !p.trim().is_empty()) else {
            continue;
        };
        match last_opened {
            Some(opened) => {
                if policy.read_days > 0
                    && now_secs - *opened > policy.read_days as i64 * day
                {
                    out.push(PurgeCandidate {
                        url_norm: url_norm.clone(),
                        doc_path: path.clone(),
                        reason: "已读且超过保留期",
                    });
                }
            }
            None => {
                if policy.unread_days > 0
                    && now_secs - *first_seen > policy.unread_days as i64 * day
                {
                    out.push(PurgeCandidate {
                        url_norm: url_norm.clone(),
                        doc_path: path.clone(),
                        reason: "从未打开且超过保留期",
                    });
                }
            }
        }
    }
    out
}
/// 每日入盘计数（闸二）。按 `<collection>` 记，跨进程由调用方持久化。
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DailyAdmitCounter {
    /// 日期（YYYY-MM-DD）→ collection → 已入盘数量
    pub by_day: HashMap<String, HashMap<String, u32>>,
}

impl DailyAdmitCounter {
    /// 今日是否还能再入盘 `want` 篇。`cap == 0` 表示不限。
    pub fn can_admit(&self, day: &str, collection: &str, want: u32, cap: u32) -> bool {
        if cap == 0 {
            return true;
        }
        let used = self
            .by_day
            .get(day)
            .and_then(|m| m.get(collection))
            .copied()
            .unwrap_or(0);
        used.saturating_add(want) <= cap
    }

    pub fn record(&mut self, day: &str, collection: &str, n: u32) {
        let entry = self
            .by_day
            .entry(day.to_string())
            .or_default()
            .entry(collection.to_string())
            .or_insert(0);
        *entry = entry.saturating_add(n);
    }

    /// 只保留最近 N 天，避免计数表无限增长。
    pub fn retain_days(&mut self, keep: usize) {
        if self.by_day.len() <= keep {
            return;
        }
        let mut days: Vec<String> = self.by_day.keys().cloned().collect();
        days.sort();
        for d in days.into_iter().rev().skip(keep) {
            self.by_day.remove(&d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc<'a>(title: &'a str, body: &'a str) -> ArticleDoc<'a> {
        ArticleDoc {
            id: "f1:https://x.com/a",
            title,
            url: "https://x.com/a",
            source: "少数派",
            source_id: "builtin-technology-0",
            collection: "technology",
            published_at: "2026-11-04T09:12:33Z",
            fetched_at: "2026-11-04T09:13:00Z",
            author: Some("张三"),
            lang: Some("zh"),
            body,
            quality: 88,
            page_kind: "article",
            extractor: "trafilatura@0.16.1",
            extractor_version: 3,
            tags: &[],
            summary: "",
            local_images: Vec::new(),
        }
    }

    #[test]
    fn frontmatter_contains_key_fields() {
        let d = doc("一篇标题：带冒号的 # 标题", "正文内容。");
        let out = render_document(&d);
        assert!(out.starts_with("---\n"));
        assert!(out.contains("quality: 88"));
        assert!(out.contains("extractor_version: 3"));
        // 标题里的冒号与 # 必须被安全转义，不能破坏 YAML
        assert!(out.contains("title: \"一篇标题：带冒号的 # 标题\""));
        assert!(out.contains("正文内容。"));
    }

    #[test]
    fn slug_handles_symbols_and_limits() {
        assert_eq!(slugify("Hello World!", "id"), "Hello-World");
        assert_eq!(slugify("A/B\\C:D", "id"), "A-B-C-D");
        assert_eq!(slugify("", "abcdef123"), "abcdef123");
        // 超长标题被截断到 80 个字符以内
        let long = "标".repeat(200);
        assert!(slugify(&long, "id").chars().count() <= 80);
    }

    #[test]
    fn date_parts_from_rfc3339_and_bare_date() {
        assert_eq!(
            date_parts("2026-11-04T09:12:33Z"),
            ("2026-11".to_string(), "20261104".to_string())
        );
        assert_eq!(
            date_parts("2026-11-04"),
            ("2026-11".to_string(), "20261104".to_string())
        );
        assert_eq!(
            date_parts("unknown"),
            ("undated".to_string(), "00000000".to_string())
        );
    }

    #[test]
    fn rel_path_layout() {
        let d = doc("测试文章", "正文");
        let p = article_rel_path(&d);
        assert_eq!(p, "technology/builtin-technology-0/2026-11/20261104-测试文章.md");
    }

    #[test]
    fn write_is_idempotent_and_creates_dirs() {
        let tmp = std::env::temp_dir().join(format!("guaxi-feeds-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let d = doc("幂等测试", "正文");
        let a = write_document(&tmp, &d).unwrap();
        let mtime_a = fs::metadata(&a.path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let b = write_document(&tmp, &d).unwrap();
        let mtime_b = fs::metadata(&b.path).unwrap().modified().unwrap();
        // 内容相同不应重写（mtime 不变），否则每次抓取都会触发 kgraph 重建
        assert_eq!(mtime_a, mtime_b, "identical content must not be rewritten");
        assert_eq!(a.rel_path, b.rel_path);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn remove_refuses_path_traversal() {
        let tmp = std::env::temp_dir();
        assert!(!remove_document(&tmp, "../../etc/passwd"));
        assert!(!remove_document(&tmp, "/etc/passwd"));
    }

    #[test]
    fn daily_cap_enforced() {
        let mut c = DailyAdmitCounter::default();
        assert!(c.can_admit("2026-11-04", "tech", 5, 10));
        c.record("2026-11-04", "tech", 8);
        assert!(!c.can_admit("2026-11-04", "tech", 5, 10));
        assert!(c.can_admit("2026-11-04", "tech", 2, 10));
        // cap = 0 表示不限
        assert!(c.can_admit("2026-11-04", "tech", 999, 0));
        // 不同 collection 互不影响
        assert!(c.can_admit("2026-11-04", "ai", 10, 10));
    }

    #[test]
    fn counter_retains_recent_days() {
        let mut c = DailyAdmitCounter::default();
        for d in ["2026-11-01", "2026-11-02", "2026-11-03"] {
            c.record(d, "tech", 1);
        }
        c.retain_days(2);
        assert_eq!(c.by_day.len(), 2);
        assert!(c.by_day.contains_key("2026-11-03"));
        assert!(c.by_day.contains_key("2026-11-02"));
        assert!(!c.by_day.contains_key("2026-11-01"));
    }

    #[test]
    fn settings_roundtrip_and_fallback() {
        let s = FeedsSettings::default();
        let json = s.to_json();
        let back = FeedsSettings::from_json(&json);
        assert_eq!(back.min_quality, s.min_quality);
        // 损坏的 JSON 必须回落默认值，而不是 panic 或返回零值。
        let broken = FeedsSettings::from_json("{not json");
        assert!(broken.markdown_enabled);
        assert_eq!(broken.min_quality, 35);
    }

    #[test]
    fn rewrite_images_only_touches_image_syntax() {
        let body = "见下图：\n\n![图注](https://cdn.example.com/a.png)\n\n\
                    普通链接 [原文](https://cdn.example.com/a.png) 应保持不动。\n\n\
                    <img src=\"https://cdn.example.com/b.jpg\" />";
        let mapped = vec![
            ("https://cdn.example.com/a.png".to_string(), "_assets/ab/x.png".to_string()),
            ("https://cdn.example.com/b.jpg".to_string(), "_assets/cd/y.jpg".to_string()),
        ];
        let out = rewrite_image_urls(body, &mapped);
        assert!(out.contains("![图注](../../../_assets/ab/x.png)"));
        assert!(out.contains("<img src=\"../../../_assets/cd/y.jpg\""));
        // 普通链接不应被改写
        assert!(
            out.contains("[原文](https://cdn.example.com/a.png)"),
            "plain links must not be rewritten: {}",
            out
        );
    }

    #[test]
    fn image_ext_from_content_type() {
        assert_eq!(image_ext("image/jpeg; charset=binary"), Some("jpg"));
        assert_eq!(image_ext("image/png"), Some("png"));
        assert_eq!(image_ext("text/html"), None);
    }

    #[test]
    fn localize_disabled_returns_empty() {
        let settings = FeedsSettings { localize_images: false, ..Default::default() };
        let tmp = std::env::temp_dir();
        let r = localize_images(&tmp, &["https://x.com/a.png".to_string()], &settings);
        assert!(r.mapped.is_empty() && r.failed.is_empty());
    }

    #[test]
    fn retention_purges_unread_after_window() {
        let day = 86_400i64;
        let now = 1_700_000_000i64;
        let policy = RetentionPolicy::default(); // unread 21 天

        let items = vec![
            // 从未打开、25 天前入库 → 应清理
            (
                "u1".to_string(),
                Some("tech/src/2026-10/a.md".to_string()),
                now - 25 * day,
                None,
            ),
            // 从未打开、5 天前入库 → 保留
            (
                "u2".to_string(),
                Some("tech/src/2026-11/b.md".to_string()),
                now - 5 * day,
                None,
            ),
            // 已读、10 天前 → 保留（远未到 180 天）
            (
                "u3".to_string(),
                Some("tech/src/2026-11/c.md".to_string()),
                now - 400 * day,
                Some(now - 10 * day),
            ),
            // 没有文件 → 不管
            ("u4".to_string(), None, now - 400 * day, None),
        ];

        let purged = select_purge_candidates(&items, &policy, now);
        assert_eq!(purged.len(), 1, "only the stale unread item should be purged");
        assert_eq!(purged[0].url_norm, "u1");
    }

    #[test]
    fn retention_keeps_forever_when_disabled() {
        let day = 86_400i64;
        let now = 1_700_000_000i64;
        let policy = RetentionPolicy { unread_days: 0, read_days: 0, undo_hours: 24 };
        let items = vec![(
            "u".to_string(),
            Some("a/b/c.md".to_string()),
            now - 10_000 * day,
            None,
        )];
        assert!(
            select_purge_candidates(&items, &policy, now).is_empty(),
            "unread_days=0 must disable purging"
        );
    }

    #[test]
    fn read_items_follow_read_window() {
        let day = 86_400i64;
        let now = 1_700_000_000i64;
        let policy = RetentionPolicy::default(); // read 180 天
        let items = vec![
            (
                "old-read".to_string(),
                Some("a.md".to_string()),
                now - 500 * day,
                Some(now - 200 * day),
            ),
            (
                "recent-read".to_string(),
                Some("b.md".to_string()),
                now - 500 * day,
                Some(now - 10 * day),
            ),
        ];
        let purged = select_purge_candidates(&items, &policy, now);
        assert_eq!(purged.len(), 1);
        assert_eq!(purged[0].url_norm, "old-read");
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// 端到端：真实文章 → 写盘 → 检查目录布局与 frontmatter 可解析。
    #[test]
    fn writes_real_article_with_correct_layout() {
        let tmp = std::env::temp_dir().join(format!("guaxi-feeds-e2e-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let body = "## 引言\n\n这是一篇真实结构的文章正文。\n\n## 结论\n\n总结要点。";
        let doc = ArticleDoc {
            id: "builtin-ai-0:https://arxiv.org/abs/1706.03762",
            title: "Attention Is All You Need",
            url: "https://arxiv.org/abs/1706.03762",
            source: "arXiv cs.AI",
            source_id: "builtin-ai-0",
            collection: "ai",
            published_at: "2017-06-12T00:00:00Z",
            fetched_at: "2026-11-04T09:00:00Z",
            author: Some("Vaswani"),
            lang: Some("en"),
            body,
            quality: 92,
            page_kind: "article",
            extractor: "trafilatura@0.16.1",
            extractor_version: 3,
            tags: &["transformer".to_string()],
            summary: "提出 Transformer 架构。",
            local_images: vec![],
        };

        let saved = write_document(&tmp, &doc).unwrap();
        assert_eq!(
            saved.rel_path,
            "ai/builtin-ai-0/2017-06/20170612-Attention-Is-All-You-Need.md"
        );
        assert!(saved.path.exists());

        let content = fs::read_to_string(&saved.path).unwrap();
        // frontmatter 必须以 --- 开头并闭合
        assert!(content.starts_with("---\n"));
        let fm_end = content[4..].find("\n---\n").expect("frontmatter must be closed");
        let frontmatter = &content[4..4 + fm_end];
        assert!(frontmatter.contains("quality: 92"));
        assert!(frontmatter.contains("extractor_version: 3"));
        assert!(frontmatter.contains("page_kind: \"article\""));
        assert!(frontmatter.contains("  - \"transformer\""));
        // 正文在 frontmatter 之后
        assert!(content.contains("## 引言"));
        assert!(content.contains("## 结论"));

        // 幂等：再写一次内容不变
        let content2 = fs::read_to_string(write_document(&tmp, &doc).unwrap().path).unwrap();
        assert_eq!(content, content2);

        let _ = fs::remove_dir_all(&tmp);
    }

    /// 删除后清理空目录，不留下空壳。
    #[test]
    fn removing_last_document_prunes_empty_dirs() {
        let tmp = std::env::temp_dir().join(format!("guaxi-feeds-prune-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let doc = ArticleDoc {
            id: "x",
            title: "临时文档",
            url: "https://x.com/1",
            source: "S",
            source_id: "src1",
            collection: "col",
            published_at: "2026-01-02",
            fetched_at: "2026-01-02",
            author: None,
            lang: None,
            body: "正文",
            quality: 60,
            page_kind: "article",
            extractor: "x",
            extractor_version: 1,
            tags: &[],
            summary: "",
            local_images: vec![],
        };
        let saved = write_document(&tmp, &doc).unwrap();
        let dir = saved.path.parent().unwrap().to_path_buf();
        assert!(dir.exists());
        assert!(remove_document(&tmp, &saved.rel_path));
        assert!(!dir.exists(), "empty month dir should be pruned");
        // 但库根必须还在
        assert!(tmp.exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}

#[cfg(test)]
mod trash_tests {
    use super::*;

    fn mk(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    #[test]
    fn soft_delete_moves_and_restores() {
        let tmp = std::env::temp_dir().join(format!("guaxi-trash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        mk(&tmp, "ai/src/2026-11/a.md", "内容 A");
        let trash = soft_delete_document(&tmp, "ai/src/2026-11/a.md", "20261104-120000")
            .expect("soft delete should succeed");
        assert!(!tmp.join("ai/src/2026-11/a.md").exists(), "original must be gone from its spot");
        assert!(tmp.join(&trash).exists(), "file must live in trash");
        assert!(trash.starts_with("_trash/20261104-120000/"));

        let restored = restore_document(&tmp, &trash).expect("restore should succeed");
        assert_eq!(restored, "ai/src/2026-11/a.md");
        assert_eq!(
            fs::read_to_string(tmp.join("ai/src/2026-11/a.md")).unwrap(),
            "内容 A"
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn restore_refuses_when_target_exists() {
        let tmp = std::env::temp_dir().join(format!("guaxi-trash2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        mk(&tmp, "a/b/c.md", "旧内容");
        let trash = soft_delete_document(&tmp, "a/b/c.md", "20261104-120000").unwrap();
        // 期间又抓到了同名文章
        mk(&tmp, "a/b/c.md", "新内容");
        assert!(
            restore_document(&tmp, &trash).is_none(),
            "must not overwrite a newer file"
        );
        assert_eq!(fs::read_to_string(tmp.join("a/b/c.md")).unwrap(), "新内容");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn trash_path_traversal_rejected() {
        let tmp = std::env::temp_dir();
        assert!(soft_delete_document(&tmp, "../etc/passwd", "x").is_none());
        assert!(restore_document(&tmp, "_trash/x/../../../etc/passwd").is_none());
        assert!(restore_document(&tmp, "not-a-trash-path").is_none());
    }

    #[test]
    fn empty_trash_removes_only_older_stamps() {
        let tmp = std::env::temp_dir().join(format!("guaxi-trash3-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        for (rel, stamp) in [("x/1.md", "20260101-000000"), ("y/2.md", "20261104-120000")] {
            mk(&tmp, rel, "c");
            soft_delete_document(&tmp, rel, stamp).unwrap();
        }
        // 只清理早于 20260601 的
        let n = empty_trash_older_than(&tmp, "20260601-000000");
        assert_eq!(n, 1);
        assert!(!tmp.join("_trash/20260101-000000").exists());
        assert!(tmp.join("_trash/20261104-120000").exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
