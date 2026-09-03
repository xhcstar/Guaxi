//! 情节记忆：append-only 的行为轨迹。
//!
//! 一切"系统做过什么"都落这里：对话、Agent 步骤、工具调用、投递的提醒、生成的简报。
//! 它有三个下游：
//! - **召回**：相似的历史问答本身就是上下文（`episode_topk`）。
//! - **学习**：引证是否真的被用上／被点开，是检索权重与打断策略唯一的免费标签。
//! - **审计**：用户能回看系统在什么时候、依据什么、做了什么。
//!
//! 写入必须廉价且绝不失败上抛——记轨迹不能拖垮主流程，所有写入错误只记日志。

use crate::cognition::store::{blob_to_vec, ensure_db, now_iso, vec_to_blob};
use rusqlite::params;
use serde::Serialize;
use tracing::warn;

/// 一条引证：召回给出的候选，附带它来自哪一路。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Citation {
    /// kg_node | percept | episode
    pub ref_kind: String,
    pub ref_id: String,
    pub kb_name: Option<String>,
    /// 展示用标签（文件名 / 章节标题 / 文章标题）。
    pub label: String,
    /// semantic | graph | episodic | percept
    pub channel: String,
    pub rank: usize,
    pub score: f64,
    /// 供前端跳转：文件路径与行号。
    pub path: Option<String>,
    pub line: Option<i64>,
    /// 片段正文（已截断）。
    pub snippet: String,
}

/// 情节列表项（审计视图）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeRow {
    pub id: String,
    pub kind: String,
    pub session_id: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub model: Option<String>,
    pub question: Option<String>,
    pub answer_excerpt: Option<String>,
    pub kb_name: Option<String>,
    pub recall_used: i64,
    pub outcome: Option<String>,
    pub latency_ms: i64,
    pub step_count: i64,
    pub citation_count: i64,
}

/// 开一段情节。返回 id；失败时返回一个仍可用的本地 id（不阻断主流程）。
pub fn start(
    kind: &str,
    session_id: &str,
    model: &str,
    question: &str,
    kb_name: &str,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    // question 只留前 4000 字：超长问题多半带了整份文件，正文在引证里另存。
    let q: String = question.chars().take(4000).collect();
    if let Ok(conn) = ensure_db() {
        if let Err(e) = conn.execute(
            "INSERT INTO episodes(id,kind,session_id,started_at,model,question,kb_name,outcome) \
             VALUES(?1,?2,?3,?4,?5,?6,?7,'running')",
            params![id, kind, session_id, now_iso(), model, q, kb_name],
        ) {
            warn!("[cognition] 情节写入失败: {}", e);
        }
    }
    id
}

/// 收一段情节。`outcome` ∈ ok | error | cancelled。
pub fn finish(id: &str, answer: &str, outcome: &str, tokens_out: i64, latency_ms: i64) {
    if id.is_empty() {
        return;
    }
    let a: String = answer.chars().take(20_000).collect();
    if let Ok(conn) = ensure_db() {
        if let Err(e) = conn.execute(
            "UPDATE episodes SET ended_at=?2, answer=?3, outcome=?4, tokens_out=?5, latency_ms=?6 \
             WHERE id=?1",
            params![id, now_iso(), a, outcome, tokens_out, latency_ms],
        ) {
            warn!("[cognition] 情节收尾失败: {}", e);
        }
    }
    // 回填引证的 used 标记：答案里出现了引证标签 → 认为这条引证真被用上了。
    // 这是检索权重学习的核心标签，且完全免费。
    mark_used_by_answer(id, answer);
}

/// 记一步（工具调用 / 结果 / 错误 / 状态）。seq 由库内自增序号推导，调用方不用管。
pub fn add_step(episode_id: &str, step_type: &str, name: &str, payload: &str) {
    if episode_id.is_empty() {
        return;
    }
    let p: String = payload.chars().take(8000).collect();
    if let Ok(conn) = ensure_db() {
        let seq: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(seq),0)+1 FROM episode_steps WHERE episode_id=?1",
                params![episode_id],
                |r| r.get(0),
            )
            .unwrap_or(1);
        if let Err(e) = conn.execute(
            "INSERT INTO episode_steps(episode_id,seq,step_type,name,payload,at) \
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![episode_id, seq, step_type, name, p, now_iso()],
        ) {
            warn!("[cognition] 步骤写入失败: {}", e);
        }
    }
}

/// 落一批引证。
pub fn record_citations(episode_id: &str, cites: &[Citation]) {
    if episode_id.is_empty() || cites.is_empty() {
        return;
    }
    let Ok(conn) = ensure_db() else { return };
    for c in cites {
        if let Err(e) = conn.execute(
            "INSERT INTO citations(episode_id,ref_kind,ref_id,kb_name,label,channel,rank,score,at) \
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                episode_id,
                c.ref_kind,
                c.ref_id,
                c.kb_name,
                c.label,
                c.channel,
                c.rank as i64,
                c.score,
                now_iso()
            ],
        ) {
            warn!("[cognition] 引证写入失败: {}", e);
        }
    }
    let _ = conn.execute(
        "UPDATE episodes SET recall_used=?2 WHERE id=?1",
        params![episode_id, cites.len() as i64],
    );
}

/// 答案里是否引用了某条引证。
///
/// 主判据是**引证编号标记**（`[1]`、`[2]`…）：召回把候选按 rank 编号交给模型，
/// 提示词要求引用时写编号，所以标记出现即精确命中。标签名匹配只作为兜底，
/// 且要求长度 ≥6 字以避免常见词误判——这是学习信号，错判会持续把权重推歪。
fn mark_used_by_answer(episode_id: &str, answer: &str) {
    if answer.trim().is_empty() {
        return;
    }
    let Ok(conn) = ensure_db() else { return };
    let mut stmt = match conn
        .prepare("SELECT id,rank,COALESCE(label,'') FROM citations WHERE episode_id=?1")
    {
        Ok(s) => s,
        Err(_) => return,
    };
    let rows: Vec<(i64, i64, String)> = match stmt.query_map(params![episode_id], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?))
    }) {
        Ok(it) => it.filter_map(|x| x.ok()).collect(),
        Err(_) => return,
    };
    drop(stmt);
    for (cid, rank, label) in rows {
        let marker = format!("[{rank}]");
        let hit = answer.contains(&marker)
            || (label.chars().count() >= 6 && answer.contains(&label));
        if hit {
            let _ = conn.execute("UPDATE citations SET used=1 WHERE id=?1", params![cid]);
        }
    }
}

/// 用户点开了某条引证（前端上报）。这是比 `used` 更强的正标签。
pub fn mark_citation_clicked(episode_id: &str, ref_id: &str) -> Result<(), String> {
    let conn = ensure_db()?;
    conn.execute(
        "UPDATE citations SET clicked=1 WHERE episode_id=?1 AND ref_id=?2",
        params![episode_id, ref_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 各召回通路的历史命中率：`(channel, 命中数, 总数)`。
///
/// 这是检索权重自调的输入。只统计有引证记录且已收尾的情节。
pub fn channel_stats(days: i64) -> Vec<(String, i64, i64)> {
    let Ok(conn) = ensure_db() else { return Vec::new() };
    let since = (chrono::Local::now() - chrono::Duration::days(days.max(1))).to_rfc3339();
    let Ok(mut stmt) = conn.prepare(
        "SELECT channel, SUM(CASE WHEN used=1 OR clicked=1 THEN 1 ELSE 0 END), COUNT(*) \
         FROM citations WHERE at>=?1 GROUP BY channel",
    ) else {
        return Vec::new();
    };
    stmt.query_map(params![since], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
    })
    .map(|rows| rows.filter_map(|x| x.ok()).collect())
    .unwrap_or_default()
}

/// 最近的情节（审计视图）。
pub fn recent(limit: usize, kind: Option<&str>) -> Result<Vec<EpisodeRow>, String> {
    let conn = ensure_db()?;
    let limit = limit.clamp(1, 500) as i64;
    let sql = "SELECT e.id,e.kind,e.session_id,e.started_at,e.ended_at,e.model,e.question,\
               e.answer,e.kb_name,e.recall_used,e.outcome,e.latency_ms,\
               (SELECT COUNT(*) FROM episode_steps s WHERE s.episode_id=e.id),\
               (SELECT COUNT(*) FROM citations c WHERE c.episode_id=e.id) \
               FROM episodes e"
        .to_string();

    let map_row = |r: &rusqlite::Row| -> rusqlite::Result<EpisodeRow> {
        let answer: Option<String> = r.get(7)?;
        Ok(EpisodeRow {
            id: r.get(0)?,
            kind: r.get(1)?,
            session_id: r.get(2)?,
            started_at: r.get(3)?,
            ended_at: r.get(4)?,
            model: r.get(5)?,
            question: r.get(6)?,
            answer_excerpt: answer.map(|a| a.chars().take(300).collect()),
            kb_name: r.get(8)?,
            recall_used: r.get(9)?,
            outcome: r.get(10)?,
            latency_ms: r.get(11)?,
            step_count: r.get(12)?,
            citation_count: r.get(13)?,
        })
    };
    let rows = match kind.filter(|k| !k.is_empty()) {
        Some(k) => {
            let sql = format!("{sql} WHERE e.kind=?1 ORDER BY e.started_at DESC LIMIT ?2");
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let it = stmt
                .query_map(params![k, limit], map_row)
                .map_err(|e| e.to_string())?;
            it.filter_map(|x| x.ok()).collect::<Vec<_>>()
        }
        None => {
            let sql = format!("{sql} ORDER BY e.started_at DESC LIMIT ?1");
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let it = stmt
                .query_map(params![limit], map_row)
                .map_err(|e| e.to_string())?;
            it.filter_map(|x| x.ok()).collect::<Vec<_>>()
        }
    };
    Ok(rows)
}

/// 取一段情节的全部步骤（展开审计详情）。
pub fn steps(episode_id: &str) -> Result<Vec<serde_json::Value>, String> {
    let conn = ensure_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT seq,step_type,COALESCE(name,''),COALESCE(payload,''),at \
             FROM episode_steps WHERE episode_id=?1 ORDER BY seq",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![episode_id], |r| {
            Ok(serde_json::json!({
                "seq": r.get::<_, i64>(0)?,
                "type": r.get::<_, String>(1)?,
                "name": r.get::<_, String>(2)?,
                "payload": r.get::<_, String>(3)?,
                "at": r.get::<_, String>(4)?,
            }))
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|x| x.ok()).collect())
}

/// 取一段情节的引证（前端渲染引证条 + 点击回填）。
pub fn citations_of(episode_id: &str) -> Result<Vec<serde_json::Value>, String> {
    let conn = ensure_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT ref_kind,ref_id,COALESCE(kb_name,''),COALESCE(label,''),channel,rank,score,used,clicked \
             FROM citations WHERE episode_id=?1 ORDER BY rank",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![episode_id], |r| {
            Ok(serde_json::json!({
                "refKind": r.get::<_, String>(0)?,
                "refId": r.get::<_, String>(1)?,
                "kbName": r.get::<_, String>(2)?,
                "label": r.get::<_, String>(3)?,
                "channel": r.get::<_, String>(4)?,
                "rank": r.get::<_, i64>(5)?,
                "score": r.get::<_, f64>(6)?,
                "used": r.get::<_, i64>(7)? == 1,
                "clicked": r.get::<_, i64>(8)? == 1,
            }))
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|x| x.ok()).collect())
}

/// 给已收尾但还没向量的情节补 embedding（供 episodic 召回）。
///
/// 只 embed「问题 + 答案前 1500 字」：情节召回要的是"我以前问过类似的事"，
/// 全文入向量既贵又会被长答案里的枝节拉偏。
pub async fn embed_pending(limit: usize) -> Result<usize, String> {
    let pending: Vec<(String, String)> = {
        let conn = ensure_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, COALESCE(question,'')||'\n'||SUBSTR(COALESCE(answer,''),1,1500) \
                 FROM episodes WHERE vec IS NULL AND outcome='ok' AND question IS NOT NULL \
                 ORDER BY started_at DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let list: Vec<(String, String)> = stmt
            .query_map(params![limit.clamp(1, 64) as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .filter(|(_, t)| t.trim().chars().count() >= 8)
            .collect();
        list
    };
    if pending.is_empty() {
        return Ok(0);
    }

    let texts: Vec<String> = pending.iter().map(|(_, t)| t.clone()).collect();
    let vecs = crate::commands::embedding::embed_texts(&texts, false).await?;
    let conn = ensure_db()?;
    let mut n = 0usize;
    for ((id, _), v) in pending.iter().zip(vecs.iter()) {
        if conn
            .execute(
                "UPDATE episodes SET vec=?2 WHERE id=?1",
                params![id, vec_to_blob(v)],
            )
            .is_ok()
        {
            n += 1;
        }
    }
    Ok(n)
}

/// 情节召回：找相似的历史问答。返回 `(id, question, answer, score)`。
pub fn episode_topk(query: &[f32], top_k: usize) -> Result<Vec<(String, String, String, f32)>, String> {
    let conn = ensure_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id,COALESCE(question,''),COALESCE(answer,''),vec FROM episodes \
             WHERE vec IS NOT NULL AND outcome='ok' ORDER BY started_at DESC LIMIT 4000",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String, String, Vec<u8>)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|x| x.ok())
        .collect();
    let mut scored: Vec<(String, String, String, f32)> = rows
        .into_iter()
        .filter_map(|(id, q, a, blob)| {
            let v = blob_to_vec(&blob);
            if v.len() != query.len() {
                return None;
            }
            let dot: f32 = v.iter().zip(query).map(|(x, y)| x * y).sum();
            Some((id, q, a, dot))
        })
        .collect();
    scored.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);
    Ok(scored)
}

/// 情节层统计。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeStats {
    pub total: i64,
    pub with_recall: i64,
    pub embedded: i64,
    pub citation_total: i64,
    pub citation_used: i64,
    pub citation_clicked: i64,
    pub avg_latency_ms: f64,
}

pub fn stats() -> Result<EpisodeStats, String> {
    let conn = ensure_db()?;
    let mut s = EpisodeStats::default();
    s.total = conn
        .query_row("SELECT COUNT(*) FROM episodes", [], |r| r.get(0))
        .unwrap_or(0);
    s.with_recall = conn
        .query_row("SELECT COUNT(*) FROM episodes WHERE recall_used>0", [], |r| r.get(0))
        .unwrap_or(0);
    s.embedded = conn
        .query_row("SELECT COUNT(*) FROM episodes WHERE vec IS NOT NULL", [], |r| r.get(0))
        .unwrap_or(0);
    s.citation_total = conn
        .query_row("SELECT COUNT(*) FROM citations", [], |r| r.get(0))
        .unwrap_or(0);
    s.citation_used = conn
        .query_row("SELECT COUNT(*) FROM citations WHERE used=1", [], |r| r.get(0))
        .unwrap_or(0);
    s.citation_clicked = conn
        .query_row("SELECT COUNT(*) FROM citations WHERE clicked=1", [], |r| r.get(0))
        .unwrap_or(0);
    s.avg_latency_ms = conn
        .query_row(
            "SELECT COALESCE(AVG(latency_ms),0) FROM episodes WHERE latency_ms>0",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);
    Ok(s)
}

/// 保留窗口清理：只留最近 N 天的情节与步骤（引证随情节一起走）。
pub fn prune(keep_days: i64) -> Result<usize, String> {
    let conn = ensure_db()?;
    let cutoff = (chrono::Local::now() - chrono::Duration::days(keep_days.max(7))).to_rfc3339();
    let ids: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT id FROM episodes WHERE started_at < ?1")
            .map_err(|e| e.to_string())?;
        let list: Vec<String> = stmt
            .query_map(params![cutoff], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        list
    };
    for id in &ids {
        let _ = conn.execute("DELETE FROM episode_steps WHERE episode_id=?1", params![id]);
        let _ = conn.execute("DELETE FROM citations WHERE episode_id=?1", params![id]);
        let _ = conn.execute("DELETE FROM episodes WHERE id=?1", params![id]);
    }
    Ok(ids.len())
}
