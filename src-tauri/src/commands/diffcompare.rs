//! 通用差异对比引擎：任意两个文件、任意两个目录，和 git 仓库无关。
//!
//! 面向“巨大文件 + 巨大目录”的四条设计约束：
//! 1. 先比元数据再比内容。大小不同直接判定不同；大小相同先算 blake3，
//!    相同就完全不进 diff 算法——几百 MB 的同内容文件是毫秒级返回。
//! 2. 不拿字符串做比较。每行先 intern 成 u32 编号，diff 跑在整数切片上，
//!    比较代价与行长无关。
//! 3. 掐头去尾。巨大文件通常只在局部有差异，先剥掉公共前缀/后缀，
//!    真正送进 diff 算法的只有中间那一小段。
//! 4. 结果留在 Rust 侧。目录对比的条目按页取，不把百万条记录塞进一个 JSON；
//!    扫描进度用事件流式回传，可随时取消。

use blake3::Hasher;
use encoding_rs::{GB18030, UTF_8};
use ignore::overrides::OverrideBuilder;
use ignore::{WalkBuilder, WalkState};
use lru::LruCache;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use similar::{capture_diff_slices_deadline, Algorithm, DiffOp};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

/// 单侧允许全文对比的字节上限。超过就只给统计和 blake3 判定，不做行级 diff。
const DEFAULT_MAX_BYTES: u64 = 128 * 1024 * 1024;
/// 掐头去尾之后仍然过大的输入不再硬算：Myers/Patience 在这个量级会失控。
const MAX_DIFF_LINES: usize = 1_000_000;
/// 默认上下文行数，和 git 一致。
const DEFAULT_CONTEXT: usize = 3;
/// 行内高亮只对成对且行数不多的改动计算，避免大改动块上做无意义的细粒度活。
const INLINE_PAIR_LIMIT: usize = 400;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct DiffOptions {
    /// "patience"(默认) | "myers" | "lcs"
    pub algorithm: Option<String>,
    pub context_lines: Option<usize>,
    /// 忽略全部空白（对应 git diff -w）。
    pub ignore_whitespace: bool,
    /// 只忽略行尾空白。
    pub ignore_trailing_whitespace: bool,
    pub ignore_case: bool,
    /// 忽略空行的增删。
    pub ignore_blank_lines: bool,
    pub max_bytes: Option<u64>,
    /// 只要统计不要 hunk，用于列表里显示“改了多少行”。
    pub stats_only: bool,
    /// 关掉行内字符级高亮（默认开）。
    pub no_inline: bool,
}

impl DiffOptions {
    fn algorithm(&self) -> Algorithm {
        match self.algorithm.as_deref().unwrap_or("patience") {
            "myers" => Algorithm::Myers,
            "lcs" => Algorithm::Lcs,
            _ => Algorithm::Patience,
        }
    }

    fn context(&self) -> usize {
        self.context_lines.unwrap_or(DEFAULT_CONTEXT).min(2000)
    }

    fn max_bytes(&self) -> u64 {
        self.max_bytes.unwrap_or(DEFAULT_MAX_BYTES)
    }

    fn normalizes(&self) -> bool {
        self.ignore_whitespace || self.ignore_trailing_whitespace || self.ignore_case
    }
}

/// 一行差异。tag: "ctx" 上下文 / "del" 左侧独有 / "add" 右侧独有。
#[derive(Debug, Clone, Serialize)]
pub struct DiffLine {
    pub tag: &'static str,
    pub left_no: Option<u32>,
    pub right_no: Option<u32>,
    pub text: String,
    /// 行内改动的字符区间 [start, end)，按 UTF-16 code unit 计（前端直接可用）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline: Option<Vec<[u32; 2]>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffHunk {
    pub index: u32,
    pub left_start: u32,
    pub left_count: u32,
    pub right_start: u32,
    pub right_count: u32,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiffStats {
    pub added: u32,
    pub removed: u32,
    pub hunks: u32,
    pub left_lines: u32,
    pub right_lines: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileMeta {
    pub path: String,
    pub exists: bool,
    pub size: u64,
    pub mtime_ms: Option<i64>,
    pub encoding: String,
    pub line_ending: &'static str,
    pub line_count: u32,
    pub binary: bool,
    pub blake3: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffFileResult {
    /// "text" 正常行级差异 / "identical" 内容完全相同 / "binary" 二进制
    /// / "too_large" 超过字节上限 / "too_divergent" 差异过大只给统计
    pub mode: &'static str,
    pub identical: bool,
    pub left: FileMeta,
    pub right: FileMeta,
    pub stats: DiffStats,
    pub hunks: Vec<DiffHunk>,
    pub hunks_truncated: bool,
    /// 仅行尾符不同（内容逐行一致）时置位，避免整文件被判成全改。
    pub line_ending_only: bool,
    pub message: Option<String>,
    pub elapsed_ms: u64,
}

/// 已解码的一侧文本：text 是整块内容，lines 是每行在 text 中的字节区间。
/// 不为每行单独分配 String，百万行也只有一次大分配加一个索引表。
struct LoadedText {
    text: String,
    lines: Vec<(u32, u32)>,
    encoding: String,
    line_ending: &'static str,
}

impl LoadedText {
    fn line(&self, idx: usize) -> &str {
        match self.lines.get(idx) {
            Some((s, e)) => &self.text[*s as usize..*e as usize],
            None => "",
        }
    }

    fn len(&self) -> usize {
        self.lines.len()
    }
}

fn mtime_ms(meta: &std::fs::Metadata) -> Option<i64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
}

fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8192).any(|b| *b == 0)
}

/// UTF-8 优先，失败退 GB18030（中文笔记库里 GBK 系编码很常见）。
fn decode_bytes(bytes: &[u8]) -> (String, String) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (cow, _, _) = UTF_8.decode(&bytes[3..]);
        return (cow.into_owned(), "utf-8-bom".to_string());
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_string(), "utf-8".to_string()),
        Err(_) => {
            let (cow, enc, had_errors) = GB18030.decode(bytes);
            let mut label = enc.name().to_lowercase();
            if had_errors {
                label.push_str("(含非法字节)");
            }
            (cow.into_owned(), label)
        }
    }
}

/// 拆行：统一吃掉行尾的 \r，让“只有行尾符不同”不至于被判成整文件全改，
/// 同时把左右两侧真实的行尾风格单独记下来汇报。
fn split_lines(text: &str) -> (Vec<(u32, u32)>, &'static str) {
    let bytes = text.as_bytes();
    let mut lines: Vec<(u32, u32)> = Vec::with_capacity(bytes.len() / 40 + 8);
    let (mut lf, mut crlf) = (0usize, 0usize);
    let mut start = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        if *b != b'\n' {
            continue;
        }
        let mut end = i;
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
            crlf += 1;
        } else {
            lf += 1;
        }
        lines.push((start as u32, end as u32));
        start = i + 1;
    }
    if start < bytes.len() {
        lines.push((start as u32, bytes.len() as u32));
    }
    let ending = match (crlf > 0, lf > 0) {
        (true, true) => "mixed",
        (true, false) => "crlf",
        (false, true) => "lf",
        _ => "none",
    };
    (lines, ending)
}

/// 流式算 blake3，不把大文件整块读进内存。
fn hash_file(path: &Path, cancel: Option<&AtomicBool>) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Hasher::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        if cancel.map_or(false, |c| c.load(Ordering::Relaxed)) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "已取消",
            ));
        }
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn empty_meta(path: &str) -> FileMeta {
    FileMeta {
        path: path.to_string(),
        exists: false,
        size: 0,
        mtime_ms: None,
        encoding: String::new(),
        line_ending: "none",
        line_count: 0,
        binary: false,
        blake3: None,
    }
}

fn text_to_loaded(text: String, encoding: String) -> LoadedText {
    let (lines, line_ending) = split_lines(&text);
    LoadedText {
        text,
        lines,
        encoding,
        line_ending,
    }
}

fn normalize_line<'a>(s: &'a str, opts: &DiffOptions) -> Cow<'a, str> {
    let mut out: Cow<'a, str> = if opts.ignore_whitespace {
        Cow::Owned(s.chars().filter(|c| !c.is_whitespace()).collect())
    } else if opts.ignore_trailing_whitespace {
        Cow::Borrowed(s.trim_end())
    } else {
        Cow::Borrowed(s)
    };
    if opts.ignore_case {
        out = Cow::Owned(out.to_lowercase());
    }
    out
}

/// 把两侧的行映射成整数编号：后面的 diff 只比较 u32，与行长无关。
fn intern<'x>(left: &'x [Cow<'x, str>], right: &'x [Cow<'x, str>]) -> (Vec<u32>, Vec<u32>) {
    let mut map: HashMap<&'x str, u32> = HashMap::with_capacity(left.len() + right.len());
    let mut next: u32 = 0;
    let take = |src: &'x [Cow<'x, str>], map: &mut HashMap<&'x str, u32>, next: &mut u32| {
        let mut ids = Vec::with_capacity(src.len());
        for item in src {
            let id = *map.entry(item.as_ref()).or_insert_with(|| {
                let v = *next;
                *next += 1;
                v
            });
            ids.push(id);
        }
        ids
    };
    let l = take(left, &mut map, &mut next);
    let r = take(right, &mut map, &mut next);
    (l, r)
}

fn common_prefix(a: &[u32], b: &[u32]) -> usize {
    let n = a.len().min(b.len());
    let mut i = 0;
    while i < n && a[i] == b[i] {
        i += 1;
    }
    i
}

fn common_suffix(a: &[u32], b: &[u32], prefix: usize) -> usize {
    let max = (a.len() - prefix).min(b.len() - prefix);
    let mut i = 0;
    while i < max && a[a.len() - 1 - i] == b[b.len() - 1 - i] {
        i += 1;
    }
    i
}

/// 行内差异：只剥离字符级公共前后缀，不做词级 LCS。
/// 对“改了一个变量名”这类真实改动效果已经足够，代价是 O(行长)。
/// 返回区间按 UTF-16 code unit 计，前端拿到可直接 slice。
fn inline_ranges(left: &str, right: &str) -> Option<([u32; 2], [u32; 2])> {
    if left == right || left.is_empty() || right.is_empty() {
        return None;
    }
    let lc: Vec<char> = left.chars().collect();
    let rc: Vec<char> = right.chars().collect();
    let mut p = 0usize;
    while p < lc.len() && p < rc.len() && lc[p] == rc[p] {
        p += 1;
    }
    let mut s = 0usize;
    while s < lc.len() - p && s < rc.len() - p && lc[lc.len() - 1 - s] == rc[rc.len() - 1 - s] {
        s += 1;
    }
    let u16len = |cs: &[char]| -> u32 { cs.iter().map(|c| c.len_utf16() as u32).sum() };
    let lp = u16len(&lc[..p]);
    let le = u16len(&lc[..lc.len() - s]);
    let rp = u16len(&rc[..p]);
    let re = u16len(&rc[..rc.len() - s]);
    Some(([lp, le], [rp, re]))
}

/// 一处改动在两侧的绝对行区间（半开区间，0 基）。
#[derive(Debug, Clone, Copy)]
struct Change {
    l0: usize,
    l1: usize,
    r0: usize,
    r1: usize,
}

/// diff 结果跑在掐头之后的切片上，这里加回 prefix 偏移换成整文件行号。
fn ops_to_changes(ops: &[DiffOp], offset: usize) -> Vec<Change> {
    let mut out = Vec::with_capacity(ops.len());
    for op in ops {
        let c = match *op {
            DiffOp::Equal { .. } => continue,
            DiffOp::Delete {
                old_index,
                old_len,
                new_index,
            } => Change {
                l0: old_index + offset,
                l1: old_index + old_len + offset,
                r0: new_index + offset,
                r1: new_index + offset,
            },
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => Change {
                l0: old_index + offset,
                l1: old_index + offset,
                r0: new_index + offset,
                r1: new_index + new_len + offset,
            },
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => Change {
                l0: old_index + offset,
                l1: old_index + old_len + offset,
                r0: new_index + offset,
                r1: new_index + new_len + offset,
            },
        };
        out.push(c);
    }
    out
}

/// 单次返回的 hunk 数量与总行数上限。统计始终是全量准确的，
/// 超出的部分只是不再渲染，并置 hunks_truncated 让前端提示。
const MAX_HUNKS: usize = 20_000;
const MAX_HUNK_LINES: usize = 400_000;
/// diff 算法的软时限：超时后 similar 会退化返回一个合法但非最优的结果，
/// 不会挂死，也不会报错。
const DIFF_DEADLINE_MS: u64 = 5_000;

fn all_blank(text: &LoadedText, from: usize, to: usize) -> bool {
    (from..to).all(|i| text.line(i).trim().is_empty())
}

/// ignore_blank_lines：整块改动两侧都只有空行时丢弃这处改动。
fn filter_blank_changes(
    changes: Vec<Change>,
    left: &LoadedText,
    right: &LoadedText,
) -> Vec<Change> {
    changes
        .into_iter()
        .filter(|c| !(all_blank(left, c.l0, c.l1) && all_blank(right, c.r0, c.r1)))
        .collect()
}

/// 两处改动之间的间隔不超过 2*ctx 时，它们的上下文会连成一片，合成一个 hunk。
fn merge_changes(changes: Vec<Change>, ctx: usize) -> Vec<Change> {
    let gap = ctx.saturating_mul(2);
    let mut out: Vec<Change> = Vec::with_capacity(changes.len());
    for c in changes {
        match out.last_mut() {
            Some(prev) if c.l0 <= prev.l1 + gap && c.r0 <= prev.r1 + gap => {
                prev.l1 = prev.l1.max(c.l1);
                prev.r1 = prev.r1.max(c.r1);
            }
            _ => out.push(c),
        }
    }
    out
}

/// 把合并好的改动列表渲染成带上下文的 hunk，并给出全量统计。
fn build_hunks(
    left: &LoadedText,
    right: &LoadedText,
    changes: &[Change],
    opts: &DiffOptions,
) -> (Vec<DiffHunk>, DiffStats, bool) {
    let ctx = opts.context();
    let mut stats = DiffStats {
        left_lines: left.len() as u32,
        right_lines: right.len() as u32,
        hunks: changes.len() as u32,
        ..Default::default()
    };
    for c in changes {
        stats.removed += (c.l1 - c.l0) as u32;
        stats.added += (c.r1 - c.r0) as u32;
    }
    if opts.stats_only {
        return (Vec::new(), stats, false);
    }

    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut emitted = 0usize;
    let mut truncated = false;
    for (idx, c) in changes.iter().enumerate() {
        if hunks.len() >= MAX_HUNKS || emitted >= MAX_HUNK_LINES {
            truncated = true;
            break;
        }
        // 改动贴着文件头/尾时，上下文要按两侧都不越界的那个来取，否则左右行号会错位。
        let head = ctx.min(c.l0).min(c.r0);
        let tail = ctx
            .min(left.len().saturating_sub(c.l1))
            .min(right.len().saturating_sub(c.r1));
        let (ls, rs) = (c.l0 - head, c.r0 - head);
        let (le, re) = (c.l1 + tail, c.r1 + tail);
        let mut lines: Vec<DiffLine> = Vec::with_capacity((le - ls) + (re - rs));
        for k in 0..head {
            lines.push(DiffLine {
                tag: "ctx",
                left_no: Some((ls + k + 1) as u32),
                right_no: Some((rs + k + 1) as u32),
                text: left.line(ls + k).to_string(),
                inline: None,
            });
        }
        let (lcount, rcount) = (c.l1 - c.l0, c.r1 - c.r0);
        // 只有等行数的成对改动才算行内高亮：行数不等时配对本身就是猜的。
        let paired = !opts.no_inline
            && lcount == rcount
            && lcount > 0
            && lcount <= INLINE_PAIR_LIMIT;
        let mut pairs: Vec<Option<([u32; 2], [u32; 2])>> = Vec::new();
        if paired {
            pairs = (0..lcount)
                .map(|k| inline_ranges(left.line(c.l0 + k), right.line(c.r0 + k)))
                .collect();
        }
        for k in 0..lcount {
            lines.push(DiffLine {
                tag: "del",
                left_no: Some((c.l0 + k + 1) as u32),
                right_no: None,
                text: left.line(c.l0 + k).to_string(),
                inline: pairs.get(k).and_then(|p| p.map(|(l, _)| vec![l])),
            });
        }
        for k in 0..rcount {
            lines.push(DiffLine {
                tag: "add",
                left_no: None,
                right_no: Some((c.r0 + k + 1) as u32),
                text: right.line(c.r0 + k).to_string(),
                inline: pairs.get(k).and_then(|p| p.map(|(_, r)| vec![r])),
            });
        }
        for k in 0..tail {
            lines.push(DiffLine {
                tag: "ctx",
                left_no: Some((c.l1 + k + 1) as u32),
                right_no: Some((c.r1 + k + 1) as u32),
                text: left.line(c.l1 + k).to_string(),
                inline: None,
            });
        }
        emitted += lines.len();
        let (lc, rc) = ((le - ls) as u32, (re - rs) as u32);
        let (lstart, rstart) = (
            if lc == 0 { 0 } else { (ls + 1) as u32 },
            if rc == 0 { 0 } else { (rs + 1) as u32 },
        );
        hunks.push(DiffHunk {
            index: idx as u32,
            left_start: lstart,
            left_count: lc,
            right_start: rstart,
            right_count: rc,
            header: format!("@@ -{},{} +{},{} @@", lstart, lc, rstart, rc),
            lines,
        });
    }
    (hunks, stats, truncated)
}

struct DiffBody {
    mode: &'static str,
    identical: bool,
    stats: DiffStats,
    hunks: Vec<DiffHunk>,
    truncated: bool,
    message: Option<String>,
}

/// 已解码的两侧文本 → 差异结果。这里是整个引擎的热路径：
/// 归一化 → intern 成 u32 → 掐头去尾 → 只对中间那段跑 diff。
fn diff_loaded(left: &LoadedText, right: &LoadedText, opts: &DiffOptions) -> DiffBody {
    let ln: Vec<Cow<'_, str>> = (0..left.len())
        .map(|i| normalize_line(left.line(i), opts))
        .collect();
    let rn: Vec<Cow<'_, str>> = (0..right.len())
        .map(|i| normalize_line(right.line(i), opts))
        .collect();
    let (lids, rids) = intern(&ln, &rn);
    let prefix = common_prefix(&lids, &rids);
    let suffix = common_suffix(&lids, &rids, prefix);
    let lslice = &lids[prefix..lids.len() - suffix];
    let rslice = &rids[prefix..rids.len() - suffix];
    let base = DiffStats {
        left_lines: left.len() as u32,
        right_lines: right.len() as u32,
        ..Default::default()
    };
    let same = DiffBody {
        mode: "identical",
        identical: true,
        stats: base.clone(),
        hunks: Vec::new(),
        truncated: false,
        message: None,
    };
    if lslice.is_empty() && rslice.is_empty() {
        return same;
    }
    if lslice.len().max(rslice.len()) > MAX_DIFF_LINES {
        return DiffBody {
            mode: "too_divergent",
            identical: false,
            stats: DiffStats {
                added: rslice.len() as u32,
                removed: lslice.len() as u32,
                hunks: 1,
                ..base
            },
            hunks: Vec::new(),
            truncated: true,
            message: Some(format!(
                "差异区域 {} / {} 行，超过 {} 行上限，只返回统计",
                lslice.len(),
                rslice.len(),
                MAX_DIFF_LINES
            )),
        };
    }
    let deadline = Instant::now().checked_add(std::time::Duration::from_millis(DIFF_DEADLINE_MS));
    let ops = capture_diff_slices_deadline(opts.algorithm(), lslice, rslice, deadline);
    let mut changes = ops_to_changes(&ops, prefix);
    if opts.ignore_blank_lines {
        changes = filter_blank_changes(changes, left, right);
    }
    let changes = merge_changes(changes, opts.context());
    if changes.is_empty() {
        return same;
    }
    let (hunks, stats, truncated) = build_hunks(left, right, &changes, opts);
    DiffBody {
        mode: "text",
        identical: false,
        stats,
        hunks,
        truncated,
        message: None,
    }
}

/// blake3 判定“完全相同”后，多大以内还愿意再扫一遍只为数行数。
const IDENTICAL_COUNT_LIMIT: u64 = 16 * 1024 * 1024;

struct Stat {
    exists: bool,
    size: u64,
    mtime_ms: Option<i64>,
}

fn stat_of(path: &Path) -> Result<Stat, String> {
    match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => Err(format!("{} 是目录，请用目录对比", path.display())),
        Ok(m) => Ok(Stat {
            exists: true,
            size: m.len(),
            mtime_ms: mtime_ms(&m),
        }),
        Err(_) => Ok(Stat {
            exists: false,
            size: 0,
            mtime_ms: None,
        }),
    }
}

/// 只数行数不建索引：给“内容相同”的大文件用，省掉整块解码。
fn count_lines(path: &Path) -> std::io::Result<u32> {
    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 256 * 1024];
    let (mut lines, mut last) = (0u32, 0u8);
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        lines += buf[..n].iter().filter(|b| **b == b'\n').count() as u32;
        last = buf[n - 1];
    }
    if last != 0 && last != b'\n' {
        lines += 1; // 末行没有换行符
    }
    Ok(lines)
}

fn read_head(path: &Path, n: usize) -> Result<Vec<u8>, String> {
    let mut f =
        std::fs::File::open(path).map_err(|e| format!("打开 {} 失败：{}", path.display(), e))?;
    let mut buf = vec![0u8; n];
    let read = f
        .read(&mut buf)
        .map_err(|e| format!("读取 {} 失败：{}", path.display(), e))?;
    buf.truncate(read);
    Ok(buf)
}

/// 两个文件的差异。顺序刻意如此：元数据 → blake3 → 二进制探测 → 才解码。
/// 越贵的步骤越靠后，绝大多数调用在前两步就能返回。
fn diff_two_files(
    left_path: &str,
    right_path: &str,
    opts: &DiffOptions,
) -> Result<DiffFileResult, String> {
    let t0 = Instant::now();
    let (lp, rp) = (Path::new(left_path), Path::new(right_path));
    let (ls, rs) = (stat_of(lp)?, stat_of(rp)?);
    if !ls.exists && !rs.exists {
        return Err("两侧路径都不存在".to_string());
    }
    let mut lm = empty_meta(left_path);
    let mut rm = empty_meta(right_path);
    lm.exists = ls.exists;
    lm.size = ls.size;
    lm.mtime_ms = ls.mtime_ms;
    rm.exists = rs.exists;
    rm.size = rs.size;
    rm.mtime_ms = rs.mtime_ms;

    // 大小不同必然内容不同，没必要算哈希；大小相同才并行算一次 blake3。
    let mut hashes_equal = false;
    if ls.exists && rs.exists && ls.size == rs.size {
        let (lh, rh) = rayon::join(|| hash_file(lp, None), || hash_file(rp, None));
        let lh = lh.map_err(|e| format!("读取 {} 失败：{}", left_path, e))?;
        let rh = rh.map_err(|e| format!("读取 {} 失败：{}", right_path, e))?;
        hashes_equal = lh == rh;
        lm.blake3 = Some(lh);
        rm.blake3 = Some(rh);
    }
    if hashes_equal {
        if ls.size <= IDENTICAL_COUNT_LIMIT {
            let n = count_lines(lp).unwrap_or(0);
            lm.line_count = n;
            rm.line_count = n;
        }
        let (lc, rc) = (lm.line_count, rm.line_count);
        return Ok(DiffFileResult {
            mode: "identical",
            identical: true,
            left: lm,
            right: rm,
            stats: DiffStats {
                left_lines: lc,
                right_lines: rc,
                ..Default::default()
            },
            hunks: Vec::new(),
            hunks_truncated: false,
            line_ending_only: false,
            message: Some("内容完全相同（blake3 一致）".to_string()),
            elapsed_ms: t0.elapsed().as_millis() as u64,
        });
    }
    let max = opts.max_bytes();
    if ls.size > max || rs.size > max {
        if ls.exists {
            lm.binary = looks_binary(&read_head(lp, 8192)?);
        }
        if rs.exists {
            rm.binary = looks_binary(&read_head(rp, 8192)?);
        }
        return Ok(DiffFileResult {
            mode: "too_large",
            identical: false,
            left: lm,
            right: rm,
            stats: DiffStats::default(),
            hunks: Vec::new(),
            hunks_truncated: true,
            line_ending_only: false,
            message: Some(format!(
                "单侧超过 {} MB 上限，只做哈希判定，未做行级对比",
                max / (1024 * 1024)
            )),
            elapsed_ms: t0.elapsed().as_millis() as u64,
        });
    }

    let lbytes = if ls.exists {
        std::fs::read(lp).map_err(|e| format!("读取 {} 失败：{}", left_path, e))?
    } else {
        Vec::new()
    };
    let rbytes = if rs.exists {
        std::fs::read(rp).map_err(|e| format!("读取 {} 失败：{}", right_path, e))?
    } else {
        Vec::new()
    };
    lm.binary = looks_binary(&lbytes);
    rm.binary = looks_binary(&rbytes);
    if lm.binary || rm.binary {
        return Ok(DiffFileResult {
            mode: "binary",
            identical: false,
            left: lm,
            right: rm,
            stats: DiffStats::default(),
            hunks: Vec::new(),
            hunks_truncated: false,
            line_ending_only: false,
            message: Some("二进制文件，只比较哈希与大小".to_string()),
            elapsed_ms: t0.elapsed().as_millis() as u64,
        });
    }
    let (ltext, lenc) = decode_bytes(&lbytes);
    let (rtext, renc) = decode_bytes(&rbytes);
    drop(lbytes);
    drop(rbytes);
    let left = text_to_loaded(ltext, lenc);
    let right = text_to_loaded(rtext, renc);
    lm.encoding = left.encoding.clone();
    lm.line_ending = left.line_ending;
    lm.line_count = left.len() as u32;
    rm.encoding = right.encoding.clone();
    rm.line_ending = right.line_ending;
    rm.line_count = right.len() as u32;

    let body = diff_loaded(&left, &right, opts);
    // 能走到这里字节一定不同（相同的在哈希那步就返回了），
    // 所以行级判定为一致时，差别只可能在行尾符、末尾换行或 BOM 上。
    let line_ending_only = body.identical && !opts.normalizes();
    let message = if body.identical {
        Some(if opts.normalizes() {
            "在当前忽略规则下内容一致".to_string()
        } else {
            "仅行尾符、末尾换行或 BOM 不同".to_string()
        })
    } else {
        body.message
    };
    Ok(DiffFileResult {
        mode: body.mode,
        identical: body.identical,
        left: lm,
        right: rm,
        stats: body.stats,
        hunks: body.hunks,
        hunks_truncated: body.truncated,
        line_ending_only,
        message,
        elapsed_ms: t0.elapsed().as_millis() as u64,
    })
}

fn meta_from_loaded(label: &str, t: &LoadedText) -> FileMeta {
    FileMeta {
        path: label.to_string(),
        exists: true,
        size: t.text.len() as u64,
        mtime_ms: None,
        encoding: t.encoding.clone(),
        line_ending: t.line_ending,
        line_count: t.len() as u32,
        binary: false,
        blake3: None,
    }
}

fn result_from_body(
    left: FileMeta,
    right: FileMeta,
    body: DiffBody,
    t0: Instant,
) -> DiffFileResult {
    DiffFileResult {
        mode: body.mode,
        identical: body.identical,
        left,
        right,
        stats: body.stats,
        hunks: body.hunks,
        hunks_truncated: body.truncated,
        line_ending_only: false,
        message: body.message,
        elapsed_ms: t0.elapsed().as_millis() as u64,
    }
}

/// 磁盘上的两个文件对比。
#[tauri::command]
pub async fn diff_files(
    left_path: String,
    right_path: String,
    options: Option<DiffOptions>,
) -> Result<DiffFileResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        diff_two_files(&left_path, &right_path, &options.unwrap_or_default())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 任意两段文本对比：未落盘的编辑器缓冲、剪贴板、选区都走这里。
#[tauri::command]
pub async fn diff_texts(
    left_text: String,
    right_text: String,
    left_label: Option<String>,
    right_label: Option<String>,
    options: Option<DiffOptions>,
) -> Result<DiffFileResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let t0 = Instant::now();
        let opts = options.unwrap_or_default();
        let left = text_to_loaded(left_text, "utf-8".to_string());
        let right = text_to_loaded(right_text, "utf-8".to_string());
        let lm = meta_from_loaded(&left_label.unwrap_or_else(|| "左侧".to_string()), &left);
        let rm = meta_from_loaded(&right_label.unwrap_or_else(|| "右侧".to_string()), &right);
        let body = diff_loaded(&left, &right, &opts);
        Ok(result_from_body(lm, rm, body, t0))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 编辑器缓冲区 vs 磁盘上的同一文件（"我改了哪些还没存"）。左=磁盘，右=缓冲。
#[tauri::command]
pub async fn diff_buffer_vs_file(
    path: String,
    buffer_text: String,
    options: Option<DiffOptions>,
) -> Result<DiffFileResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let t0 = Instant::now();
        let opts = options.unwrap_or_default();
        let p = Path::new(&path);
        let st = stat_of(p)?;
        if !st.exists {
            return Err(format!("{} 不存在", path));
        }
        if st.size > opts.max_bytes() {
            return Err(format!(
                "{} 超过 {} MB 上限",
                path,
                opts.max_bytes() / (1024 * 1024)
            ));
        }
        let bytes = std::fs::read(p).map_err(|e| format!("读取 {} 失败：{}", path, e))?;
        if looks_binary(&bytes) {
            return Err(format!("{} 是二进制文件，无法做文本对比", path));
        }
        let (dtext, enc) = decode_bytes(&bytes);
        drop(bytes);
        let disk = text_to_loaded(dtext, enc);
        let buf = text_to_loaded(buffer_text, "utf-8".to_string());
        let mut dm = meta_from_loaded(&path, &disk);
        dm.size = st.size;
        dm.mtime_ms = st.mtime_ms;
        let bm = meta_from_loaded("编辑器缓冲区", &buf);
        let body = diff_loaded(&disk, &buf, &opts);
        Ok(result_from_body(dm, bm, body, t0))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ─────────────────────────── 目录对比 ───────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct DirDiffOptions {
    pub include_hidden: bool,
    /// 尊重 .gitignore（默认关，目录对比通常想看到全部）。
    pub respect_gitignore: bool,
    pub follow_links: bool,
    /// 把两侧都存在且相同的文件也放进结果（默认不放，只看差异）。
    pub include_same: bool,
    /// 快速模式：大小 + mtime 判定，完全不算哈希。第一次浏览巨大目录用。
    pub quick: bool,
    /// 在"只在一侧"的文件里按大小 + 哈希找改名。
    pub detect_renames: bool,
    /// 单个文件参与哈希的字节上限，超过就退回 mtime 判定。
    pub max_hash_bytes: Option<u64>,
    pub max_depth: Option<usize>,
    pub page_size: Option<u32>,
    /// 结果条数上限，防止百万条目撑爆内存。
    pub max_entries: Option<u32>,
    pub include_globs: Option<Vec<String>>,
    pub exclude_globs: Option<Vec<String>>,
}

impl DirDiffOptions {
    fn page_size(&self) -> u32 {
        self.page_size.unwrap_or(500).clamp(1, 5000)
    }

    fn max_entries(&self) -> usize {
        self.max_entries.unwrap_or(500_000).max(1) as usize
    }

    fn max_hash_bytes(&self) -> u64 {
        self.max_hash_bytes.unwrap_or(256 * 1024 * 1024)
    }
}

/// 一条目录差异记录。status: same|added|removed|modified|renamed|type_changed
#[derive(Debug, Clone, Serialize)]
pub struct DirEntryDiff {
    pub rel: String,
    pub status: &'static str,
    pub is_dir: bool,
    pub left_size: u64,
    pub right_size: u64,
    pub left_mtime_ms: Option<i64>,
    pub right_mtime_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<String>,
    /// 整棵子树被判为新增/删除时只回一条目录记录，这里是被折叠掉的条目数。
    pub child_count: u32,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DirDiffSummary {
    pub added: u32,
    pub removed: u32,
    pub modified: u32,
    pub renamed: u32,
    pub type_changed: u32,
    pub same: u32,
    pub total: u32,
    pub left_files: u32,
    pub right_files: u32,
    pub left_dirs: u32,
    pub right_dirs: u32,
    pub compared_files: u32,
    pub compared_bytes: u64,
    pub entries_truncated: bool,
    pub cancelled: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirDiffResult {
    pub session_id: String,
    pub left_root: String,
    pub right_root: String,
    pub summary: DirDiffSummary,
    /// 首页条目，其余用 diff_dirs_page 按需取。
    pub entries: Vec<DirEntryDiff>,
    pub page_size: u32,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirDiffPage {
    pub entries: Vec<DirEntryDiff>,
    pub offset: u32,
    pub total: u32,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
struct DirProgress {
    session_id: String,
    phase: &'static str,
    scanned: u64,
    compared: u64,
    elapsed_ms: u64,
}

struct FsNode {
    is_dir: bool,
    size: u64,
    mtime_ms: Option<i64>,
}

struct DirDiffSession {
    left_root: String,
    right_root: String,
    entries: Vec<DirEntryDiff>,
    summary: DirDiffSummary,
}

/// 结果留在 Rust 侧，前端按页取。只留最近几次，够用又不涨内存。
static DIR_SESSIONS: Lazy<Mutex<LruCache<String, Arc<DirDiffSession>>>> =
    Lazy::new(|| Mutex::new(LruCache::new(NonZeroUsize::new(6).unwrap())));
/// 取消开关独立于结果缓存：取消必须能在结果产生之前就生效。
static DIR_CANCELS: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static DIR_SEQ: AtomicU64 = AtomicU64::new(1);

struct Progress {
    app: Option<AppHandle>,
    session_id: String,
    scanned: AtomicU64,
    compared: AtomicU64,
    last_ms: AtomicU64,
    t0: Instant,
}

impl Progress {
    fn bump(&self, phase: &'static str, scanned: u64, compared: u64) {
        if scanned > 0 {
            self.scanned.fetch_add(scanned, Ordering::Relaxed);
        }
        if compared > 0 {
            self.compared.fetch_add(compared, Ordering::Relaxed);
        }
        let app = match self.app.as_ref() {
            Some(a) => a,
            None => return,
        };
        let now = self.t0.elapsed().as_millis() as u64;
        let last = self.last_ms.load(Ordering::Relaxed);
        if now < last.saturating_add(120) {
            return;
        }
        // 同一时刻只让抢到时间戳的那个线程发，避免事件雪崩。
        if self
            .last_ms
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        let _ = app.emit(
            "diff-dirs-progress",
            DirProgress {
                session_id: self.session_id.clone(),
                phase,
                scanned: self.scanned.load(Ordering::Relaxed),
                compared: self.compared.load(Ordering::Relaxed),
                elapsed_ms: now,
            },
        );
    }
}

/// Read::read 允许短读，这里补齐到 buf 满或 EOF，保证两侧块边界对齐。
fn read_full(f: &mut std::fs::File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

/// 成对比较用逐块 memcmp，第一处不同就返回，不做全量哈希。
/// 比"两边各算一次 blake3"更快，差异越靠前越明显。
fn files_equal(a: &Path, b: &Path, cancel: &AtomicBool) -> std::io::Result<bool> {
    let mut fa = std::fs::File::open(a)?;
    let mut fb = std::fs::File::open(b)?;
    let mut ba = vec![0u8; 128 * 1024];
    let mut bb = vec![0u8; 128 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "已取消",
            ));
        }
        let na = read_full(&mut fa, &mut ba)?;
        let nb = read_full(&mut fb, &mut bb)?;
        if na != nb {
            return Ok(false);
        }
        if na == 0 {
            return Ok(true);
        }
        if ba[..na] != bb[..nb] {
            return Ok(false);
        }
    }
}

fn walk_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 8)
}

fn build_walker(root: &Path, opts: &DirDiffOptions) -> Result<ignore::WalkParallel, String> {
    let mut b = WalkBuilder::new(root);
    let git = opts.respect_gitignore;
    b.hidden(!opts.include_hidden)
        .ignore(git)
        .git_ignore(git)
        .git_global(git)
        .git_exclude(git)
        .parents(git)
        // 不要求目标一定在 git 仓库里，否则 .gitignore 规则会被静默忽略。
        .require_git(false)
        .follow_links(opts.follow_links)
        .max_depth(opts.max_depth)
        .threads(walk_threads());
    let has_globs = opts.include_globs.as_ref().map_or(false, |v| !v.is_empty())
        || opts.exclude_globs.as_ref().map_or(false, |v| !v.is_empty());
    if has_globs {
        let mut ob = OverrideBuilder::new(root);
        for g in opts.include_globs.iter().flatten() {
            ob.add(g)
                .map_err(|e| format!("include 规则 {} 无效：{}", g, e))?;
        }
        // Override 里 `!` 的含义是反的：前缀 `!` 表示排除。
        for g in opts.exclude_globs.iter().flatten() {
            ob.add(&format!("!{}", g))
                .map_err(|e| format!("exclude 规则 {} 无效：{}", g, e))?;
        }
        b.overrides(ob.build().map_err(|e| e.to_string())?);
    }
    Ok(b.build_parallel())
}

/// ignore 的并行遍历没有"线程结束"钩子，用 Drop 把每个线程的尾巴刷进共享 buffer。
struct NodeSink<'a> {
    local: Vec<(String, FsNode)>,
    shared: &'a Mutex<Vec<(String, FsNode)>>,
}

impl NodeSink<'_> {
    fn push(&mut self, item: (String, FsNode)) {
        self.local.push(item);
        if self.local.len() >= 1024 {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.local.is_empty() {
            return;
        }
        self.shared.lock().append(&mut self.local);
    }
}

impl Drop for NodeSink<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}

fn scan_side(
    root: &Path,
    opts: &DirDiffOptions,
    cancel: &AtomicBool,
    progress: &Progress,
) -> Result<HashMap<String, FsNode>, String> {
    let shared: Mutex<Vec<(String, FsNode)>> = Mutex::new(Vec::new());
    build_walker(root, opts)?.run(|| {
        let mut sink = NodeSink {
            local: Vec::with_capacity(1024),
            shared: &shared,
        };
        Box::new(move |res| {
            if cancel.load(Ordering::Relaxed) {
                return WalkState::Quit;
            }
            let entry = match res {
                Ok(e) => e,
                // 单个条目读不到（权限、符号链接环）不该让整次对比失败。
                Err(_) => return WalkState::Continue,
            };
            if entry.depth() == 0 {
                return WalkState::Continue;
            }
            let rel = match entry.path().strip_prefix(root) {
                Ok(r) => r.to_string_lossy().replace('\\', "/"),
                Err(_) => return WalkState::Continue,
            };
            if rel.is_empty() {
                return WalkState::Continue;
            }
            let is_dir = entry.file_type().map_or(false, |t| t.is_dir());
            let (size, mtime) = match entry.metadata() {
                Ok(m) => (if is_dir { 0 } else { m.len() }, mtime_ms(&m)),
                Err(_) => (0, None),
            };
            sink.push((
                rel,
                FsNode {
                    is_dir,
                    size,
                    mtime_ms: mtime,
                },
            ));
            progress.bump("scan", 1, 0);
            WalkState::Continue
        })
    });
    if cancel.load(Ordering::Relaxed) {
        return Err("已取消".to_string());
    }
    let items = shared.into_inner();
    let mut map = HashMap::with_capacity(items.len());
    for (rel, node) in items {
        map.insert(rel, node);
    }
    Ok(map)
}

/// 改名探测的候选上限：再多就不值得为此读一堆文件。
const RENAME_CANDIDATE_LIMIT: usize = 20_000;

fn entry_of(
    rel: &str,
    status: &'static str,
    l: Option<&FsNode>,
    r: Option<&FsNode>,
) -> DirEntryDiff {
    DirEntryDiff {
        rel: rel.to_string(),
        status,
        is_dir: l.map_or(false, |n| n.is_dir) || r.map_or(false, |n| n.is_dir),
        left_size: l.map_or(0, |n| n.size),
        right_size: r.map_or(0, |n| n.size),
        left_mtime_ms: l.and_then(|n| n.mtime_ms),
        right_mtime_ms: r.and_then(|n| n.mtime_ms),
        renamed_from: None,
        child_count: 0,
    }
}

/// 找到 rel 最浅的那个"同方向且只在一侧"的祖先目录，用来折叠整棵子树。
fn fold_root<'a>(rel: &'a str, added: bool, dirs: &HashMap<&str, bool>) -> Option<&'a str> {
    for (i, ch) in rel.char_indices() {
        if ch != '/' {
            continue;
        }
        let prefix = &rel[..i];
        if dirs.get(prefix) == Some(&added) {
            return Some(prefix);
        }
    }
    None
}

/// 相对路径按"路径段"排序，让 a < a/b < a-b，父目录紧跟着自己的子树。
fn rel_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut ai = a.split('/');
    let mut bi = b.split('/');
    loop {
        match (ai.next(), bi.next()) {
            (Some(x), Some(y)) => {
                let o = x.cmp(y);
                if o != Ordering::Equal {
                    return o;
                }
            }
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

/// 两侧文件表 → 差异条目 + 汇总。判定顺序同样是"先便宜的再贵的"：
/// 类型 → 大小 → mtime → 真读内容。
fn classify(
    left_root: &Path,
    right_root: &Path,
    lmap: &HashMap<String, FsNode>,
    rmap: &HashMap<String, FsNode>,
    opts: &DirDiffOptions,
    cancel: &AtomicBool,
    progress: &Progress,
) -> (Vec<DirEntryDiff>, DirDiffSummary) {
    let mut summary = DirDiffSummary::default();
    for n in lmap.values() {
        if n.is_dir {
            summary.left_dirs += 1;
        } else {
            summary.left_files += 1;
        }
    }
    for n in rmap.values() {
        if n.is_dir {
            summary.right_dirs += 1;
        } else {
            summary.right_files += 1;
        }
    }

    let mut entries: Vec<DirEntryDiff> = Vec::new();
    // 大小一样、看元数据分不出来的，攒起来最后并行读内容。
    let mut pending: Vec<(usize, u64)> = Vec::new();
    let max_hash = opts.max_hash_bytes();

    for (rel, ln) in lmap.iter() {
        let rn = match rmap.get(rel) {
            Some(v) => v,
            None => continue,
        };
        if ln.is_dir && rn.is_dir {
            continue; // 两侧都在的目录本身不带信息，只看里面的文件
        }
        if ln.is_dir != rn.is_dir {
            entries.push(entry_of(rel, "type_changed", Some(ln), Some(rn)));
            continue;
        }
        if ln.size != rn.size {
            entries.push(entry_of(rel, "modified", Some(ln), Some(rn)));
            continue;
        }
        // 大小相同：空文件直接算相同；quick 模式或超大文件退回 mtime 判定。
        if ln.size == 0 || opts.quick || ln.size > max_hash {
            let differ = ln.size > 0
                && match (ln.mtime_ms, rn.mtime_ms) {
                    (Some(a), Some(b)) => a != b,
                    _ => false,
                };
            if differ {
                entries.push(entry_of(rel, "modified", Some(ln), Some(rn)));
            } else {
                summary.same += 1;
                if opts.include_same {
                    entries.push(entry_of(rel, "same", Some(ln), Some(rn)));
                }
            }
            continue;
        }
        entries.push(entry_of(rel, "modified", Some(ln), Some(rn)));
        pending.push((entries.len() - 1, ln.size));
    }
    // ── 只在一侧的条目：整棵子树只在一侧时只回一条目录记录 ──
    // 不折叠的话，删掉一个 node_modules 就是十万条记录。
    let mut one_sided_dirs: HashMap<&str, bool> = HashMap::new();
    for (rel, n) in lmap.iter() {
        if n.is_dir && !rmap.contains_key(rel) {
            one_sided_dirs.insert(rel.as_str(), false);
        }
    }
    for (rel, n) in rmap.iter() {
        if n.is_dir && !lmap.contains_key(rel) {
            one_sided_dirs.insert(rel.as_str(), true);
        }
    }
    let mut one_sided: Vec<(&str, bool)> = Vec::new();
    for rel in lmap.keys() {
        if !rmap.contains_key(rel) {
            one_sided.push((rel.as_str(), false));
        }
    }
    for rel in rmap.keys() {
        if !lmap.contains_key(rel) {
            one_sided.push((rel.as_str(), true));
        }
    }
    let mut fold_counts: HashMap<&str, u32> = HashMap::new();
    let mut folded_dirs: Vec<(&str, usize)> = Vec::new();
    for (rel, added) in one_sided {
        if let Some(root) = fold_root(rel, added, &one_sided_dirs) {
            *fold_counts.entry(root).or_insert(0) += 1;
            continue;
        }
        let (l, r) = if added {
            (None, rmap.get(rel))
        } else {
            (lmap.get(rel), None)
        };
        entries.push(entry_of(rel, if added { "added" } else { "removed" }, l, r));
        if one_sided_dirs.contains_key(rel) {
            folded_dirs.push((rel, entries.len() - 1));
        }
    }
    for (rel, idx) in folded_dirs {
        if let Some(c) = fold_counts.get(rel) {
            entries[idx].child_count = *c;
        }
    }
    // ── 大小相同、元数据分不出来的，只能真读内容（并行 + 首字不同即止）──
    if !pending.is_empty() && !cancel.load(Ordering::Relaxed) {
        let verdicts: Vec<(usize, bool, u64)> = pending
            .par_iter()
            .map(|(idx, size)| {
                if cancel.load(Ordering::Relaxed) {
                    return (*idx, false, 0);
                }
                let rel = entries[*idx].rel.as_str();
                let same =
                    files_equal(&left_root.join(rel), &right_root.join(rel), cancel).unwrap_or(false);
                progress.bump("compare", 0, 1);
                (*idx, same, *size)
            })
            .collect();
        for (idx, same, size) in verdicts {
            summary.compared_files += 1;
            summary.compared_bytes += size;
            if same {
                entries[idx].status = "same";
                summary.same += 1;
            }
        }
        if !opts.include_same {
            entries.retain(|e| e.status != "same");
        }
    }

    if opts.detect_renames && !cancel.load(Ordering::Relaxed) {
        detect_renames(&mut entries, left_root, right_root, opts, cancel, progress);
    }

    entries.sort_by(|a, b| rel_cmp(&a.rel, &b.rel));
    for e in entries.iter() {
        match e.status {
            "added" => summary.added += 1,
            "removed" => summary.removed += 1,
            "modified" => summary.modified += 1,
            "renamed" => summary.renamed += 1,
            "type_changed" => summary.type_changed += 1,
            _ => {}
        }
    }
    let cap = opts.max_entries();
    if entries.len() > cap {
        entries.truncate(cap);
        summary.entries_truncated = true;
    }
    summary.total = entries.len() as u32;
    summary.cancelled = cancel.load(Ordering::Relaxed);
    (entries, summary)
}

/// 改名探测：只在"只在一侧"的文件里找，先按大小分组，只有两侧都出现过的
/// 大小才值得算哈希——绝大多数候选在这一步就被排除了。
fn detect_renames(
    entries: &mut Vec<DirEntryDiff>,
    left_root: &Path,
    right_root: &Path,
    opts: &DirDiffOptions,
    cancel: &AtomicBool,
    progress: &Progress,
) {
    let max_hash = opts.max_hash_bytes();
    let mut removed: HashMap<u64, Vec<usize>> = HashMap::new();
    let mut added: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        if e.is_dir {
            continue;
        }
        match e.status {
            "removed" if e.left_size > 0 && e.left_size <= max_hash => {
                removed.entry(e.left_size).or_default().push(i)
            }
            "added" if e.right_size > 0 && e.right_size <= max_hash => {
                added.entry(e.right_size).or_default().push(i)
            }
            _ => continue,
        }
    }
    let mut groups: Vec<(Vec<usize>, Vec<usize>)> = Vec::new();
    for (size, ridx) in removed {
        if let Some(aidx) = added.get(&size) {
            groups.push((ridx, aidx.clone()));
        }
    }
    let total: usize = groups.iter().map(|(r, a)| r.len() + a.len()).sum();
    if total == 0 || total > RENAME_CANDIDATE_LIMIT {
        return;
    }
    let jobs: Vec<(usize, bool)> = groups
        .iter()
        .flat_map(|(r, a)| {
            r.iter()
                .map(|i| (*i, false))
                .chain(a.iter().map(|i| (*i, true)))
                .collect::<Vec<_>>()
        })
        .collect();
    let hashes: HashMap<usize, String> = jobs
        .par_iter()
        .filter_map(|(i, is_add)| {
            let root = if *is_add { right_root } else { left_root };
            let h = hash_file(&root.join(&entries[*i].rel), Some(cancel)).ok()?;
            progress.bump("rename", 0, 1);
            Some((*i, h))
        })
        .collect();
    let mut claimed: HashSet<usize> = HashSet::new();
    let mut dropped: HashSet<usize> = HashSet::new();
    for (ridx, aidx) in groups {
        for r in &ridx {
            let rh = match hashes.get(r) {
                Some(v) => v,
                None => continue,
            };
            let hit = aidx
                .iter()
                .find(|a| !claimed.contains(*a) && hashes.get(*a) == Some(rh));
            let a = match hit {
                Some(a) => *a,
                None => continue,
            };
            let (from, lsize, lmtime) = {
                let src = &entries[*r];
                (src.rel.clone(), src.left_size, src.left_mtime_ms)
            };
            let dst = &mut entries[a];
            dst.status = "renamed";
            dst.renamed_from = Some(from);
            dst.left_size = lsize;
            dst.left_mtime_ms = lmtime;
            claimed.insert(a);
            dropped.insert(*r);
        }
    }
    if !dropped.is_empty() {
        let mut i = 0usize;
        entries.retain(|_| {
            let keep = !dropped.contains(&i);
            i += 1;
            keep
        });
    }
}

/// 目录对比。两侧并行扫描 → 元数据分类 → 只对真分不清的文件读内容。
/// 结果整份留在 Rust 侧，前端用 diff_dirs_page 按页取。
#[tauri::command]
pub async fn diff_dirs(
    app: AppHandle,
    left_root: String,
    right_root: String,
    session_id: Option<String>,
    options: Option<DirDiffOptions>,
) -> Result<DirDiffResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let t0 = Instant::now();
        let opts = options.unwrap_or_default();
        let lp = PathBuf::from(&left_root);
        let rp = PathBuf::from(&right_root);
        for p in [&lp, &rp] {
            if !p.is_dir() {
                return Err(format!("{} 不是目录", p.display()));
            }
        }
        let sid = session_id
            .unwrap_or_else(|| format!("dir-{}", DIR_SEQ.fetch_add(1, Ordering::Relaxed)));
        let cancel = Arc::new(AtomicBool::new(false));
        DIR_CANCELS.lock().insert(sid.clone(), cancel.clone());
        let progress = Progress {
            app: Some(app),
            session_id: sid.clone(),
            scanned: AtomicU64::new(0),
            compared: AtomicU64::new(0),
            last_ms: AtomicU64::new(0),
            t0,
        };
        let (l, r) = rayon::join(
            || scan_side(&lp, &opts, &cancel, &progress),
            || scan_side(&rp, &opts, &cancel, &progress),
        );
        let maps = l.and_then(|lm| r.map(|rm| (lm, rm)));
        let (lmap, rmap) = match maps {
            Ok(v) => v,
            Err(e) => {
                DIR_CANCELS.lock().remove(&sid);
                return Err(e);
            }
        };
        let (entries, mut summary) = classify(&lp, &rp, &lmap, &rmap, &opts, &cancel, &progress);
        drop(lmap);
        drop(rmap);
        DIR_CANCELS.lock().remove(&sid);
        summary.elapsed_ms = t0.elapsed().as_millis() as u64;
        info!(
            "diff_dirs {} ↔ {}：{} 条差异，比对 {} 个文件，耗时 {} ms",
            left_root, right_root, summary.total, summary.compared_files, summary.elapsed_ms
        );
        let page = opts.page_size() as usize;
        let first: Vec<DirEntryDiff> = entries.iter().take(page).cloned().collect();
        let has_more = entries.len() > first.len();
        DIR_SESSIONS.lock().put(
            sid.clone(),
            Arc::new(DirDiffSession {
                left_root: left_root.clone(),
                right_root: right_root.clone(),
                entries,
                summary: summary.clone(),
            }),
        );
        Ok(DirDiffResult {
            session_id: sid,
            left_root,
            right_root,
            summary,
            entries: first,
            page_size: page as u32,
            has_more,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 按页取目录差异条目，支持状态过滤和路径关键字过滤。
#[tauri::command]
pub async fn diff_dirs_page(
    session_id: String,
    offset: Option<u32>,
    limit: Option<u32>,
    statuses: Option<Vec<String>>,
    query: Option<String>,
) -> Result<DirDiffPage, String> {
    let session = DIR_SESSIONS
        .lock()
        .get(&session_id)
        .cloned()
        .ok_or_else(|| "对比会话已过期，请重新对比".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let limit = limit.unwrap_or(500).clamp(1, 5000) as usize;
        let q = query.unwrap_or_default().trim().to_lowercase();
        let want = statuses.unwrap_or_default();
        let filtered: Vec<&DirEntryDiff> = session
            .entries
            .iter()
            .filter(|e| {
                (want.is_empty() || want.iter().any(|s| s == e.status))
                    && (q.is_empty() || e.rel.to_lowercase().contains(&q))
            })
            .collect();
        let start = (offset.unwrap_or(0) as usize).min(filtered.len());
        let end = (start + limit).min(filtered.len());
        Ok(DirDiffPage {
            entries: filtered[start..end].iter().map(|e| (*e).clone()).collect(),
            offset: start as u32,
            total: filtered.len() as u32,
            has_more: end < filtered.len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 目录树里的一层节点。目录节点不是扫描结果里的记录，而是按 rel 前缀现聚合出来的：
/// 两侧都在的目录本身不带差异信息，只有"里面有多少条差异"值得显示。
#[derive(Debug, Clone, Serialize)]
pub struct DirTreeNode {
    pub rel: String,
    pub name: String,
    pub is_dir: bool,
    /// 能不能展开：底下还有差异的聚合目录能；整棵子树只在一侧的折叠目录也能（展开时读盘）。
    pub expandable: bool,
    /// 叶子是条目自身的状态；聚合目录只有一种状态时跟着它，混着就是 mixed。
    pub status: &'static str,
    pub left_size: u64,
    pub right_size: u64,
    pub left_mtime_ms: Option<i64>,
    pub right_mtime_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<String>,
    /// 聚合目录：子孙差异条数；折叠目录：被折叠掉的条数；文件恒为 0。
    pub child_count: u32,
    pub added: u32,
    pub removed: u32,
    pub modified: u32,
    pub renamed: u32,
    pub type_changed: u32,
    pub same: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirTreeLevel {
    pub parent: String,
    pub nodes: Vec<DirTreeNode>,
    pub total: u32,
    pub truncated: bool,
    /// true 表示这一层是直接读磁盘列出来的（整棵子树只在一侧，结果集里没有这些条目）。
    pub from_disk: bool,
}

/// 一层最多回多少个节点。再多前端也读不完，不如截断了给个提示。
const TREE_LEVEL_LIMIT: u32 = 2000;

fn tree_leaf(e: &DirEntryDiff, name: &str) -> DirTreeNode {
    DirTreeNode {
        rel: e.rel.clone(),
        name: name.to_string(),
        is_dir: e.is_dir,
        // 单侧折叠目录的内容没进结果集，child_count 是它唯一的"里面还有东西"的证据。
        expandable: e.is_dir && e.child_count > 0,
        status: e.status,
        left_size: e.left_size,
        right_size: e.right_size,
        left_mtime_ms: e.left_mtime_ms,
        right_mtime_ms: e.right_mtime_ms,
        renamed_from: e.renamed_from.clone(),
        child_count: e.child_count,
        added: 0,
        removed: 0,
        modified: 0,
        renamed: 0,
        type_changed: 0,
        same: 0,
    }
}

fn tree_dir(rel: String, name: String) -> DirTreeNode {
    DirTreeNode {
        rel,
        name,
        is_dir: true,
        expandable: true,
        status: "mixed",
        left_size: 0,
        right_size: 0,
        left_mtime_ms: None,
        right_mtime_ms: None,
        renamed_from: None,
        child_count: 0,
        added: 0,
        removed: 0,
        modified: 0,
        renamed: 0,
        type_changed: 0,
        same: 0,
    }
}

fn bump_rollup(node: &mut DirTreeNode, status: &str) {
    match status {
        "added" => node.added += 1,
        "removed" => node.removed += 1,
        "modified" => node.modified += 1,
        "renamed" => node.renamed += 1,
        "type_changed" => node.type_changed += 1,
        _ => node.same += 1,
    }
}

/// 子孙状态单一就让目录跟着它（一眼看出"这下面全是新增"），混了才叫 mixed。
fn rollup_status(n: &DirTreeNode) -> &'static str {
    let mut hit: Option<&'static str> = None;
    for (count, status) in [
        (n.added, "added"),
        (n.removed, "removed"),
        (n.modified, "modified"),
        (n.renamed, "renamed"),
        (n.type_changed, "type_changed"),
        (n.same, "same"),
    ] {
        if count == 0 {
            continue;
        }
        if hit.is_some() {
            return "mixed";
        }
        hit = Some(status);
    }
    hit.unwrap_or("mixed")
}

/// 目录在前、同类按名字，跟文件树的习惯一致。
fn sort_tree_nodes(nodes: &mut [DirTreeNode]) {
    nodes.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// 排序过的条目里精确找一条。entries 按 rel_cmp 排过，可以二分。
fn entry_at<'a>(entries: &'a [DirEntryDiff], rel: &str) -> Option<&'a DirEntryDiff> {
    let i = entries.partition_point(|e| rel_cmp(&e.rel, rel) == std::cmp::Ordering::Less);
    entries.get(i).filter(|e| e.rel == rel)
}

/// parent 子树对应的那一段条目（不含 parent 自己）。
/// rel_cmp 按路径段排序，所以一棵子树必然连续，取一段就够，不用全表扫。
fn descendants<'a>(entries: &'a [DirEntryDiff], parent: &str) -> &'a [DirEntryDiff] {
    if parent.is_empty() {
        return entries;
    }
    let start = entries.partition_point(|e| rel_cmp(&e.rel, parent) == std::cmp::Ordering::Less);
    let rest = &entries[start..];
    // parent 自己可能就在段首（比如它是 type_changed），跳过。
    let from = usize::from(rest.first().map_or(false, |e| e.rel == parent));
    let prefix = format!("{}/", parent);
    let mut to = from;
    while to < rest.len() && rest[to].rel.starts_with(&prefix) {
        to += 1;
    }
    &rest[from..to]
}

/// parent 自己或它的某个祖先是"整棵子树只在一侧"的折叠目录吗？
/// 是的话回 parent 在那一侧的绝对路径和状态——展开这种目录只能去读盘。
fn folded_side(session: &DirDiffSession, parent: &str) -> Option<(PathBuf, &'static str)> {
    let mut cur = if parent.is_empty() { None } else { Some(parent) };
    while let Some(rel) = cur {
        if let Some(e) = entry_at(&session.entries, rel) {
            if e.is_dir && (e.status == "added" || e.status == "removed") {
                let root = if e.status == "added" {
                    &session.right_root
                } else {
                    &session.left_root
                };
                return Some((Path::new(root).join(parent), e.status));
            }
            return None;
        }
        cur = rel.rsplit_once('/').map(|(head, _)| head);
    }
    None
}

/// 折叠目录展开时直接读盘：里面整棵都只在一侧，状态跟父目录一样。
/// 这里不套关键字过滤——能看见这个折叠目录说明它已经过了过滤，里面的内容应当能翻。
fn list_disk_children(
    dir: &Path,
    parent_rel: &str,
    status: &'static str,
    want: &[String],
) -> Vec<DirTreeNode> {
    if !(want.is_empty() || want.iter().any(|s| s == status)) {
        return Vec::new();
    }
    let rd = match std::fs::read_dir(dir) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let md = match ent.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let name = ent.file_name().to_string_lossy().to_string();
        let is_dir = md.is_dir();
        let size = if is_dir { 0 } else { md.len() };
        let when = mtime_ms(&md);
        let rel = if parent_rel.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", parent_rel, name)
        };
        let added = status == "added";
        out.push(DirTreeNode {
            rel,
            name,
            is_dir,
            expandable: is_dir,
            status,
            left_size: if added { 0 } else { size },
            right_size: if added { size } else { 0 },
            left_mtime_ms: if added { None } else { when },
            right_mtime_ms: if added { when } else { None },
            renamed_from: None,
            child_count: 0,
            added: 0,
            removed: 0,
            modified: 0,
            renamed: 0,
            type_changed: 0,
            same: 0,
        });
    }
    out
}

/// 把结果集切成"parent 的直接子项"：带斜杠的归到聚合目录，不带的就是叶子。
fn build_tree_level(
    session: &DirDiffSession,
    parent: &str,
    want: &[String],
    query: &str,
    limit: usize,
) -> DirTreeLevel {
    let prefix_len = if parent.is_empty() { 0 } else { parent.len() + 1 };
    let mut leaves: Vec<DirTreeNode> = Vec::new();
    let mut dirs: Vec<DirTreeNode> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for e in descendants(&session.entries, parent) {
        let pass = (want.is_empty() || want.iter().any(|s| s == e.status))
            && (query.is_empty() || e.rel.to_lowercase().contains(query));
        if !pass {
            continue;
        }
        let rest = &e.rel[prefix_len..];
        match rest.find('/') {
            None => leaves.push(tree_leaf(e, rest)),
            Some(cut) => {
                let name = &rest[..cut];
                let rel = if parent.is_empty() {
                    name.to_string()
                } else {
                    format!("{}/{}", parent, name)
                };
                let idx = match seen.get(&rel) {
                    Some(i) => *i,
                    None => {
                        dirs.push(tree_dir(rel.clone(), name.to_string()));
                        seen.insert(rel, dirs.len() - 1);
                        dirs.len() - 1
                    }
                };
                let node = &mut dirs[idx];
                node.child_count += 1;
                node.left_size += e.left_size;
                node.right_size += e.right_size;
                bump_rollup(node, e.status);
            }
        }
    }
    for d in dirs.iter_mut() {
        d.status = rollup_status(d);
    }

    let mut from_disk = false;
    if leaves.is_empty() && dirs.is_empty() {
        if let Some((dir, status)) = folded_side(session, parent) {
            from_disk = true;
            leaves = list_disk_children(&dir, parent, status, want);
        }
    }

    let mut nodes = dirs;
    nodes.append(&mut leaves);
    sort_tree_nodes(&mut nodes);
    let total = nodes.len() as u32;
    let truncated = nodes.len() > limit;
    if truncated {
        nodes.truncate(limit);
    }
    DirTreeLevel {
        parent: parent.to_string(),
        nodes,
        total,
        truncated,
        from_disk,
    }
}

/// 取目录树的一层（parent 为空是根）。逐级展开一次只问一层：
/// 几十万条差异的结果集也不必在前端建整棵树，展开哪层算哪层。
#[tauri::command]
pub async fn diff_dirs_children(
    session_id: String,
    parent: Option<String>,
    statuses: Option<Vec<String>>,
    query: Option<String>,
    limit: Option<u32>,
) -> Result<DirTreeLevel, String> {
    let session = DIR_SESSIONS
        .lock()
        .get(&session_id)
        .cloned()
        .ok_or_else(|| "对比会话已过期，请重新对比".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let limit = limit.unwrap_or(TREE_LEVEL_LIMIT).clamp(1, 20_000) as usize;
        let q = query.unwrap_or_default().trim().to_lowercase();
        let want = statuses.unwrap_or_default();
        let parent = parent.unwrap_or_default().replace('\\', "/");
        let parent = parent.trim_matches('/');
        Ok(build_tree_level(&session, parent, &want, &q, limit))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 取消正在跑的目录扫描。返回 false 表示这个会话已经跑完了。
#[tauri::command]
pub async fn diff_dirs_cancel(session_id: String) -> Result<bool, String> {
    let flag = DIR_CANCELS.lock().get(&session_id).cloned();
    match flag {
        Some(f) => {
            f.store(true, Ordering::Relaxed);
            warn!("diff_dirs 会话 {} 收到取消请求", session_id);
            Ok(true)
        }
        None => Ok(false),
    }
}

/// 主动释放一次目录对比的结果（面板关掉时调，几十万条能省不少内存）。
#[tauri::command]
pub async fn diff_dirs_close(session_id: String) -> Result<bool, String> {
    DIR_CANCELS.lock().remove(&session_id);
    Ok(DIR_SESSIONS.lock().pop(&session_id).is_some())
}

/// 重新取一次会话的汇总（面板重挂载后不必重扫）。
#[tauri::command]
pub async fn diff_dirs_summary(session_id: String) -> Result<DirDiffSummary, String> {
    DIR_SESSIONS
        .lock()
        .get(&session_id)
        .map(|s| s.summary.clone())
        .ok_or_else(|| "对比会话已过期，请重新对比".to_string())
}

/// 目录对比会话里某一条的两侧绝对路径，给"点开看文件差异"用。
#[tauri::command]
pub async fn diff_dirs_entry_paths(
    session_id: String,
    rel: String,
) -> Result<(String, String), String> {
    let session = DIR_SESSIONS
        .lock()
        .get(&session_id)
        .cloned()
        .ok_or_else(|| "对比会话已过期，请重新对比".to_string())?;
    let l = Path::new(&session.left_root).join(&rel);
    let r = Path::new(&session.right_root).join(&rel);
    Ok((
        l.to_string_lossy().to_string(),
        r.to_string_lossy().to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(s: &str) -> LoadedText {
        text_to_loaded(s.to_string(), "utf-8".to_string())
    }

    fn tags(h: &DiffHunk) -> Vec<(&'static str, Option<u32>, Option<u32>, &str)> {
        h.lines
            .iter()
            .map(|l| (l.tag, l.left_no, l.right_no, l.text.as_str()))
            .collect()
    }

    /// 改动落在文件中间：上下文各 3 行，左右行号必须同步推进。
    #[test]
    fn middle_change_keeps_line_numbers_aligned() {
        let l = load("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");
        let r = load("1\n2\n3\n4\n5x\n6\n7\n8\n9\n10\n");
        let body = diff_loaded(&l, &r, &DiffOptions::default());
        assert_eq!(body.mode, "text");
        assert_eq!((body.stats.added, body.stats.removed), (1, 1));
        assert_eq!((body.stats.left_lines, body.stats.right_lines), (10, 10));
        assert_eq!(body.hunks.len(), 1);
        let h = &body.hunks[0];
        assert_eq!(h.header, "@@ -2,7 +2,7 @@");
        assert_eq!(
            tags(h),
            vec![
                ("ctx", Some(2), Some(2), "2"),
                ("ctx", Some(3), Some(3), "3"),
                ("ctx", Some(4), Some(4), "4"),
                ("del", Some(5), None, "5"),
                ("add", None, Some(5), "5x"),
                ("ctx", Some(6), Some(6), "6"),
                ("ctx", Some(7), Some(7), "7"),
                ("ctx", Some(8), Some(8), "8"),
            ]
        );
    }

    /// 改动贴着文件头/尾时，上下文要按两侧都不越界的那个取，否则行号会错位。
    #[test]
    fn change_at_file_boundary_clamps_context() {
        let body = diff_loaded(&load("a\nb\nc\n"), &load("A\nb\nc\n"), &DiffOptions::default());
        let h = &body.hunks[0];
        assert_eq!(h.header, "@@ -1,3 +1,3 @@");
        assert_eq!(h.lines[0].tag, "del");
        assert_eq!(h.lines.last().unwrap().left_no, Some(3));

        let body = diff_loaded(&load("a\nb\nc\n"), &load("a\nb\nC\n"), &DiffOptions::default());
        let h = &body.hunks[0];
        assert_eq!(h.header, "@@ -1,3 +1,3 @@");
        assert_eq!(h.lines.last().unwrap().tag, "add");
    }

    /// 纯插入：左侧不该出现 del 行，尾部上下文的左右行号会错开一格。
    #[test]
    fn pure_insert_only_emits_add_lines() {
        let body = diff_loaded(&load("a\nb\n"), &load("a\nx\nb\n"), &DiffOptions::default());
        assert_eq!((body.stats.added, body.stats.removed), (1, 0));
        let h = &body.hunks[0];
        assert_eq!(h.header, "@@ -1,2 +1,3 @@");
        assert_eq!(
            tags(h),
            vec![
                ("ctx", Some(1), Some(1), "a"),
                ("add", None, Some(2), "x"),
                ("ctx", Some(2), Some(3), "b"),
            ]
        );
    }

    /// 只有行尾符不同不该被判成整文件全改（split_lines 会吃掉 \r）。
    #[test]
    fn crlf_vs_lf_is_identical() {
        let body = diff_loaded(&load("a\r\nb\r\n"), &load("a\nb\n"), &DiffOptions::default());
        assert!(body.identical);
        assert_eq!(body.mode, "identical");
        assert!(body.hunks.is_empty());
    }

    /// 末行有无换行符同样不算内容差异。
    #[test]
    fn trailing_newline_is_identical() {
        let body = diff_loaded(&load("a\nb"), &load("a\nb\n"), &DiffOptions::default());
        assert!(body.identical);
    }

    #[test]
    fn ignore_flags_take_effect() {
        let ws = DiffOptions {
            ignore_whitespace: true,
            ..Default::default()
        };
        assert!(diff_loaded(&load("a b\n"), &load("  a\tb  \n"), &ws).identical);
        assert!(!diff_loaded(&load("a b\n"), &load("  a\tb  \n"), &DiffOptions::default()).identical);

        let case = DiffOptions {
            ignore_case: true,
            ..Default::default()
        };
        assert!(diff_loaded(&load("Hello\n"), &load("hello\n"), &case).identical);

        let trail = DiffOptions {
            ignore_trailing_whitespace: true,
            ..Default::default()
        };
        assert!(diff_loaded(&load("a   \n"), &load("a\n"), &trail).identical);
        assert!(!diff_loaded(&load("  a\n"), &load("a\n"), &trail).identical);
    }

    /// 只增删空行时，ignore_blank_lines 应该把这处改动整块丢掉。
    #[test]
    fn ignore_blank_lines_drops_blank_only_change() {
        let opts = DiffOptions {
            ignore_blank_lines: true,
            ..Default::default()
        };
        assert!(diff_loaded(&load("a\n\nb\n"), &load("a\nb\n"), &opts).identical);
        assert!(!diff_loaded(&load("a\n\nb\n"), &load("a\nb\n"), &DiffOptions::default()).identical);
    }

    /// 行内高亮的区间按 UTF-16 code unit 算，前端能直接 slice。
    #[test]
    fn inline_ranges_use_utf16_offsets() {
        assert_eq!(inline_ranges("你好世界", "你好啊界"), Some(([2, 3], [2, 3])));
        // 代理对占 2 个 code unit
        assert_eq!(inline_ranges("a😀b", "a😁b"), Some(([1, 3], [1, 3])));
        assert_eq!(inline_ranges("abc", "abc"), None);
        // 长度不同：前缀 1、后缀 1，中间那段是新增
        assert_eq!(inline_ranges("ab", "axxb"), Some(([1, 1], [1, 3])));
    }

    /// 成对改动才带行内高亮，行数不等时配对本身就是猜的。
    #[test]
    fn inline_only_on_paired_changes() {
        let body = diff_loaded(&load("foo bar\n"), &load("foo baz\n"), &DiffOptions::default());
        let inline = body.hunks[0].lines[0].inline.clone().expect("应有行内区间");
        assert_eq!(inline, vec![[6, 7]]);

        let no_inline = DiffOptions {
            no_inline: true,
            ..Default::default()
        };
        let body = diff_loaded(&load("foo bar\n"), &load("foo baz\n"), &no_inline);
        assert!(body.hunks[0].lines[0].inline.is_none());
    }

    /// 间隔小于 2*ctx 的两处改动合成一个 hunk，超过就分开。
    #[test]
    fn nearby_changes_merge_into_one_hunk() {
        let l = load("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n");
        let r = load("1x\n2\n3\n4\n5\n6\n7x\n8\n9\n10\n11\n12\n13\n14\n15\n16x\n");
        assert_eq!(diff_loaded(&l, &r, &DiffOptions::default()).hunks.len(), 2);
        let tight = DiffOptions {
            context_lines: Some(1),
            ..Default::default()
        };
        assert_eq!(diff_loaded(&l, &r, &tight).hunks.len(), 3);
    }

    #[test]
    fn stats_only_skips_hunk_rendering() {
        let opts = DiffOptions {
            stats_only: true,
            ..Default::default()
        };
        let body = diff_loaded(&load("a\nb\n"), &load("a\nB\n"), &opts);
        assert!(body.hunks.is_empty());
        assert_eq!((body.stats.added, body.stats.removed, body.stats.hunks), (1, 1, 1));
    }

    /// 相对路径按路径段排序：父目录紧跟自己的子树，a < a/b < a-b。
    #[test]
    fn rel_cmp_orders_by_path_segment() {
        let mut v = vec!["a-b", "a/b", "a", "a/b/c", "b"];
        v.sort_by(|x, y| rel_cmp(x, y));
        assert_eq!(v, vec!["a", "a/b", "a/b/c", "a-b", "b"]);
    }

    /// 只在一侧的整棵子树折叠成一条目录记录，child_count 记下被折叠的条数。
    #[test]
    fn one_sided_subtree_folds_to_single_entry() {
        let dirs: HashMap<&str, bool> = [("node_modules", false)].into_iter().collect();
        assert_eq!(fold_root("node_modules/a/b.js", false, &dirs), Some("node_modules"));
        // 方向不同不能折叠
        assert_eq!(fold_root("node_modules/a/b.js", true, &dirs), None);
        assert_eq!(fold_root("src/main.rs", false, &dirs), None);
        // 同名前缀但不是父目录
        assert_eq!(fold_root("node_modules_old/a.js", false, &dirs), None);
    }

    fn mk_entry(rel: &str, status: &'static str, is_dir: bool, child_count: u32) -> DirEntryDiff {
        DirEntryDiff {
            rel: rel.to_string(),
            status,
            is_dir,
            left_size: 0,
            right_size: 0,
            left_mtime_ms: None,
            right_mtime_ms: None,
            renamed_from: None,
            child_count,
        }
    }

    /// 逐级展开：带斜杠的条目聚合成目录节点，不带的就是这一层的叶子。
    #[test]
    fn tree_level_groups_children_by_first_segment() {
        let session = DirDiffSession {
            left_root: "L".into(),
            right_root: "R".into(),
            entries: vec![
                mk_entry("a/b/c.txt", "modified", false, 0),
                mk_entry("a/d.txt", "added", false, 0),
                mk_entry("e.txt", "removed", false, 0),
            ],
            summary: DirDiffSummary::default(),
        };

        let root = build_tree_level(&session, "", &[], "", 100);
        assert_eq!(root.nodes.len(), 2);
        assert!(root.nodes[0].is_dir && root.nodes[0].name == "a");
        assert_eq!(root.nodes[0].child_count, 2); // 子孙里两条差异
        assert_eq!(root.nodes[0].status, "mixed"); // 一条改一条增
        assert_eq!(root.nodes[1].name, "e.txt");
        assert!(!root.nodes[1].expandable);

        let level_a = build_tree_level(&session, "a", &[], "", 100);
        assert_eq!(level_a.nodes.len(), 2);
        assert_eq!(level_a.nodes[0].name, "b");
        assert_eq!(level_a.nodes[0].status, "modified"); // 子孙只有一种状态就跟着它
        assert_eq!(level_a.nodes[1].name, "d.txt");

        // 状态过滤穿透到目录：a/b 底下没有新增，整个目录就不该出现
        let only_added = build_tree_level(&session, "a", &["added".to_string()], "", 100);
        assert_eq!(only_added.nodes.len(), 1);
        assert_eq!(only_added.nodes[0].name, "d.txt");
    }

    /// 折叠掉的单侧子树在结果集里只有一条记录，展开它只能去磁盘上现列。
    #[test]
    fn folded_one_sided_dir_expands_from_disk() {
        let root = std::env::temp_dir().join("guaxi-diff-tree-test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("right/pkg/inner")).unwrap();
        std::fs::write(root.join("right/pkg/a.txt"), b"hello").unwrap();
        std::fs::write(root.join("right/pkg/inner/b.txt"), b"hi").unwrap();

        let session = DirDiffSession {
            left_root: root.join("left").to_string_lossy().to_string(),
            right_root: root.join("right").to_string_lossy().to_string(),
            entries: vec![mk_entry("pkg", "added", true, 2)],
            summary: DirDiffSummary::default(),
        };

        // 根一层还是结果集里的那条记录，只是标成可展开
        let top = build_tree_level(&session, "", &[], "", 100);
        assert!(!top.from_disk);
        assert_eq!(top.nodes.len(), 1);
        assert!(top.nodes[0].expandable);

        let lvl = build_tree_level(&session, "pkg", &[], "", 100);
        assert!(lvl.from_disk);
        assert_eq!(lvl.nodes.len(), 2);
        assert!(lvl.nodes[0].is_dir && lvl.nodes[0].name == "inner" && lvl.nodes[0].expandable);
        assert_eq!(lvl.nodes[1].name, "a.txt");
        assert_eq!((lvl.nodes[1].status, lvl.nodes[1].right_size), ("added", 5));

        // 再深一层：祖先是折叠目录，照样读盘
        let deeper = build_tree_level(&session, "pkg/inner", &[], "", 100);
        assert!(deeper.from_disk);
        assert_eq!(deeper.nodes.len(), 1);
        assert_eq!(deeper.nodes[0].rel, "pkg/inner/b.txt");

        let _ = std::fs::remove_dir_all(&root);
    }
}
