use std::{collections::HashMap, num::NonZeroUsize};

use lru::LruCache;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

static NOTEBOOK_SEARCH_INDEX_CACHE: Lazy<Mutex<LruCache<String, NotebookSearchIndex>>> =
    Lazy::new(|| Mutex::new(LruCache::new(NonZeroUsize::new(24).expect("non zero cache size"))));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookCell {
    #[serde(rename = "type")]
    pub cell_type: String,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sourceType")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "fenceLanguage")]
    pub fence_language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookSyncResult {
    pub cells: Vec<NotebookCell>,
    pub current_digest: String,
    pub sidecar_digest_match: bool,
    pub restored_from_sidecar: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookSearchDocument {
    pub cell_key: String,
    pub cell_type: String,
    pub digest: String,
    #[serde(default)]
    pub search_text: String,
    #[serde(default)]
    pub summary_text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotebookSearchIndexUpdateResult {
    pub indexed_file: String,
    pub document_count: usize,
    pub changed_count: usize,
    pub reused_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotebookSearchMatch {
    pub cell_key: String,
    pub cell_type: String,
    pub match_count: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotebookSearchResponse {
    pub query: String,
    pub total_matches: usize,
    pub results: Vec<NotebookSearchMatch>,
}

#[derive(Debug, Clone, Deserialize)]
struct NotebookSidecarPayload {
    #[serde(default)]
    #[serde(rename = "sourceDigest")]
    source_digest: String,
    #[serde(default)]
    cells: Vec<NotebookCell>,
}

#[derive(Debug, Clone)]
struct PyBlock {
    start_line: usize,
    end_line: usize,
    code: String,
    meta: Map<String, Value>,
}

#[derive(Debug, Clone)]
struct MathBlock {
    start_line: usize,
    end_line: usize,
    expression: String,
    meta: Map<String, Value>,
}

#[derive(Debug, Clone)]
struct NotebookSearchIndexedDocument {
    cell_key: String,
    cell_type: String,
    digest: String,
    search_text: String,
    search_text_lower: String,
    summary_text: String,
}

#[derive(Debug, Clone)]
struct NotebookSearchIndex {
    documents: HashMap<String, NotebookSearchIndexedDocument>,
    order: Vec<String>,
}

fn normalize_markdown_text(markdown: &str) -> String {
    markdown.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn strip_output_blocks(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut kept = Vec::new();
    let mut inside_output = false;
    for line in lines {
        let text = line.trim();
        if text.starts_with("<!-- guaxi:pyout:") && text.ends_with(":start -->") {
            inside_output = true;
            continue;
        }
        if inside_output && text.starts_with("<!-- guaxi:pyout:") && text.ends_with(":end -->") {
            inside_output = false;
            continue;
        }
        if !inside_output {
            kept.push(line);
        }
    }
    kept.join("\n")
}

fn compute_digest(text: &str) -> String {
    let mut hash: u32 = 2166136261;
    for byte in text.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    format!("nb-{:x}-{}", hash, text.len())
}

fn default_python_cell_meta() -> Map<String, Value> {
    let mut meta = Map::new();
    meta.insert("title".to_string(), Value::String(String::new()));
    meta.insert("codeCollapsed".to_string(), Value::Bool(false));
    meta.insert("outputCollapsed".to_string(), Value::Bool(false));
    meta.insert("sessionCollapsed".to_string(), Value::Bool(true));
    meta.insert("consoleCollapsed".to_string(), Value::Bool(false));
    meta
}

fn normalize_python_cell_meta(raw: Option<&Value>) -> Map<String, Value> {
    let mut meta = default_python_cell_meta();
    if let Some(Value::Object(obj)) = raw {
        for (key, value) in obj {
            meta.insert(key.clone(), value.clone());
        }
    }
    if let Some(Value::String(title)) = meta.get("title") {
        meta.insert("title".to_string(), Value::String(title.trim().to_string()));
    }
    meta
}

fn try_parse_python_cell_meta(raw_meta: &str) -> Map<String, Value> {
    let text = raw_meta.trim();
    if text.is_empty() {
        return default_python_cell_meta();
    }
    match serde_json::from_str::<Value>(text) {
        Ok(value) => normalize_python_cell_meta(Some(&value)),
        Err(_) => default_python_cell_meta(),
    }
}

fn create_fallback_stable_id(seed: &str) -> String {
    format!("py-rs-{}", compute_digest(seed).replace("nb-", ""))
}

// 设计说明：
// 1. 后端 Notebook 同步只识别统一后的 Markdown 注释标记：<!--py--> / <!--py {...}--> / <!--/py-->。
// 2. 旧分支已移除，避免前后端对 Cell 边界产生两套语义，导致保存、索引和搜索结果不一致。
fn parse_python_cell_start_marker(line: &str) -> Option<&str> {
    let text = line.trim();
    if text == "<!--py-->" {
        return Some("");
    }
    if text.starts_with("<!--py ") && text.ends_with("-->") {
        return Some(text.trim_start_matches("<!--py").trim_end_matches("-->").trim());
    }
    None
}

fn is_python_cell_end_marker(line: &str) -> bool {
    line.trim() == "<!--/py-->"
}

fn default_math_cell_meta() -> Map<String, Value> {
    let mut meta = Map::new();
    meta.insert("title".to_string(), Value::String(String::new()));
    meta.insert("outputCollapsed".to_string(), Value::Bool(false));
    meta
}

fn normalize_math_cell_meta(raw: Option<&Value>) -> Map<String, Value> {
    let mut meta = default_math_cell_meta();
    if let Some(Value::Object(obj)) = raw {
        for (key, value) in obj {
            meta.insert(key.clone(), value.clone());
        }
    }
    if let Some(Value::String(title)) = meta.get("title") {
        meta.insert("title".to_string(), Value::String(title.trim().to_string()));
    }
    meta
}

fn try_parse_math_cell_meta(raw_meta: &str) -> Map<String, Value> {
    let text = raw_meta.trim();
    if text.is_empty() {
        return default_math_cell_meta();
    }
    match serde_json::from_str::<Value>(text) {
        Ok(value) => normalize_math_cell_meta(Some(&value)),
        Err(_) => default_math_cell_meta(),
    }
}

fn parse_math_cell_start_marker(line: &str) -> Option<&str> {
    let text = line.trim();
    if text == "<!--math-->" {
        return Some("");
    }
    if text.starts_with("<!--math ") && text.ends_with("-->") {
        return Some(text.trim_start_matches("<!--math").trim_end_matches("-->").trim());
    }
    None
}

fn is_math_cell_end_marker(line: &str) -> bool {
    line.trim() == "<!--/math-->"
}

// 设计说明：
// 1. Math Cell 统一使用 Markdown HTML 注释哨兵：<!--math--> ... <!--/math-->。
// 2. 普通 $$...$$ / \[...\] / ```math fenced code block 不再被识别为 math cell，
//    而是作为普通 Markdown 内容保留在 markdown cell 中。
// 3. 与 parse_python_cell_blocks 对称设计，后端同步路径与前端解析逻辑保持一致。
fn parse_math_cell_blocks(markdown: &str) -> Vec<MathBlock> {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut blocks = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let Some(meta_text) = parse_math_cell_start_marker(lines[index]) else {
            index += 1;
            continue;
        };

        let mut end_index = None;
        for j in (index + 1)..lines.len() {
            if is_math_cell_end_marker(lines[j]) {
                end_index = Some(j);
                break;
            }
        }
        let Some(end) = end_index else {
            index += 1;
            continue;
        };

        let meta = try_parse_math_cell_meta(meta_text);

        blocks.push(MathBlock {
            start_line: index + 1,
            end_line: end + 1,
            expression: lines[(index + 1)..end].join("\n"),
            meta,
        });
        index = end + 1;
    }

    blocks
}

pub fn strip_math_cell_blocks(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut kept = Vec::new();
    let mut inside_math_cell = false;

    for line in lines {
        if parse_math_cell_start_marker(line).is_some() {
            inside_math_cell = true;
            continue;
        }
        if inside_math_cell && is_math_cell_end_marker(line) {
            inside_math_cell = false;
            continue;
        }
        if !inside_math_cell {
            kept.push(line);
        }
    }

    kept.join("\n")
}

fn parse_python_cell_blocks(markdown: &str) -> Vec<PyBlock> {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut blocks = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let Some(meta_text) = parse_python_cell_start_marker(lines[index]) else {
            index += 1;
            continue;
        };

        let mut end_index = None;
        for j in (index + 1)..lines.len() {
            if is_python_cell_end_marker(lines[j]) {
                end_index = Some(j);
                break;
            }
        }
        let Some(end) = end_index else {
            index += 1;
            continue;
        };

        let mut meta = try_parse_python_cell_meta(meta_text);
        if !meta.contains_key("id") || meta.get("id").and_then(Value::as_str).unwrap_or("").trim().is_empty() {
            let seed = format!("{}:{}:{}", index + 1, end + 1, lines[(index + 1)..end].join("\n"));
            meta.insert("id".to_string(), Value::String(create_fallback_stable_id(&seed)));
        }

        blocks.push(PyBlock {
            start_line: index + 1,
            end_line: end + 1,
            code: lines[(index + 1)..end].join("\n"),
            meta,
        });
        index = end + 1;
    }

    blocks
}

fn detect_frontmatter_end(lines: &[&str]) -> Option<usize> {
    if lines.len() < 3 || lines.first().map(|v| v.trim()) != Some("---") {
        return None;
    }
    for index in 1..lines.len() {
        let text = lines[index].trim();
        if text == "---" || text == "..." {
            let mut end_exclusive = index + 1;
            while end_exclusive < lines.len() && lines[end_exclusive].trim().is_empty() {
                end_exclusive += 1;
            }
            return Some(end_exclusive);
        }
    }
    None
}

fn split_markdown_segments(markdown: &str) -> Vec<String> {
    if markdown.is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut segments = Vec::new();
    let mut start_line = 0usize;

    if let Some(frontmatter_end) = detect_frontmatter_end(&lines) {
        let segment = lines[0..frontmatter_end].join("\n");
        if !segment.is_empty() {
            segments.push(segment);
        }
        start_line = frontmatter_end;
    }

    let mut heading_indexes = Vec::new();
    let mut fence_active = false;
    let mut fence_char = '\0';
    let mut fence_len = 0usize;

    for index in start_line..lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if !fence_active && trimmed.starts_with('#') && trimmed.chars().take_while(|c| *c == '#').count() <= 6 {
            let hash_count = trimmed.chars().take_while(|c| *c == '#').count();
            let rest = trimmed.chars().nth(hash_count);
            if matches!(rest, Some(' ') | None) {
                heading_indexes.push(index);
            }
        }
        let fence_trimmed = trimmed;
        if fence_trimmed.starts_with("```") || fence_trimmed.starts_with("~~~") {
            let current_char = fence_trimmed.chars().next().unwrap_or('\0');
            let current_len = fence_trimmed.chars().take_while(|c| *c == current_char).count();
            if !fence_active {
                fence_active = true;
                fence_char = current_char;
                fence_len = current_len;
            } else if fence_char == current_char && current_len >= fence_len {
                fence_active = false;
                fence_char = '\0';
                fence_len = 0;
            }
        }
    }

    let push_segment = |segments: &mut Vec<String>, slice: &[&str]| {
        let segment = slice.join("\n");
        if !segment.is_empty() {
            segments.push(segment);
        }
    };

    if !heading_indexes.is_empty() {
        if heading_indexes[0] > start_line {
            push_segment(&mut segments, &lines[start_line..heading_indexes[0]]);
        }
        for (idx, heading_start) in heading_indexes.iter().enumerate() {
            let end = *heading_indexes.get(idx + 1).unwrap_or(&lines.len());
            push_segment(&mut segments, &lines[*heading_start..end]);
        }
    } else {
        let mut chunk_start = start_line;
        fence_active = false;
        fence_char = '\0';
        fence_len = 0;
        let mut index = start_line;
        while index < lines.len() {
            let line = lines[index];
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                let current_char = trimmed.chars().next().unwrap_or('\0');
                let current_len = trimmed.chars().take_while(|c| *c == current_char).count();
                if !fence_active {
                    fence_active = true;
                    fence_char = current_char;
                    fence_len = current_len;
                } else if fence_char == current_char && current_len >= fence_len {
                    fence_active = false;
                    fence_char = '\0';
                    fence_len = 0;
                }
            }
            if !fence_active && line.trim().is_empty() {
                let mut blank_end = index;
                while blank_end + 1 < lines.len() && lines[blank_end + 1].trim().is_empty() {
                    blank_end += 1;
                }
                if blank_end > index {
                    push_segment(&mut segments, &lines[chunk_start..(blank_end + 1)]);
                    chunk_start = blank_end + 1;
                    index = blank_end;
                }
            }
            index += 1;
        }
        if chunk_start < lines.len() {
            push_segment(&mut segments, &lines[chunk_start..]);
        }
    }

    segments
}

fn push_markdown_notebook_cells(cells: &mut Vec<NotebookCell>, markdown: &str, key_prefix: &str) {
    for (index, content) in split_markdown_segments(markdown).into_iter().enumerate() {
        cells.push(NotebookCell {
            cell_type: "markdown".to_string(),
            key: format!("nb-md-{}-{}", key_prefix, index),
            content: Some(content),
            code: None,
            meta: None,
            expression: None,
            source_type: None,
            fence_language: None,
        });
    }
}

fn split_markdown_into_notebook_cells(markdown: &str, key_prefix: &str) -> Vec<NotebookCell> {
    // 设计说明：
    // 1. $$...$$ / \[...\] / ```math 代码块不再自动创建 math cell，
    //    而是作为普通 Markdown 内容保留在 markdown cell 中。
    // 2. Math cell 现在只能通过 <!--math-->...<!--/math--> 注释语法显式创建。
    // 3. 后端不再需要检测数学块，只需要按标题/段落切分 markdown cell。
    let mut cells = Vec::new();
    push_markdown_notebook_cells(&mut cells, markdown, key_prefix);
    cells
}

pub fn strip_python_cell_blocks(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut kept = Vec::new();
    let mut inside_py_cell = false;

    for line in lines {
        if parse_python_cell_start_marker(line).is_some() {
            inside_py_cell = true;
            continue;
        }
        if inside_py_cell && is_python_cell_end_marker(line) {
            inside_py_cell = false;
            continue;
        }
        if !inside_py_cell {
            kept.push(line);
        }
    }

    kept.join("\n")
}

fn is_toc_marker_line(line: &str) -> bool {
    // 编辑器里目录标记经常写成转义形式 `\[TOC\]`（避免被当成普通中括号链接渲染），
    // 导出时两者都要认，否则用户写了目录却拿不到目录。
    let normalized = line.trim().replace("\\[", "[").replace("\\]", "]").replace("\\\\", "\\");
    matches!(
        normalized.to_ascii_lowercase().as_str(),
        "[toc]" | "[目录]" | "[[toc]]" | "[[目录]]"
    )
}

fn is_standalone_horizontal_rule_line(line: &str) -> bool {
    matches!(line.trim(), "---" | "___")
}

fn normalize_static_wikilink_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let (target_part, alias_part) = match trimmed.split_once('|') {
        Some((target, alias)) => (target.trim(), alias.trim()),
        None => (trimmed, ""),
    };

    if !alias_part.is_empty() {
        return alias_part.to_string();
    }

    let (file_part, anchor_part) = match target_part.split_once('#') {
        Some((file, anchor)) => (file.trim(), anchor.trim()),
        None => (target_part, ""),
    };

    if !file_part.is_empty() {
        return file_part.to_string();
    }

    anchor_part
        .trim_start_matches('^')
        .trim()
        .to_string()
}

fn replace_static_wikilinks(line: &str) -> String {
    let mut result = String::new();
    let mut remaining = line;

    while let Some(start) = remaining.find("[[") {
        result.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("]]") else {
            result.push_str(&remaining[start..]);
            return result;
        };

        let inner = &after_start[..end];
        let normalized = inner.trim().to_ascii_lowercase();
        if normalized == "toc" || normalized == "目录" {
            result.push_str("目录");
        } else {
            result.push_str(&normalize_static_wikilink_text(inner));
        }
        remaining = &after_start[end + 2..];
    }

    result.push_str(remaining);
    result
}

#[derive(Debug, Clone)]
struct StaticHeading {
    line_index: usize,
    level: usize,
    title: String,
    anchor: String,
}

fn slugify_static_heading_title(title: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;

    for ch in title.trim().chars() {
        if ch.is_alphanumeric() || ch == '_' || (!ch.is_ascii() && !ch.is_whitespace()) {
            if ch.is_ascii() {
                out.push(ch.to_ascii_lowercase());
            } else {
                out.push(ch);
            }
            last_was_sep = false;
        } else if !last_was_sep && !out.is_empty() {
            out.push('-');
            last_was_sep = true;
        }
    }

    let normalized = out.trim_matches('-').trim();
    if normalized.is_empty() {
        "section".to_string()
    } else {
        normalized.to_string()
    }
}

fn collect_headings_for_static_toc(markdown: &str) -> Vec<StaticHeading> {
    let mut headings = Vec::new();
    let mut fence_active = false;
    let mut fence_marker = String::new();
    let mut anchor_counts: HashMap<String, usize> = HashMap::new();

    for (line_index, raw_line) in markdown.split('\n').enumerate() {
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker_len = trimmed.chars().take_while(|ch| *ch == '`' || *ch == '~').count();
            let marker = trimmed.chars().next().unwrap_or('`').to_string().repeat(marker_len.max(3));
            if !fence_active {
                fence_active = true;
                fence_marker = marker;
            } else if trimmed.starts_with(&fence_marker) {
                fence_active = false;
                fence_marker.clear();
            }
            continue;
        }
        if fence_active {
            continue;
        }

        let level = trimmed.chars().take_while(|ch| *ch == '#').count();
        if !(1..=6).contains(&level) {
            continue;
        }
        let title = trimmed[level..].trim();
        if title.is_empty() {
            continue;
        }
        let normalized_title = replace_static_wikilinks(title.trim_end_matches('#').trim());
        let slug = slugify_static_heading_title(&normalized_title);
        let counter = anchor_counts.entry(slug.clone()).or_insert(0);
        *counter += 1;
        let unique_anchor = if *counter == 1 {
            format!("guaxi-toc-{slug}")
        } else {
            format!("guaxi-toc-{slug}-{}", *counter)
        };
        headings.push(StaticHeading {
            line_index,
            level,
            title: normalized_title,
            anchor: unique_anchor,
        });
    }

    headings
}

/// 去掉行首的引用标记（`>` 链），返回剩下的内容。
fn strip_quote_markers(line: &str) -> &str {
    let mut rest = line.trim_start();
    while let Some(after) = rest.strip_prefix('>') {
        rest = after.trim_start();
    }
    rest
}

/// 这一行是否为 ATX 标题（`#` ~ `######` + 空白）。缩进 4 格以上视为代码块。
fn is_atx_heading_line(line: &str) -> bool {
    let without_indent = line.trim_start_matches([' ', '\t']);
    if line.len() - without_indent.len() > 3 {
        return false;
    }
    let hashes = without_indent.chars().take_while(|ch| *ch == '#').count();
    if hashes == 0 || hashes > 6 {
        return false;
    }
    let rest = &without_indent[hashes..];
    rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')
}

/// 管道表格的数据行（`| a | b |`，也接受引用块里的 `> | a | b |`）。
fn is_pipe_table_row(line: &str) -> bool {
    let trimmed = strip_quote_markers(line).trim_end();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 2
}

/// 管道表格的分隔行（`| --- | :---: |`）。
fn is_pipe_table_delimiter(line: &str) -> bool {
    let trimmed = strip_quote_markers(line).trim();
    if !(trimmed.starts_with('|') && trimmed.ends_with('|')) {
        return false;
    }
    let inner = &trimmed[1..trimmed.len().saturating_sub(1)];
    if inner.trim().is_empty() {
        return false;
    }
    inner
        .chars()
        .all(|ch| matches!(ch, '-' | ':' | '|' | ' ' | '\t'))
        && inner.contains('-')
}

/// 补上 Pandoc 块解析所需的空行。
///
/// Pandoc 的块解析比预览端（markdown-it / CommonMark）严格：ATX 标题和管道表格
/// 都要求前面有空行，表格还要靠空行来**结束**。用户写的 Markdown 常常省略这些空行
/// （软件内预览完全正常），但 pandoc 会把后续内容一路吞进表格、把标题退化成正文
/// —— 表现就是「导出的 Word 里第 N 节之后正文挤成窄栏、代码全没了」。
///
/// 这里只在必要位置插入空行，不改动任何内容；代码围栏与 YAML frontmatter 内一律不动。
fn ensure_block_boundaries(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 64);
    let mut fence: Option<String> = None;
    // frontmatter：首个非空行是 `---` 时，到下一个 `---`（含）为止都不碰。
    let mut frontmatter_end: Option<usize> = None;
    if let Some(first) = lines.iter().position(|line| !line.trim().is_empty()) {
        if lines[first].trim() == "---" {
            if let Some(close) = lines[first + 1..]
                .iter()
                .position(|line| line.trim() == "---")
            {
                frontmatter_end = Some(first + 1 + close);
            }
        }
    }

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();

        if matches!(frontmatter_end, Some(end) if index <= end) {
            out.push((*line).to_string());
            continue;
        }

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker_char = trimmed.chars().next().unwrap_or('`');
            let count = trimmed.chars().take_while(|ch| *ch == marker_char).count();
            let marker = marker_char.to_string().repeat(count.max(3));
            match &fence {
                None => fence = Some(marker),
                Some(open) if trimmed.starts_with(open.as_str()) => fence = None,
                _ => {}
            }
            out.push((*line).to_string());
            continue;
        }
        if fence.is_some() {
            out.push((*line).to_string());
            continue;
        }

        let current_blank = trimmed.trim_end().is_empty();
        let current_quoted = trimmed.starts_with('>');
        let prev_non_blank = out.last().map(|prev| !prev.trim().is_empty()).unwrap_or(false);
        let prev_is_table_row = out.last().map(|prev| is_pipe_table_row(prev)).unwrap_or(false);
        let prev_is_heading = out.last().map(|prev| is_atx_heading_line(prev)).unwrap_or(false);
        let current_is_table_row = is_pipe_table_row(line);
        let next_is_delimiter = lines
            .get(index + 1)
            .map(|next| is_pipe_table_delimiter(next))
            .unwrap_or(false);

        let needs_blank = prev_non_blank
            && !current_blank
            && ((is_atx_heading_line(line) && !prev_is_heading)
                // 表格开头不能被当成上一段落的续行（引用块里的表格不在此列：
                // 补空行会把引用块截断）。
                || (current_is_table_row && !current_quoted && !prev_is_table_row && next_is_delimiter)
                // 表格结束：下一行不是表格行、也不是引用块的续行。
                || (prev_is_table_row && !current_is_table_row && !current_quoted));
        if needs_blank {
            out.push(String::new());
        }
        out.push((*line).to_string());
    }

    out.join("\n")
}

pub fn preprocess_markdown_for_static_render(markdown: &str) -> String {
    let normalized = normalize_markdown_text(markdown);
    let normalized = ensure_block_boundaries(&normalized);
    let without_outputs = strip_output_blocks(&normalized);
    let without_python_cells = strip_python_cell_blocks(&without_outputs);
    let without_math_cells = strip_math_cell_blocks(&without_python_cells);
    let lines: Vec<&str> = without_math_cells.split('\n').collect();
    let frontmatter_end = detect_frontmatter_end(&lines);
    let headings = collect_headings_for_static_toc(&without_math_cells);
    let toc_markdown = if headings.is_empty() {
        "## 目录\n\n> 当前文档未检测到可用于生成目录的标题".to_string()
    } else {
        let min_level = headings.iter().map(|item| item.level).min().unwrap_or(1);
        let mut lines = vec!["## 目录".to_string(), String::new()];
        for item in &headings {
            let indent = "  ".repeat(item.level.saturating_sub(min_level));
            lines.push(format!("{indent}- [{}](#{})", item.title, item.anchor));
        }
        lines.join("\n")
    };

    let mut output = Vec::new();
    let mut fence_active = false;
    let mut fence_marker = String::new();
    let mut heading_iter = headings.iter().peekable();

    for (line_index, raw_line) in lines.iter().enumerate() {
        let raw_line = *raw_line;
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker_len = trimmed.chars().take_while(|ch| *ch == '`' || *ch == '~').count();
            let marker = trimmed.chars().next().unwrap_or('`').to_string().repeat(marker_len.max(3));
            if !fence_active {
                fence_active = true;
                fence_marker = marker;
            } else if trimmed.starts_with(&fence_marker) {
                fence_active = false;
                fence_marker.clear();
            }
            output.push(raw_line.to_string());
            continue;
        }

        if fence_active {
            output.push(raw_line.to_string());
            continue;
        }

        // 设计说明（关键）：
        // 绝不能把锚点 <a id></a> 放在标题行的“上一行”单独成行。CommonMark / pandoc 会把这种
        // 裸 HTML 标签当作 HTML block 起点，且该 block 会一直延伸到下一个空行——于是紧跟其后的
        // `### 标题` 会被整体吞进 HTML block，被原样输出（### 不再被识别为标题），
        // 进而 pandoc --toc 也无法收集到标题、Word/EPUB 生成不出导航书签。
        // 这里改为把锚点“内联”到标题行尾：pandoc 解析为标题内的 inline raw HTML，
        // 既保留标题语义（书签正常），又能让注入式 TOC 的 [title](#anchor) 正确跳转。
        let mut heading_anchor: Option<String> = None;
        while heading_iter
            .peek()
            .map(|item| item.line_index == line_index)
            .unwrap_or(false)
        {
            if let Some(item) = heading_iter.next() {
                heading_anchor = Some(item.anchor.clone());
            }
        }

        if is_toc_marker_line(raw_line) {
            output.push(toc_markdown.clone());
            continue;
        }

        if !matches!(frontmatter_end, Some(end) if line_index < end)
            && is_standalone_horizontal_rule_line(raw_line)
        {
            // 设计说明：
            // 1. Pandoc 会把某些位置上的 `---` 误判成 YAML metadata block 起始符。
            // 2. 导出/浏览器预览只需要保留“水平分割线”的语义，因此这里改写成等价的 `***`。
            // 3. 顶部 frontmatter 区域必须原样保留，所以显式排除 frontmatter 范围。
            output.push("***".to_string());
            continue;
        }

        let rendered_line = replace_static_wikilinks(raw_line);
        if let Some(anchor) = heading_anchor {
            output.push(format!("{} <a id=\"{}\"></a>", rendered_line.trim_end(), anchor));
        } else {
            output.push(rendered_line);
        }
    }

    output.join("\n")
}

fn build_markdown_cells(markdown: &str) -> Vec<NotebookCell> {
    let text = normalize_markdown_text(markdown);
    let lines: Vec<&str> = text.split('\n').collect();

    // 设计说明：
    // 1. 同时解析 Python Cell（<!--py-->）和 Math Cell（<!--math-->）标记块。
    // 2. 合并后按行号排序，逐一插入 cells 数组；块之间的 Markdown 文本拆分为 markdown cells。
    // 3. $$、\[、```math 不再被识别为 math cell —— 它们现在作为普通 Markdown 内容保留。
    let py_blocks = parse_python_cell_blocks(&text);
    let math_blocks = parse_math_cell_blocks(&text);

    #[derive(Debug, Clone)]
    struct MergedBlock {
        start_line: usize,
        end_line: usize,
        block_type: String,
        code: Option<String>,
        expression: Option<String>,
        meta: Map<String, Value>,
    }

    let mut all_blocks: Vec<MergedBlock> = Vec::new();
    for block in &py_blocks {
        all_blocks.push(MergedBlock {
            start_line: block.start_line,
            end_line: block.end_line,
            block_type: "python".to_string(),
            code: Some(block.code.clone()),
            expression: None,
            meta: block.meta.clone(),
        });
    }
    for block in &math_blocks {
        all_blocks.push(MergedBlock {
            start_line: block.start_line,
            end_line: block.end_line,
            block_type: "math".to_string(),
            code: None,
            expression: Some(block.expression.clone()),
            meta: block.meta.clone(),
        });
    }
    all_blocks.sort_by_key(|b| b.start_line);

    let mut cells = Vec::new();
    let mut cursor = 0usize;

    for block in &all_blocks {
        let start_index = block.start_line.saturating_sub(1);
        let end_exclusive = block.end_line;

        // 跳过已被前一个块覆盖的区域（理论上不应重叠，但做防护）
        if start_index < cursor {
            continue;
        }

        let leading = lines[cursor..start_index].join("\n");
        let leading_cells =
            split_markdown_into_notebook_cells(&leading, &format!("{}-{}", block.block_type, cursor));
        if !leading_cells.is_empty() {
            cells.extend(leading_cells);
        } else if cells.is_empty() {
            cells.push(NotebookCell {
                cell_type: "markdown".to_string(),
                key: format!("nb-md-{}-{}-empty", block.block_type, cursor),
                content: Some(String::new()),
                code: None,
                meta: None,
                expression: None,
                source_type: None,
                fence_language: None,
            });
        }

        match block.block_type.as_str() {
            "python" => {
                let stable_id = block
                    .meta
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                cells.push(NotebookCell {
                    cell_type: "python".to_string(),
                    key: stable_id.clone(),
                    content: None,
                    code: block.code.clone(),
                    meta: Some(Value::Object(block.meta.clone())),
                    expression: None,
                    source_type: None,
                    fence_language: None,
                });
            }
            "math" => {
                let meta = block.meta.clone();
                // 与 Python Cell 对称：优先用稳定 meta.id 作为 key，保证 cellmeta 表达式回填
                // 与输出快照能跨次加载稳定匹配（位置型 key 会随行号漂移失配）。
                let stable_id = meta
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let key = if stable_id.is_empty() {
                    format!("nb-math-{}-{}", block.block_type, start_index)
                } else {
                    stable_id
                };
                cells.push(NotebookCell {
                    cell_type: "math".to_string(),
                    key,
                    content: None,
                    code: None,
                    meta: Some(Value::Object(meta)),
                    expression: block.expression.clone(),
                    source_type: Some("marker".to_string()),
                    fence_language: Some("math".to_string()),
                });
            }
            _ => {}
        }
        cursor = end_exclusive;
    }

    let trailing = lines[cursor..].join("\n");
    let trailing_cells = split_markdown_into_notebook_cells(&trailing, &format!("tail-{}", cursor));
    if !trailing_cells.is_empty() {
        cells.extend(trailing_cells);
    } else if cells.is_empty() || matches!(cells.last().map(|cell| cell.cell_type.as_str()), Some("python" | "math"))
    {
        cells.push(NotebookCell {
            cell_type: "markdown".to_string(),
            key: format!("nb-md-tail-{}-empty", cursor),
            content: Some(String::new()),
            code: None,
            meta: None,
            expression: None,
            source_type: None,
            fence_language: None,
        });
    }

    cells
}

fn cell_digest(cell: &NotebookCell) -> String {
    match cell.cell_type.as_str() {
        "python" => compute_digest(cell.code.as_deref().unwrap_or("")),
        "math" => compute_digest(cell.expression.as_deref().unwrap_or("")),
        _ => compute_digest(cell.content.as_deref().unwrap_or("")),
    }
}

fn cell_stable_id(cell: &NotebookCell) -> String {
    cell.meta
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn cell_meta_storage(cell: &NotebookCell) -> String {
    cell.meta
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("storage"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

// 把 sidecar 持久化的 meta 作为底，再用扫描结果的 meta 覆盖（源文件里手工改过的字段优先）。
fn merge_persisted_meta(persisted: Option<Value>, current: Option<Value>) -> Option<Value> {
    match (persisted, current) {
        (Some(Value::Object(mut persisted_meta)), Some(Value::Object(current_meta))) => {
            for (key, value) in current_meta {
                persisted_meta.insert(key, value);
            }
            Some(Value::Object(persisted_meta))
        }
        (_, Some(current)) => Some(current),
        (persisted, None) => persisted,
    }
}

// 在 sidecar 的某一类型 cell 中按 stable-id / 内容 digest 找到对应的持久化 cell。
fn find_persisted_cell(
    cell: &NotebookCell,
    by_stable_id: &std::collections::HashMap<String, NotebookCell>,
    by_digest: &std::collections::HashMap<String, Vec<NotebookCell>>,
) -> Option<NotebookCell> {
    let stable_id = cell_stable_id(cell);
    if !stable_id.is_empty() {
        if let Some(found) = by_stable_id.get(&stable_id) {
            return Some(found.clone());
        }
    }
    let candidates = by_digest.get(&cell_digest(cell))?;
    if candidates.len() == 1 {
        Some(candidates[0].clone())
    } else {
        None
    }
}

fn reconcile_cells_with_sidecar(scanned_cells: Vec<NotebookCell>, sidecar: Option<&NotebookSidecarPayload>) -> Vec<NotebookCell> {
    let Some(sidecar_payload) = sidecar else {
        return scanned_cells;
    };
    if sidecar_payload.cells.is_empty() {
        return scanned_cells;
    }

    // 设计说明：Python Cell 与 Math Cell 都采用「源 Markdown 只存引用块、正文存 .cellmeta」的方案，
    // 因此这里按类型分别建立 stable-id / 内容 digest 索引（python 用 code，math 用 expression）。
    let mut py_by_stable_id = std::collections::HashMap::new();
    let mut py_by_digest: std::collections::HashMap<String, Vec<NotebookCell>> = std::collections::HashMap::new();
    let mut math_by_stable_id = std::collections::HashMap::new();
    let mut math_by_digest: std::collections::HashMap<String, Vec<NotebookCell>> = std::collections::HashMap::new();
    for cell in &sidecar_payload.cells {
        let stable_id = cell_stable_id(cell);
        match cell.cell_type.as_str() {
            "python" => {
                if !stable_id.is_empty() {
                    py_by_stable_id.insert(stable_id, cell.clone());
                }
                py_by_digest.entry(cell_digest(cell)).or_default().push(cell.clone());
            }
            "math" => {
                if !stable_id.is_empty() {
                    math_by_stable_id.insert(stable_id, cell.clone());
                }
                math_by_digest.entry(cell_digest(cell)).or_default().push(cell.clone());
            }
            _ => {}
        }
    }

    scanned_cells
        .into_iter()
        .map(|cell| match cell.cell_type.as_str() {
            "python" => {
                let Some(persisted_cell) = find_persisted_cell(&cell, &py_by_stable_id, &py_by_digest) else {
                    return cell;
                };
                let mut next = cell.clone();
                // 引用块（storage=cellmeta 且代码为空）从 sidecar 恢复代码；
                // 旧版完整源码块按 Markdown 内容优先，避免覆盖用户在源文件里的手工修改。
                let current_code_empty = cell.code.as_deref().unwrap_or("").is_empty();
                let persisted_code = persisted_cell.code.clone().unwrap_or_default();
                if cell_meta_storage(&cell) == "cellmeta" && current_code_empty && !persisted_code.is_empty() {
                    next.code = Some(persisted_code);
                }
                next.meta = merge_persisted_meta(persisted_cell.meta.clone(), cell.meta.clone());
                next
            }
            "math" => {
                let Some(persisted_cell) = find_persisted_cell(&cell, &math_by_stable_id, &math_by_digest) else {
                    return cell;
                };
                let mut next = cell.clone();
                // 引用块（storage=cellmeta 且表达式为空）从 sidecar 恢复数学表达式；
                // 旧版内联块自带表达式，保持原样。
                let current_expr_empty = cell.expression.as_deref().unwrap_or("").is_empty();
                let persisted_expr = persisted_cell.expression.clone().unwrap_or_default();
                if cell_meta_storage(&cell) == "cellmeta" && current_expr_empty && !persisted_expr.is_empty() {
                    next.expression = Some(persisted_expr);
                }
                next.meta = merge_persisted_meta(persisted_cell.meta.clone(), cell.meta.clone());
                next
            }
            _ => cell,
        })
        .collect()
}

fn build_search_index(documents: &[NotebookSearchDocument]) -> (NotebookSearchIndex, usize, usize) {
    let mut changed_count = 0usize;
    let mut reused_count = 0usize;
    let mut existing_cache = HashMap::new();

    let cache = NOTEBOOK_SEARCH_INDEX_CACHE.lock();
    for index in cache.iter() {
        for (cell_key, entry) in &index.1.documents {
            existing_cache.insert(cell_key.clone(), entry.clone());
        }
    }
    drop(cache);

    let mut indexed_documents = HashMap::new();
    let mut order = Vec::with_capacity(documents.len());

    for document in documents {
        let prev = existing_cache.get(&document.cell_key);
        let search_text = document.search_text.clone();
        let summary_text = if document.summary_text.trim().is_empty() {
            search_text.clone()
        } else {
            document.summary_text.clone()
        };
        let entry = if let Some(previous) = prev {
            if previous.digest == document.digest {
                reused_count += 1;
                previous.clone()
            } else {
                changed_count += 1;
                NotebookSearchIndexedDocument {
                    cell_key: document.cell_key.clone(),
                    cell_type: document.cell_type.clone(),
                    digest: document.digest.clone(),
                    search_text_lower: search_text.to_lowercase(),
                    search_text,
                    summary_text,
                }
            }
        } else {
            changed_count += 1;
            NotebookSearchIndexedDocument {
                cell_key: document.cell_key.clone(),
                cell_type: document.cell_type.clone(),
                digest: document.digest.clone(),
                search_text_lower: search_text.to_lowercase(),
                search_text,
                summary_text,
            }
        };

        order.push(document.cell_key.clone());
        indexed_documents.insert(document.cell_key.clone(), entry);
    }

    (
        NotebookSearchIndex {
            documents: indexed_documents,
            order,
        },
        changed_count,
        reused_count,
    )
}

fn build_match_snippet(text: &str, query: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();
    if lower_query.trim().is_empty() {
        return text.chars().take(120).collect();
    }
    if let Some(byte_index) = lower_text.find(&lower_query) {
        let char_index = text[..byte_index].chars().count();
        let total_chars = text.chars().count();
        let start = char_index.saturating_sub(28);
        let end = (char_index + lower_query.chars().count() + 52).min(total_chars);
        let snippet: String = text
            .chars()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect();
        let prefix = if start > 0 { "..." } else { "" };
        let suffix = if end < total_chars { "..." } else { "" };
        return format!("{}{}{}", prefix, snippet, suffix);
    }
    text.chars().take(120).collect()
}

#[tauri::command]
pub fn notebook_update_search_index(
    file_path: String,
    documents: Vec<NotebookSearchDocument>,
) -> Result<NotebookSearchIndexUpdateResult, String> {
    let normalized_file = file_path.trim().to_string();
    if normalized_file.is_empty() {
        return Err("file_path is required".to_string());
    }

    let (index, changed_count, reused_count) = build_search_index(&documents);
    let document_count = index.documents.len();

    let mut cache = NOTEBOOK_SEARCH_INDEX_CACHE.lock();
    cache.put(normalized_file.clone(), index);

    Ok(NotebookSearchIndexUpdateResult {
        indexed_file: normalized_file,
        document_count,
        changed_count,
        reused_count,
    })
}

#[tauri::command]
pub fn notebook_search(file_path: String, query: String, limit: Option<usize>) -> Result<NotebookSearchResponse, String> {
    let normalized_file = file_path.trim().to_string();
    if normalized_file.is_empty() {
        return Err("file_path is required".to_string());
    }

    let normalized_query = query.trim().to_string();
    if normalized_query.is_empty() {
        return Ok(NotebookSearchResponse {
            query: normalized_query,
            total_matches: 0,
            results: Vec::new(),
        });
    }

    let max_results = limit.unwrap_or(200).max(1);
    let lower_query = normalized_query.to_lowercase();
    let cache = NOTEBOOK_SEARCH_INDEX_CACHE.lock();
    let Some(index) = cache.peek(&normalized_file) else {
        return Ok(NotebookSearchResponse {
            query: normalized_query,
            total_matches: 0,
            results: Vec::new(),
        });
    };

    let mut results = Vec::new();
    let mut total_matches = 0usize;

    for cell_key in &index.order {
      let Some(document) = index.documents.get(cell_key) else {
          continue;
      };
      let count = document.search_text_lower.matches(&lower_query).count();
      if count == 0 {
          continue;
      }
      total_matches += count;
      let snippet_source = if document.summary_text.trim().is_empty() {
          &document.search_text
      } else {
          &document.summary_text
      };
      results.push(NotebookSearchMatch {
          cell_key: document.cell_key.clone(),
          cell_type: document.cell_type.clone(),
          match_count: count,
          snippet: build_match_snippet(snippet_source, &normalized_query),
      });
    }

    results.sort_by(|left, right| {
        right
            .match_count
            .cmp(&left.match_count)
            .then_with(|| left.cell_key.cmp(&right.cell_key))
    });
    if results.len() > max_results {
        results.truncate(max_results);
    }

    Ok(NotebookSearchResponse {
        query: normalized_query,
        total_matches,
        results,
    })
}

#[tauri::command]
pub fn notebook_sync_from_markdown(markdown: String, sidecar_json: Option<String>) -> Result<NotebookSyncResult, String> {
    let normalized = normalize_markdown_text(&markdown);
    let normalized_for_digest = strip_output_blocks(&normalized);
    let current_digest = compute_digest(&normalized_for_digest);
    let sidecar = sidecar_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<NotebookSidecarPayload>(raw).ok());
    let sidecar_digest_match = sidecar
        .as_ref()
        .map(|payload| !payload.source_digest.trim().is_empty() && payload.source_digest.trim() == current_digest)
        .unwrap_or(false);
    let scanned_cells = build_markdown_cells(&normalized);
    let reconciled_cells = reconcile_cells_with_sidecar(scanned_cells, sidecar.as_ref());

    Ok(NotebookSyncResult {
        cells: reconciled_cells,
        current_digest,
        sidecar_digest_match,
        restored_from_sidecar: sidecar.as_ref().map(|payload| !payload.cells.is_empty()).unwrap_or(false),
    })
}

// ==================== 文档内容隔离回归测试 ====================
//
// 这些用例针对“切换/新建文件时内容串档”这一类问题：
// Notebook 的解析必须是纯函数、只依赖传入的 Markdown，不能保留上一份文档的状态。
#[cfg(test)]
mod content_isolation_tests {
    use super::*;

    #[test]
    fn empty_markdown_yields_no_custom_cells() {
        for empty in ["", "\n", "\n\n", "   \n"] {
            assert!(parse_python_cell_blocks(empty).is_empty(), "python cells on {:?}", empty);
            assert!(parse_math_cell_blocks(empty).is_empty(), "math cells on {:?}", empty);
        }
    }

    #[test]
    fn python_cells_are_scoped_to_their_own_markdown() {
        let doc_a = r#"<!--py {"id":"a"}-->
print('a')
<!--/py-->
"#;
        let doc_b = r#"<!--py {"id":"b"}-->
print('b')
<!--/py-->
"#;

        let blocks_a = parse_python_cell_blocks(doc_a);
        let blocks_b = parse_python_cell_blocks(doc_b);

        assert_eq!(blocks_a.len(), 1);
        assert_eq!(blocks_b.len(), 1);
        assert_eq!(blocks_a[0].code.trim(), "print('a')");
        assert_eq!(blocks_b[0].code.trim(), "print('b')");
        assert_ne!(blocks_a[0].code, blocks_b[0].code);
    }

    #[test]
    fn math_cells_are_scoped_to_their_own_markdown() {
        let doc_a = r#"<!--math {"id":"m-a"}-->
a^2 + b^2 = c^2
<!--/math-->
"#;
        let doc_b = r#"<!--math {"id":"m-b"}-->
\int_0^1 x dx
<!--/math-->
"#;

        let blocks_a = parse_math_cell_blocks(doc_a);
        let blocks_b = parse_math_cell_blocks(doc_b);

        assert_eq!(blocks_a.len(), 1);
        assert_eq!(blocks_b.len(), 1);
        assert_eq!(blocks_a[0].expression.trim(), "a^2 + b^2 = c^2");
        assert_eq!(blocks_b[0].expression.trim(), r"\int_0^1 x dx");
    }

    #[test]
    fn strip_output_blocks_only_removes_paired_output_markers() {
        let markdown = "before\n<!-- guaxi:pyout:doc-1:start -->\nOUTPUT\n<!-- guaxi:pyout:doc-1:end -->\nafter\n";
        let stripped = strip_output_blocks(markdown);

        assert!(stripped.contains("before"));
        assert!(stripped.contains("after"));
        assert!(!stripped.contains("OUTPUT"));
        assert!(!stripped.contains("guaxi:pyout"));
    }

    #[test]
    fn unclosed_custom_cell_is_ignored_instead_of_swallowing_content() {
        // 用户正在编辑、尚未补上结束标记时，不能把后续内容吞进 cell。
        assert!(parse_python_cell_blocks("<!--py-->\nprint('x')\n").is_empty());
        assert!(parse_math_cell_blocks("<!--math-->\nx^2\n").is_empty());
    }

    #[test]
    fn inserts_blank_line_after_quoted_table_before_heading() {
        // 实测事故：引用块里的表格后紧跟标题，pandoc 会把第 8 节之后整篇吞进
        // 一个 5.5% 宽的单列表格，Word 里代码全部消失。
        let input = "> | a | b |\n> | --- | --- |\n> | 1 | 2 |\n## 8. next\ntext\n";
        let out = ensure_block_boundaries(input);
        assert!(
            out.contains("> | 1 | 2 |\n\n## 8. next"),
            "table must be terminated before the heading, got: {out}"
        );
        // 引用块内部不能被空行截断。
        assert!(
            out.contains("> | a | b |\n> | --- | --- |\n> | 1 | 2 |"),
            "blockquote table must stay intact, got: {out}"
        );
    }

    #[test]
    fn inserts_blank_line_before_heading_after_paragraph() {
        assert_eq!(
            ensure_block_boundaries("some text\n## 5. code\n"),
            "some text\n\n## 5. code\n"
        );
        // 相邻标题之间不需要额外空行。
        assert_eq!(
            ensure_block_boundaries("## A\n### B\n"),
            "## A\n### B\n"
        );
    }

    #[test]
    fn inserts_blank_line_before_pipe_table_after_paragraph() {
        let input = "text\n| a | b |\n| --- | --- |\n| 1 | 2 |\n";
        let out = ensure_block_boundaries(input);
        assert!(out.starts_with("text\n\n| a | b |"), "got: {out}");
    }

    #[test]
    fn leaves_code_fences_untouched() {
        let input = "```mermaid\ngraph TD\n## not a heading\n```\n## H\n";
        let out = ensure_block_boundaries(input);
        assert!(
            out.contains("```mermaid\ngraph TD\n## not a heading\n```"),
            "fence content must not be modified, got: {out}"
        );
    }
}
