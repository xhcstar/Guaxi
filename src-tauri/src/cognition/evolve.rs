//! 进化闩：自调参数在**冻结评测集**上验过才允许落库，且一切可回滚。
//!
//! 没有这一层，"自我进化"和"静默退化"在外部看来完全一样。所以这里提供三件东西：
//! 1. **评测集自动挖掘**：从用户自己的知识库里挖 (问题, 标准出处) 对。
//!    一个章节由模型生成一个只能靠该章节回答的问题，章节本身就是标准答案。
//! 2. **评测运行**：hit@1 / hit@5 / MRR。候选生成一次、融合多次，调参才跑得动。
//! 3. **坐标上升调参 + 门禁**：只有 MRR 不退化才写 `policy.recall_weights`，
//!    并且每一版都留在 `eval_runs` 里，可一键回滚。
//!
//! 评测集一旦生成就标记 `frozen=1`。**不允许**用调参当天的数据反过来改评测集——
//! 那就是自己给自己出题、自己打分。

use crate::cognition::store::{ensure_db, now_iso};
use crate::cognition::recall;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MineReport {
    pub kb_name: String,
    pub scanned: usize,
    pub added: usize,
    pub failures: usize,
    pub total_items: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EvalRun {
    pub id: String,
    pub at: String,
    pub target: String,
    pub version: i64,
    pub params: String,
    pub n: usize,
    pub hit_at_1: f64,
    pub hit_at_5: f64,
    pub mrr: f64,
    pub accepted: bool,
    pub note: String,
}

fn mine_prompt(title: &str, body: &str) -> String {
    format!(
        r#"根据下面这段笔记，写**一个**问题，要求：
1. 只有读过这段笔记才能准确回答，问题里不能出现"这段""上文"之类的指代。
2. 用用户自己会说的话来问，像真人提问，不要像考题。
3. 不要把答案写进问题里。
4. 不超过 40 字，中文。

只输出问题本身，不要引号、不要解释。

标题：{title}

正文：
{body}"#,
        title = title,
        body = body.chars().take(2500).collect::<String>()
    )
}

/// 从知识库挖评测集。每个章节一条，已挖过的不重复。
pub async fn mine_eval_set(
    kb_name: &str,
    modelid: i32,
    target_n: usize,
) -> Result<MineReport, String> {
    let mut rep = MineReport { kb_name: kb_name.to_string(), ..Default::default() };
    let target_n = target_n.clamp(1, 300);

    let existing: std::collections::HashSet<String> = {
        let conn = ensure_db()?;
        let mut stmt = conn
            .prepare("SELECT gold_ref FROM eval_items WHERE kb_name=?1")
            .map_err(|e| e.to_string())?;
        let set: std::collections::HashSet<String> = stmt
            .query_map(params![kb_name], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        set
    };

    let graph = crate::commands::kgraph::kg_query_graph(
        kb_name.to_string(),
        Some(crate::commands::kgraph::KgQueryOpts {
            max_depth: Some(99),
            limit: Some(100_000),
            node_types: Some(vec!["section".into()]),
            ..Default::default()
        }),
    )?;

    let mut targets: Vec<_> = graph
        .nodes
        .into_iter()
        .filter(|n| n.node_type == "section" && !existing.contains(&n.id))
        .collect();
    // 按重要度取样，让评测集覆盖库里真正被引用的部分。
    targets.sort_by(|a, b| {
        b.pagerank.partial_cmp(&a.pagerank).unwrap_or(std::cmp::Ordering::Equal)
    });
    targets.truncate(target_n);

    let conn = ensure_db()?;
    for node in targets {
        let body = crate::commands::kgraph::kg_node_text(&node, 2500);
        if body.chars().count() < 120 {
            continue; // 太短的章节出不了有区分度的问题
        }
        rep.scanned += 1;
        let prompt = mine_prompt(&node.name, &body);
        let question = match crate::commands::chatview::kg_llm_complete(
            modelid,
            &prompt,
            &format!("cog-eval-{}", node.id),
        )
        .await
        {
            Ok((a, _)) => a.trim().trim_matches('"').trim_matches('「').trim_matches('」').to_string(),
            Err(e) => {
                warn!("[cognition] 评测集挖掘失败 node={}: {}", node.id, e);
                rep.failures += 1;
                continue;
            }
        };
        crate::cognition::store::record_tokens("eval_mine", (question.chars().count() / 3) as i64);
        // 只留一行、长度合理的问题；模型偶尔会返回一整段解释。
        let question = question.lines().next().unwrap_or("").trim().to_string();
        if question.chars().count() < 6 || question.chars().count() > 120 {
            rep.failures += 1;
            continue;
        }
        let id = uuid::Uuid::new_v4().to_string();
        if conn
            .execute(
                "INSERT INTO eval_items(id,kb_name,question,gold_ref,gold_label,created_at,frozen) \
                 VALUES(?1,?2,?3,?4,?5,?6,1)",
                params![id, kb_name, question, node.id, node.name, now_iso()],
            )
            .is_ok()
        {
            rep.added += 1;
        }
    }
    rep.total_items = conn
        .query_row("SELECT COUNT(*) FROM eval_items", [], |r| r.get(0))
        .unwrap_or(0);
    info!(
        "[cognition] 评测集挖掘 kb={} 扫描={} 新增={} 失败={} 总量={}",
        kb_name, rep.scanned, rep.added, rep.failures, rep.total_items
    );
    Ok(rep)
}

/// 一条已展开候选的评测样本。
struct Case {
    gold: String,
    channels: Vec<(String, Vec<String>)>,
}

/// 收集评测样本的候选（每条只跑一次 embedding + 检索）。
async fn collect_cases(limit: usize) -> Result<Vec<Case>, String> {
    let items: Vec<(String, String, String)> = {
        let conn = ensure_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT question,gold_ref,kb_name FROM eval_items WHERE frozen=1 \
                 ORDER BY created_at LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let list: Vec<_> = stmt
            .query_map(params![limit.clamp(1, 500) as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        list
    };
    if items.is_empty() {
        return Err("评测集为空：先运行 cognition_mine_eval_set".to_string());
    }
    let mut cases = Vec::with_capacity(items.len());
    for (question, gold, kb) in items {
        let channels = recall::eval_channels(&question, Some(&kb)).await;
        if channels.iter().all(|(_, ids)| ids.is_empty()) {
            continue; // 该库还没建向量，跳过而不是记成 0 分
        }
        cases.push(Case { gold, channels });
    }
    Ok(cases)
}

/// 按给定权重给一批样本打分。返回 `(hit@1, hit@5, MRR)`。
fn score(cases: &[Case], weights: &HashMap<String, f64>) -> (f64, f64, f64) {
    if cases.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut h1 = 0usize;
    let mut h5 = 0usize;
    let mut rr = 0.0f64;
    for case in cases {
        // 汇总每个 ref_id 在各通路里的名次
        let mut ranks: HashMap<String, Vec<(String, usize)>> = HashMap::new();
        for (channel, ids) in &case.channels {
            for (i, id) in ids.iter().enumerate() {
                ranks
                    .entry(id.clone())
                    .or_default()
                    .push((channel.clone(), i + 1));
            }
        }
        let mut scored: Vec<(String, f64)> = ranks
            .into_iter()
            .map(|(id, rs)| (id, recall::rrf_fuse(&rs, weights)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(pos) = scored.iter().position(|(id, _)| *id == case.gold) {
            if pos == 0 {
                h1 += 1;
            }
            if pos < 5 {
                h5 += 1;
            }
            rr += 1.0 / (pos as f64 + 1.0);
        }
    }
    let n = cases.len() as f64;
    (h1 as f64 / n, h5 as f64 / n, rr / n)
}

fn save_run(run: &EvalRun) {
    if let Ok(conn) = ensure_db() {
        let _ = conn.execute(
            "INSERT INTO eval_runs(id,at,target,version,params,n,hit_at_1,hit_at_5,mrr,accepted,note) \
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                run.id, run.at, run.target, run.version, run.params, run.n as i64,
                run.hit_at_1, run.hit_at_5, run.mrr,
                if run.accepted { 1i64 } else { 0i64 }, run.note
            ],
        );
    }
}

/// 用**当前生效权重**跑一次评测（不改任何东西）。这是"现在到底多准"的答案。
pub async fn run_eval(limit: usize) -> Result<EvalRun, String> {
    let cases = collect_cases(limit).await?;
    let weights = recall::channel_weights();
    let (h1, h5, mrr) = score(&cases, &weights);
    let run = EvalRun {
        id: uuid::Uuid::new_v4().to_string(),
        at: now_iso(),
        target: "recall_weights".into(),
        version: current_weights_version(),
        params: serde_json::to_string(&weights).unwrap_or_default(),
        n: cases.len(),
        hit_at_1: h1,
        hit_at_5: h5,
        mrr,
        accepted: true,
        note: "基线评测（未改动参数）".into(),
    };
    save_run(&run);
    Ok(run)
}

fn current_weights_version() -> i64 {
    let Ok(conn) = ensure_db() else { return 0 };
    conn.query_row(
        "SELECT version FROM policy WHERE name='recall_weights'",
        [],
        |r| r.get::<_, i64>(0),
    )
    .optional()
    .ok()
    .flatten()
    .unwrap_or(0)
}

/// 坐标上升调参 + 门禁。
///
/// 逐个通路在候选倍率里试，取 MRR 最高的组合；最后只有比现行权重**严格更好**
/// （MRR 提升 ≥ 0.005）才写库。提升不足就什么都不改并记一条 accepted=0 的评测记录，
/// 让用户能看到"系统试过，没找到更好的"。
pub async fn tune_recall_weights(limit: usize) -> Result<EvalRun, String> {
    let cases = collect_cases(limit).await?;
    if cases.len() < 20 {
        return Err(format!(
            "评测样本只有 {} 条，少于 20 条时调参结果不可信。先扩充评测集。",
            cases.len()
        ));
    }
    let base = recall::channel_weights();
    let (_, _, base_mrr) = score(&cases, &base);

    const CANDIDATES: [f64; 7] = [0.2, 0.4, 0.6, 0.8, 1.0, 1.3, 1.6];
    const CHANNELS: [&str; 5] = ["semantic", "graph", "percept", "episodic", "memory"];
    let mut best = base.clone();
    let mut best_mrr = base_mrr;
    // 两轮坐标上升足够收敛：只有 5 个维度，且目标函数平滑。
    for _round in 0..2 {
        for ch in CHANNELS {
            let mut local_best = best.get(ch).copied().unwrap_or(0.5);
            let mut local_mrr = best_mrr;
            for cand in CANDIDATES {
                let mut trial = best.clone();
                trial.insert(ch.to_string(), cand);
                let (_, _, mrr) = score(&cases, &trial);
                if mrr > local_mrr + 1e-9 {
                    local_mrr = mrr;
                    local_best = cand;
                }
            }
            best.insert(ch.to_string(), local_best);
            best_mrr = local_mrr;
        }
    }
    let (h1, h5, mrr) = score(&cases, &best);
    let improved = mrr >= base_mrr + 0.005;
    let version = current_weights_version() + 1;

    let mut run = EvalRun {
        id: uuid::Uuid::new_v4().to_string(),
        at: now_iso(),
        target: "recall_weights".into(),
        version,
        params: serde_json::to_string(&best).unwrap_or_default(),
        n: cases.len(),
        hit_at_1: h1,
        hit_at_5: h5,
        mrr,
        accepted: improved,
        note: String::new(),
    };

    if improved {
        let conn = ensure_db()?;
        conn.execute(
            "INSERT INTO policy(name,version,weights,bias,trained_at,samples,metric) \
             VALUES('recall_weights',?1,?2,0,?3,?4,?5) \
             ON CONFLICT(name) DO UPDATE SET version=?1,weights=?2,trained_at=?3,samples=?4,metric=?5",
            params![version, run.params, now_iso(), cases.len() as i64, mrr],
        )
        .map_err(|e| e.to_string())?;
        run.note = format!(
            "已落库 v{}：MRR {:.4} → {:.4}（hit@1 {:.3}，hit@5 {:.3}，{} 条样本）",
            version, base_mrr, mrr, h1, h5, cases.len()
        );
        info!("[cognition] 召回权重 {}", run.note);
    } else {
        run.note = format!(
            "未采纳：新组合 MRR {:.4} 未超过现行 {:.4} + 0.005 的门槛，参数保持不变",
            mrr, base_mrr
        );
        debug!("[cognition] 召回权重 {}", run.note);
    }
    save_run(&run);
    Ok(run)
}

/// 回滚召回权重到内置缺省值。评测历史保留。
pub fn rollback_recall_weights() -> Result<(), String> {
    let conn = ensure_db()?;
    conn.execute("DELETE FROM policy WHERE name='recall_weights'", [])
        .map_err(|e| e.to_string())?;
    info!("[cognition] 召回权重已回滚到内置缺省");
    Ok(())
}

/// 评测历史（前端展示"每一版做了什么、好了多少"）。
pub fn list_runs(target: Option<&str>, limit: usize) -> Result<Vec<EvalRun>, String> {
    let conn = ensure_db()?;
    let limit = limit.clamp(1, 200) as i64;
    let map_row = |r: &rusqlite::Row| -> rusqlite::Result<EvalRun> {
        Ok(EvalRun {
            id: r.get(0)?,
            at: r.get(1)?,
            target: r.get(2)?,
            version: r.get(3)?,
            params: r.get(4)?,
            n: r.get::<_, i64>(5)? as usize,
            hit_at_1: r.get(6)?,
            hit_at_5: r.get(7)?,
            mrr: r.get(8)?,
            accepted: r.get::<_, i64>(9)? == 1,
            note: r.get::<_, Option<String>>(10)?.unwrap_or_default(),
        })
    };
    let cols = "id,at,target,version,params,n,hit_at_1,hit_at_5,mrr,accepted,note";
    let rows = match target.filter(|s| !s.is_empty()) {
        Some(t) => {
            let sql = format!("SELECT {cols} FROM eval_runs WHERE target=?1 ORDER BY at DESC LIMIT ?2");
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let it = stmt.query_map(params![t, limit], map_row).map_err(|e| e.to_string())?;
            it.filter_map(|x| x.ok()).collect::<Vec<_>>()
        }
        None => {
            let sql = format!("SELECT {cols} FROM eval_runs ORDER BY at DESC LIMIT ?1");
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let it = stmt.query_map(params![limit], map_row).map_err(|e| e.to_string())?;
            it.filter_map(|x| x.ok()).collect::<Vec<_>>()
        }
    };
    Ok(rows)
}

/// 评测集规模（前端提示"样本够不够"）。
pub fn eval_set_size() -> i64 {
    let Ok(conn) = ensure_db() else { return 0 };
    conn.query_row("SELECT COUNT(*) FROM eval_items WHERE frozen=1", [], |r| r.get(0))
        .unwrap_or(0)
}
