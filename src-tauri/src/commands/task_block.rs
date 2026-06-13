//! GUAXI 任务块 DSL 解析（替代 org-mode）。
//!
//! 每个任务/bug 是一个由 `##GUAXI_TASK_START` 与 `##GUAXI_TASK_END` 包裹的块，
//! 块内是 `#FIELD: value` 字段。一个文件可含多个块。.org 文件为真相源，
//! SQLite 仅作索引（看板/日历/统计/报表）。
//!
//! 字段约定见 TaskBlock 注释。解析规则：
//! - 字段行形如 `#<KNOWN FIELD NAME>:`，名字必须命中已知字段集合；其余行视为上一字段的续行（多行值）。
//! - `#EXTRA IMPACT:` 可出现多次，收集为 Vec。
//! - `#NOTES START:` … `#NOTES END:` 之间为内联笔记；`#NOTES START("path"):` 为外链笔记文件。
//! - `#STATUS:` 值形如 “进行中 DOING”，解析出英文规范状态；`#PRIORITY:` 形如 `[#A]` → "A"。

use serde::{Deserialize, Serialize};

/// 笔记：内联文本，或指向外部 .org 文件的相对/绝对路径。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "value")]
pub enum TaskNotes {
    #[serde(rename = "inline")]
    Inline(String),
    #[serde(rename = "file")]
    FileRef(String),
    #[serde(rename = "none")]
    None,
}

impl Default for TaskNotes {
    fn default() -> Self {
        TaskNotes::None
    }
}

/// 一个任务/bug 块的结构化表示。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaskBlock {
    /// 系统生成的唯一 id（UUID），块的稳定主键。空则由系统生成并回写。
    pub internal_id: String,
    /// 用户填写的 bug/task id；TYPE=other 且为空时系统生成（带 GX-OTHER- 前缀）。
    pub id: String,
    pub parent_id: String,
    /// work/life/study，由所在目录推导（字段值仅作展示）。
    pub manage_type: String,
    /// bug/task/other。
    pub task_type: String,
    pub deadline: String,
    pub create_time: String,
    /// 完成时间：STATUS 变为 DONE/CANCELLED 时由系统写入（持久化进 .org，存活于重扫描）。
    pub completed_time: String,
    pub origin: String,
    pub severity: String,
    /// A/B/C 或空。
    pub priority: String,
    /// 英文规范状态：TODO/NEXT/DOING/WAITING/PAUSED/HOLD/CANCELLED/DONE。
    pub status: String,
    /// 是否完成型（DONE/CANCELLED）。
    pub done: bool,
    pub progress: String,
    pub fix_status: String,
    pub description: String,
    pub root_cause: String,
    pub condition_of_occurrence: String,
    pub fixed_details: String,
    pub testing_suggestions: String,
    pub affected_release: String,
    pub review_link: String,
    pub extra_impacts: Vec<String>,
    pub content: String,
    pub notes: TaskNotes,

    pub file_path: String,
    /// 块起始行（`##GUAXI_TASK_START` 所在行，1 基）。
    pub block_start_line: usize,
    /// 块结束行（`##GUAXI_TASK_END` 所在行，1 基）。
    pub block_end_line: usize,
}

pub const BLOCK_START: &str = "##GUAXI_TASK_START";
pub const BLOCK_END: &str = "##GUAXI_TASK_END";

/// 已知属性/字段名（用于识别字段行与续行）。顺序即模板顺序。
pub const KNOWN_FIELDS: &[&str] = &[
    "MANAGE TASK TYPE",
    "INTERNAL ID",
    "ID",
    "PARENT TASK",
    "TYPE",
    "DEADLINE",
    "CREATE TIME",
    "COMPLETED TIME",
    "ORIGIN",
    "SEVERITY",
    "PRIORITY",
    "STATUS",
    "PROGRESS",
    "FIX STATUS",
    "DESCRIPTION",
    "ROOT CAUSE",
    "CONDITION OF OCCURRENCE",
    "FIXED DETAILS",
    "TESTING SUGGESTIONS",
    "AFFECTED RELEASE",
    "REVIEW LINK",
    "EXTRA IMPACT",
    "CONTENT",
];

const STATUS_KEYWORDS: &[&str] = &[
    "TODO", "NEXT", "DOING", "WAITING", "PAUSED", "HOLD", "CANCELLED", "DONE",
];
const DONE_STATUS: &[&str] = &["DONE", "CANCELLED"];

/// 若一行是 `#<KNOWN FIELD>:`（仅英文冒号），返回 (字段名, 冒号后的值)。
fn parse_field_line(line: &str) -> Option<(&'static str, String)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') || trimmed.starts_with("##") {
        return None;
    }
    let after_hash = &trimmed[1..];
    // 仅识别英文冒号；全角冒号交由校验器报错。
    let colon = after_hash.find(':')?;
    let name_raw = after_hash[..colon].trim();
    let name_upper = name_raw.to_uppercase();
    for f in KNOWN_FIELDS {
        if name_upper == *f {
            let val = after_hash[colon + 1..].trim().to_string();
            return Some((f, val));
        }
    }
    None
}

/// 是否是 NOTES 起始行；返回 Some(Option<path>)：Some(Some(path)) 外链；Some(None) 内联起始。
fn parse_notes_start(line: &str) -> Option<Option<String>> {
    let trimmed = line.trim_start();
    let upper = trimmed.to_uppercase();
    if !upper.starts_with("#NOTES START") {
        return None;
    }
    // 提取 ("...") 中的路径
    if let Some(lp) = trimmed.find('(') {
        if let Some(rp) = trimmed[lp..].find(')') {
            let inside = &trimmed[lp + 1..lp + rp];
            let path = inside.trim().trim_matches('"').trim().to_string();
            if !path.is_empty() {
                return Some(Some(path));
            }
        }
    }
    Some(None)
}

fn is_notes_end(line: &str) -> bool {
    line.trim_start().to_uppercase().starts_with("#NOTES END")
}

/// 从 STATUS 值里抽取英文规范状态关键字。
fn canonical_status(value: &str) -> String {
    let upper = value.to_uppercase();
    for kw in STATUS_KEYWORDS {
        if upper.contains(kw) {
            return kw.to_string();
        }
    }
    String::new()
}

/// 从 PRIORITY 值里抽取 A/B/C。
fn canonical_priority(value: &str) -> String {
    let upper = value.to_uppercase();
    if let Some(pos) = upper.find("[#") {
        let rest = &upper[pos + 2..];
        if let Some(c) = rest.chars().next() {
            if ('A'..='Z').contains(&c) {
                return c.to_string();
            }
        }
    }
    // 也允许直接写 A/B/C
    let t = value.trim();
    if t.len() == 1 {
        let c = t.chars().next().unwrap().to_ascii_uppercase();
        if ('A'..='C').contains(&c) {
            return c.to_string();
        }
    }
    String::new()
}

/// 解析一个文件的全部任务块。
pub fn parse_blocks(content: &str, file_path: &str, category: &str) -> Vec<TaskBlock> {
    let lines: Vec<&str> = content.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        if lines[i].trim_start().starts_with(BLOCK_START) {
            let start_line = i + 1; // 1 基
            // 收集到 BLOCK_END
            let mut j = i + 1;
            let mut body: Vec<&str> = Vec::new();
            let mut end_line = lines.len();
            while j < lines.len() {
                if lines[j].trim_start().starts_with(BLOCK_END) {
                    end_line = j + 1;
                    break;
                }
                body.push(lines[j]);
                j += 1;
            }
            let mut block = parse_one_block(&body, file_path, category);
            block.block_start_line = start_line;
            block.block_end_line = end_line;
            blocks.push(block);
            i = j + 1;
            continue;
        }
        i += 1;
    }
    blocks
}

/// 解析单个块体（不含起止标记行）。
fn parse_one_block(body: &[&str], file_path: &str, category: &str) -> TaskBlock {
    let mut b = TaskBlock {
        file_path: file_path.to_string(),
        manage_type: category.to_string(),
        notes: TaskNotes::None,
        ..Default::default()
    };
    // 续行累加到“当前字段”
    let mut current: Option<&'static str> = None;
    let mut buf: String = String::new();

    // 把缓冲值落到对应字段
    fn flush(b: &mut TaskBlock, field: Option<&'static str>, buf: &str) {
        let val = buf.trim_end_matches('\n').to_string();
        let trimmed = val.trim().to_string();
        match field {
            Some("MANAGE TASK TYPE") => { /* 目录推导优先，忽略文件值 */ }
            Some("INTERNAL ID") => b.internal_id = trimmed,
            Some("ID") => b.id = trimmed,
            Some("PARENT TASK") => b.parent_id = trimmed,
            Some("TYPE") => b.task_type = trimmed.to_lowercase(),
            Some("DEADLINE") => b.deadline = trimmed,
            Some("CREATE TIME") => b.create_time = trimmed,
            Some("COMPLETED TIME") => b.completed_time = trimmed,
            Some("ORIGIN") => b.origin = trimmed,
            Some("SEVERITY") => b.severity = trimmed,
            Some("PRIORITY") => b.priority = canonical_priority(&trimmed),
            Some("STATUS") => {
                b.status = canonical_status(&trimmed);
                b.done = DONE_STATUS.iter().any(|s| *s == b.status);
            }
            Some("PROGRESS") => b.progress = trimmed,
            Some("FIX STATUS") => b.fix_status = trimmed,
            Some("DESCRIPTION") => b.description = trimmed,
            Some("ROOT CAUSE") => b.root_cause = trimmed,
            Some("CONDITION OF OCCURRENCE") => b.condition_of_occurrence = val,
            Some("FIXED DETAILS") => b.fixed_details = val,
            Some("TESTING SUGGESTIONS") => b.testing_suggestions = val,
            Some("AFFECTED RELEASE") => b.affected_release = trimmed,
            Some("REVIEW LINK") => b.review_link = trimmed,
            Some("EXTRA IMPACT") => {
                if !val.trim().is_empty() {
                    b.extra_impacts.push(val);
                }
            }
            Some("CONTENT") => b.content = val,
            _ => {}
        }
    }

    let mut k = 0usize;
    while k < body.len() {
        let line = body[k];

        // NOTES 起始：特殊整体消费
        if let Some(path_opt) = parse_notes_start(line) {
            // 先 flush 之前的字段
            flush(&mut b, current, &buf);
            current = None;
            buf.clear();
            match path_opt {
                Some(path) => {
                    b.notes = TaskNotes::FileRef(path);
                    // 跳过可能紧随的 NOTES END
                    k += 1;
                    if k < body.len() && is_notes_end(body[k]) {
                        k += 1;
                    }
                }
                None => {
                    // 内联：消费到 NOTES END
                    let mut notes_lines: Vec<&str> = Vec::new();
                    k += 1;
                    while k < body.len() && !is_notes_end(body[k]) {
                        notes_lines.push(body[k]);
                        k += 1;
                    }
                    if k < body.len() && is_notes_end(body[k]) {
                        k += 1;
                    }
                    b.notes = TaskNotes::Inline(notes_lines.join("\n").trim_matches('\n').to_string());
                }
            }
            continue;
        }

        if let Some((field, val)) = parse_field_line(line) {
            // 落上一字段
            flush(&mut b, current, &buf);
            buf.clear();
            current = Some(field);
            buf.push_str(&val);
            buf.push('\n');
            k += 1;
            continue;
        }

        // 续行
        if current.is_some() {
            buf.push_str(line);
            buf.push('\n');
        }
        k += 1;
    }
    flush(&mut b, current, &buf);
    b
}

/// 生成 other 类型的 id（带前缀，避免与用户 id 冲突）。
pub fn gen_other_id() -> String {
    format!("GX-OTHER-{}", short_uuid())
}

/// 为缺失 internal_id 的块就地补上 UUID（最小行编辑，保留原格式）；
/// 对 type=other 且 id 为空的块补上 GX-OTHER- id。返回 (新内容, 是否改动)。
pub fn backfill_ids(content: &str, category: &str) -> (String, bool) {
    let blocks = parse_blocks(content, "", category);
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut changed = false;
    // 反向处理，避免插入改变后续块行号
    for b in blocks.iter().rev() {
        let body_start = b.block_start_line; // 0 基首条体行 = 1 基 START 行号
        let marker_end = b.block_end_line.saturating_sub(1); // 0 基 END 行（不含）
        if b.internal_id.trim().is_empty() {
            patch_or_insert_field(&mut lines, body_start, marker_end, "INTERNAL ID", &gen_internal_id());
            changed = true;
        }
        if b.task_type == "other" && b.id.trim().is_empty() {
            patch_or_insert_field(&mut lines, body_start, marker_end, "ID", &gen_other_id());
            changed = true;
        }
        // 完成态但缺完成时间 → 补当前时间；非完成态但残留完成时间 → 清空
        let is_done = b.status == "DONE" || b.status == "CANCELLED";
        if is_done && b.completed_time.trim().is_empty() {
            patch_or_insert_field(&mut lines, body_start, marker_end, "COMPLETED TIME", &now_cn_datetime());
            changed = true;
        } else if !is_done && !b.completed_time.trim().is_empty() {
            patch_or_insert_field(&mut lines, body_start, marker_end, "COMPLETED TIME", "");
            changed = true;
        }
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    (out, changed)
}

fn patch_or_insert_field(
    lines: &mut Vec<String>,
    body_start: usize,
    marker_end: usize,
    field: &str,
    value: &str,
) {
    let upper = marker_end.min(lines.len());
    for idx in body_start..upper {
        if let Some((name, _)) = parse_field_line(&lines[idx]) {
            if name.eq_ignore_ascii_case(field) {
                lines[idx] = format!("#{}: {}", field, value);
                return;
            }
        }
    }
    let insert_at = body_start.min(lines.len());
    lines.insert(insert_at, format!("#{}: {}", field, value));
}

/// 生成 internal id（UUID v4）。
pub fn gen_internal_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 当前时间中文格式 2026年06月13日 13:13:44
pub fn now_cn_datetime() -> String {
    chrono::Local::now().format("%Y年%m月%d日 %H:%M:%S").to_string()
}

fn short_uuid() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

/// 用一个块的最新字段值重建块文本（用于预览回写 / 新建块）。
/// 仅输出非空字段与必要的模板字段，保持稳定顺序。
pub fn serialize_block(b: &TaskBlock) -> String {
    let mut out = String::new();
    out.push_str(BLOCK_START);
    out.push('\n');
    let push_field = |out: &mut String, name: &str, val: &str| {
        out.push_str(&format!("#{}: {}\n", name, val));
    };
    let manage_label = match b.manage_type.as_str() {
        "work" => "work 工作",
        "life" => "life 生活",
        "study" => "study 学习",
        other => other,
    };
    push_field(&mut out, "MANAGE TASK TYPE", manage_label);
    push_field(&mut out, "INTERNAL ID", &b.internal_id);
    push_field(&mut out, "ID", &b.id);
    push_field(&mut out, "PARENT TASK", &b.parent_id);
    push_field(&mut out, "TYPE", &b.task_type);
    push_field(&mut out, "DEADLINE", &b.deadline);
    push_field(&mut out, "CREATE TIME", &b.create_time);
    push_field(&mut out, "COMPLETED TIME", &b.completed_time);
    push_field(&mut out, "ORIGIN", &b.origin);
    push_field(&mut out, "SEVERITY", &b.severity);
    push_field(
        &mut out,
        "PRIORITY",
        &(if b.priority.is_empty() {
            String::new()
        } else {
            format!("[#{}]", b.priority)
        }),
    );
    push_field(&mut out, "STATUS", &b.status);
    push_field(&mut out, "PROGRESS", &b.progress);
    push_field(&mut out, "FIX STATUS", &b.fix_status);
    push_field(&mut out, "DESCRIPTION", &b.description);
    push_field(&mut out, "ROOT CAUSE", &b.root_cause);
    // 多行字段：标题行 + 续行原样
    let push_multi = |out: &mut String, name: &str, val: &str| {
        if val.contains('\n') {
            let mut it = val.lines();
            let first = it.next().unwrap_or("");
            out.push_str(&format!("#{}: {}\n", name, first));
            for l in it {
                out.push_str(l);
                out.push('\n');
            }
        } else {
            out.push_str(&format!("#{}: {}\n", name, val));
        }
    };
    push_multi(&mut out, "CONDITION OF OCCURRENCE", &b.condition_of_occurrence);
    push_multi(&mut out, "FIXED DETAILS", &b.fixed_details);
    push_multi(&mut out, "TESTING SUGGESTIONS", &b.testing_suggestions);
    push_field(&mut out, "AFFECTED RELEASE", &b.affected_release);
    push_field(&mut out, "REVIEW LINK", &b.review_link);
    for ei in &b.extra_impacts {
        push_multi(&mut out, "EXTRA IMPACT", ei);
    }
    push_multi(&mut out, "CONTENT", &b.content);
    match &b.notes {
        TaskNotes::Inline(text) => {
            out.push_str("#NOTES START:\n");
            out.push_str(text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("#NOTES END:\n");
        }
        TaskNotes::FileRef(path) => {
            out.push_str(&format!("#NOTES START(\"{}\"):\n#NOTES END:\n", path));
        }
        TaskNotes::None => {}
    }
    out.push_str(BLOCK_END);
    out.push('\n');
    out
}

/// 用新块文本替换源文件中第 start..=end 行（1 基）的旧块，返回新全文。
pub fn replace_block_in_content(
    content: &str,
    start_line: usize,
    end_line: usize,
    new_block_text: &str,
) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let s = start_line.saturating_sub(1);
    let e = end_line.saturating_sub(1);
    for (idx, l) in lines.iter().enumerate() {
        if idx == s {
            // 插入新块（去掉尾随换行后逐行）
            for nl in new_block_text.trim_end_matches('\n').lines() {
                out.push(nl.to_string());
            }
        }
        if idx >= s && idx <= e {
            continue;
        }
        out.push(l.to_string());
    }
    // 边界：start 超出范围时直接追加
    if s >= lines.len() {
        for nl in new_block_text.trim_end_matches('\n').lines() {
            out.push(nl.to_string());
        }
    }
    let mut result = out.join("\n");
    result.push('\n');
    result
}

// ───────── 语法校验 ─────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// 1 基行号。
    pub line: usize,
    /// "error" | "warning"
    pub severity: String,
    pub message: String,
}

fn diag(line: usize, severity: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic { line, severity: severity.to_string(), message: message.into() }
}

const SEVERITY_TOKENS: &[&str] = &["critical", "major", "normal", "minor", "trivial"];
const ORIGIN_TOKENS: &[&str] = &["customer", "qa", "dev", "other"];
const FIX_STATUS_TOKENS: &[&str] = &["debug", "test", "review", "approval", "fixed", "other"];
const TYPE_TOKENS: &[&str] = &["bug", "task", "other"];
const MANAGE_TYPE_TOKENS: &[&str] = &["work", "life", "study"];

/// 检测「字段声明行」：`#NAME:` 或 `#NAME：`（NAME 为大写字母与空格）。
/// 返回 (name, full_width_colon, value)。仅匹配看起来像字段声明的行，避免误伤 markdown。
fn detect_field_decl(line: &str) -> Option<(String, bool, String)> {
    let t = line.trim_start();
    if !t.starts_with('#') || t.starts_with("##") {
        return None;
    }
    let after = &t[1..];
    let mut name = String::new();
    let mut rest_idx = 0usize;
    let mut full = false;
    let mut found = false;
    let mut byte_i = 0usize;
    for ch in after.chars() {
        if ch == ':' {
            rest_idx = byte_i + 1;
            found = true;
            break;
        }
        if ch == '：' {
            rest_idx = byte_i + ch.len_utf8();
            found = true;
            full = true;
            break;
        }
        if ch.is_ascii_uppercase() || ch == ' ' {
            name.push(ch);
            byte_i += ch.len_utf8();
        } else {
            return None;
        }
    }
    if !found {
        return None;
    }
    let value = after[rest_idx..].trim().to_string();
    Some((name.trim().to_string(), full, value))
}

fn contains_token_ci(value: &str, tokens: &[&str]) -> bool {
    let lower = value.to_lowercase();
    tokens
        .iter()
        .any(|t| lower.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == *t))
}

fn looks_like_date(value: &str) -> bool {
    let v = value.trim();
    if v.is_empty() {
        return true;
    }
    // 中文 2026年06月13日
    if v.contains('年') && v.contains('月') && v.contains('日') {
        return true;
    }
    // ISO/斜杠：前 4 位为数字且含 - 或 /
    let bytes = v.as_bytes();
    let starts_4_digits = bytes.len() >= 4 && bytes[..4].iter().all(|b| b.is_ascii_digit());
    starts_4_digits && (v.contains('-') || v.contains('/'))
}

/// 校验整篇内容的任务块语法，返回诊断列表。
pub fn validate_content(content: &str, category: &str) -> Vec<Diagnostic> {
    let lines: Vec<&str> = content.lines().collect();
    let mut diags = Vec::new();
    let mut in_block = false;
    let mut block_start = 0usize;
    let mut i = 0usize;
    while i < lines.len() {
        let t = lines[i].trim_start();
        if t.starts_with(BLOCK_START) {
            if in_block {
                diags.push(diag(i + 1, "error", "上一个任务块未用 ##GUAXI_TASK_END 结束"));
                validate_block(&lines, block_start, i, category, &mut diags);
            }
            in_block = true;
            block_start = i;
            i += 1;
            continue;
        }
        if t.starts_with(BLOCK_END) {
            if !in_block {
                diags.push(diag(i + 1, "error", "##GUAXI_TASK_END 没有对应的 ##GUAXI_TASK_START"));
            } else {
                validate_block(&lines, block_start, i, category, &mut diags);
                in_block = false;
            }
            i += 1;
            continue;
        }
        i += 1;
    }
    if in_block {
        diags.push(diag(block_start + 1, "error", "任务块未闭合：缺少 ##GUAXI_TASK_END"));
        validate_block(&lines, block_start, lines.len(), category, &mut diags);
    }
    diags
}

fn validate_block(lines: &[&str], start_idx: usize, end_idx: usize, category: &str, diags: &mut Vec<Diagnostic>) {
    use std::collections::BTreeMap;
    let mut fields: BTreeMap<String, (usize, String, usize)> = BTreeMap::new();
    let mut notes_open: Option<usize> = None;
    let mut notes_closed = false;
    let mut in_notes = false;

    let body_start = start_idx + 1;
    let upper_bound = end_idx.min(lines.len());
    let mut k = body_start;
    while k < upper_bound {
        let line = lines[k];
        let lt = line.trim_start();
        if lt.to_uppercase().starts_with("#NOTES START") {
            notes_open = Some(k + 1);
            let is_fileref = lt.contains('(') && lt.contains(')');
            in_notes = !is_fileref;
            if is_fileref {
                notes_closed = true;
            }
            k += 1;
            continue;
        }
        if lt.to_uppercase().starts_with("#NOTES END") {
            in_notes = false;
            notes_closed = true;
            k += 1;
            continue;
        }
        if in_notes {
            k += 1;
            continue;
        }
        if let Some((name, full, value)) = detect_field_decl(line) {
            if full {
                diags.push(diag(k + 1, "error", format!("字段「{}」请使用英文冒号 ':'（不要用全角 '：'）", name)));
            } else if !KNOWN_FIELDS.contains(&name.as_str()) {
                diags.push(diag(k + 1, "warning", format!("未知字段「{}」，将被忽略；请检查拼写", name)));
            } else {
                let entry = fields.entry(name.clone()).or_insert((k + 1, value.clone(), 0));
                entry.2 += 1;
                if entry.2 == 1 {
                    entry.0 = k + 1;
                    entry.1 = value;
                }
            }
        }
        k += 1;
    }

    let type_val = fields.get("TYPE").map(|x| x.1.to_lowercase()).unwrap_or_default();
    if let Some((ln, v, _)) = fields.get("TYPE") {
        let lv = v.to_lowercase();
        if !lv.is_empty() && !TYPE_TOKENS.iter().any(|t| lv == *t) {
            diags.push(diag(*ln, "error", "TYPE 必须为 bug / task / other 之一"));
        }
        if lv == "bug" && (category == "life" || category == "study") {
            diags.push(diag(*ln, "error", "生活/学习 分类不支持 bug 类型，仅 task / other"));
        }
    }
    if let Some((ln, v, _)) = fields.get("STATUS") {
        if !v.trim().is_empty() && canonical_status(v).is_empty() {
            diags.push(diag(*ln, "error", "STATUS 无法识别，应包含 TODO/NEXT/DOING/WAITING/PAUSED/HOLD/CANCELLED/DONE 之一"));
        }
    }
    if let Some((ln, v, _)) = fields.get("PRIORITY") {
        if !v.trim().is_empty() && canonical_priority(v).is_empty() {
            diags.push(diag(*ln, "error", "PRIORITY 仅支持 [#A] / [#B] / [#C] 或留空"));
        }
    }
    if let Some((ln, v, _)) = fields.get("SEVERITY") {
        if !v.trim().is_empty() && !contains_token_ci(v, SEVERITY_TOKENS) {
            diags.push(diag(*ln, "warning", "SEVERITY 建议为 critical/major/normal/minor/trivial 之一"));
        }
    }
    if let Some((ln, v, _)) = fields.get("ORIGIN") {
        if !v.trim().is_empty() && !contains_token_ci(v, ORIGIN_TOKENS) {
            diags.push(diag(*ln, "warning", "ORIGIN 建议为 customer/QA/dev/other 之一"));
        }
    }
    if let Some((ln, v, _)) = fields.get("FIX STATUS") {
        if !v.trim().is_empty() && !contains_token_ci(v, FIX_STATUS_TOKENS) {
            diags.push(diag(*ln, "warning", "FIX STATUS 建议为 debug/test/review/approval/fixed/other 之一"));
        }
    }
    if let Some((ln, v, _)) = fields.get("PROGRESS") {
        let t = v.trim();
        if !t.is_empty() {
            let body = &t[..t.len().saturating_sub(1)];
            let ok = t.ends_with('%') && !body.is_empty() && body.chars().all(|c| c.is_ascii_digit());
            if !ok {
                diags.push(diag(*ln, "warning", "PROGRESS 建议形如 50%"));
            }
        }
    }
    if let Some((ln, v, _)) = fields.get("MANAGE TASK TYPE") {
        if !v.trim().is_empty() && !contains_token_ci(v, MANAGE_TYPE_TOKENS) {
            diags.push(diag(*ln, "warning", "MANAGE TASK TYPE 建议为 work/life/study"));
        }
    }
    for f in ["DEADLINE", "CREATE TIME"] {
        if let Some((ln, v, _)) = fields.get(f) {
            if !looks_like_date(v) {
                diags.push(diag(*ln, "warning", format!("{} 建议形如 2026年06月13日（CREATE TIME 可含 时:分:秒）", f)));
            }
        }
    }
    match fields.get("DESCRIPTION") {
        Some((ln, v, _)) if v.trim().is_empty() => {
            diags.push(diag(*ln, "warning", "建议填写 DESCRIPTION"));
        }
        None => {
            diags.push(diag(start_idx + 1, "warning", "缺少 DESCRIPTION 字段"));
        }
        _ => {}
    }
    let _ = type_val;
    for (name, (ln, _, count)) in &fields {
        if *count > 1 && name != "EXTRA IMPACT" {
            diags.push(diag(*ln, "warning", format!("字段「{}」重复出现 {} 次", name, count)));
        }
    }
    if let Some(ln) = notes_open {
        if !notes_closed {
            diags.push(diag(ln, "warning", "#NOTES START 没有对应的 #NOTES END"));
        }
    }
}
