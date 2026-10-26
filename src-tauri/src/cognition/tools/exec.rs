//! 重活工具：重建索引、生成向量、生成简报、抽取信念。
//!
//! 会拉起外部进程或调模型，耗时较长，默认关闭。

use super::*;
use crate::cognition::{belief, brief};
use serde_json::{json, Value};

pub(crate) fn handles(name: &str) -> bool {
    matches!(
        name,
        "guaxi_kb_build_index"
        | "guaxi_kb_embed"
        | "guaxi_brief_generate"
        | "guaxi_belief_extract"
    )
}

pub(crate) async fn run(name: &str, a: &Value) -> Result<Value, String> {
    match name {
        "guaxi_kb_build_index" => {
            let kb = req_str(a, "kbName")?;
            let (nodes, edges) = crate::commands::kgraph::build_kb_index(&kb)?;
            Ok(json!({ "ok": true, "nodes": nodes, "edges": edges }))
        }
        "guaxi_kb_embed" => {
            let r = crate::commands::kgraph::kg_embed_kb(req_str(a, "kbName")?).await?;
            serde_json::to_value(r).map_err(|e| e.to_string())
        }
        "guaxi_brief_generate" => {
            let b = brief::generate(
                None,
                model_of(a),
                &opt_str(a, "kind").unwrap_or_else(|| "morning".into()),
            )
            .await?;
            Ok(json!({ "ok": true, "day": b.day, "path": b.path, "chars": b.markdown.len() }))
        }
        "guaxi_belief_extract" => {
            let r = belief::extract_beliefs(
                &req_str(a, "kbName")?,
                model_of(a),
                opt_usize(a, "maxSections", 20),
            )
            .await?;
            serde_json::to_value(r).map_err(|e| e.to_string())
        }
        other => Err(format!("工具 {other} 尚未实现分发")),
    }
}
