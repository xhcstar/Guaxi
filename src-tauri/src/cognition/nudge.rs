//! 主动层：审议循环产出提醒（nudge），治理器决定**是否/何时/以何强度**投递。
//!
//! 治理器是这一层的重点。产出提醒不难，难的是不变成垃圾邮件。三道闸：
//! 1. **硬预算**：每天/每小时的打断次数是令牌桶，用完就只能进看板与简报。
//! 2. **免打扰**：画像里的时段、以及「不要提醒我」黑名单。
//! 3. **自适应阈值**：每条提醒的展示/忽略/采纳都是标签，训一个几十参数的
//!    逻辑回归；接受率下滑，阈值自动抬高。不需要 LLM，完全可解释。
//!
//! 通道按侵入性递进：`board`（看板，永远可用）→ `brief`（简报，绝大多数提醒的归宿）
//! → `notify`（系统通知）→ `interrupt`（应用内打断卡片，只给最高效用）。

use crate::cognition::store::{ensure_db, now_iso, today};
use crate::cognition::{belief, profile};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// 提醒种类。顺序即默认优先级。
pub const KINDS: [&str; 6] = ["contradiction", "gap", "due", "review", "emerging", "stale"];

/// 每天最多几次真打断（notify + interrupt 合计）。看板与简报不计入。
const DEFAULT_MAX_INTERRUPTS_PER_DAY: i64 = 3;
/// 每小时最多几次。
const DEFAULT_MAX_INTERRUPTS_PER_HOUR: i64 = 1;
/// 审议本身的每日 token 预算（belief 判决 + 缺口摘要）。
const DEFAULT_DAILY_TOKEN_BUDGET: i64 = 60_000;

/// 提醒引用的对象。
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NudgeRef {
    /// kg_node | percept | belief | task | file
    pub kind: String,
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub line: Option<i64>,
    #[serde(default)]
    pub uri: Option<String>,
}

/// 一条提醒。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Nudge {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    /// 证据链：为什么现在拿这件事打扰你。每条都是人话。
    pub why: Vec<String>,
    pub refs: Vec<NudgeRef>,
    pub kb_name: Option<String>,
    pub topic: Option<String>,
    pub utility: f64,
    pub cost: f64,
    pub channel: String,
    pub created_at: String,
    pub valid_until: Option<String>,
    pub state: String,
    pub delivered_at: Option<String>,
    pub policy_score: f64,
}

/// 待插入的提醒（未定通道、未打策略分）。
#[derive(Debug, Clone)]
pub struct NudgeDraft {
    pub kind: String,
    pub title: String,
    pub body: String,
    pub why: Vec<String>,
    pub refs: Vec<NudgeRef>,
    pub kb_name: Option<String>,
    pub topic: Option<String>,
    /// 产出方给的基础效用 0~1，治理器会再乘策略分。
    pub utility: f64,
    /// 处理这条提醒大致要花用户多少精力 0~1（用于成本敏感排序）。
    pub cost: f64,
    /// 去重键：同一件事重复产出时只留一条。
    pub dedup_key: String,
    /// 有效期天数；过期自动 expired。
    pub valid_days: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeliberateReport {
    pub produced: usize,
    pub deduped: usize,
    pub by_kind: HashMap<String, usize>,
    pub delivered: usize,
    pub skipped_budget: usize,
    pub skipped_quiet: usize,
    pub skipped_threshold: usize,
    pub tokens_spent: i64,
    pub elapsed_ms: u64,
    /// 审议是否因为预算耗尽而提前收尾。
    pub budget_exhausted: bool,
}

/// 特征向量：策略模型的输入。刻意全部是可解释的标量。
fn features_of(d: &NudgeDraft) -> HashMap<String, f64> {
    let hour = chrono::Local::now().format("%H").to_string().parse::<f64>().unwrap_or(12.0);
    let topic_w = d.topic.as_deref().map(profile::topic_weight).unwrap_or(0.0);
    let mut f = HashMap::new();
    f.insert("utility".to_string(), d.utility);
    f.insert("cost".to_string(), d.cost);
    f.insert("topic_weight".to_string(), topic_w);
    f.insert("ref_count".to_string(), (d.refs.len() as f64 / 5.0).min(1.0));
    f.insert("hour_norm".to_string(), hour / 23.0);
    f.insert("is_workhour".to_string(), if (9.0..19.0).contains(&hour) { 1.0 } else { 0.0 });
    for k in KINDS {
        f.insert(format!("kind_{k}"), if d.kind == k { 1.0 } else { 0.0 });
    }
    f.insert("recent_accept".to_string(), recent_accept_rate(14));
    f
}

/// 近 N 天的提醒接受率。冷启动（样本 < 5）返回 0.5，避免一开始就把阈值推到极端。
pub fn recent_accept_rate(days: i64) -> f64 {
    let Ok(conn) = ensure_db() else { return 0.5 };
    let since = (chrono::Local::now() - chrono::Duration::days(days.max(1))).to_rfc3339();
    let (yes, total): (i64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(label),0), COUNT(*) FROM nudge_feedback WHERE at>=?1",
            params![since],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    if total < 5 {
        return 0.5;
    }
    (yes as f64 / total as f64).clamp(0.0, 1.0)
}

/// 逻辑回归打分。权重存 `policy.nudge_gate`，没有训练过时返回 utility 本身。
pub fn policy_score(features: &HashMap<String, f64>) -> (f64, i64) {
    let Ok(conn) = ensure_db() else {
        return (features.get("utility").copied().unwrap_or(0.5), 0);
    };
    let row: Option<(String, f64, i64)> = conn
        .query_row(
            "SELECT weights,bias,version FROM policy WHERE name='nudge_gate'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .ok()
        .flatten();
    let Some((json, bias, version)) = row else {
        return (features.get("utility").copied().unwrap_or(0.5), 0);
    };
    let Ok(w) = serde_json::from_str::<HashMap<String, f64>>(&json) else {
        return (features.get("utility").copied().unwrap_or(0.5), 0);
    };
    let mut z = bias;
    for (k, v) in features {
        z += w.get(k).copied().unwrap_or(0.0) * v;
    }
    (1.0 / (1.0 + (-z).exp()), version)
}

/// 当前生效的投递阈值。
///
/// 基线 0.5，按近期接受率**反向**微调：用户最近很少采纳 → 抬高门槛少打扰；
/// 采纳率高 → 略微放宽。夹在 [0.35, 0.85]，避免自我强化到极端。
pub fn delivery_threshold() -> f64 {
    let base = crate::cognition::store::get_f64("nudge_threshold_base", 0.5);
    let accept = recent_accept_rate(14);
    (base + (0.5 - accept) * 0.4).clamp(0.35, 0.85)
}

fn max_interrupts_per_day() -> i64 {
    crate::cognition::store::get_i64("max_interrupts_per_day", DEFAULT_MAX_INTERRUPTS_PER_DAY)
}

fn max_interrupts_per_hour() -> i64 {
    crate::cognition::store::get_i64("max_interrupts_per_hour", DEFAULT_MAX_INTERRUPTS_PER_HOUR)
}

fn daily_token_budget() -> i64 {
    crate::cognition::store::get_i64("daily_token_budget", DEFAULT_DAILY_TOKEN_BUDGET)
}

/// 令牌桶：今天/这一小时还剩几次打断。
fn interrupt_budget_left() -> (i64, i64) {
    let Ok(conn) = ensure_db() else { return (0, 0) };
    let day_used: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM delivery_log WHERE channel IN ('notify','interrupt') AND at>=?1",
            params![format!("{}T00:00:00", today())],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let hour_ago = (chrono::Local::now() - chrono::Duration::hours(1)).to_rfc3339();
    let hour_used: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM delivery_log WHERE channel IN ('notify','interrupt') AND at>=?1",
            params![hour_ago],
            |r| r.get(0),
        )
        .unwrap_or(0);
    (
        (max_interrupts_per_day() - day_used).max(0),
        (max_interrupts_per_hour() - hour_used).max(0),
    )
}

/// 插入一条草稿。命中去重键返回 false（不算失败）。
pub fn insert(draft: &NudgeDraft) -> Result<bool, String> {
    // 黑名单：标题、正文、主题任一命中「不要提醒我」就整条丢掉。
    let probe = format!(
        "{} {} {}",
        draft.title,
        draft.body,
        draft.topic.clone().unwrap_or_default()
    );
    if profile::is_muted(&probe) {
        debug!("[cognition] 提醒命中黑名单，丢弃: {}", draft.title);
        return Ok(false);
    }
    let conn = ensure_db()?;
    let features = features_of(draft);
    let (score, version) = policy_score(&features);
    let valid_until = (chrono::Local::now() + chrono::Duration::days(draft.valid_days.max(1)))
        .to_rfc3339();
    let id = uuid::Uuid::new_v4().to_string();

    let n = conn
        .execute(
            "INSERT INTO nudges(id,kind,title,body,why,refs,kb_name,topic,utility,cost,channel,\
             created_at,valid_until,state,dedup_key,features,policy_score,policy_version) \
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'board',?11,?12,'pending',?13,?14,?15,?16) \
             ON CONFLICT(dedup_key) DO NOTHING",
            params![
                id,
                draft.kind,
                draft.title,
                draft.body,
                serde_json::to_string(&draft.why).unwrap_or_else(|_| "[]".into()),
                serde_json::to_string(&draft.refs).unwrap_or_else(|_| "[]".into()),
                draft.kb_name,
                draft.topic,
                draft.utility.clamp(0.0, 1.0),
                draft.cost.clamp(0.0, 1.0),
                now_iso(),
                valid_until,
                draft.dedup_key,
                serde_json::to_string(&features).unwrap_or_else(|_| "{}".into()),
                score,
                version
            ],
        )
        .map_err(|e| format!("写入提醒失败: {e}"))?;
    Ok(n == 1)
}

/// 产出①矛盾：新材料反驳了知识库里的既有主张。
///
/// 这是最高价值的一类——效用给到 0.9 起，因为"你相信的事情不成立了"
/// 几乎总比"有篇新文章"更值得知道。
async fn produce_contradictions(modelid: i32, report: &mut DeliberateReport) {
    let found = match belief::detect_contradictions(modelid, 4).await {
        Ok(v) => v,
        Err(e) => {
            debug!("[cognition] 矛盾检测跳过: {}", e);
            return;
        }
    };
    for c in found {
        let mut refs = vec![NudgeRef {
            kind: "belief".into(),
            id: c.belief_id.clone(),
            label: c.statement.clone(),
            path: c.source_rel_path.clone(),
            line: Some(c.source_line),
            uri: None,
        }];
        refs.push(NudgeRef {
            kind: "percept".into(),
            id: c.percept_id.clone(),
            label: c.percept_title.clone(),
            path: None,
            line: None,
            uri: c.percept_uri.clone(),
        });
        let draft = NudgeDraft {
            kind: "contradiction".into(),
            title: format!("有新材料反驳了你的一条结论"),
            body: format!(
                "你记下的结论：{}\n\n反驳它的材料：{}\n\n模型给的理由：{}",
                c.statement, c.percept_title, c.reason
            ),
            why: vec![
                format!("这条结论出自 {}", c.source_rel_path.clone().unwrap_or_else(|| "知识库".into())),
                format!("新材料与它主题相似度 {:.2}，判定置信 {:.2}", c.similarity, c.confidence),
                "冲突是唯一一类不处理就会让知识库持续错下去的问题".to_string(),
            ],
            refs,
            kb_name: Some(c.kb_name.clone()),
            topic: None,
            utility: (0.9 * c.confidence).clamp(0.5, 1.0),
            cost: 0.4,
            dedup_key: format!("contradiction:{}:{}", c.belief_id, c.percept_id),
            valid_days: 30,
        };
        record_draft(draft, report);
    }
}

fn record_draft(draft: NudgeDraft, report: &mut DeliberateReport) {
    let kind = draft.kind.clone();
    match insert(&draft) {
        Ok(true) => {
            report.produced += 1;
            *report.by_kind.entry(kind).or_insert(0) += 1;
        }
        Ok(false) => report.deduped += 1,
        Err(e) => warn!("[cognition] 提醒入库失败: {}", e),
    }
}

/// 一次审议里各库的图快照。
///
/// 为什么要它：缺口 / 复习 / 停滞三个产出器都要遍历节点，如果各自去查
/// `kg_query_graph(max_depth=99, limit=100k)`，一轮审议就会把整张图克隆三遍——
/// 十万节点的库上这是每 5 分钟几百 MB 的无谓分配。所以每轮只查一次、共享给三者。
type GraphSnapshots = Vec<(String, crate::commands::kgraph::KgGraphData)>;

fn take_graph_snapshots() -> GraphSnapshots {
    let mut out: GraphSnapshots = Vec::new();
    for kb in crate::commands::kgraph::list_kb_names() {
        match crate::commands::kgraph::kg_query_graph(
            kb.clone(),
            Some(crate::commands::kgraph::KgQueryOpts {
                max_depth: Some(99),
                limit: Some(100_000),
                node_types: Some(vec!["section".into(), "file".into(), "concept".into()]),
                ..Default::default()
            }),
        ) {
            Ok(g) => out.push((kb, g)),
            Err(e) => debug!("[cognition] 取图快照失败 kb={}: {}", kb, e),
        }
    }
    out
}

/// 产出②结构缺口：这个主题你读得多、却没有一处把它串起来。
///
/// 判据是图上的结构信号，不需要模型：概念/章节节点 PageRank 高（被反复提到）
/// 但 degree 低（没有连接）。对用户自述的专长主题跳过——不要教人已经会的东西。
fn produce_gaps(snaps: &GraphSnapshots, report: &mut DeliberateReport) {
    for (kb, graph) in snaps {
        let kb = kb.clone();
        let structural: Vec<&crate::commands::kgraph::MemNode> = graph
            .nodes
            .iter()
            .filter(|n| n.node_type == "section" || n.node_type == "concept")
            .collect();
        if structural.len() < 20 {
            continue; // 库太小，谈不上结构缺口
        }
        let mean_pr: f64 =
            structural.iter().map(|n| n.pagerank).sum::<f64>() / structural.len() as f64;
        let mut cands: Vec<_> = structural
            .iter()
            .copied()
            .filter(|n| n.pagerank > mean_pr * 2.0 && n.degree <= 2)
            .filter(|n| !profile::is_expertise(&n.name))
            .collect();
        cands.sort_by(|a, b| b.pagerank.partial_cmp(&a.pagerank).unwrap_or(std::cmp::Ordering::Equal));

        for node in cands.into_iter().take(2) {
            let draft = NudgeDraft {
                kind: "gap".into(),
                title: format!("「{}」被反复提到，却没有一处把它串起来", node.name),
                body: format!(
                    "这个节点在图谱里的重要度排在前面（PageRank {:.4}，是全库均值的 {:.1} 倍），\
                     但只有 {} 条连接。通常意味着你在它周边读了不少东西，却还没写一篇把它们连起来的笔记。",
                    node.pagerank,
                    node.pagerank / mean_pr.max(1e-9),
                    node.degree
                ),
                why: vec![
                    format!("重要度 {:.4} > 全库均值 {:.4} 的两倍", node.pagerank, mean_pr),
                    format!("连接数只有 {}", node.degree),
                    "结构缺口不会自己消失，它会让相关材料一直散着".to_string(),
                ],
                refs: vec![NudgeRef {
                    kind: "kg_node".into(),
                    id: node.id.clone(),
                    label: node.name.clone(),
                    path: node.file_path.clone().or_else(|| node.path.clone()),
                    line: node.start_line,
                    uri: None,
                }],
                kb_name: Some(kb.clone()),
                topic: Some(node.name.to_lowercase()),
                utility: 0.55,
                cost: 0.7,
                dedup_key: format!("gap:{}:{}", kb, node.id),
                valid_days: 21,
            };
            record_draft(draft, report);
        }
    }
}

/// 产出③到期任务 + 它需要的材料。
///
/// 单纯"任务到期"org 模式自己就会提醒，这里的增量价值在于**把材料一起端上来**：
/// 对最紧的一条任务做一次召回，附上最相关的三份东西。
async fn produce_due(report: &mut DeliberateReport) {
    let today_str = today();
    let soon = (chrono::Local::now() + chrono::Duration::days(2))
        .format("%Y-%m-%d")
        .to_string();
    let tasks = match crate::commands::taskmanager::task_query(
        crate::commands::taskmanager::TaskQuery {
            category: None,
            status: None,
            done: Some(false),
            date_from: Some(today_str.clone()),
            date_to: Some(soon.clone()),
            keyword: None,
        },
    ) {
        Ok(t) => t,
        Err(e) => {
            debug!("[cognition] 任务查询跳过: {}", e);
            return;
        }
    };
    let mut due: Vec<_> = tasks
        .into_iter()
        .filter(|t| !t.done && !t.deadline.trim().is_empty())
        .collect();
    due.sort_by(|a, b| a.deadline.cmp(&b.deadline));
    if due.is_empty() {
        return;
    }

    let overdue = due.iter().filter(|t| t.deadline.as_str() < today_str.as_str()).count();
    let top = &due[0];
    let query = if top.description.trim().is_empty() {
        top.id.clone()
    } else {
        top.description.clone()
    };
    let mut refs = vec![NudgeRef {
        kind: "task".into(),
        id: if top.internal_id.is_empty() { top.id.clone() } else { top.internal_id.clone() },
        label: format!("{} · {}", top.id, query.chars().take(50).collect::<String>()),
        path: None,
        line: None,
        uri: None,
    }];
    // 只对最紧的一条做召回：把 token 花在最可能被点开的地方。
    if let Ok(cites) = crate::cognition::recall::recall_brief(&query, None, 3).await {
        for c in cites {
            refs.push(NudgeRef {
                kind: c.ref_kind.clone(),
                id: c.ref_id.clone(),
                label: c.label.clone(),
                path: c.path.clone(),
                line: c.line,
                uri: None,
            });
        }
    }
    let draft = NudgeDraft {
        kind: "due".into(),
        title: if overdue > 0 {
            format!("{} 条任务已过期，{} 条即将到期", overdue, due.len() - overdue)
        } else {
            format!("{} 条任务在两天内到期", due.len())
        },
        body: format!(
            "最紧的一条：{}（截止 {}，状态 {}）。\n\n下面附上与它最相关的材料，点开即可继续。",
            query.chars().take(120).collect::<String>(),
            top.deadline,
            top.status
        ),
        why: vec![
            format!("截止日期 {} 落在今天到 {} 之间", top.deadline, soon),
            format!("共 {} 条未完成任务命中该窗口", due.len()),
        ],
        refs,
        kb_name: None,
        topic: None,
        utility: if overdue > 0 { 0.8 } else { 0.6 },
        cost: 0.5,
        dedup_key: format!("due:{}:{}", today_str, top.deadline),
        valid_days: 2,
    };
    record_draft(draft, report);
}

/// 产出④遗忘曲线复习：把语言学习里已经验证过的 SRS 泛化到知识库章节。
///
/// 排期用 SM-2 的简化版（ease 2.5 起，记住则乘 ease，忘了回到 1 天）。
/// 种子来自"写下超过 7 天、且从未复习过"的章节——自己写的东西才值得复习。
fn produce_review(snaps: &GraphSnapshots, report: &mut DeliberateReport) {
    let Ok(conn) = ensure_db() else { return };
    let today_str = today();

    // 1) 补种子
    for (kb, graph) in snaps {
        let cutoff = (chrono::Local::now() - chrono::Duration::days(7)).to_rfc3339();
        for node in graph.nodes.iter().filter(|n| n.node_type == "section").take(2000) {
            let old_enough = node
                .modified_at
                .as_deref()
                .map(|m| m < cutoff.as_str())
                .unwrap_or(false);
            if !old_enough {
                continue;
            }
            let _ = conn.execute(
                "INSERT INTO review_state(node_id,kb_name,label,rel_path,line,due_date) \
                 VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(node_id) DO NOTHING",
                params![
                    node.id,
                    kb,
                    node.name,
                    node.rel_path,
                    node.start_line.unwrap_or(0),
                    today_str
                ],
            );
        }
    }

    // 2) 取今天到期的前 3 条，打包成一条提醒（不要一天发三条）
    let due: Vec<(String, String, String, Option<String>, i64, i64)> = {
        let Ok(mut stmt) = conn.prepare(
            "SELECT node_id,kb_name,label,rel_path,line,reps FROM review_state \
             WHERE due_date<=?1 ORDER BY reps ASC, due_date ASC LIMIT 3",
        ) else {
            return;
        };
        let list: Vec<_> = stmt
            .query_map(params![today_str], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        list
    };
    if due.is_empty() {
        return;
    }
    let refs: Vec<NudgeRef> = due
        .iter()
        .map(|(id, _kb, label, path, line, _)| NudgeRef {
            kind: "kg_node".into(),
            id: id.clone(),
            label: label.clone(),
            path: path.clone(),
            line: Some(*line),
            uri: None,
        })
        .collect();
    let names: Vec<String> = due.iter().map(|d| d.2.clone()).collect();
    let draft = NudgeDraft {
        kind: "review".into(),
        title: format!("{} 处笔记到复习点了", due.len()),
        body: format!(
            "按遗忘曲线排期，今天该回看这几处：\n- {}\n\n点开确认「还记得」会把间隔拉长，「忘了」会重新排到明天。",
            names.join("\n- ")
        ),
        why: vec![
            "这些章节是你自己写的，且已过初次巩固窗口".to_string(),
            format!("复习次数最少的排在最前（当前 {} 次）", due[0].5),
        ],
        refs,
        kb_name: Some(due[0].1.clone()),
        topic: None,
        utility: 0.45,
        cost: 0.3,
        dedup_key: format!("review:{}", today_str),
        valid_days: 1,
    };
    record_draft(draft, report);
}

/// 复习反馈：记住 → 间隔 × ease；忘了 → 回到 1 天并降 ease。
pub fn review_feedback(node_id: &str, remembered: bool) -> Result<(), String> {
    let conn = ensure_db()?;
    let row: Option<(f64, i64, i64, i64)> = conn
        .query_row(
            "SELECT ease,interval_days,reps,lapses FROM review_state WHERE node_id=?1",
            params![node_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (ease, interval, reps, lapses) = row.ok_or_else(|| "该节点未在复习队列中".to_string())?;
    let (next_ease, next_interval, next_reps, next_lapses) = if remembered {
        (
            (ease + 0.1).min(3.0),
            ((interval as f64 * ease).round() as i64).clamp(1, 365),
            reps + 1,
            lapses,
        )
    } else {
        ((ease - 0.25).max(1.3), 1, reps, lapses + 1)
    };
    let due = (chrono::Local::now() + chrono::Duration::days(next_interval))
        .format("%Y-%m-%d")
        .to_string();
    conn.execute(
        "UPDATE review_state SET ease=?2,interval_days=?3,due_date=?4,reps=?5,lapses=?6,last_review=?7 \
         WHERE node_id=?1",
        params![node_id, next_ease, next_interval, due, next_reps, next_lapses, now_iso()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 产出⑤主题跃迁：一批新材料自己聚成了知识库里还不存在的主题。
///
/// 贪心聚类（阈值 0.7），≥3 条才算"正在形成"。只看高新颖度的条目——
/// 与已有知识接壤的材料属于既有主题，不构成跃迁。
fn produce_emerging(report: &mut DeliberateReport) {
    let Ok(conn) = ensure_db() else { return };
    let since = (chrono::Local::now() - chrono::Duration::days(7)).to_rfc3339();
    let rows: Vec<(String, String, f64, Vec<u8>)> = {
        let Ok(mut stmt) = conn.prepare(
            "SELECT id,COALESCE(title,''),novelty,vec FROM percepts \
             WHERE vec IS NOT NULL AND captured_at>=?1 AND novelty>=0.6 AND salience>=0.35 \
               AND state IN ('new','triaged') ORDER BY captured_at DESC LIMIT 120",
        ) else {
            return;
        };
        let list: Vec<_> = stmt
            .query_map(params![since], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map(|it| it.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        list
    };
    if rows.len() < 3 {
        return;
    }
    let vecs: Vec<(usize, Vec<f32>)> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (i, crate::cognition::store::blob_to_vec(&r.3)))
        .filter(|(_, v)| !v.is_empty())
        .collect();
    let dim = vecs.first().map(|(_, v)| v.len()).unwrap_or(0);
    if dim == 0 {
        return;
    }

    const CLUSTER_SIM: f32 = 0.70;
    let mut assigned = vec![false; vecs.len()];
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    for i in 0..vecs.len() {
        if assigned[i] || vecs[i].1.len() != dim {
            continue;
        }
        let mut group = vec![i];
        assigned[i] = true;
        for j in (i + 1)..vecs.len() {
            if assigned[j] || vecs[j].1.len() != dim {
                continue;
            }
            let dot: f32 = vecs[i].1.iter().zip(&vecs[j].1).map(|(a, b)| a * b).sum();
            if dot >= CLUSTER_SIM {
                group.push(j);
                assigned[j] = true;
            }
        }
        if group.len() >= 3 {
            clusters.push(group);
        }
    }
    clusters.sort_by_key(|g| std::cmp::Reverse(g.len()));

    for group in clusters.into_iter().take(1) {
        let members: Vec<&(String, String, f64, Vec<u8>)> =
            group.iter().map(|&gi| &rows[vecs[gi].0]).collect();
        let titles: Vec<String> = members
            .iter()
            .map(|m| {
                if m.1.trim().is_empty() {
                    "（无标题）".to_string()
                } else {
                    m.1.chars().take(60).collect()
                }
            })
            .collect();
        let avg_novelty: f64 = members.iter().map(|m| m.2).sum::<f64>() / members.len() as f64;
        let topic = crate::cognition::store::blob_to_vec(&members[0].3);
        let topic_name = profile::best_topic(&topic);
        let refs: Vec<NudgeRef> = members
            .iter()
            .map(|m| NudgeRef {
                kind: "percept".into(),
                id: m.0.clone(),
                label: if m.1.trim().is_empty() { m.0.clone() } else { m.1.clone() },
                path: None,
                line: None,
                uri: None,
            })
            .collect();
        let key_titles: String = titles.join("|");
        let draft = NudgeDraft {
            kind: "emerging".into(),
            title: format!("一个新主题正在形成（{} 条材料）", members.len()),
            body: format!(
                "最近 7 天进来的这几条互相很像，但在你的知识库里找不到落点：\n- {}\n\n\
                 平均新颖度 {:.2}。要不要为它开一篇笔记，把它们收进去？",
                titles.join("\n- "),
                avg_novelty
            ),
            why: vec![
                format!("{} 条材料两两相似度 ≥ {:.2}", members.len(), CLUSTER_SIM),
                format!("平均新颖度 {:.2}，说明与现有知识几乎不接壤", avg_novelty),
            ],
            refs,
            kb_name: None,
            topic: topic_name,
            utility: (0.45 + 0.1 * members.len() as f64).min(0.75),
            cost: 0.6,
            dedup_key: format!("emerging:{}", crate::cognition::store::hash_hex(&key_titles)),
            valid_days: 14,
        };
        record_draft(draft, report);
    }
}

/// 产出⑥项目停滞：项目笔记很久没动，但里面还留着悬空的问题。
///
/// 只针对画像里「在做的项目」——对不在做的东西提"你很久没动了"是纯噪声。
fn produce_stale(snaps: &GraphSnapshots, report: &mut DeliberateReport) {
    let Ok(prof) = profile::load_profile() else { return };
    if prof.projects.is_empty() {
        return;
    }
    let cutoff = (chrono::Local::now() - chrono::Duration::days(14)).to_rfc3339();
    for (kb, graph) in snaps {
        for node in graph.nodes.iter().filter(|n| n.node_type == "file") {
            let name_l = node.name.to_lowercase();
            let rel_l = node.rel_path.clone().unwrap_or_default().to_lowercase();
            let matched = prof.projects.iter().find(|p| {
                let p = p.to_lowercase();
                !p.is_empty() && (name_l.contains(&p) || rel_l.contains(&p))
            });
            let Some(project) = matched else { continue };
            let stale = node
                .modified_at
                .as_deref()
                .map(|m| m < cutoff.as_str())
                .unwrap_or(false);
            if !stale {
                continue;
            }
            let body_text = crate::commands::kgraph::kg_node_text(node, 12_000);
            let open: Vec<String> = body_text
                .lines()
                .map(|l| l.trim())
                .filter(|l| {
                    l.ends_with('?')
                        || l.ends_with('？')
                        || l.contains("TODO")
                        || l.contains("待确认")
                        || l.contains("待定")
                })
                .take(4)
                .map(|l| l.chars().take(90).collect::<String>())
                .collect();
            if open.is_empty() {
                continue;
            }

            let draft = NudgeDraft {
                kind: "stale".into(),
                title: format!("「{}」停了两周，还留着 {} 个悬空问题", project, open.len()),
                body: format!(
                    "文件：{}\n最后修改：{}\n\n里面还没有结论的地方：\n- {}",
                    node.rel_path.clone().unwrap_or_else(|| node.name.clone()),
                    node.modified_at.clone().unwrap_or_else(|| "未知".into()),
                    open.join("\n- ")
                ),
                why: vec![
                    format!("「{}」在你的画像里列为在做的项目", project),
                    "超过 14 天未修改".to_string(),
                    format!("正文里有 {} 处问句或 TODO 未收口", open.len()),
                ],
                refs: vec![NudgeRef {
                    kind: "kg_node".into(),
                    id: node.id.clone(),
                    label: node.name.clone(),
                    path: node.file_path.clone().or_else(|| node.path.clone()),
                    line: Some(1),
                    uri: None,
                }],
                kb_name: Some(kb.clone()),
                topic: Some(project.to_lowercase()),
                utility: 0.5,
                cost: 0.6,
                dedup_key: format!("stale:{}:{}", kb, node.id),
                valid_days: 14,
            };
            record_draft(draft, report);
            break; // 每个库每轮最多一条停滞提醒
        }
    }
}

/// 审议一轮：跑全部产出器 → 过期清理 → 治理器投递。
///
/// 预算优先级：token 预算耗尽时**先砍要花钱的**（矛盾检测），
/// 纯结构信号的产出器（缺口/复习/停滞）始终运行，因为它们不花 token。
pub async fn deliberate(app: Option<&tauri::AppHandle>, modelid: i32) -> DeliberateReport {
    let started = std::time::Instant::now();
    let mut report = DeliberateReport::default();
    let spent_before = crate::cognition::store::tokens_spent_today(None);
    let budget = daily_token_budget();
    report.budget_exhausted = spent_before >= budget;

    let _ = expire_stale();

    // 三个纯结构产出器共享一次图快照（见 take_graph_snapshots 的注释）。
    let snaps = take_graph_snapshots();
    // 免费产出器：只读图与库，不调模型。
    produce_gaps(&snaps, &mut report);
    produce_review(&snaps, &mut report);
    produce_emerging(&mut report);
    produce_stale(&snaps, &mut report);
    drop(snaps);
    // 需要 embedding（本地，便宜）：
    produce_due(&mut report).await;
    // 需要 LLM（可能是远端，贵）：只有预算充足才跑。
    if !report.budget_exhausted {
        produce_contradictions(modelid, &mut report).await;
    } else {
        info!("[cognition] 今日 token 预算已用尽（{}/{}），跳过矛盾检测", spent_before, budget);
    }

    let delivery = deliver_pending(app);
    report.delivered = delivery.0;
    report.skipped_budget = delivery.1;
    report.skipped_quiet = delivery.2;
    report.skipped_threshold = delivery.3;
    report.tokens_spent = crate::cognition::store::tokens_spent_today(None) - spent_before;
    report.elapsed_ms = started.elapsed().as_millis() as u64;
    debug!(
        "[cognition] 审议完成 产出={} 去重={} 投递={} 用时={}ms",
        report.produced, report.deduped, report.delivered, report.elapsed_ms
    );
    report
}

/// 治理器：给待投递的提醒定通道。返回 `(投递数, 预算拦, 免打扰拦, 阈值拦)`。
///
/// 关键设计：**没有任何提醒会被丢掉**。够不上打断的一律降级到 `board`/`brief`，
/// 用户随时能在看板里看到全部。治理器只决定"要不要现在戳你一下"。
pub fn deliver_pending(app: Option<&tauri::AppHandle>) -> (usize, usize, usize, usize) {
    let Ok(conn) = ensure_db() else { return (0, 0, 0, 0) };
    let threshold = delivery_threshold();
    let quiet = profile::in_quiet_hours();
    let (mut day_left, mut hour_left) = interrupt_budget_left();

    let pending: Vec<(String, String, f64, f64, f64)> = {
        let Ok(mut stmt) = conn.prepare(
            "SELECT id,kind,utility,cost,policy_score FROM nudges \
             WHERE state='pending' ORDER BY policy_score DESC, utility DESC LIMIT 50",
        ) else {
            return (0, 0, 0, 0);
        };
        let list: Vec<_> = stmt
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .map(|it| it.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        list
    };

    let mut delivered = 0usize;
    let mut by_budget = 0usize;
    let mut by_quiet = 0usize;
    let mut by_threshold = 0usize;

    for (id, kind, utility, cost, score) in pending {
        // 效用要能覆盖成本：便宜的事情门槛低，费神的事情要更值得。
        let effective = (score * 0.7 + utility * 0.3) - cost * 0.15;
        let channel = if effective < threshold {
            by_threshold += 1;
            "brief"
        } else if quiet {
            by_quiet += 1;
            "brief"
        } else if day_left <= 0 || hour_left <= 0 {
            by_budget += 1;
            "brief"
        } else if kind == "contradiction" && effective >= threshold + 0.15 {
            // 只有矛盾这一类、且明显超阈值，才允许应用内打断。
            day_left -= 1;
            hour_left -= 1;
            "interrupt"
        } else {
            day_left -= 1;
            hour_left -= 1;
            "notify"
        };

        let is_push = channel == "notify" || channel == "interrupt";
        if conn
            .execute(
                "UPDATE nudges SET state='delivered', channel=?2, delivered_at=?3 WHERE id=?1",
                params![id, channel, now_iso()],
            )
            .is_err()
        {
            continue;
        }
        if is_push {
            let _ = conn.execute(
                "INSERT INTO delivery_log(channel,at,nudge_id) VALUES(?1,?2,?3)",
                params![channel, now_iso(), id],
            );
            delivered += 1;
        }
        // 无论通道如何都广播一次，让看板实时更新。
        if let Some(handle) = app {
            use tauri::Emitter;
            let _ = handle.emit(
                "cognition_nudge",
                serde_json::json!({ "id": id, "kind": kind, "channel": channel, "push": is_push }),
            );
        }
    }

    (delivered, by_budget, by_quiet, by_threshold)
}

/// 用户对提醒的裁决。accepted=true 记正标签，false 记负标签，两者都会训练策略。
pub fn record_decision(nudge_id: &str, accepted: bool) -> Result<(), String> {
    let conn = ensure_db()?;
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT kind, COALESCE(features,'{}') FROM nudges WHERE id=?1",
            params![nudge_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (kind, features) = row.ok_or_else(|| "提醒不存在".to_string())?;
    conn.execute(
        "UPDATE nudges SET state=?2, decided_at=?3 WHERE id=?1",
        params![
            nudge_id,
            if accepted { "accepted" } else { "dismissed" },
            now_iso()
        ],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO nudge_feedback(nudge_id,kind,label,features,at) VALUES(?1,?2,?3,?4,?5)",
        params![nudge_id, kind, if accepted { 1i64 } else { 0i64 }, features, now_iso()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 过期清理：超过 valid_until 且还没被裁决的提醒标记 expired。
pub fn expire_stale() -> Result<usize, String> {
    let conn = ensure_db()?;
    let n = conn
        .execute(
            "UPDATE nudges SET state='expired' \
             WHERE state IN ('pending','delivered') AND valid_until IS NOT NULL AND valid_until < ?1",
            params![now_iso()],
        )
        .map_err(|e| e.to_string())?;
    Ok(n)
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrainReport {
    pub samples: usize,
    pub trained: bool,
    pub version: i64,
    /// 留出集准确率。样本不足时为 0。
    pub holdout_acc: f64,
    pub previous_acc: f64,
    pub reason: String,
}

/// 训练打断策略（逻辑回归，批量梯度下降 + L2）。
///
/// 只有在留出集上不比上一版差才落库——这是"自我进化"里唯一的安全阀。
/// 样本少于 20 条时只做引导训练（不设卡），因为此时任何度量都不可信。
pub fn train_policy() -> Result<TrainReport, String> {
    let conn = ensure_db()?;
    let mut rep = TrainReport::default();
    let rows: Vec<(i64, String)> = {
        let mut stmt = conn
            .prepare("SELECT label,features FROM nudge_feedback ORDER BY at ASC")
            .map_err(|e| e.to_string())?;
        let list: Vec<_> = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|x| x.ok())
            .collect();
        list
    };
    let samples: Vec<(f64, HashMap<String, f64>)> = rows
        .into_iter()
        .filter_map(|(label, json)| {
            serde_json::from_str::<HashMap<String, f64>>(&json)
                .ok()
                .map(|f| (label as f64, f))
        })
        .collect();
    rep.samples = samples.len();
    if samples.len() < 8 {
        rep.reason = "反馈样本不足 8 条，暂不训练".into();
        return Ok(rep);
    }

    // 特征名全集（不同版本产出的特征集可能不同，取并集，缺失按 0）。
    let mut names: Vec<String> = samples
        .iter()
        .flat_map(|(_, f)| f.keys().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    names.sort();

    let split = ((samples.len() as f64) * 0.8).floor() as usize;
    let split = split.clamp(1, samples.len().saturating_sub(1).max(1));
    let (train, holdout) = samples.split_at(split);

    let mut w = vec![0.0f64; names.len()];
    let mut bias = 0.0f64;
    const EPOCHS: usize = 400;
    const LR: f64 = 0.15;
    const L2: f64 = 0.01;
    for _ in 0..EPOCHS {
        let mut gw = vec![0.0f64; names.len()];
        let mut gb = 0.0f64;
        for (y, f) in train.iter() {
            let mut z = bias;
            for (i, n) in names.iter().enumerate() {
                z += w[i] * f.get(n).copied().unwrap_or(0.0);
            }
            let p = 1.0 / (1.0 + (-z).exp());
            let err = p - y;
            for (i, n) in names.iter().enumerate() {
                gw[i] += err * f.get(n).copied().unwrap_or(0.0);
            }
            gb += err;
        }
        let m = train.len() as f64;
        for i in 0..w.len() {
            w[i] -= LR * (gw[i] / m + L2 * w[i]);
        }
        bias -= LR * gb / m;
    }

    let acc_of = |w: &[f64], bias: f64, set: &[(f64, HashMap<String, f64>)]| -> f64 {
        if set.is_empty() {
            return 0.0;
        }
        let mut ok = 0usize;
        for (y, f) in set {
            let mut z = bias;
            for (i, n) in names.iter().enumerate() {
                z += w[i] * f.get(n).copied().unwrap_or(0.0);
            }
            let p = 1.0 / (1.0 + (-z).exp());
            if (p >= 0.5) == (*y >= 0.5) {
                ok += 1;
            }
        }
        ok as f64 / set.len() as f64
    };
    rep.holdout_acc = acc_of(&w, bias, holdout);

    let prev: Option<(String, f64, i64, f64)> = conn
        .query_row(
            "SELECT weights,bias,version,metric FROM policy WHERE name='nudge_gate'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    rep.previous_acc = prev.as_ref().map(|p| p.3).unwrap_or(0.0);
    let next_version = prev.as_ref().map(|p| p.2 + 1).unwrap_or(1);

    // 评测闩：样本够多时，新版必须不比旧版差 2 个百分点以上才允许落库。
    let bootstrap = samples.len() < 20;
    if !bootstrap && rep.holdout_acc + 0.02 < rep.previous_acc {
        rep.reason = format!(
            "新版留出准确率 {:.3} 低于现版 {:.3}，已放弃（保留旧策略）",
            rep.holdout_acc, rep.previous_acc
        );
        return Ok(rep);
    }

    let weights: HashMap<String, f64> =
        names.iter().cloned().zip(w.iter().copied()).collect();
    conn.execute(
        "INSERT INTO policy(name,version,weights,bias,trained_at,samples,metric) \
         VALUES('nudge_gate',?1,?2,?3,?4,?5,?6) \
         ON CONFLICT(name) DO UPDATE SET version=?1,weights=?2,bias=?3,trained_at=?4,samples=?5,metric=?6",
        params![
            next_version,
            serde_json::to_string(&weights).unwrap_or_else(|_| "{}".into()),
            bias,
            now_iso(),
            samples.len() as i64,
            rep.holdout_acc
        ],
    )
    .map_err(|e| e.to_string())?;
    rep.trained = true;
    rep.version = next_version;
    rep.reason = if bootstrap {
        format!("引导训练（样本 {} 条，尚未设卡）", samples.len())
    } else {
        format!("已落库 v{}，留出准确率 {:.3}", next_version, rep.holdout_acc)
    };
    info!("[cognition] 打断策略 {}", rep.reason);
    Ok(rep)
}

/// 列出提醒（看板）。默认给 delivered + pending，按策略分降序。
pub fn list_nudges(
    state: Option<&str>,
    kind: Option<&str>,
    limit: usize,
) -> Result<Vec<Nudge>, String> {
    let conn = ensure_db()?;
    let limit = limit.clamp(1, 300) as i64;
    let mut sql = String::from(
        "SELECT id,kind,title,body,why,refs,kb_name,topic,utility,cost,channel,created_at,\
         valid_until,state,delivered_at,policy_score FROM nudges WHERE 1=1",
    );
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    match state.filter(|s| !s.is_empty()) {
        Some("open") => sql.push_str(" AND state IN ('pending','delivered')"),
        Some("all") => {}
        Some(s) => {
            sql.push_str(" AND state=?");
            binds.push(Box::new(s.to_string()));
        }
        None => sql.push_str(" AND state IN ('pending','delivered')"),
    }
    if let Some(k) = kind.filter(|s| !s.is_empty()) {
        sql.push_str(" AND kind=?");
        binds.push(Box::new(k.to_string()));
    }
    sql.push_str(" ORDER BY policy_score DESC, utility DESC, created_at DESC LIMIT ?");
    binds.push(Box::new(limit));

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let rows = stmt
        .query_map(refs.as_slice(), |r| {
            let why_raw: String = r.get(4)?;
            let refs_raw: String = r.get(5)?;
            Ok(Nudge {
                id: r.get(0)?,
                kind: r.get(1)?,
                title: r.get(2)?,
                body: r.get(3)?,
                why: serde_json::from_str(&why_raw).unwrap_or_default(),
                refs: serde_json::from_str(&refs_raw).unwrap_or_default(),
                kb_name: r.get(6)?,
                topic: r.get(7)?,
                utility: r.get(8)?,
                cost: r.get(9)?,
                channel: r.get(10)?,
                created_at: r.get(11)?,
                valid_until: r.get(12)?,
                state: r.get(13)?,
                delivered_at: r.get(14)?,
                policy_score: r.get(15)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|x| x.ok()).collect())
}

/// 提醒层统计（设置页与看板顶部）。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NudgeStats {
    pub open: i64,
    pub delivered_today: i64,
    pub accepted: i64,
    pub dismissed: i64,
    pub expired: i64,
    pub accept_rate_14d: f64,
    pub threshold: f64,
    pub interrupts_left_today: i64,
    pub policy_version: i64,
    pub policy_samples: i64,
    pub policy_metric: f64,
    pub tokens_today: i64,
    pub token_budget: i64,
    pub by_kind: Vec<(String, i64)>,
}

pub fn stats() -> Result<NudgeStats, String> {
    let conn = ensure_db()?;
    let mut s = NudgeStats::default();
    s.open = conn
        .query_row(
            "SELECT COUNT(*) FROM nudges WHERE state IN ('pending','delivered')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    s.delivered_today = conn
        .query_row(
            "SELECT COUNT(*) FROM delivery_log WHERE at>=?1",
            params![format!("{}T00:00:00", today())],
            |r| r.get(0),
        )
        .unwrap_or(0);
    for (field, state) in [
        (&mut s.accepted, "accepted"),
        (&mut s.dismissed, "dismissed"),
        (&mut s.expired, "expired"),
    ] {
        *field = conn
            .query_row(
                "SELECT COUNT(*) FROM nudges WHERE state=?1",
                params![state],
                |r| r.get(0),
            )
            .unwrap_or(0);
    }
    s.accept_rate_14d = recent_accept_rate(14);
    s.threshold = delivery_threshold();
    s.interrupts_left_today = interrupt_budget_left().0;
    s.tokens_today = crate::cognition::store::tokens_spent_today(None);
    s.token_budget = daily_token_budget();

    if let Some((v, n, m)) = conn
        .query_row(
            "SELECT version,samples,metric FROM policy WHERE name='nudge_gate'",
            [],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, f64>(2)?)),
        )
        .optional()
        .ok()
        .flatten()
    {
        s.policy_version = v;
        s.policy_samples = n;
        s.policy_metric = m;
    }
    if let Ok(mut stmt) = conn.prepare(
        "SELECT kind, COUNT(*) FROM nudges WHERE state IN ('pending','delivered') \
         GROUP BY kind ORDER BY 2 DESC",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))) {
            s.by_kind = rows.filter_map(|x| x.ok()).collect();
        }
    }
    Ok(s)
}

/// 回滚打断策略到"未训练"状态（阈值退回基线 + utility 直通）。
///
/// 自调系统必须有这个开关：用户一旦觉得提醒变差了，能立刻把学习成果清掉，
/// 而不是只能等它自己收敛回来。反馈样本保留，随时可以重训。
pub fn reset_policy() -> Result<(), String> {
    let conn = ensure_db()?;
    conn.execute("DELETE FROM policy WHERE name='nudge_gate'", [])
        .map_err(|e| e.to_string())?;
    let _ = conn.execute(
        "UPDATE nudges SET policy_score=utility, policy_version=0 WHERE state='pending'",
        [],
    );
    info!("[cognition] 打断策略已回滚到基线");
    Ok(())
}

/// 单条提醒详情。
pub fn get_nudge(id: &str) -> Result<Option<Nudge>, String> {
    let list = list_nudges(Some("all"), None, 300)?;
    Ok(list.into_iter().find(|n| n.id == id))
}
