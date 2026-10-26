//! 用户模型：`config/cognition/profile.md` 是真相，SQLite 里的 `interests` 是派生索引。
//!
//! 为什么真相是 markdown：用户必须能逐行看见、改掉、git 掉系统对自己的判断。
//! 这是"这是我的画像"和"这是一个黑箱推荐算法"的分界线。派生层随时可由
//! `rederive_from_markdown()` 重建，删库不丢东西。
//!
//! 派生权重的更新有两条路：
//! - `reinforce_from_node`：显著的感知锚定到某节点 → 该节点的标签权重上浮。
//! - `decay_interests`：每天衰减，让不再关注的主题自然沉下去（手工项不衰减）。

use crate::cognition::store::{blob_to_vec, cog_data_dir, ensure_db, now_iso, vec_to_blob};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, warn};

/// 画像文件名。
const PROFILE_FILE: &str = "profile.md";

/// 派生兴趣的衰减半衰期（天）。14 天没再命中，权重减半。
const DECAY_HALFLIFE_DAYS: f64 = 14.0;

/// 单次强化的最大增量，防止一条爆款文章把画像带偏。
const MAX_REINFORCE_STEP: f64 = 0.08;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QuietHours {
    /// 24 小时制起点，如 22。
    pub start_hour: u32,
    /// 24 小时制终点，如 8。跨零点时 start > end。
    pub end_hour: u32,
    pub enabled: bool,
}

/// 一条兴趣项。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Interest {
    pub topic: String,
    pub weight: f64,
    /// manual = 用户在 profile.md 里写的，derived = 系统从行为推出来的。
    pub source: String,
    pub hits: i64,
    pub last_seen: Option<String>,
    pub embedded: bool,
}

/// 画像视图（前端设置页展示 + 简报/打断策略读取）。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// profile.md 原文，前端直接编辑这个。
    pub markdown: String,
    pub path: String,
    pub interests: Vec<Interest>,
    /// 用户声明的专长（降低这些主题的"缺口"提醒强度）。
    pub expertise: Vec<String>,
    /// 在做的项目名（提高相关主题的显著性）。
    pub projects: Vec<String>,
    /// 回答风格偏好，原样拼进系统提示词。
    pub answer_style: String,
    pub quiet_hours: QuietHours,
    /// 简报投递时刻（0-23）。
    pub brief_hour: u32,
    pub updated_at: String,
}

/// 兴趣向量缓存。写画像或强化后置脏，下次读时重建。
struct InterestVecs {
    topics: Vec<String>,
    weights: Vec<f64>,
    data: Vec<f32>,
    dim: usize,
}

static IVEC_CACHE: Lazy<RwLock<Option<InterestVecs>>> = Lazy::new(|| RwLock::new(None));

fn invalidate_ivec_cache() {
    *IVEC_CACHE.write() = None;
}

pub fn profile_path() -> Result<PathBuf, String> {
    Ok(cog_data_dir()?.join(PROFILE_FILE))
}

/// 首次运行时写出的模板。刻意写成"可读的说明书"，用户改它就是在配置系统。
const DEFAULT_PROFILE_MD: &str = r#"# 我的画像

> 这个文件是**真相**。系统只会往下面的「派生兴趣」区追加它的观察，
> 其余各节完全由你掌握。删掉一行，系统就不再据此判断。

## 关注

<!-- 一行一个主题；系统会按它决定什么值得提醒你。 -->
- 知识管理
- 检索增强

## 专长

<!-- 你已经很熟的领域。系统会降低这些主题的「缺口提醒」强度，避免教你已经会的东西。 -->
-

## 在做的项目

<!-- 一行一个。与这些项目相关的输入会被判为更显著。 -->
-

## 回答偏好

简洁、给结论、带引证。不要复述我的问题。

## 免打扰

- 时段: 22-8
- 简报时刻: 8

## 不要提醒我

<!-- 一行一个主题或关键词，命中即不进入主动提醒。 -->
-
"#;

/// 保证 profile.md 存在（幂等）。
pub fn ensure_profile_file() -> Result<PathBuf, String> {
    let p = profile_path()?;
    if !p.exists() {
        fs::write(&p, DEFAULT_PROFILE_MD).map_err(|e| format!("写画像模板失败: {e}"))?;
        debug!("[cognition] 已生成默认画像 {}", p.to_string_lossy());
    }
    Ok(p)
}

/// 取出 markdown 里某个二级标题下的列表项。
fn section_items(md: &str, heading: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in md.lines() {
        let t = line.trim();
        if let Some(h) = t.strip_prefix("## ") {
            inside = h.trim() == heading;
            continue;
        }
        if t.starts_with("# ") {
            inside = false;
            continue;
        }
        if !inside || t.starts_with("<!--") {
            continue;
        }
        if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            let v = item.trim();
            if !v.is_empty() {
                out.push(v.to_string());
            }
        }
    }
    out
}

/// 取出某个二级标题下的自由文本（用于「回答偏好」）。
fn section_text(md: &str, heading: &str) -> String {
    let mut buf: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in md.lines() {
        let t = line.trim();
        if let Some(h) = t.strip_prefix("## ") {
            inside = h.trim() == heading;
            continue;
        }
        if t.starts_with("# ") {
            inside = false;
            continue;
        }
        if inside && !t.is_empty() && !t.starts_with("<!--") && !t.starts_with("- ") {
            buf.push(t);
        }
    }
    buf.join(" ").trim().to_string()
}

/// 解析「免打扰」小节里的 `时段: 22-8` 与 `简报时刻: 8`。
fn parse_quiet(md: &str) -> (QuietHours, u32) {
    let mut q = QuietHours { start_hour: 22, end_hour: 8, enabled: true };
    let mut brief = 8u32;
    for item in section_items(md, "免打扰") {
        let (k, v) = match item.split_once(':').or_else(|| item.split_once('：')) {
            Some(kv) => kv,
            None => continue,
        };
        let key = k.trim();
        let val = v.trim();
        if key.contains("时段") {
            if val.eq_ignore_ascii_case("off") || val == "关闭" {
                q.enabled = false;
                continue;
            }
            if let Some((a, b)) = val.split_once('-') {
                if let (Ok(a), Ok(b)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                    q.start_hour = a.min(23);
                    q.end_hour = b.min(23);
                    q.enabled = true;
                }
            }
        } else if key.contains("简报") {
            if let Ok(h) = val.parse::<u32>() {
                brief = h.min(23);
            }
        }
    }
    (q, brief)
}

/// 现在是否处于免打扰时段。
pub fn in_quiet_hours() -> bool {
    let p = match load_profile() {
        Ok(p) => p,
        Err(_) => return false,
    };
    if !p.quiet_hours.enabled {
        return false;
    }
    let hour = chrono::Local::now().format("%H").to_string().parse::<u32>().unwrap_or(12);
    let (s, e) = (p.quiet_hours.start_hour, p.quiet_hours.end_hour);
    if s <= e {
        hour >= s && hour < e
    } else {
        hour >= s || hour < e
    }
}

/// 读画像，但**不做派生层同步**。
///
/// 给「只想看一眼状态」的调用方用（状态面板要 quiet hours 与简报时刻）。
/// `load_profile()` 会顺手把 markdown 里的手工关注项同步进兴趣表——那是写库，
/// 一次 UI 刷新不该触发它。
pub fn load_profile_unchecked() -> Result<Profile, String> {
    let path = ensure_profile_file()?;
    let markdown = fs::read_to_string(&path).map_err(|e| format!("读画像失败: {e}"))?;
    let (quiet_hours, brief_hour) = parse_quiet(&markdown);
    Ok(Profile {
        expertise: section_items(&markdown, "专长"),
        projects: section_items(&markdown, "在做的项目"),
        answer_style: section_text(&markdown, "回答偏好"),
        path: path.to_string_lossy().to_string(),
        updated_at: fs::metadata(&path)
            .and_then(|m| m.modified())
            .map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339())
            .unwrap_or_else(|_| now_iso()),
        interests: list_interests(64).unwrap_or_default(),
        markdown,
        quiet_hours,
        brief_hour,
    })
}

/// 读画像：markdown 为准，兴趣表合并了手工项与派生项。
pub fn load_profile() -> Result<Profile, String> {
    let path = ensure_profile_file()?;
    let markdown = fs::read_to_string(&path).map_err(|e| format!("读画像失败: {e}"))?;
    let (quiet_hours, brief_hour) = parse_quiet(&markdown);
    let manual: Vec<String> = section_items(&markdown, "关注");

    // markdown 里的手工项即时同步进派生表（权重固定 1.0，且不参与衰减）。
    sync_manual_interests(&manual)?;

    let expertise = section_items(&markdown, "专长");
    let projects = section_items(&markdown, "在做的项目");
    let answer_style = section_text(&markdown, "回答偏好");
    let updated_at = fs::metadata(&path)
        .and_then(|m| m.modified())
        .map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339())
        .unwrap_or_else(|_| now_iso());

    Ok(Profile {
        markdown,
        path: path.to_string_lossy().to_string(),
        interests: list_interests(64)?,
        expertise,
        projects,
        answer_style,
        quiet_hours,
        brief_hour,
        updated_at,
    })
}

/// 写画像（前端保存按钮）。写完立刻重建派生层。
pub fn save_profile(markdown: &str) -> Result<Profile, String> {
    let path = profile_path()?;
    fs::write(&path, markdown).map_err(|e| format!("写画像失败: {e}"))?;
    let manual = section_items(markdown, "关注");
    sync_manual_interests(&manual)?;
    invalidate_ivec_cache();
    load_profile()
}

/// 「不要提醒我」黑名单。命中即不产出 nudge。
pub fn mute_list() -> Vec<String> {
    let Ok(p) = profile_path() else { return Vec::new() };
    let Ok(md) = fs::read_to_string(p) else { return Vec::new() };
    section_items(&md, "不要提醒我")
        .into_iter()
        .map(|s| s.to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 把 markdown 的手工兴趣同步进 interests 表。
/// 已不在 markdown 里的手工项降级为 derived（保留学到的权重，但从此可衰减）。
fn sync_manual_interests(manual: &[String]) -> Result<(), String> {
    let conn = ensure_db()?;
    let wanted: Vec<String> = manual
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    for topic in &wanted {
        conn.execute(
            "INSERT INTO interests(topic,weight,source,hits,last_seen) VALUES(?1,1.0,'manual',0,?2) \
             ON CONFLICT(topic) DO UPDATE SET source='manual', weight=1.0",
            params![topic, now_iso()],
        )
        .map_err(|e| e.to_string())?;
    }
    // 降级：曾是 manual 但已从文件删掉的
    let mut stmt = conn
        .prepare("SELECT topic FROM interests WHERE source='manual'")
        .map_err(|e| e.to_string())?;
    let existing: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .filter_map(|x| x.ok())
        .collect();
    drop(stmt);
    for t in existing {
        if !wanted.contains(&t) {
            let _ = conn.execute(
                "UPDATE interests SET source='derived', weight=MIN(weight,0.6) WHERE topic=?1",
                params![t],
            );
        }
    }
    invalidate_ivec_cache();
    Ok(())
}

/// 列出兴趣（按权重降序）。
pub fn list_interests(limit: usize) -> Result<Vec<Interest>, String> {
    let conn = ensure_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT topic,weight,source,hits,last_seen,vec IS NOT NULL \
             FROM interests ORDER BY weight DESC, hits DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit as i64], |r| {
            Ok(Interest {
                topic: r.get(0)?,
                weight: r.get(1)?,
                source: r.get(2)?,
                hits: r.get(3)?,
                last_seen: r.get(4)?,
                embedded: r.get::<_, i64>(5)? == 1,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|x| x.ok()).collect())
}

/// 感知锚定到某节点后，把该节点的标签作为主题信号强化进画像。
///
/// 强化幅度受 `MAX_REINFORCE_STEP` 限制，并且对已有权重做递减增益
/// （越高的权重越难继续上涨），避免单一主题吞掉整个画像。
pub fn reinforce_from_node(kb_name: &str, node_id: &str, salience: f64) {
    let Some(node) = crate::commands::kgraph::kg_node_of(kb_name, node_id) else {
        return;
    };
    let mut topics: Vec<String> = node.tags.iter().map(|t| t.trim().to_lowercase()).collect();
    // 没有标签时退回节点名（章节标题往往就是主题）。
    if topics.is_empty() {
        let n = node.name.trim().to_lowercase();
        if n.chars().count() >= 2 && n.chars().count() <= 40 {
            topics.push(n);
        }
    }
    if topics.is_empty() {
        return;
    }
    let Ok(conn) = ensure_db() else { return };
    for topic in topics.into_iter().take(4) {
        if topic.is_empty() {
            continue;
        }
        let current: f64 = conn
            .query_row(
                "SELECT weight FROM interests WHERE topic=?1",
                params![topic],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten()
            .unwrap_or(0.0);
        let step = (MAX_REINFORCE_STEP * salience * (1.0 - current)).max(0.0);
        let next = (current + step).clamp(0.0, 1.0);
        let _ = conn.execute(
            "INSERT INTO interests(topic,weight,source,hits,last_seen) VALUES(?1,?2,'derived',1,?3) \
             ON CONFLICT(topic) DO UPDATE SET \
               weight=CASE WHEN source='manual' THEN weight ELSE ?2 END, \
               hits=hits+1, last_seen=?3",
            params![topic, next, now_iso()],
        );
    }
    invalidate_ivec_cache();
}

/// 每日衰减：派生兴趣按半衰期缩小，手工项不动。低于阈值的直接清掉。
pub fn decay_interests() -> Result<usize, String> {
    let conn = ensure_db()?;
    let factor = 0.5f64.powf(1.0 / DECAY_HALFLIFE_DAYS);
    let n = conn
        .execute(
            "UPDATE interests SET weight = weight * ?1 WHERE source<>'manual'",
            params![factor],
        )
        .map_err(|e| e.to_string())?;
    let _ = conn.execute(
        "DELETE FROM interests WHERE source<>'manual' AND weight < 0.02 AND hits < 3",
        [],
    );
    invalidate_ivec_cache();
    Ok(n)
}

/// 给还没有向量的兴趣主题补 embedding（后台，本地模型）。
pub async fn ensure_interest_vectors(limit: usize) -> Result<usize, String> {
    let pending: Vec<String> = {
        let conn = ensure_db()?;
        let mut stmt = conn
            .prepare("SELECT topic FROM interests WHERE vec IS NULL ORDER BY weight DESC LIMIT ?1")
            .map_err(|e| e.to_string())?;
        let list: Vec<String> = stmt
            .query_map(params![limit.clamp(1, 128) as i64], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        list
    };
    if pending.is_empty() {
        return Ok(0);
    }
    let vecs = crate::commands::embedding::embed_texts(&pending, false).await?;
    let conn = ensure_db()?;
    let mut n = 0usize;
    for (topic, v) in pending.iter().zip(vecs.iter()) {
        if conn
            .execute(
                "UPDATE interests SET vec=?2 WHERE topic=?1",
                params![topic, vec_to_blob(v)],
            )
            .is_ok()
        {
            n += 1;
        }
    }
    invalidate_ivec_cache();
    Ok(n)
}

fn load_ivecs(dim: usize) -> Option<()> {
    if let Some(c) = IVEC_CACHE.read().as_ref() {
        if c.dim == dim {
            return Some(());
        }
    }
    let conn = ensure_db().ok()?;
    let mut stmt = conn
        .prepare("SELECT topic,weight,vec FROM interests WHERE vec IS NOT NULL")
        .ok()?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?, r.get::<_, Vec<u8>>(2)?))
        })
        .ok()?;
    let mut topics = Vec::new();
    let mut weights = Vec::new();
    let mut data: Vec<f32> = Vec::new();
    for row in rows.filter_map(|x| x.ok()) {
        let v = blob_to_vec(&row.2);
        if v.len() != dim {
            continue;
        }
        topics.push(row.0);
        weights.push(row.1);
        data.extend_from_slice(&v);
    }
    *IVEC_CACHE.write() = Some(InterestVecs { topics, weights, data, dim });
    Some(())
}

/// 向量与画像的亲和度 0~1：对各兴趣取"余弦 × 权重"的最大值。
///
/// 乘权重而不是只看余弦，是为了让"我很关心但只沾一点"和"我不关心却很像"区分开。
pub fn interest_affinity(vec: &[f32]) -> f32 {
    if vec.is_empty() {
        return 0.0;
    }
    if load_ivecs(vec.len()).is_none() {
        return 0.0;
    }
    let guard = IVEC_CACHE.read();
    let Some(c) = guard.as_ref() else { return 0.0 };
    if c.dim != vec.len() || c.topics.is_empty() {
        return 0.0;
    }
    let mut best = 0.0f32;
    for i in 0..c.topics.len() {
        let row = &c.data[i * c.dim..(i + 1) * c.dim];
        let dot: f32 = row.iter().zip(vec).map(|(a, b)| a * b).sum();
        let scaled = dot.max(0.0) * c.weights[i] as f32;
        if scaled > best {
            best = scaled;
        }
    }
    best.clamp(0.0, 1.0)
}

/// 与向量最亲和的兴趣主题名（用于 nudge 的 topic 字段与去重）。
pub fn best_topic(vec: &[f32]) -> Option<String> {
    if vec.is_empty() || load_ivecs(vec.len()).is_none() {
        return None;
    }
    let guard = IVEC_CACHE.read();
    let c = guard.as_ref()?;
    if c.dim != vec.len() {
        return None;
    }
    let mut best: Option<(usize, f32)> = None;
    for i in 0..c.topics.len() {
        let row = &c.data[i * c.dim..(i + 1) * c.dim];
        let dot: f32 = row.iter().zip(vec).map(|(a, b)| a * b).sum();
        let scaled = dot.max(0.0) * c.weights[i] as f32;
        if best.map(|(_, s)| scaled > s).unwrap_or(true) {
            best = Some((i, scaled));
        }
    }
    best.filter(|(_, s)| *s > 0.15).map(|(i, _)| c.topics[i].clone())
}

/// 某主题是否属于用户自述的专长（缺口提醒要对专长降权）。
pub fn is_expertise(topic: &str) -> bool {
    let Ok(p) = profile_path() else { return false };
    let Ok(md) = fs::read_to_string(p) else { return false };
    let t = topic.to_lowercase();
    section_items(&md, "专长")
        .iter()
        .any(|e| !e.is_empty() && (t.contains(&e.to_lowercase()) || e.to_lowercase().contains(&t)))
}

/// 主题命中「不要提醒我」黑名单。
pub fn is_muted(text: &str) -> bool {
    let t = text.to_lowercase();
    mute_list().iter().any(|m| !m.is_empty() && t.contains(m))
}

/// 供系统提示词拼接的画像摘要。刻意短——它每轮对话都要进上下文。
pub fn prompt_snippet() -> String {
    let Ok(p) = load_profile() else { return String::new() };
    let mut parts: Vec<String> = Vec::new();
    let top: Vec<String> = p
        .interests
        .iter()
        .filter(|i| i.weight >= 0.3)
        .take(8)
        .map(|i| i.topic.clone())
        .collect();
    if !top.is_empty() {
        parts.push(format!("用户长期关注：{}", top.join("、")));
    }
    if !p.projects.is_empty() {
        parts.push(format!("在做的项目：{}", p.projects.join("、")));
    }
    if !p.expertise.is_empty() {
        parts.push(format!("已熟悉（无需从头解释）：{}", p.expertise.join("、")));
    }
    if !p.answer_style.is_empty() {
        parts.push(format!("回答偏好：{}", p.answer_style));
    }
    parts.join("\n")
}

/// 从 markdown 重建派生层（删库或手工大改后调用）。
pub fn rederive_from_markdown() -> Result<usize, String> {
    let path = ensure_profile_file()?;
    let md = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let conn = ensure_db()?;
    conn.execute("DELETE FROM interests WHERE source='manual'", [])
        .map_err(|e| e.to_string())?;
    let manual = section_items(&md, "关注");
    sync_manual_interests(&manual)?;
    Ok(manual.len())
}

/// 手工增删一条派生兴趣（设置页的快捷操作）。删除返回受影响行数。
pub fn upsert_interest(topic: &str, weight: f64) -> Result<(), String> {
    let t = topic.trim().to_lowercase();
    if t.is_empty() {
        return Err("主题不能为空".into());
    }
    let conn = ensure_db()?;
    conn.execute(
        "INSERT INTO interests(topic,weight,source,hits,last_seen) VALUES(?1,?2,'derived',0,?3) \
         ON CONFLICT(topic) DO UPDATE SET weight=?2, last_seen=?3",
        params![t, weight.clamp(0.0, 1.0), now_iso()],
    )
    .map_err(|e| e.to_string())?;
    invalidate_ivec_cache();
    Ok(())
}

pub fn remove_interest(topic: &str) -> Result<usize, String> {
    let conn = ensure_db()?;
    let n = conn
        .execute(
            "DELETE FROM interests WHERE topic=?1",
            params![topic.trim().to_lowercase()],
        )
        .map_err(|e| e.to_string())?;
    invalidate_ivec_cache();
    if n == 0 {
        warn!("[cognition] 删除兴趣未命中: {}", topic);
    }
    Ok(n)
}

/// 主题权重查表（nudge 打分特征）。
pub fn topic_weight(topic: &str) -> f64 {
    let Ok(conn) = ensure_db() else { return 0.0 };
    conn.query_row(
        "SELECT weight FROM interests WHERE topic=?1",
        params![topic.trim().to_lowercase()],
        |r| r.get::<_, f64>(0),
    )
    .optional()
    .ok()
    .flatten()
    .unwrap_or(0.0)
}
