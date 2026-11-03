//! 轻量引证排序器：用「引证是否真的被答案用上 / 被点开」训练一个逻辑回归，
//! 在召回融合之后对候选做一次小幅重排。
//!
//! 三条纪律：
//! 1. **标签免费且现成**：`citations.used`（答案里出现引证编号）与 `clicked`（用户点开）
//!    是召回链路本来就在写的两个标签，不需要额外标注。
//! 2. **门禁先于落库**：按时间切出后 20% 做冻结留出集，模型 AUC 必须同时
//!    ≥ 0.55 且比「按融合分排序」高 0.02，才写入 policy 表；否则只报告不生效。
//! 3. **只做微调**：最终排序键是 `fused × (0.4 + 1.2×p)`，模型的全部影响力被夹在这一小块里。
//!    模型坏了最差也就是回到接近 RRF 的顺序，不会把明显更相关的资料踢出上下文。

use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use tracing::{debug, info};

use crate::cognition::store::{ensure_db, now_iso};

/// 特征维度。改这里要同步 `features()` 与训练/应用的 JSON 长度校验。
pub const FEATURE_DIM: usize = 18;

const POLICY_NAME: &str = "citation_ranker";
/// 训练门禁。
const MIN_SAMPLES: usize = 80;
const MIN_POSITIVES: usize = 12;
const MIN_EVAL_SIDE: usize = 5;
const MIN_AUC: f64 = 0.55;
const MIN_AUC_GAIN: f64 = 0.02;
/// L2 与学习率。
const L2: f64 = 0.01;
const LR: f64 = 0.5;
const EPOCHS: usize = 400;
/// 自动试训的最小样本与间隔。
const AUTO_MIN_LABELED: i64 = 120;
const AUTO_INTERVAL_SECS: i64 = 7 * 86400;

/// 构造特征所需的原始输入。全部来自召回候选，不额外查询正文。
#[derive(Debug, Clone, Default)]
pub struct FeatureInput {
    pub fused: f64,
    pub best_rank: usize,
    pub channel_count: usize,
    /// semantic / graph / percept / episodic / memory 的名次，0 表示未命中。
    pub channel_ranks: [usize; 5],
    pub ref_kind: String,
    pub text_len: usize,
    pub has_kb: bool,
    pub pagerank: f64,
    pub trust: f64,
    pub salience: f64,
    pub recency_days: f64,
}

/// 特征向量：全部压到 0..1，训练时无需再做标准化。
pub fn features(input: &FeatureInput) -> Vec<f64> {
    let inv = |rank: usize| if rank == 0 { 0.0 } else { 1.0 / rank as f64 };
    let kind = input.ref_kind.as_str();
    vec![
        input.fused.max(0.0),
        inv(input.best_rank),
        (input.channel_count as f64 / 5.0).clamp(0.0, 1.0),
        inv(input.channel_ranks[0]),
        inv(input.channel_ranks[1]),
        inv(input.channel_ranks[2]),
        inv(input.channel_ranks[3]),
        inv(input.channel_ranks[4]),
        if kind == "kg_node" { 1.0 } else { 0.0 },
        if kind == "percept" { 1.0 } else { 0.0 },
        if kind == "episode" { 1.0 } else { 0.0 },
        if kind == "memory" { 1.0 } else { 0.0 },
        ((input.text_len as f64 + 1.0).ln() / 12.0).clamp(0.0, 1.0),
        if input.has_kb { 1.0 } else { 0.0 },
        input.pagerank.clamp(0.0, 1.0),
        input.trust.clamp(0.0, 1.0),
        input.salience.clamp(0.0, 1.0),
        (1.0 / (1.0 + input.recency_days.max(0.0) / 30.0)).clamp(0.0, 1.0),
    ]
}

pub fn features_json(features: &[f64]) -> String {
    serde_json::to_string(features).unwrap_or_else(|_| "[]".to_string())
}

#[derive(Debug, Clone)]
pub struct RankerModel {
    pub bias: f64,
    pub weights: Vec<f64>,
    pub version: i64,
    pub metric: f64,
    pub samples: i64,
    pub trained_at: String,
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x.clamp(-60.0, 60.0)).exp())
}

impl RankerModel {
    pub fn predict(&self, features: &[f64]) -> f64 {
        if features.len() != self.weights.len() {
            return 0.5;
        }
        let mut z = self.bias;
        for (w, x) in self.weights.iter().zip(features.iter()) {
            z += w * x;
        }
        sigmoid(z)
    }
}

/// 当前生效的排序器；没有训练过就返回 None（召回完全按 RRF）。
pub fn current_model() -> Option<RankerModel> {
    let conn = ensure_db().ok()?;
    let row: Option<(String, f64, i64, f64, i64, Option<String>)> = conn
        .query_row(
            "SELECT weights,bias,version,metric,samples,trained_at FROM policy WHERE name=?1",
            params![POLICY_NAME],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .ok()
        .flatten();
    let (weights_json, bias, version, metric, samples, trained_at) = row?;
    let weights: Vec<f64> = serde_json::from_str(&weights_json).ok()?;
    if weights.len() != FEATURE_DIM {
        return None;
    }
    Some(RankerModel {
        bias,
        weights,
        version,
        metric,
        samples,
        trained_at: trained_at.unwrap_or_default(),
    })
}

/// 把融合分与模型概率合成最终排序键。
pub fn adjust_fused(fused: f64, features: &[f64], model: Option<&RankerModel>) -> f64 {
    match model {
        Some(model) => fused * (0.4 + 1.2 * model.predict(features).clamp(0.0, 1.0)),
        None => fused,
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RankerStatus {
    pub trained: bool,
    pub version: i64,
    pub metric: f64,
    pub samples: i64,
    pub trained_at: String,
    pub labeled: i64,
    pub positives: i64,
}

fn labeled_counts(conn: &rusqlite::Connection) -> (i64, i64) {
    let labeled: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM citations c JOIN episodes e ON e.id=c.episode_id \
             WHERE c.features IS NOT NULL AND e.outcome='ok'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let positives: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM citations c JOIN episodes e ON e.id=c.episode_id \
             WHERE c.features IS NOT NULL AND e.outcome='ok' AND (c.used=1 OR c.clicked=1)",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    (labeled, positives)
}

pub fn status() -> Result<RankerStatus, String> {
    let conn = ensure_db()?;
    let (labeled, positives) = labeled_counts(&conn);
    let model = current_model();
    Ok(match model {
        Some(m) => RankerStatus {
            trained: true,
            version: m.version,
            metric: m.metric,
            samples: m.samples,
            trained_at: m.trained_at,
            labeled,
            positives,
        },
        None => RankerStatus {
            trained: false,
            labeled,
            positives,
            ..Default::default()
        },
    })
}

/// 删除排序器：召回回到纯 RRF。标签与引证记录保留，可以再训。
pub fn reset() -> Result<(), String> {
    let conn = ensure_db()?;
    conn.execute("DELETE FROM policy WHERE name=?1", params![POLICY_NAME])
        .map_err(|e| e.to_string())?;
    info!("[cognition] 引证排序器已回滚（policy 行删除）");
    Ok(())
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrainReport {
    pub accepted: bool,
    pub version: i64,
    pub samples: usize,
    pub positives: usize,
    pub train_size: usize,
    pub eval_size: usize,
    pub auc: f64,
    pub auc_base: f64,
    pub note: String,
}

fn auc(scores: &[f64], labels: &[bool]) -> f64 {
    let mut idx: Vec<usize> = (0..scores.len()).collect();
    idx.sort_by(|&a, &b| {
        scores[a]
            .partial_cmp(&scores[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ranks = vec![0f64; scores.len()];
    let mut i = 0usize;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && (scores[idx[j + 1]] - scores[idx[i]]).abs() < 1e-12 {
            j += 1;
        }
        let avg = ((i + 1) as f64 + (j + 1) as f64) / 2.0;
        for k in i..=j {
            ranks[idx[k]] = avg;
        }
        i = j + 1;
    }
    let mut pos = 0usize;
    let mut rank_sum = 0.0;
    for (k, &label) in labels.iter().enumerate() {
        if label {
            pos += 1;
            rank_sum += ranks[k];
        }
    }
    let neg = labels.len().saturating_sub(pos);
    if pos == 0 || neg == 0 {
        return 0.5;
    }
    (rank_sum - (pos * (pos + 1)) as f64 / 2.0) / (pos * neg) as f64
}

fn train_model(rows: &[(Vec<f64>, bool)]) -> (Vec<f64>, f64) {
    let positives = rows.iter().filter(|(_, y)| *y).count().max(1);
    let negatives = rows.len().saturating_sub(positives).max(1);
    let pos_weight = negatives as f64 / positives as f64;
    let mut w = vec![0f64; FEATURE_DIM];
    let mut bias = 0f64;
    let n = rows.len().max(1) as f64;
    for _ in 0..EPOCHS {
        let mut gw = vec![0f64; FEATURE_DIM];
        let mut gb = 0f64;
        for (x, y) in rows {
            let mut z = bias;
            for i in 0..FEATURE_DIM {
                z += w[i] * x[i];
            }
            let p = sigmoid(z);
            let weight = if *y { pos_weight } else { 1.0 };
            let err = (p - if *y { 1.0 } else { 0.0 }) * weight;
            for i in 0..FEATURE_DIM {
                gw[i] += err * x[i];
            }
            gb += err;
        }
        for i in 0..FEATURE_DIM {
            w[i] -= LR * (gw[i] / n + L2 * w[i]);
        }
        bias -= LR * (gb / n);
    }
    (w, bias)
}

/// 训练一次。样本不足或门禁未过都返回报告（accepted=false），不落库。
pub fn train() -> Result<TrainReport, String> {
    let conn = ensure_db()?;
    let rows: Vec<(String, i64, i64)> = {
        let mut stmt = conn
            .prepare(
                "SELECT c.features, c.used, c.clicked FROM citations c \
                 JOIN episodes e ON e.id=c.episode_id \
                 WHERE c.features IS NOT NULL AND e.outcome='ok' \
                 ORDER BY e.started_at ASC",
            )
            .map_err(|e| e.to_string())?;
        let it = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        it.filter_map(|x| x.ok()).collect()
    };
    drop(conn);

    let mut data: Vec<(Vec<f64>, bool)> = Vec::new();
    for (features_json, used, clicked) in rows {
        let Ok(features) = serde_json::from_str::<Vec<f64>>(&features_json) else {
            continue;
        };
        if features.len() != FEATURE_DIM {
            continue;
        }
        data.push((features, used == 1 || clicked == 1));
    }

    let total = data.len();
    let positives = data.iter().filter(|(_, y)| *y).count();
    if total < MIN_SAMPLES || positives < MIN_POSITIVES {
        return Ok(TrainReport {
            accepted: false,
            samples: total,
            positives,
            note: format!(
                "样本不足：需要至少 {MIN_SAMPLES} 条带特征引证与 {MIN_POSITIVES} 条正例（当前 {total} / {positives}）"
            ),
            ..Default::default()
        });
    }

    let eval_size = (((total as f64) * 0.2).round() as usize)
        .max(MIN_EVAL_SIDE)
        .min(total.saturating_sub(1));
    let split = total.saturating_sub(eval_size);
    let (train_rows, eval_rows) = data.split_at(split);
    let eval_labels: Vec<bool> = eval_rows.iter().map(|(_, y)| *y).collect();
    let eval_pos = eval_labels.iter().filter(|y| **y).count();
    let eval_neg = eval_labels.len().saturating_sub(eval_pos);
    if eval_pos < MIN_EVAL_SIDE || eval_neg < MIN_EVAL_SIDE {
        return Ok(TrainReport {
            accepted: false,
            samples: total,
            positives,
            train_size: train_rows.len(),
            eval_size: eval_rows.len(),
            note: format!(
                "留出集类别不足：需要至少 {MIN_EVAL_SIDE} 正 / {MIN_EVAL_SIDE} 负（当前 {eval_pos} / {eval_neg}）"
            ),
            ..Default::default()
        });
    }

    let (weights, bias) = train_model(train_rows);
    let model = RankerModel {
        bias,
        weights,
        version: 0,
        metric: 0.0,
        samples: 0,
        trained_at: String::new(),
    };
    let model_scores: Vec<f64> = eval_rows
        .iter()
        .map(|(x, _)| model.predict(x))
        .collect();
    // 基线 = 完全按 RRF 融合分排序（特征 0 就是 fused）。
    let base_scores: Vec<f64> = eval_rows.iter().map(|(x, _)| x[0]).collect();
    let auc_model = auc(&model_scores, &eval_labels);
    let auc_base = auc(&base_scores, &eval_labels);
    let accepted = auc_model >= MIN_AUC && auc_model >= auc_base + MIN_AUC_GAIN;

    let prev_version: i64 = crate::cognition::store::get_setting(POLICY_NAME)
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let version = prev_version + 1;
    let mut note = format!(
        "AUC {:.3} vs 基线 {:.3}；训练 {} / 留出 {}",
        auc_model,
        auc_base,
        train_rows.len(),
        eval_rows.len()
    );
    if accepted {
        let weights_json = serde_json::to_string(&model.weights).unwrap_or_default();
        let conn = ensure_db()?;
        conn.execute(
            "INSERT INTO policy(name,version,weights,bias,trained_at,samples,metric) \
             VALUES(?1,?2,?3,?4,?5,?6,?7) \
             ON CONFLICT(name) DO UPDATE SET version=excluded.version, weights=excluded.weights, \
             bias=excluded.bias, trained_at=excluded.trained_at, samples=excluded.samples, \
             metric=excluded.metric",
            params![
                POLICY_NAME,
                version,
                weights_json,
                model.bias,
                now_iso(),
                total as i64,
                auc_model
            ],
        )
        .map_err(|e| e.to_string())?;
        let _ = crate::cognition::store::set_setting(POLICY_NAME, &version.to_string());
        info!(
            "[cognition] 引证排序器 v{} 落库：AUC {:.3}（基线 {:.3}，{} 条样本）",
            version, auc_model, auc_base, total
        );
    } else {
        note.push_str("；未过门禁，保持原排序器");
        debug!("[cognition] 引证排序器训练未过门禁：{}", note);
    }

    Ok(TrainReport {
        accepted,
        version: if accepted { version } else { prev_version },
        samples: total,
        positives,
        train_size: train_rows.len(),
        eval_size: eval_rows.len(),
        auc: auc_model,
        auc_base,
        note,
    })
}

/// 每周最多试训一次；样本不够时静默跳过（自动训练的调用点只记 debug）。
pub fn maybe_auto_train() {
    let last = crate::cognition::store::get_setting("ranker_train_attempt_at").unwrap_or_default();
    if !last.is_empty() {
        if let Ok(time) = chrono::DateTime::parse_from_rfc3339(&last) {
            let age = chrono::Local::now()
                .signed_duration_since(time.with_timezone(&chrono::Local))
                .num_seconds();
            if age < AUTO_INTERVAL_SECS {
                return;
            }
        }
    }
    let Ok(conn) = ensure_db() else { return };
    let (labeled, _) = labeled_counts(&conn);
    if labeled < AUTO_MIN_LABELED {
        return;
    }
    drop(conn);
    let _ = crate::cognition::store::set_setting("ranker_train_attempt_at", &now_iso());
    match train() {
        Ok(r) => info!(
            "[cognition] 周度排序器训练：接受={} AUC {:.3}（基线 {:.3}）",
            r.accepted, r.auc, r.auc_base
        ),
        Err(e) => debug!("[cognition] 周度排序器训练失败: {}", e),
    }
}
