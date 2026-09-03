//! 召回：把「语义 / 图 / 感知 / 情节」四路证据融合成一段带引证的上下文。
//!
//! 这是整个认知层对外最重要的一个函数。设计要点：
//! - **四路独立、RRF 融合**。倒数排名融合不需要各路分数可比，天然抗量纲差异。
//! - **权重可学**。各路权重按历史命中率（引证是否被用上/点开）微调，
//!   幅度被夹在 [0.5, 1.6]，且必须过评测闩才落库（见 `evolve`）。
//! - **每段都有出处**。返回的 `citations` 带文件路径与行号，前端可直接跳转。
//! - **不可信内容单独包裹**。percept 通路的正文一律走 `untrusted_block`。

use crate::cognition::episode::Citation;
use crate::cognition::store::ensure_db;
use crate::cognition::{episode, percept, profile};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use tracing::debug;

/// RRF 的平滑常数。60 是文献常用值：让前几名之间差距明显，尾部快速趋平。
const RRF_K: f64 = 60.0;

/// 各通路默认权重。语义为主，图补结构邻近，感知给新鲜度，情节给连续性。
const W_SEMANTIC: f64 = 1.0;
const W_GRAPH: f64 = 0.6;
const W_PERCEPT: f64 = 0.5;
const W_EPISODIC: f64 = 0.4;

/// 召回参数。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallOptions {
    #[serde(default)]
    pub kb_name: Option<String>,
    #[serde(default)]
    pub k: Option<usize>,
    #[serde(default)]
    pub max_chars: Option<usize>,
    #[serde(default)]
    pub include_percepts: Option<bool>,
    #[serde(default)]
    pub include_episodes: Option<bool>,
    #[serde(default)]
    pub graph_hops: Option<usize>,
}

impl Default for RecallOptions {
    fn default() -> Self {
        Self {
            kb_name: None,
            k: Some(8),
            max_chars: Some(6000),
            include_percepts: Some(true),
            include_episodes: Some(true),
            graph_hops: Some(1),
        }
    }
}

/// 召回结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallResult {
    pub query: String,
    pub citations: Vec<Citation>,
    /// 可直接拼进系统提示词的上下文块（已含引证编号与不可信包裹）。
    pub context_block: String,
    /// 各通路贡献的候选数，便于诊断"为什么没召回到"。
    pub channels: HashMap<String, usize>,
    pub weights: HashMap<String, f64>,
    pub elapsed_ms: u64,
    /// true 表示 embedding 服务不可用，已降级为纯关键词（此时 citations 可能为空）。
    pub degraded: bool,
}

/// 融合中的候选。
#[derive(Debug, Clone)]
struct Candidate {
    ref_kind: String,
    ref_id: String,
    kb_name: Option<String>,
    label: String,
    path: Option<String>,
    line: Option<i64>,
    /// 各通路给它的名次（1-based）。
    ranks: Vec<(String, usize, f64)>,
    fused: f64,
    text: String,
}

/// 读取当前生效的通路权重。缺省值 × 学习到的调整系数。
///
/// 调整系数存 `policy` 表的 `recall_weights`，由 `evolve::tune_recall_weights` 写入，
/// 而后者只有在评测集上不退化时才会落库。所以这里读到的一定是验过的。
pub fn channel_weights() -> HashMap<String, f64> {
    let mut w: HashMap<String, f64> = HashMap::from([
        ("semantic".to_string(), W_SEMANTIC),
        ("graph".to_string(), W_GRAPH),
        ("percept".to_string(), W_PERCEPT),
        ("episodic".to_string(), W_EPISODIC),
    ]);
    let Ok(conn) = ensure_db() else { return w };
    let stored: Option<String> = conn
        .query_row(
            "SELECT weights FROM policy WHERE name='recall_weights'",
            [],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    if let Some(json) = stored {
        if let Ok(map) = serde_json::from_str::<HashMap<String, f64>>(&json) {
            for (k, v) in map {
                if let Some(slot) = w.get_mut(&k) {
                    *slot = v.clamp(0.1, 3.0);
                }
            }
        }
    }
    w
}

/// 候选节点类型：只召可定位到具体内容的层级。
/// 目录与库根不进召回——它们没有正文，只会挤掉真正有内容的段落。
const RECALL_NODE_TYPES: [&str; 4] = ["section", "paragraph", "file", "concept"];

fn key_of(kind: &str, id: &str) -> String {
    format!("{kind}::{id}")
}

/// 倒数排名融合。**唯一**的融合实现——召回与评测共用它，
/// 否则调参调的是一套公式、生效的是另一套。
pub fn rrf_fuse(ranks: &[(String, usize)], weights: &HashMap<String, f64>) -> f64 {
    ranks
        .iter()
        .map(|(ch, rank)| {
            let w = weights.get(ch).copied().unwrap_or(0.5);
            w / (RRF_K + *rank as f64)
        })
        .sum()
}

/// 主入口。失败时返回 degraded 结果而不是 Err——召回不可用不应该让对话失败。
pub async fn recall(query: &str, opts: RecallOptions) -> RecallResult {
    let started = Instant::now();
    let k = opts.k.unwrap_or(8).clamp(1, 40);
    let max_chars = opts.max_chars.unwrap_or(6000).clamp(500, 60_000);
    let hops = opts.graph_hops.unwrap_or(1).min(3);
    let weights = channel_weights();
    let mut channels: HashMap<String, usize> = HashMap::new();
    let q = query.trim();

    if q.chars().count() < 2 {
        return RecallResult {
            query: q.to_string(),
            citations: Vec::new(),
            context_block: String::new(),
            channels,
            weights,
            elapsed_ms: started.elapsed().as_millis() as u64,
            degraded: false,
        };
    }

    // 唯一一次 embedding 调用，四路共用。
    let qv = match crate::commands::embedding::embed_texts(&[q.to_string()], true).await {
        Ok(v) => v.into_iter().next().unwrap_or_default(),
        Err(e) => {
            debug!("[cognition] 召回降级（embedding 不可用）: {}", e);
            return RecallResult {
                query: q.to_string(),
                citations: Vec::new(),
                context_block: String::new(),
                channels,
                weights,
                elapsed_ms: started.elapsed().as_millis() as u64,
                degraded: true,
            };
        }
    };

    let kbs: Vec<String> = match opts.kb_name.as_deref().filter(|s| !s.is_empty()) {
        Some(kb) => vec![kb.to_string()],
        None => crate::commands::kgraph::list_kb_names(),
    };
    let mut pool: HashMap<String, Candidate> = HashMap::new();

    // ── 第 1 路：语义。每库各取 k*3，交给融合层去重排序。
    let mut anchors: Vec<(String, String)> = Vec::new(); // (kb, node_id)
    let types = Some(RECALL_NODE_TYPES.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    for kb in &kbs {
        let hits = match crate::commands::kgraph::kg_vec_topk(kb, &qv, k * 3, types.clone()) {
            Ok(h) => h,
            Err(e) => {
                debug!("[cognition] 语义通路跳过 kb={}: {}", kb, e);
                continue;
            }
        };
        for (rank, (node, score)) in hits.into_iter().enumerate() {
            if rank < 3 {
                anchors.push((kb.clone(), node.id.clone()));
            }
            let key = key_of("kg_node", &node.id);
            let text = crate::commands::kgraph::kg_node_text(&node, 1200);
            let entry = pool.entry(key).or_insert_with(|| Candidate {
                ref_kind: "kg_node".into(),
                ref_id: node.id.clone(),
                kb_name: Some(kb.clone()),
                label: node.name.clone(),
                path: node.file_path.clone().or_else(|| node.path.clone()),
                line: node.start_line,
                ranks: Vec::new(),
                fused: 0.0,
                text,
            });
            entry.ranks.push(("semantic".into(), rank + 1, score as f64));
            *channels.entry("semantic".into()).or_insert(0) += 1;
        }
    }

    // ── 第 2 路：图。从语义锚点出发扩 n 跳，按 PageRank 排序。
    //
    // 图这一路的价值不是"更像"，而是"结构上相邻却措辞不同"——
    // 前置知识、被引用的定义、同一章节的兄弟段落，向量常常抓不到。
    if hops > 0 {
        for (kb, anchor) in anchors.iter() {
            let sub = match crate::commands::kgraph::kg_node_neighbors(
                kb.clone(),
                anchor.clone(),
                Some(hops as i64),
                Some(24),
                None,
            ) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let mut nodes: Vec<_> = sub
                .nodes
                .into_iter()
                .filter(|n| n.id != *anchor && RECALL_NODE_TYPES.contains(&n.node_type.as_str()))
                .collect();
            nodes.sort_by(|a, b| {
                b.pagerank
                    .partial_cmp(&a.pagerank)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for (rank, node) in nodes.into_iter().take(8).enumerate() {
                let key = key_of("kg_node", &node.id);
                let text = crate::commands::kgraph::kg_node_text(&node, 1200);
                let entry = pool.entry(key).or_insert_with(|| Candidate {
                    ref_kind: "kg_node".into(),
                    ref_id: node.id.clone(),
                    kb_name: Some(kb.clone()),
                    label: node.name.clone(),
                    path: node.file_path.clone().or_else(|| node.path.clone()),
                    line: node.start_line,
                    ranks: Vec::new(),
                    fused: 0.0,
                    text,
                });
                entry.ranks.push(("graph".into(), rank + 1, node.pagerank));
                *channels.entry("graph".into()).or_insert(0) += 1;
            }
        }
    }

    // ── 第 3 路：感知。收件箱里还没入库的新材料——"我刚看到的东西"。
    if opts.include_percepts.unwrap_or(true) {
        match percept::percept_topk(&qv, k, 0.2, None) {
            Ok(hits) => {
                for (rank, (id, title, text, score)) in hits.into_iter().enumerate() {
                    let label = if title.trim().is_empty() {
                        format!("感知 {}", &id[..8.min(id.len())])
                    } else {
                        title.clone()
                    };
                    let key = key_of("percept", &id);
                    let entry = pool.entry(key).or_insert_with(|| Candidate {
                        ref_kind: "percept".into(),
                        ref_id: id.clone(),
                        kb_name: None,
                        label,
                        path: None,
                        line: None,
                        ranks: Vec::new(),
                        fused: 0.0,
                        text: text.chars().take(1200).collect(),
                    });
                    entry.ranks.push(("percept".into(), rank + 1, score as f64));
                    *channels.entry("percept".into()).or_insert(0) += 1;
                }
            }
            Err(e) => debug!("[cognition] 感知通路跳过: {}", e),
        }
    }

    // ── 第 4 路：情节。相似的历史问答，给对话连续性。
    if opts.include_episodes.unwrap_or(true) {
        if let Ok(hits) = episode::episode_topk(&qv, 3) {
            for (rank, (id, question, answer, score)) in hits.into_iter().enumerate() {
                let key = key_of("episode", &id);
                let entry = pool.entry(key).or_insert_with(|| Candidate {
                    ref_kind: "episode".into(),
                    ref_id: id.clone(),
                    kb_name: None,
                    label: format!("历史问答：{}", question.chars().take(40).collect::<String>()),
                    path: None,
                    line: None,
                    ranks: Vec::new(),
                    fused: 0.0,
                    text: format!("问：{}\n答：{}", question, answer.chars().take(800).collect::<String>()),
                });
                entry.ranks.push(("episodic".into(), rank + 1, score as f64));
                *channels.entry("episodic".into()).or_insert(0) += 1;
            }
        }
    }
    finalize(q, pool, k, max_chars, channels, weights, started)
}

/// RRF 融合 + 预算切分 + 上下文装配。
fn finalize(
    query: &str,
    pool: HashMap<String, Candidate>,
    k: usize,
    max_chars: usize,
    channels: HashMap<String, usize>,
    weights: HashMap<String, f64>,
    started: Instant,
) -> RecallResult {
    let mut items: Vec<Candidate> = pool.into_values().collect();
    for c in items.iter_mut() {
        // 倒数排名融合：同一条被多路命中会自然叠加，这正是我们想要的"多方印证"。
        let ranks: Vec<(String, usize)> =
            c.ranks.iter().map(|(ch, r, _)| (ch.clone(), *r)).collect();
        c.fused = rrf_fuse(&ranks, &weights);
    }
    items.sort_by(|a, b| b.fused.partial_cmp(&a.fused).unwrap_or(std::cmp::Ordering::Equal));
    items.truncate(k);

    // 预算按名次递减切分：靠前的引证拿更多字符，尾部只留摘要级片段。
    let per = (max_chars / k.max(1)).max(180);
    let mut citations: Vec<Citation> = Vec::with_capacity(items.len());
    let mut blocks: Vec<String> = Vec::with_capacity(items.len());
    let mut used_chars = 0usize;

    for (i, c) in items.iter().enumerate() {
        let rank = i + 1;
        let allow = if rank <= 3 { per + per / 2 } else { per };
        let remaining = max_chars.saturating_sub(used_chars);
        if remaining < 120 {
            break;
        }
        let take = allow.min(remaining);
        let snippet: String = c.text.chars().take(take).collect();
        used_chars += snippet.chars().count();

        let origin = c
            .ranks
            .iter()
            .map(|(ch, _, _)| ch.as_str())
            .collect::<Vec<_>>()
            .join("+");
        let where_at = match (&c.path, c.line) {
            (Some(p), Some(l)) => format!("{p}:{l}"),
            (Some(p), None) => p.clone(),
            _ => c.ref_kind.clone(),
        };

        // percept 来自外部，必须包在不可信块里；知识库与情节是用户自己的内容，直出。
        let body = if c.ref_kind == "percept" {
            percept::untrusted_block(&c.label, &snippet)
        } else {
            snippet.clone()
        };
        blocks.push(format!(
            "[{rank}] {} 〔{}〕\n来源：{}\n{}",
            c.label, origin, where_at, body
        ));

        citations.push(Citation {
            ref_kind: c.ref_kind.clone(),
            ref_id: c.ref_id.clone(),
            kb_name: c.kb_name.clone(),
            label: c.label.clone(),
            channel: c.ranks.first().map(|(ch, _, _)| ch.clone()).unwrap_or_default(),
            rank,
            score: c.fused,
            path: c.path.clone(),
            line: c.line,
            snippet,
        });
    }

    let context_block = if blocks.is_empty() {
        String::new()
    } else {
        format!(
            "以下是从用户自己的知识库与感知收件箱里召回的资料，按相关度排序。\n\
             引用其中内容时**必须**写出对应编号（如 [1]）；资料不足以回答时如实说明，不要编造。\n\n{}",
            blocks.join("\n\n")
        )
    };

    RecallResult {
        query: query.to_string(),
        citations,
        context_block,
        channels,
        weights,
        elapsed_ms: started.elapsed().as_millis() as u64,
        degraded: false,
    }
}

/// 对话链路的一步到位入口：召回 → 落引证 → 返回可直接前置到问题上的接地块。
///
/// 返回空串表示这一轮没有可用证据（新库、embedding 未就绪、或问题太短），
/// 调用方应当**原样**走原有链路——召回失败不允许改变对话行为。
pub async fn ground_question(
    episode_id: &str,
    question: &str,
    kb_name: Option<String>,
) -> (String, RecallResult) {
    let opts = RecallOptions { kb_name, ..Default::default() };
    let res = recall(question, opts).await;
    if !res.citations.is_empty() && !episode_id.is_empty() {
        episode::record_citations(episode_id, &res.citations);
    }
    if res.context_block.is_empty() {
        return (String::new(), res);
    }
    let mut parts = vec![res.context_block.clone()];
    let snip = profile::prompt_snippet();
    if !snip.is_empty() {
        parts.push(format!("关于提问者：\n{snip}"));
    }
    (
        format!("{}\n\n────────\n用户的问题：\n", parts.join("\n\n")),
        res,
    )
}

/// 供 MCP / Agent 工具调用的精简召回：只返回引证列表，不拼提示词。
pub async fn recall_brief(
    query: &str,
    kb_name: Option<String>,
    k: usize,
) -> Result<Vec<Citation>, String> {
    let res = recall(
        query,
        RecallOptions {
            kb_name,
            k: Some(k),
            max_chars: Some(k.saturating_mul(900).max(1200)),
            ..Default::default()
        },
    )
    .await;
    if res.degraded {
        return Err("语义检索不可用：embedding 服务未就绪".to_string());
    }
    Ok(res.citations)
}

/// 评测用：只返回各通路的**排序后 ref_id 列表**，不取正文、不拼提示词。
///
/// 调参需要对同一批候选反复换权重重排；如果每次都走完整 `recall`，
/// 一轮坐标上升就要几百次 embedding。这里把"生成候选"与"融合"分开，
/// 生成一次、融合上百次，调参才跑得起来。
pub async fn eval_channels(query: &str, kb_name: Option<&str>) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let Ok(vs) = crate::commands::embedding::embed_texts(&[query.to_string()], true).await else {
        return out;
    };
    let Some(qv) = vs.into_iter().next() else { return out };
    let kbs: Vec<String> = match kb_name.filter(|s| !s.is_empty()) {
        Some(kb) => vec![kb.to_string()],
        None => crate::commands::kgraph::list_kb_names(),
    };
    let types = Some(RECALL_NODE_TYPES.iter().map(|s| s.to_string()).collect::<Vec<_>>());

    let mut semantic: Vec<String> = Vec::new();
    let mut anchors: Vec<(String, String)> = Vec::new();
    for kb in &kbs {
        if let Ok(hits) = crate::commands::kgraph::kg_vec_topk(kb, &qv, 24, types.clone()) {
            for (i, (node, _)) in hits.into_iter().enumerate() {
                if i < 3 {
                    anchors.push((kb.clone(), node.id.clone()));
                }
                semantic.push(node.id);
            }
        }
    }
    out.push(("semantic".into(), semantic));

    let mut graph: Vec<String> = Vec::new();
    for (kb, anchor) in anchors {
        if let Ok(sub) =
            crate::commands::kgraph::kg_node_neighbors(kb, anchor.clone(), Some(1), Some(24), None)
        {
            let mut nodes: Vec<_> = sub
                .nodes
                .into_iter()
                .filter(|n| n.id != anchor && RECALL_NODE_TYPES.contains(&n.node_type.as_str()))
                .collect();
            nodes.sort_by(|a, b| {
                b.pagerank.partial_cmp(&a.pagerank).unwrap_or(std::cmp::Ordering::Equal)
            });
            graph.extend(nodes.into_iter().take(8).map(|n| n.id));
        }
    }
    out.push(("graph".into(), graph));

    let percepts = percept::percept_topk(&qv, 8, 0.2, None)
        .map(|v| v.into_iter().map(|(id, _, _, _)| id).collect())
        .unwrap_or_default();
    out.push(("percept".into(), percepts));

    let eps = episode::episode_topk(&qv, 3)
        .map(|v| v.into_iter().map(|(id, _, _, _)| id).collect())
        .unwrap_or_default();
    out.push(("episodic".into(), eps));
    out
}

/// 判断某问题是否值得接地。太短、纯寒暄、或明显与知识无关的不必浪费一次召回。
pub fn should_ground(question: &str) -> bool {
    let q = question.trim();
    let n = q.chars().count();
    if n < 4 || n > 4000 {
        return false;
    }
    const SMALL_TALK: [&str; 10] = [
        "你好", "hi", "hello", "谢谢", "thanks", "在吗", "早上好", "晚上好", "ok", "好的",
    ];
    let lower = q.to_lowercase();
    if SMALL_TALK.iter().any(|s| lower == *s) {
        return false;
    }
    true
}
