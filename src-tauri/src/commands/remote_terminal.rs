//! 终端调试工坊 —— 日志分析管道地基（批1）。
//!
//! 目标数据是定时快照式监控日志（形如 monitor.out0）：132MB / 百万行级别。
//! 设计原则：不把整份文件读进内存，用内存映射（mmap）按需切片；行与快照帧
//! 只保存字节偏移，前端靠虚拟滚动按需拉取。后续批次的检索/解析/图表都复用
//! 这里产出的“会话 + 帧 + 行索引”结构。

use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, Mutex, OnceLock};

use chrono::NaiveDateTime;
use memmap2::Mmap;
use once_cell::sync::Lazy;
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::command;
use uuid::Uuid;

/// 解析规则：一条带命名捕获组的正则，每个命名组产出一个指标序列。
#[derive(Clone, Serialize, Deserialize)]
pub struct MetricRule {
    pub name: String,
    pub pattern: String,
}

/// 日志解析 Profile：决定如何切帧、取时间戳、抽指标。内置若干常见类型，也可自定义。
#[derive(Clone, Serialize, Deserialize)]
pub struct LogParseProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub builtin: bool,
    /// "snapshot"（banner 分帧 + 内置多形态解析，如 monitor.out）
    /// 或 "line"（逐行：可选时间戳 + 自定义规则抽指标，适配 syslog/kernel/程序日志/任意日志）
    pub mode: String,
    /// line 模式：从每行提取时间戳的正则（命名组 ts 或第 1 组）。
    #[serde(default)]
    pub timestamp_regex: Option<String>,
    /// line 模式：尝试的 chrono 时间格式（strftime）。
    #[serde(default)]
    pub timestamp_formats: Vec<String>,
    /// 自定义抽取规则（两种模式都可叠加）。命名组名即指标键。
    #[serde(default)]
    pub rules: Vec<MetricRule>,
}

fn default_profile() -> LogParseProfile {
    LogParseProfile {
        id: "snapshot".into(),
        name: "快照监控 (monitor.out / ****时间戳****)".into(),
        builtin: true,
        mode: "snapshot".into(),
        timestamp_regex: None,
        timestamp_formats: Vec::new(),
        rules: Vec::new(),
    }
}

/// 已解析状态：随所选 Profile 变化，可重解析。
struct ParseState {
    profile: LogParseProfile,
    frames: Vec<Frame>,
    parsed: Option<ParsedIndex>,
}

/// 已导入日志的会话：mmap 原文 + 行偏移索引 + 可重解析状态。
struct LogSession {
    mmap: Mmap,
    /// 每行首字节偏移，末尾追加文件长度作哨兵；line i 覆盖 [offsets[i], offsets[i+1])。
    line_offsets: Vec<usize>,
    state: Mutex<ParseState>,
}

#[derive(Clone, Serialize)]
pub struct Frame {
    /// 帧序号（0 基）。
    pub index: usize,
    /// 解析成功的 UTC 毫秒时间戳；解析失败为 None。
    pub timestamp_ms: Option<i64>,
    /// 原始时间戳行文本。
    pub timestamp_raw: String,
    /// 帧首行（banner 起始行）行号（0 基），供行视图跳转。
    pub line_start: usize,
}

#[derive(Serialize)]
pub struct ImportResult {
    pub session_id: String,
    pub path: String,
    pub byte_size: usize,
    pub line_count: usize,
    pub frame_count: usize,
    /// 全部帧（快照通常几百个，一次返回即可构建时间线）。
    pub frames: Vec<Frame>,
}

#[derive(Serialize)]
pub struct LinesResult {
    pub start: usize,
    pub lines: Vec<String>,
}

type SessionMap = HashMap<String, Arc<LogSession>>;

static LOG_SESSIONS: OnceLock<Mutex<SessionMap>> = OnceLock::new();

fn session_store() -> &'static Mutex<SessionMap> {
    LOG_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_session(session_id: &str) -> Result<Arc<LogSession>, String> {
    let store = session_store().lock().map_err(|_| "会话锁失败".to_string())?;
    store
        .get(session_id)
        .cloned()
        .ok_or_else(|| format!("找不到日志会话: {session_id}"))
}

/// 一行是否为全 `*` 的 banner 分隔线（去掉首尾空白后非空且全部是 `*`）。
fn is_star_line(bytes: &[u8]) -> bool {
    let t = trim_ascii(bytes);
    !t.is_empty() && t.iter().all(|&b| b == b'*')
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &bytes[start..end]
}

/// 尝试把时间戳行解析成 UTC 毫秒。样本形如 `Tue Sep 22 06:41:07 GMT 2026`。
/// 解析失败返回 None——保留原始文本，绝不因个别畸形行让整份导入失败。
fn parse_timestamp_ms(raw: &str) -> Option<i64> {
    let s = raw.trim();
    // 归一化多空格，兼容 `%e` 的空格补位。
    let normalized: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    for fmt in [
        "%a %b %e %H:%M:%S GMT %Y",
        "%a %b %e %H:%M:%S UTC %Y",
        "%a %b %e %H:%M:%S %Y",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(&normalized, fmt) {
            return Some(dt.and_utc().timestamp_millis());
        }
    }
    None
}

/// 单遍扫描字节，构建行偏移索引。末尾追加文件长度作哨兵。
fn build_line_offsets(bytes: &[u8]) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(bytes.len() / 40 + 2);
    offsets.push(0);
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            offsets.push(i + 1);
        }
    }
    // 若文件不以换行结尾，最后一段也已由 offsets 覆盖；追加哨兵 = 文件长度。
    if *offsets.last().unwrap() != bytes.len() {
        offsets.push(bytes.len());
    }
    offsets
}

fn line_bytes<'a>(bytes: &'a [u8], offsets: &[usize], i: usize) -> &'a [u8] {
    let start = offsets[i];
    let end = offsets.get(i + 1).copied().unwrap_or(bytes.len());
    let seg = &bytes[start..end];
    // 去掉行尾 \n 和 \r。
    let mut e = seg.len();
    if e > 0 && seg[e - 1] == b'\n' {
        e -= 1;
    }
    if e > 0 && seg[e - 1] == b'\r' {
        e -= 1;
    }
    &seg[..e]
}

/// 识别快照帧。banner 形如 三行：星线 / 时间戳 / 星线。
/// 命中后帧起始为该星线行，时间戳取中间行。
fn detect_frames(bytes: &[u8], offsets: &[usize]) -> Vec<Frame> {
    let line_count = offsets.len().saturating_sub(1);
    let mut frames = Vec::new();
    let mut i = 0usize;
    while i + 2 < line_count {
        let l0 = line_bytes(bytes, offsets, i);
        if is_star_line(l0) && is_star_line(line_bytes(bytes, offsets, i + 2)) {
            let raw = String::from_utf8_lossy(line_bytes(bytes, offsets, i + 1))
                .trim()
                .to_string();
            frames.push(Frame {
                index: frames.len(),
                timestamp_ms: parse_timestamp_ms(&raw),
                timestamp_raw: raw,
                line_start: i,
            });
            i += 3;
        } else {
            i += 1;
        }
    }
    frames
}

#[command]
pub fn log_import_file(path: String) -> Result<ImportResult, String> {
    let file = File::open(&path).map_err(|e| format!("打开文件失败: {e}"))?;
    // SAFETY: 只读映射；导入期间不修改该文件，符合 mmap 使用约束。
    let mmap = unsafe { Mmap::map(&file).map_err(|e| format!("内存映射失败: {e}"))? };
    let bytes: &[u8] = &mmap;

    let line_offsets = build_line_offsets(bytes);
    let line_count = line_offsets.len().saturating_sub(1);
    let frames = detect_frames(bytes, &line_offsets);
    let byte_size = bytes.len();
    let frames_clone = frames.clone();

    let session_id = Uuid::new_v4().to_string();
    let session = Arc::new(LogSession {
        mmap,
        line_offsets,
        state: Mutex::new(ParseState {
            profile: default_profile(),
            frames,
            parsed: None,
        }),
    });

    session_store()
        .lock()
        .map_err(|_| "会话锁失败".to_string())?
        .insert(session_id.clone(), session);

    Ok(ImportResult {
        session_id,
        path,
        byte_size,
        line_count,
        frame_count: frames_clone.len(),
        frames: frames_clone,
    })
}

#[command]
pub fn log_list_frames(session_id: String) -> Result<Vec<Frame>, String> {
    let session = get_session(&session_id)?;
    let st = session.state.lock().map_err(|_| "会话锁失败".to_string())?;
    Ok(st.frames.clone())
}

/// 读取 [start, start+count) 行；越界自动截断。供前端虚拟滚动按窗口拉取。
#[command]
pub fn log_read_lines(
    session_id: String,
    start: usize,
    count: usize,
) -> Result<LinesResult, String> {
    let session = get_session(&session_id)?;
    let bytes: &[u8] = &session.mmap;
    let total = session.line_offsets.len().saturating_sub(1);
    if start >= total {
        return Ok(LinesResult { start, lines: Vec::new() });
    }
    let end = start.saturating_add(count).min(total);
    let mut lines = Vec::with_capacity(end - start);
    for i in start..end {
        let raw = line_bytes(bytes, &session.line_offsets, i);
        lines.push(String::from_utf8_lossy(raw).into_owned());
    }
    Ok(LinesResult { start, lines })
}

#[command]
pub fn log_close_session(session_id: String) -> Result<(), String> {
    session_store()
        .lock()
        .map_err(|_| "会话锁失败".to_string())?
        .remove(&session_id);
    Ok(())
}

// ============================================================================
// 指标解析与分析（C 模块）
//
// 把每个快照帧内的命令输出解析成「指标键 -> 数值」。指标键在帧之间稳定，因此
// 同一指标可跨帧抽出成时间序列（C5），并据此做泄漏排名（C6）与快照对比（C8）。
// 解析规则已在真实 monitor.out0 上验证：元组、ATCP/vmstat 表格、zone_page_stats、
// key=value、领头/尾随数值等多形态，带 K/M/G 单位归一化（C3/C4）。
// ============================================================================

static RE_TUPLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\d+(?:/\d+)+)\s+(.+?)\s*\(([^)]+)\)\s*$").unwrap());
static RE_ZONE_SIZE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(.+?):\s+\d+\s*/\s*\d+\s+size/rsize").unwrap());
static RE_HDR_ATCP: Lazy<Regex> = Lazy::new(|| Regex::new(r"^ATCP\s+LIMIT\s+USED").unwrap());
static RE_HDR_Z: Lazy<Regex> = Lazy::new(|| Regex::new(r"^ITEM\s+SIZE\s+LIMIT\s+USED").unwrap());
static RE_HDR_M: Lazy<Regex> = Lazy::new(|| Regex::new(r"^Type\s+InUse\s+MemUse").unwrap());
static RE_ATCP_ROW: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\w+\s*,").unwrap());
static RE_Z_ROW: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.+?):\s+(.+)$").unwrap());
static RE_ZONE_PAGE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^zone:\s*(.+)$").unwrap());
static RE_USED_PAGES: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^used pages:\s*(\d+),\s*memory\s*(\d+)\s*(KB|MB|GB|K|M|G)?").unwrap()
});
static RE_TOTAL_FREE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Total Free:\s*(\d+)\s*(KB|MB|GB|K|M|G)?").unwrap());
static RE_LABEL_VAL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([A-Za-z][\w ]*?):\s*(\d+)\s*(KB|MB|GB|K|M|G)?\s*$").unwrap());
static RE_KV: Lazy<Regex> = Lazy::new(|| Regex::new(r"([A-Za-z][\w ]*?)\s*=\s*(\d+)").unwrap());
static RE_KV_PREFIX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([\w ]+?):").unwrap());
static RE_LEAD_NUM: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\d+)\s+([A-Za-z].+?)\s*$").unwrap());
static RE_TRAIL_NUM: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([A-Za-z].+?)\s+(\d+)\s*(KB|MB|GB|K|M|G|%)?\s*$").unwrap());
static RE_MEM_INLINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\d+)\s*(K|M|G|KB|MB|GB)\b").unwrap());
static RE_INT: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d+").unwrap());

fn norm_unit(num: &str, unit: &str) -> Option<f64> {
    let v: f64 = num.parse().ok()?;
    let scaled = match unit.to_ascii_uppercase().as_str() {
        "K" | "KB" => v * 1024.0,
        "M" | "MB" => v * 1024.0 * 1024.0,
        "G" | "GB" => v * 1024.0 * 1024.0 * 1024.0,
        _ => v,
    };
    Some(scaled)
}

/// 标签是否“像文字”：字母开头、独立整数不超过 1 个——滤掉 top 进程表这类噪声行。
fn wordy(label: &str) -> bool {
    if !label.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
        return false;
    }
    RE_INT.find_iter(label).count() <= 1
}

fn clean_label(s: &str) -> String {
    s.trim().trim_end_matches(':').trim().to_string()
}

/// 解析单个命令块的所有行，产出 (指标键, 数值)。
fn parse_block(cmd: &str, lines: &[String], out: &mut Vec<(String, f64)>) {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut zone: Option<String> = None;
    let mut cols: Vec<String> = Vec::new();
    let mut mode: u8 = 0; // 0 none, 1 atcp, 2 z, 3 m

    let push = |seen: &mut std::collections::HashSet<String>,
                    out: &mut Vec<(String, f64)>,
                    label: &str,
                    value: Option<f64>| {
        let v = match value {
            Some(v) if v.is_finite() => v,
            _ => return,
        };
        let label = clean_label(label);
        if label.is_empty() {
            return;
        }
        let key = format!("{cmd} ▸ {label}");
        if seen.insert(key.clone()) {
            out.push((key, v));
        }
    };

    for raw in lines {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }

        if RE_HDR_ATCP.is_match(t) {
            cols = t.split_whitespace().map(|s| s.to_string()).collect();
            mode = 1;
            continue;
        }
        if RE_HDR_Z.is_match(t) {
            cols = t.split_whitespace().map(|s| s.to_string()).collect();
            mode = 2;
            continue;
        }
        if RE_HDR_M.is_match(t) {
            cols = t.split_whitespace().map(|s| s.to_string()).collect();
            mode = 3;
            continue;
        }

        if let Some(c) = RE_ZONE_SIZE.captures(t) {
            zone = Some(c[1].trim().to_string());
            continue;
        }

        if mode == 1 && !cols.is_empty() && zone.is_some() && RE_ATCP_ROW.is_match(t) {
            let p: Vec<&str> = t.split(',').map(|s| s.trim()).collect();
            let z = zone.as_deref().unwrap_or("");
            for j in 1..p.len().min(cols.len()) {
                push(
                    &mut seen,
                    out,
                    &format!("{z} ▸ row {} ▸ {}", p[0], cols[j]),
                    norm_unit(p[j], ""),
                );
            }
            continue;
        }
        if mode == 2 && !cols.is_empty() {
            if let Some(c) = RE_Z_ROW.captures(t) {
                let name = c[1].trim().to_string();
                let p: Vec<&str> = c[2].split(',').map(|s| s.trim()).collect();
                let mut k = 0;
                while k < p.len() && k + 1 < cols.len() {
                    push(
                        &mut seen,
                        out,
                        &format!("{name} ▸ {}", cols[k + 1]),
                        norm_unit(p[k], ""),
                    );
                    k += 1;
                }
                continue;
            }
        }
        if mode == 3 && !cols.is_empty() {
            let p: Vec<&str> = t.split_whitespace().collect();
            if p.len() >= 3 && p[0].chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
            {
                push(&mut seen, out, &format!("{} ▸ InUse", p[0]), norm_unit(p[1], ""));
                if let Some(um) = RE_MEM_INLINE.captures(t) {
                    push(
                        &mut seen,
                        out,
                        &format!("{} ▸ MemUse", p[0]),
                        norm_unit(&um[1], &um[2]),
                    );
                }
                continue;
            }
        }

        if let Some(c) = RE_ZONE_PAGE.captures(t) {
            zone = Some(c[1].trim().to_string());
            continue;
        }
        if let Some(c) = RE_USED_PAGES.captures(t) {
            if let Some(z) = zone.as_deref() {
                push(&mut seen, out, &format!("zone {z} ▸ used pages"), norm_unit(&c[1], ""));
                let unit = c.get(3).map(|m| m.as_str()).unwrap_or("");
                push(&mut seen, out, &format!("zone {z} ▸ memory"), norm_unit(&c[2], unit));
                continue;
            }
        }
        if let Some(c) = RE_TOTAL_FREE.captures(t) {
            let unit = c.get(2).map(|m| m.as_str()).unwrap_or("");
            push(&mut seen, out, "Total Free", norm_unit(&c[1], unit));
            continue;
        }

        if let Some(c) = RE_TUPLE.captures(t) {
            let vals: Vec<&str> = c[1].split('/').collect();
            let names: Vec<&str> = c[3].split('/').map(|s| s.trim()).collect();
            if vals.len() == names.len() {
                let label = c[2].trim().to_string();
                for i in 0..vals.len() {
                    push(
                        &mut seen,
                        out,
                        &format!("{label} ▸ {}", names[i]),
                        norm_unit(vals[i], ""),
                    );
                }
                continue;
            }
        }

        if let Some(c) = RE_LABEL_VAL.captures(t) {
            if wordy(&c[1]) {
                let unit = c.get(3).map(|m| m.as_str()).unwrap_or("");
                push(&mut seen, out, c[1].trim(), norm_unit(&c[2], unit));
                continue;
            }
        }

        if t.contains('=') {
            let kvs: Vec<_> = RE_KV.captures_iter(t).collect();
            if !kvs.is_empty() {
                let prefix = RE_KV_PREFIX
                    .captures(t)
                    .map(|c| format!("{} ▸ ", c[1].trim()))
                    .unwrap_or_default();
                for kv in kvs {
                    push(
                        &mut seen,
                        out,
                        &format!("{prefix}{}", kv[1].trim()),
                        norm_unit(&kv[2], ""),
                    );
                }
                continue;
            }
        }

        if let Some(c) = RE_LEAD_NUM.captures(t) {
            if wordy(&c[2]) {
                push(&mut seen, out, c[2].trim(), norm_unit(&c[1], ""));
                continue;
            }
        }
        if let Some(c) = RE_TRAIL_NUM.captures(t) {
            if wordy(&c[1]) {
                let unit = c.get(3).map(|m| m.as_str()).unwrap_or("");
                let unit = if unit == "%" { "" } else { unit };
                push(&mut seen, out, c[1].trim(), norm_unit(&c[2], unit));
                continue;
            }
        }
    }
}

fn is_cmd_line(t: &str) -> bool {
    let b = t.as_bytes();
    b.len() > 1 && b[0] == b'/' && !b[1].is_ascii_whitespace()
}

static RE_NUM_UNIT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(-?\d+(?:\.\d+)?)\s*(KB|MB|GB|kB|K|M|G)?").unwrap());

/// 从任意字符串里取“数值[+单位]”，供自定义规则的命名组取值。
fn parse_num_with_unit(s: &str) -> Option<f64> {
    let c = RE_NUM_UNIT.captures(s)?;
    let unit = c.get(2).map(|m| m.as_str()).unwrap_or("");
    norm_unit(&c[1], unit)
}

/// 编译后的自定义规则。
struct CompiledRule {
    rule_name: String,
    re: Regex,
    names: Vec<String>,
}

fn compile_rules(rules: &[MetricRule]) -> Vec<CompiledRule> {
    rules
        .iter()
        .filter_map(|r| {
            let re = Regex::new(&r.pattern).ok()?;
            let names: Vec<String> = re.capture_names().flatten().map(|s| s.to_string()).collect();
            Some(CompiledRule { rule_name: r.name.clone(), re, names })
        })
        .collect()
}

/// 对一行应用所有自定义规则，抽出 (指标键, 数值)。命名组名即指标；无命名组用第 1 组。
fn apply_rules(text: &str, rules: &[CompiledRule], out: &mut Vec<(String, f64)>) {
    for rc in rules {
        if let Some(caps) = rc.re.captures(text) {
            if rc.names.is_empty() {
                if let Some(m) = caps.get(1) {
                    if let Some(v) = parse_num_with_unit(m.as_str()) {
                        out.push((rc.rule_name.clone(), v));
                    }
                }
            } else {
                for name in &rc.names {
                    if let Some(m) = caps.name(name) {
                        if let Some(v) = parse_num_with_unit(m.as_str()) {
                            let key = if rc.rule_name.is_empty() {
                                name.clone()
                            } else {
                                format!("{} ▸ {}", rc.rule_name, name)
                            };
                            out.push((key, v));
                        }
                    }
                }
            }
        }
    }
}

/// 解析一帧（snapshot 模式）：内置多形态 + 叠加自定义规则。
fn parse_frame(
    bytes: &[u8],
    offsets: &[usize],
    frames: &[Frame],
    frame_idx: usize,
    rules: &[CompiledRule],
) -> Vec<(String, f64)> {
    let total = offsets.len().saturating_sub(1);
    let start = frames[frame_idx].line_start;
    let end = frames.get(frame_idx + 1).map(|f| f.line_start).unwrap_or(total);

    let mut out: Vec<(String, f64)> = Vec::new();
    let mut cur_cmd: Option<String> = None;
    let mut block: Vec<String> = Vec::new();

    for i in start..end {
        let text = String::from_utf8_lossy(line_bytes(bytes, offsets, i)).into_owned();
        if !rules.is_empty() {
            apply_rules(&text, rules, &mut out);
        }
        if is_cmd_line(text.trim()) {
            if let Some(cmd) = cur_cmd.take() {
                parse_block(&cmd, &block, &mut out);
            }
            cur_cmd = Some(text.trim().to_string());
            block.clear();
        } else {
            block.push(text);
        }
    }
    if let Some(cmd) = cur_cmd.take() {
        parse_block(&cmd, &block, &mut out);
    }
    out
}

/// 时间序列单点：x 为横轴（帧序号或行内序号），ts 为可选真实时间戳（毫秒）。
#[derive(Clone, Copy)]
struct SP {
    x: f64,
    ts: Option<i64>,
    value: f64,
}

/// 全量解析后的指标索引。键做 interning 以省内存。
struct ParsedIndex {
    keys: Vec<String>,
    key_id: HashMap<String, u32>,
    series: Vec<Vec<SP>>,
    /// snapshot 模式：frame -> [(key_id, value)]；line 模式为空。
    frame_samples: Vec<Vec<(u32, f64)>>,
}

impl ParsedIndex {
    fn intern(&mut self, k: String) -> u32 {
        match self.key_id.get(&k) {
            Some(&id) => id,
            None => {
                let id = self.keys.len() as u32;
                self.key_id.insert(k.clone(), id);
                self.keys.push(k);
                self.series.push(Vec::new());
                id
            }
        }
    }
    fn empty() -> Self {
        ParsedIndex { keys: Vec::new(), key_id: HashMap::new(), series: Vec::new(), frame_samples: Vec::new() }
    }
}

/// 尝试用给定 chrono 格式（外加内置格式）解析时间戳为毫秒。
fn parse_ts_with_formats(raw: &str, formats: &[String]) -> Option<i64> {
    let s: String = raw.trim().split_whitespace().collect::<Vec<_>>().join(" ");
    for fmt in formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(&s, fmt) {
            return Some(dt.and_utc().timestamp_millis());
        }
    }
    parse_timestamp_ms(&s)
}

fn build_index_snapshot(
    bytes: &[u8],
    offsets: &[usize],
    frames: &[Frame],
    rules: &[CompiledRule],
) -> ParsedIndex {
    let frame_count = frames.len();
    let per_frame: Vec<Vec<(String, f64)>> = (0..frame_count)
        .into_par_iter()
        .map(|i| parse_frame(bytes, offsets, frames, i, rules))
        .collect();

    let mut idx = ParsedIndex::empty();
    idx.frame_samples = Vec::with_capacity(frame_count);
    for (fi, samples) in per_frame.into_iter().enumerate() {
        let mut fs: Vec<(u32, f64)> = Vec::with_capacity(samples.len());
        for (k, v) in samples {
            let id = idx.intern(k);
            idx.series[id as usize].push(SP { x: fi as f64, ts: frames[fi].timestamp_ms, value: v });
            fs.push((id, v));
        }
        idx.frame_samples.push(fs);
    }
    idx
}

fn build_index_line(
    bytes: &[u8],
    offsets: &[usize],
    profile: &LogParseProfile,
    rules: &[CompiledRule],
) -> ParsedIndex {
    let total = offsets.len().saturating_sub(1);
    let ts_re = profile
        .timestamp_regex
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| Regex::new(s).ok());

    let mut idx = ParsedIndex::empty();
    for i in 0..total {
        let text = String::from_utf8_lossy(line_bytes(bytes, offsets, i));
        let ts = ts_re.as_ref().and_then(|re| {
            re.captures(&text).and_then(|c| {
                c.name("ts").or_else(|| c.get(1)).map(|m| m.as_str().to_string())
            })
        }).and_then(|s| parse_ts_with_formats(&s, &profile.timestamp_formats));

        let mut kv: Vec<(String, f64)> = Vec::new();
        apply_rules(&text, rules, &mut kv);
        for (k, v) in kv {
            let id = idx.intern(k);
            let x = idx.series[id as usize].len() as f64;
            idx.series[id as usize].push(SP { x, ts, value: v });
        }
    }
    idx
}

fn build_index(session: &LogSession, state: &ParseState) -> ParsedIndex {
    let bytes: &[u8] = &session.mmap;
    let rules = compile_rules(&state.profile.rules);
    if state.profile.mode == "line" {
        build_index_line(bytes, &session.line_offsets, &state.profile, &rules)
    } else {
        build_index_snapshot(bytes, &session.line_offsets, &state.frames, &rules)
    }
}

/// 锁定会话状态，惰性构建索引，在持锁期间运行闭包。
fn with_index<T>(
    session: &LogSession,
    f: impl FnOnce(&ParsedIndex, &[Frame]) -> Result<T, String>,
) -> Result<T, String> {
    let mut st = session.state.lock().map_err(|_| "会话锁失败".to_string())?;
    if st.parsed.is_none() {
        let idx = build_index(session, &st);
        st.parsed = Some(idx);
    }
    // 借用拆分：frames 与 parsed 同属 st。
    let ParseState { frames, parsed, .. } = &*st;
    let parsed = parsed.as_ref().unwrap();
    f(parsed, frames)
}

#[derive(Serialize)]
pub struct MetricSample {
    pub key: String,
    pub value: f64,
}

#[derive(Serialize)]
pub struct SeriesPoint {
    pub frame: u32,
    pub timestamp_ms: Option<i64>,
    pub value: f64,
}

#[derive(Serialize)]
pub struct LeakEntry {
    pub key: String,
    pub first: f64,
    pub last: f64,
    pub delta: f64,
    pub slope: f64,
    pub monotonic_ratio: f64,
    pub samples: usize,
    pub score: f64,
}

#[derive(Serialize)]
pub struct DiffEntry {
    pub key: String,
    pub a: f64,
    pub b: f64,
    pub delta: f64,
    pub pct: f64,
}

/// 某帧的全部指标（供“点选即出时序”的指标浏览；snapshot 模式）。
#[command]
pub fn log_frame_metrics(session_id: String, frame_index: usize) -> Result<Vec<MetricSample>, String> {
    let session = get_session(&session_id)?;
    with_index(&session, |idx, _frames| {
        let fs = idx.frame_samples.get(frame_index).ok_or_else(|| "帧越界".to_string())?;
        Ok(fs
            .iter()
            .map(|(id, v)| MetricSample { key: idx.keys[*id as usize].clone(), value: *v })
            .collect())
    })
}

/// 某指标跨全部帧/行的时间序列（C5）。
#[command]
pub fn log_metric_series(session_id: String, key: String) -> Result<Vec<SeriesPoint>, String> {
    let session = get_session(&session_id)?;
    with_index(&session, |idx, _frames| {
        let id = *idx.key_id.get(&key).ok_or_else(|| "指标不存在".to_string())?;
        Ok(idx.series[id as usize]
            .iter()
            .map(|p| SeriesPoint { frame: p.x as u32, timestamp_ms: p.ts, value: p.value })
            .collect())
    })
}

/// 计算 (斜率, 单调非降比例)。x 用序号。
fn trend(points: &[SP]) -> (f64, f64) {
    let n = points.len();
    if n < 2 {
        return (0.0, 1.0);
    }
    let nf = n as f64;
    let (mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0);
    let mut non_dec = 0usize;
    for w in 0..n {
        let x = points[w].x;
        let y = points[w].value;
        sx += x;
        sy += y;
        sxx += x * x;
        sxy += x * y;
        if w > 0 && points[w].value >= points[w - 1].value {
            non_dec += 1;
        }
    }
    let denom = nf * sxx - sx * sx;
    let slope = if denom.abs() < f64::EPSILON { 0.0 } else { (nf * sxy - sx * sy) / denom };
    (slope, non_dec as f64 / (n - 1) as f64)
}

/// 自动泄漏排名（C6）：按“持续上涨”给指标打分并排序。
#[command]
pub fn log_leak_ranking(
    session_id: String,
    min_samples: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<LeakEntry>, String> {
    let session = get_session(&session_id)?;
    let min_s = min_samples.unwrap_or(5).max(2);
    let limit = limit.unwrap_or(200);
    with_index(&session, |idx, _frames| {
        let mut entries: Vec<LeakEntry> = idx
            .series
            .par_iter()
            .enumerate()
            .filter_map(|(id, pts)| {
                if pts.len() < min_s {
                    return None;
                }
                let first = pts.first().unwrap().value;
                let last = pts.last().unwrap().value;
                let delta = last - first;
                if delta <= 0.0 {
                    return None;
                }
                let (slope, mono) = trend(pts);
                if slope <= 0.0 {
                    return None;
                }
                let score = slope * mono;
                Some(LeakEntry {
                    key: idx.keys[id].clone(),
                    first,
                    last,
                    delta,
                    slope,
                    monotonic_ratio: mono,
                    samples: pts.len(),
                    score,
                })
            })
            .collect();
        entries.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        entries.truncate(limit);
        Ok(entries)
    })
}

/// 两帧对比（C8）：变化最大的指标（snapshot 模式）。
#[command]
pub fn log_snapshot_diff(
    session_id: String,
    frame_a: usize,
    frame_b: usize,
    limit: Option<usize>,
) -> Result<Vec<DiffEntry>, String> {
    let session = get_session(&session_id)?;
    let limit = limit.unwrap_or(200);
    with_index(&session, |idx, _frames| {
        let fa = idx.frame_samples.get(frame_a).ok_or_else(|| "帧 A 越界".to_string())?;
        let fb = idx.frame_samples.get(frame_b).ok_or_else(|| "帧 B 越界".to_string())?;
        let map_a: HashMap<u32, f64> = fa.iter().copied().collect();
        let map_b: HashMap<u32, f64> = fb.iter().copied().collect();
        let mut entries: Vec<DiffEntry> = Vec::new();
        for (id, &a) in &map_a {
            if let Some(&b) = map_b.get(id) {
                let delta = b - a;
                if delta != 0.0 {
                    let pct = if a != 0.0 { delta / a * 100.0 } else { f64::INFINITY };
                    entries.push(DiffEntry { key: idx.keys[*id as usize].clone(), a, b, delta, pct });
                }
            }
        }
        entries.sort_by(|x, y| {
            y.delta.abs().partial_cmp(&x.delta.abs()).unwrap_or(std::cmp::Ordering::Equal)
        });
        entries.truncate(limit);
        Ok(entries)
    })
}

// -------- 提取规则 Profile（C7）：保存/加载关注的指标键集合 --------

#[derive(Serialize, Deserialize)]
pub struct MetricProfile {
    pub name: String,
    pub keys: Vec<String>,
}

#[command]
pub fn log_save_profile(path: String, name: String, keys: Vec<String>) -> Result<(), String> {
    let profile = MetricProfile { name, keys };
    let json = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("写入 profile 失败: {e}"))
}

#[command]
pub fn log_load_profile(path: String) -> Result<MetricProfile, String> {
    let data = std::fs::read_to_string(&path).map_err(|e| format!("读取 profile 失败: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("解析 profile 失败: {e}"))
}

// -------- 解析 Profile：内置类型 + 重解析 + 自定义规则存取 --------

#[derive(Serialize)]
pub struct ReparseResult {
    pub mode: String,
    pub frame_count: usize,
    pub metric_count: usize,
}

/// 内置解析 Profile：常见 Linux/程序日志类型的开箱即用规则，用户可复制后修改。
#[command]
pub fn log_builtin_profiles() -> Vec<LogParseProfile> {
    vec![
        default_profile(),
        LogParseProfile {
            id: "linux-ts".into(),
            name: "Linux 带时间戳日志 (syslog / 应用)".into(),
            builtin: true,
            mode: "line".into(),
            timestamp_regex: Some(
                r"(?P<ts>\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2})".into(),
            ),
            timestamp_formats: vec![
                "%Y-%m-%d %H:%M:%S".into(),
                "%Y-%m-%dT%H:%M:%S".into(),
            ],
            rules: vec![MetricRule {
                name: "示例·耗时".into(),
                pattern: r"cost=(?P<cost_ms>\d+)\s*ms".into(),
            }],
        },
        LogParseProfile {
            id: "meminfo".into(),
            name: "内存信息 (/proc/meminfo 风格, kB)".into(),
            builtin: true,
            mode: "line".into(),
            timestamp_regex: None,
            timestamp_formats: vec![],
            rules: vec![
                MetricRule { name: "".into(), pattern: r"MemFree:\s+(?P<MemFree>\d+)\s*kB".into() },
                MetricRule { name: "".into(), pattern: r"MemAvailable:\s+(?P<MemAvailable>\d+)\s*kB".into() },
                MetricRule { name: "".into(), pattern: r"Cached:\s+(?P<Cached>\d+)\s*kB".into() },
            ],
        },
        LogParseProfile {
            id: "dmesg".into(),
            name: "内核 dmesg ([时间] 前缀)".into(),
            builtin: true,
            mode: "line".into(),
            timestamp_regex: Some(r"^\[\s*(?P<ts>\d+\.\d+)\]".into()),
            timestamp_formats: vec![],
            rules: vec![MetricRule {
                name: "示例·数值".into(),
                pattern: r"=\s*(?P<value>\d+)".into(),
            }],
        },
        LogParseProfile {
            id: "custom-blank".into(),
            name: "自定义（空白，自己写规则）".into(),
            builtin: true,
            mode: "line".into(),
            timestamp_regex: None,
            timestamp_formats: vec![],
            rules: vec![],
        },
    ]
}

/// 用所选/自定义 Profile 重解析当前会话（不重读文件，复用 mmap）。
#[command]
pub fn log_reparse(session_id: String, profile: LogParseProfile) -> Result<ReparseResult, String> {
    let session = get_session(&session_id)?;
    let mut st = session.state.lock().map_err(|_| "会话锁失败".to_string())?;
    st.profile = profile;
    st.parsed = None;
    let idx = build_index(&session, &st);
    let metric_count = idx.keys.len();
    let mode = st.profile.mode.clone();
    let frame_count = st.frames.len();
    st.parsed = Some(idx);
    Ok(ReparseResult { mode, frame_count, metric_count })
}

#[command]
pub fn log_save_parse_profile(path: String, profile: LogParseProfile) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("保存解析规则失败: {e}"))
}

#[command]
pub fn log_load_parse_profile(path: String) -> Result<LogParseProfile, String> {
    let data = std::fs::read_to_string(&path).map_err(|e| format!("读取解析规则失败: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("解析规则文件损坏: {e}"))
}

// ============================================================================
// 检索与杂乱日志分析（E1/D1/D2/D3）
// 面向自由文本日志（kernel/dmesg/软件日志）：快速检索、模板聚类、格式识别、
// 自定义正则抽取。都基于 batch1 的行索引，按需扫描 mmap。
// ============================================================================

#[derive(Serialize)]
pub struct SearchHit {
    pub line: usize,
    pub frame: Option<usize>,
    pub text: String,
}

/// 给定全局行号，二分定位它属于哪个快照帧（帧按 line_start 升序）。
fn frame_of_line(frames: &[Frame], line: usize) -> Option<usize> {
    if frames.is_empty() {
        return None;
    }
    let mut lo = 0usize;
    let mut hi = frames.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if frames[mid].line_start <= line {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo == 0 {
        None
    } else {
        Some(frames[lo - 1].index)
    }
}

/// E1：全文快速检索。literal 或正则、可忽略大小写，返回命中行（截断到 limit）。
#[command]
pub fn log_search(
    session_id: String,
    query: String,
    is_regex: bool,
    ignore_case: bool,
    limit: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let session = get_session(&session_id)?;
    let limit = limit.unwrap_or(1000);
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let pattern = if is_regex { query.clone() } else { regex::escape(&query) };
    let re = regex::RegexBuilder::new(&pattern)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|e| format!("正则编译失败: {e}"))?;

    let bytes: &[u8] = &session.mmap;
    let total = session.line_offsets.len().saturating_sub(1);
    let frames = {
        let st = session.state.lock().map_err(|_| "会话锁失败".to_string())?;
        st.frames.clone()
    };
    let mut hits = Vec::new();
    for i in 0..total {
        let text = String::from_utf8_lossy(line_bytes(bytes, &session.line_offsets, i));
        if re.is_match(&text) {
            hits.push(SearchHit {
                line: i,
                frame: frame_of_line(&frames, i),
                text: text.into_owned(),
            });
            if hits.len() >= limit {
                break;
            }
        }
    }
    Ok(hits)
}

#[derive(Serialize)]
pub struct TemplateEntry {
    pub template: String,
    pub count: usize,
    pub sample: String,
}

/// 把一行归一成模板：含数字的 token 替换为 <*>，从而把只有变量不同的日志行归并。
fn linify_template(line: &str) -> String {
    line.split_whitespace()
        .map(|tok| {
            if tok.chars().any(|c| c.is_ascii_digit()) {
                "<*>"
            } else {
                tok
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// D1：Drain-lite 模板挖掘。把百万行归并成几十个模板 + 占比，一屏看清日志构成。
#[command]
pub fn log_cluster_templates(
    session_id: String,
    limit: Option<usize>,
) -> Result<Vec<TemplateEntry>, String> {
    let session = get_session(&session_id)?;
    let limit = limit.unwrap_or(200);
    let bytes: &[u8] = &session.mmap;
    let total = session.line_offsets.len().saturating_sub(1);

    let mut map: HashMap<String, (usize, String)> = HashMap::new();
    for i in 0..total {
        let text = String::from_utf8_lossy(line_bytes(bytes, &session.line_offsets, i));
        let t = text.trim();
        if t.is_empty() {
            continue;
        }
        let tpl = linify_template(t);
        let e = map.entry(tpl).or_insert_with(|| (0, t.to_string()));
        e.0 += 1;
    }
    let mut entries: Vec<TemplateEntry> = map
        .into_iter()
        .map(|(template, (count, sample))| TemplateEntry { template, count, sample })
        .collect();
    entries.sort_by(|a, b| b.count.cmp(&a.count));
    entries.truncate(limit);
    Ok(entries)
}

static RE_SYSLOG: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[A-Z][a-z]{2}\s+\d+\s+\d{2}:\d{2}:\d{2}\s+\S+\s+\S+").unwrap()
});
static RE_TS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}|\d{2}:\d{2}:\d{2}").unwrap()
});

#[derive(Serialize)]
pub struct FormatReport {
    pub sample_size: usize,
    pub json_ratio: f64,
    pub syslog_ratio: f64,
    pub timestamp_ratio: f64,
    pub guess: String,
}

/// D2：格式自动识别。抽样若干非空行，判断 JSON 行 / syslog / 带时间戳的比例。
#[command]
pub fn log_detect_format(session_id: String, sample: Option<usize>) -> Result<FormatReport, String> {
    let session = get_session(&session_id)?;
    let sample = sample.unwrap_or(500);
    let bytes: &[u8] = &session.mmap;
    let total = session.line_offsets.len().saturating_sub(1);

    let (mut n, mut json, mut syslog, mut ts) = (0usize, 0usize, 0usize, 0usize);
    for i in 0..total {
        if n >= sample {
            break;
        }
        let text = String::from_utf8_lossy(line_bytes(bytes, &session.line_offsets, i));
        let t = text.trim();
        if t.is_empty() {
            continue;
        }
        n += 1;
        if (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('[') && t.ends_with(']')) {
            json += 1;
        }
        if RE_SYSLOG.is_match(t) {
            syslog += 1;
        }
        if RE_TS.is_match(t) {
            ts += 1;
        }
    }
    let ratio = |x: usize| if n == 0 { 0.0 } else { x as f64 / n as f64 };
    let (jr, sr, tr) = (ratio(json), ratio(syslog), ratio(ts));
    let guess = if jr > 0.6 {
        "JSON 行日志"
    } else if sr > 0.6 {
        "syslog"
    } else if tr > 0.6 {
        "带时间戳的文本日志"
    } else {
        "自由文本"
    }
    .to_string();
    Ok(FormatReport { sample_size: n, json_ratio: jr, syslog_ratio: sr, timestamp_ratio: tr, guess })
}

#[derive(Serialize)]
pub struct ExtractResult {
    pub columns: Vec<String>,
    pub rows: Vec<ExtractRow>,
}

#[derive(Serialize)]
pub struct ExtractRow {
    pub line: usize,
    /// 与 columns 对齐的取值（未命中的组为空串）。
    pub values: Vec<String>,
}

/// D3：自定义正则抽取。用带命名捕获组的正则把非结构化行抽成结构化表。
/// 例：`(?P<ts>\d{2}:\d{2}:\d{2}).*cost=(?P<ms>\d+)ms`
#[command]
pub fn log_extract_regex(
    session_id: String,
    pattern: String,
    limit: Option<usize>,
) -> Result<ExtractResult, String> {
    let session = get_session(&session_id)?;
    let limit = limit.unwrap_or(5000);
    let re = Regex::new(&pattern).map_err(|e| format!("正则编译失败: {e}"))?;
    // 命名组作为列；无命名组则用整体匹配 match_0。
    let names: Vec<String> = re
        .capture_names()
        .flatten()
        .map(|s| s.to_string())
        .collect();
    let columns = if names.is_empty() { vec!["match".to_string()] } else { names.clone() };

    let bytes: &[u8] = &session.mmap;
    let total = session.line_offsets.len().saturating_sub(1);
    let mut rows = Vec::new();
    for i in 0..total {
        let text = String::from_utf8_lossy(line_bytes(bytes, &session.line_offsets, i));
        if let Some(caps) = re.captures(&text) {
            let values: Vec<String> = if names.is_empty() {
                vec![caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default()]
            } else {
                names
                    .iter()
                    .map(|n| caps.name(n).map(|m| m.as_str().to_string()).unwrap_or_default())
                    .collect()
            };
            rows.push(ExtractRow { line: i, values });
            if rows.len() >= limit {
                break;
            }
        }
    }
    Ok(ExtractResult { columns, rows })
}

#[derive(Serialize)]
pub struct AggResult {
    /// 聚合结果以 CSV 返回（首行表头），前端解析成表格。
    pub csv: String,
    pub group_count: usize,
}

/// E3：列式聚合（polars）。用 parse 正则的命名组抽出 group/value 两列，按 group 聚合。
/// agg ∈ count|sum|avg|min|max。可选 filter 正则先过滤行。
#[command]
pub fn log_aggregate(
    session_id: String,
    parse_pattern: String,
    group_field: String,
    value_field: String,
    agg: String,
    filter: Option<String>,
) -> Result<AggResult, String> {
    use polars::prelude::*;

    let session = get_session(&session_id)?;
    let parse_re = Regex::new(&parse_pattern).map_err(|e| format!("parse 正则失败: {e}"))?;
    let filt_re = match filter {
        Some(f) if !f.trim().is_empty() => {
            Some(Regex::new(&f).map_err(|e| format!("filter 正则失败: {e}"))?)
        }
        _ => None,
    };

    let bytes: &[u8] = &session.mmap;
    let total = session.line_offsets.len().saturating_sub(1);
    let mut groups: Vec<String> = Vec::new();
    let mut values: Vec<f64> = Vec::new();
    for i in 0..total {
        let text = String::from_utf8_lossy(line_bytes(bytes, &session.line_offsets, i));
        if let Some(fr) = &filt_re {
            if !fr.is_match(&text) {
                continue;
            }
        }
        if let Some(caps) = parse_re.captures(&text) {
            let g = caps.name(&group_field).map(|m| m.as_str().to_string());
            let v = caps.name(&value_field).and_then(|m| m.as_str().parse::<f64>().ok());
            if let (Some(g), Some(v)) = (g, v) {
                groups.push(g);
                values.push(v);
            }
        }
    }
    if groups.is_empty() {
        return Ok(AggResult { csv: String::new(), group_count: 0 });
    }

    let row_count = groups.len();
    let df = DataFrame::new(
        row_count,
        vec![
            Column::new("group".into(), groups),
            Column::new("value".into(), values),
        ],
    )
    .map_err(|e| format!("构建 DataFrame 失败: {e}"))?;

    let agg_expr = match agg.as_str() {
        "sum" => col("value").sum().alias("agg"),
        "avg" | "mean" => col("value").mean().alias("agg"),
        "min" => col("value").min().alias("agg"),
        "max" => col("value").max().alias("agg"),
        _ => col("value").count().alias("agg"),
    };
    let mut out = df
        .lazy()
        .group_by([col("group")])
        .agg([agg_expr, len().alias("count")])
        .collect()
        .map_err(|e| format!("聚合失败: {e}"))?;
    let group_count = out.height();

    let mut buf: Vec<u8> = Vec::new();
    CsvWriter::new(&mut buf)
        .finish(&mut out)
        .map_err(|e| format!("导出 CSV 失败: {e}"))?;
    Ok(AggResult { csv: String::from_utf8_lossy(&buf).into_owned(), group_count })
}

