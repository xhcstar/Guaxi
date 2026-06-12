//! Org-mode 解析：把 .org 文件解析为标准化的 `TaskItem` 列表。
//!
//! 设计：.org 文件是任务数据的「真相源」，本模块负责把它解析成前后端共享的结构化任务，
//! 再由 taskmanager 写入 SQLite 索引（看板/日历/报表/画像查询）。
//!
//! 覆盖的 org 语法子集（尽量贴近 org-mode 常用部分）：
//! - 标题层级：`*` / `**` / `***` ...
//! - TODO 关键字：TODO / DOING / NEXT / DONE / PAUSED / WAITING / CANCELLED / HOLD（可扩展）
//! - 优先级：`[#A]` / `[#B]` / `[#C]`
//! - 标签：标题行尾 `:tag1:tag2:`
//! - 计划/截止/完成时间：`SCHEDULED: <2026-06-12 ...>` / `DEADLINE:` / `CLOSED: [..]`
//! - 属性抽屉：`:PROPERTIES: ... :END:`，键值对（如 :EFFORT: :CATEGORY: 自定义键）
//! - 工时记录：`CLOCK: [start]--[end] => HH:MM`，累加为 clock_minutes
//! - 子任务：按标题层级父子关系（parent_id / children）
//! - 标题下正文：作为 raw_body

use serde::{Deserialize, Serialize};

/// 标准化任务项（前后端共享，DB schema 与之一一对应）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    /// 稳定 id：file_path + ":" + line（1 基行号）。重解析时同位置保持一致。
    pub id: String,
    pub file_path: String,
    pub line: usize,
    pub level: usize,

    pub title: String,
    /// 状态关键字（TODO/DOING/DONE/PAUSED/...），无则空串。
    pub status: String,
    /// 是否「完成型」状态（DONE/CANCELLED）。用于看板/完成率。
    pub done: bool,
    /// 优先级 A/B/C，无则空串。
    pub priority: String,
    /// 分类：work/life/study，由所在子目录推断。
    pub category: String,
    pub tags: Vec<String>,

    /// ISO 日期（YYYY-MM-DD），无则空串。
    pub scheduled: String,
    pub deadline: String,
    pub closed: String,

    /// 累计工时（分钟），来自 CLOCK 行。
    pub clock_minutes: i64,
    /// :PROPERTIES: 抽屉键值对（键统一大写）。
    pub properties: std::collections::BTreeMap<String, String>,

    /// 父任务 id（按标题层级），顶层为空串。
    pub parent_id: String,
    /// 标题下正文（不含子标题）。
    pub raw_body: String,
}

const DEFAULT_TODO_KEYWORDS: &[&str] = &[
    "TODO", "DOING", "NEXT", "WAITING", "HOLD", "PAUSED", "DONE", "CANCELLED",
];
const DONE_KEYWORDS: &[&str] = &["DONE", "CANCELLED"];

/// 从一段日期文本里抽取 ISO 日期 YYYY-MM-DD。org 时间戳形如 `<2026-06-12 Fri 14:00>` 或 `[2026-06-12]`。
fn extract_iso_date(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 10 <= bytes.len() {
        // 匹配 dddd-dd-dd
        let w = &text[i..i + 10];
        let b = w.as_bytes();
        if b[0].is_ascii_digit()
            && b[1].is_ascii_digit()
            && b[2].is_ascii_digit()
            && b[3].is_ascii_digit()
            && b[4] == b'-'
            && b[5].is_ascii_digit()
            && b[6].is_ascii_digit()
            && b[7] == b'-'
            && b[8].is_ascii_digit()
            && b[9].is_ascii_digit()
        {
            return w.to_string();
        }
        i += 1;
    }
    String::new()
}

/// 解析标题行，返回 (level, status, priority, tags, clean_title)。
fn parse_headline(
    line: &str,
    todo_keywords: &[String],
) -> Option<(usize, String, String, Vec<String>, String)> {
    let trimmed_start = line.trim_start();
    if !trimmed_start.starts_with('*') {
        return None;
    }
    // 计算开头连续 '*' 的数量，且其后必须是空格（排除 *bold* 之类）。
    let stars = trimmed_start.chars().take_while(|c| *c == '*').count();
    let after = &trimmed_start[stars..];
    if !after.starts_with(' ') {
        return None;
    }
    let mut rest = after.trim_start().to_string();

    // 状态关键字
    let mut status = String::new();
    for kw in todo_keywords {
        if rest == *kw || rest.starts_with(&format!("{} ", kw)) {
            status = kw.clone();
            rest = rest[kw.len()..].trim_start().to_string();
            break;
        }
    }

    // 优先级 [#A]
    let mut priority = String::new();
    if rest.starts_with("[#") {
        if let Some(end) = rest.find(']') {
            let p = &rest[2..end];
            if p.len() == 1 {
                priority = p.to_uppercase();
                rest = rest[end + 1..].trim_start().to_string();
            }
        }
    }

    // 行尾 tags  :a:b:
    let mut tags = Vec::new();
    let rest_trim_end = rest.trim_end();
    if rest_trim_end.ends_with(':') {
        // 从末尾往前找 tag 串：连续 :word: 形式
        if let Some(start) = rest_trim_end.rfind(" :") {
            let tag_part = &rest_trim_end[start + 1..]; // 包含前导 :
            if tag_part.len() > 2
                && tag_part.starts_with(':')
                && tag_part.ends_with(':')
                && tag_part[1..tag_part.len() - 1]
                    .split(':')
                    .all(|t| !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '@' || c == '-'))
            {
                tags = tag_part[1..tag_part.len() - 1]
                    .split(':')
                    .filter(|t| !t.is_empty())
                    .map(|t| t.to_string())
                    .collect();
                rest = rest_trim_end[..start].trim_end().to_string();
            }
        }
    }

    Some((stars, status, priority, tags, rest.trim().to_string()))
}

/// 解析一个 .org 文件文本为 TaskItem 列表。
/// `file_path` 与 `category` 用于填充每个 item。
pub fn parse_org(content: &str, file_path: &str, category: &str) -> Vec<TaskItem> {
    let todo_keywords: Vec<String> = DEFAULT_TODO_KEYWORDS.iter().map(|s| s.to_string()).collect();
    let lines: Vec<&str> = content.lines().collect();

    let mut items: Vec<TaskItem> = Vec::new();
    // 栈：保存 (level, index_in_items) 以推断父子关系。
    let mut stack: Vec<(usize, usize)> = Vec::new();

    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        if let Some((level, status, priority, tags, title)) = parse_headline(line, &todo_keywords) {
            let line_no = i + 1;
            let id = format!("{}:{}", file_path, line_no);

            // 收集该标题的「正文」直到下一个标题，提取日期/属性/CLOCK。
            let mut scheduled = String::new();
            let mut deadline = String::new();
            let mut closed = String::new();
            let mut clock_minutes: i64 = 0;
            let mut properties: std::collections::BTreeMap<String, String> =
                std::collections::BTreeMap::new();
            let mut body_lines: Vec<String> = Vec::new();
            let mut in_props = false;

            let mut j = i + 1;
            while j < lines.len() {
                let bl = lines[j];
                // 下一个标题 → 结束本节
                if parse_headline(bl, &todo_keywords).is_some() {
                    break;
                }
                let bt = bl.trim();
                // 计划/截止/完成行（可能一行含多个）
                if bt.starts_with("SCHEDULED:")
                    || bt.starts_with("DEADLINE:")
                    || bt.starts_with("CLOSED:")
                    || bt.contains("SCHEDULED:")
                    || bt.contains("DEADLINE:")
                    || bt.contains("CLOSED:")
                {
                    if let Some(p) = bt.find("SCHEDULED:") {
                        scheduled = extract_iso_date(&bt[p + "SCHEDULED:".len()..]);
                    }
                    if let Some(p) = bt.find("DEADLINE:") {
                        deadline = extract_iso_date(&bt[p + "DEADLINE:".len()..]);
                    }
                    if let Some(p) = bt.find("CLOSED:") {
                        closed = extract_iso_date(&bt[p + "CLOSED:".len()..]);
                    }
                    j += 1;
                    continue;
                }
                // 属性抽屉
                if bt.eq_ignore_ascii_case(":PROPERTIES:") {
                    in_props = true;
                    j += 1;
                    continue;
                }
                if in_props {
                    if bt.eq_ignore_ascii_case(":END:") {
                        in_props = false;
                    } else if bt.starts_with(':') {
                        if let Some(end) = bt[1..].find(':') {
                            let key = bt[1..1 + end].to_uppercase();
                            let val = bt[1 + end + 1..].trim().to_string();
                            properties.insert(key, val);
                        }
                    }
                    j += 1;
                    continue;
                }
                // CLOCK 工时：CLOCK: [..]--[..] =>  H:MM
                if bt.starts_with("CLOCK:") {
                    if let Some(arrow) = bt.find("=>") {
                        let dur = bt[arrow + 2..].trim();
                        if let Some((h, m)) = dur.split_once(':') {
                            let hh: i64 = h.trim().parse().unwrap_or(0);
                            let mm: i64 = m.trim().parse().unwrap_or(0);
                            clock_minutes += hh * 60 + mm;
                        }
                    }
                    j += 1;
                    continue;
                }
                body_lines.push(bl.to_string());
                j += 1;
            }

            // EFFORT 属性也可贡献工时（HH:MM）
            if let Some(eff) = properties.get("EFFORT") {
                if let Some((h, m)) = eff.split_once(':') {
                    let _ = (h, m); // EFFORT 是预估，不计入实际 clock_minutes
                }
            }
            // 分类可被属性 CATEGORY 覆盖
            let cat = properties
                .get("CATEGORY")
                .map(|s| s.to_lowercase())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| category.to_string());

            // 父子关系：弹出栈中 level >= 当前的
            while let Some(&(lvl, _)) = stack.last() {
                if lvl >= level {
                    stack.pop();
                } else {
                    break;
                }
            }
            let parent_id = stack
                .last()
                .map(|&(_, idx)| items[idx].id.clone())
                .unwrap_or_default();

            let done = DONE_KEYWORDS.iter().any(|k| *k == status);

            let item = TaskItem {
                id: id.clone(),
                file_path: file_path.to_string(),
                line: line_no,
                level,
                title,
                status,
                done,
                priority,
                category: cat,
                tags,
                scheduled,
                deadline,
                closed,
                clock_minutes,
                properties,
                parent_id,
                raw_body: body_lines.join("\n").trim().to_string(),
            };
            let idx = items.len();
            items.push(item);
            stack.push((level, idx));

            i = j;
            continue;
        }
        i += 1;
    }

    items
}

/// 在 .org 文本里，对某一行(1基)所属的标题节做「字段级」最小回写。
/// 支持改：status、title、scheduled、deadline、notes(追加/替换正文首段)。
/// 返回新的完整文本。仅触碰目标节，保留其余内容/缩进。
pub struct OrgPatch {
    pub status: Option<String>,
    pub title: Option<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub notes: Option<String>,
}

pub fn apply_patch(content: &str, line_no: usize, patch: &OrgPatch) -> Result<String, String> {
    let todo_keywords: Vec<String> = DEFAULT_TODO_KEYWORDS.iter().map(|s| s.to_string()).collect();
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    if line_no == 0 || line_no > lines.len() {
        return Err(format!("行号越界: {}", line_no));
    }
    let idx = line_no - 1;
    let headline = lines[idx].clone();
    let parsed = parse_headline(&headline, &todo_keywords)
        .ok_or_else(|| format!("第 {} 行不是 org 标题", line_no))?;
    let (level, cur_status, priority, tags, cur_title) = parsed;

    // 重建标题行
    let new_status = patch.status.clone().unwrap_or(cur_status);
    let new_title = patch.title.clone().unwrap_or(cur_title);
    let mut hl = "*".repeat(level);
    hl.push(' ');
    if !new_status.is_empty() {
        hl.push_str(&new_status);
        hl.push(' ');
    }
    if !priority.is_empty() {
        hl.push_str(&format!("[#{}] ", priority));
    }
    hl.push_str(new_title.trim());
    if !tags.is_empty() {
        hl.push_str(&format!("  :{}:", tags.join(":")));
    }
    lines[idx] = hl;

    // 找到该节的范围 [idx+1, next_headline)
    let mut end = lines.len();
    for k in (idx + 1)..lines.len() {
        if parse_headline(&lines[k], &todo_keywords).is_some() {
            end = k;
            break;
        }
    }

    // 更新/插入 SCHEDULED / DEADLINE
    let upsert_planning = |lines: &mut Vec<String>, end: &mut usize, key: &str, date: &str| {
        // 在节内找含 key 的行
        let mut found: Option<usize> = None;
        for k in (idx + 1)..*end {
            if lines[k].contains(&format!("{}:", key)) {
                found = Some(k);
                break;
            }
        }
        let stamp = if date.is_empty() {
            String::new()
        } else {
            format!("{}: <{}>", key, date)
        };
        match found {
            Some(k) => {
                if date.is_empty() {
                    lines.remove(k);
                    *end -= 1;
                } else {
                    // 替换该行里的该 key 段（简单整行替换为单一 planning）
                    lines[k] = format!("   {}", stamp);
                }
            }
            None => {
                if !date.is_empty() {
                    lines.insert(idx + 1, format!("   {}", stamp));
                    *end += 1;
                }
            }
        }
    };
    if let Some(s) = &patch.scheduled {
        upsert_planning(&mut lines, &mut end, "SCHEDULED", s);
    }
    if let Some(d) = &patch.deadline {
        upsert_planning(&mut lines, &mut end, "DEADLINE", d);
    }

    // notes：替换正文（保留 planning/属性/CLOCK 行，替换其余正文段）
    if let Some(notes) = &patch.notes {
        // 计算正文起点：跳过紧随标题的 planning/properties/clock 行
        let mut body_start = idx + 1;
        let mut in_props = false;
        while body_start < end {
            let bt = lines[body_start].trim();
            if bt.eq_ignore_ascii_case(":PROPERTIES:") {
                in_props = true;
                body_start += 1;
                continue;
            }
            if in_props {
                if bt.eq_ignore_ascii_case(":END:") {
                    in_props = false;
                }
                body_start += 1;
                continue;
            }
            if bt.starts_with("SCHEDULED:")
                || bt.starts_with("DEADLINE:")
                || bt.starts_with("CLOSED:")
                || bt.starts_with("CLOCK:")
            {
                body_start += 1;
                continue;
            }
            break;
        }
        // 删除 [body_start, end) 旧正文，插入新 notes（非空时）
        lines.drain(body_start..end);
        if !notes.trim().is_empty() {
            let inserted: Vec<String> = notes.lines().map(|l| l.to_string()).collect();
            for (off, l) in inserted.into_iter().enumerate() {
                lines.insert(body_start + off, l);
            }
        }
    }

    Ok(lines.join("\n") + "\n")
}
