//! 写入类工具：改知识库、改认知状态、改画像。
//!
//! 默认关闭，且每一条都会真的改用户的文件——调用前应让用户知道要改什么。

use super::*;
use crate::cognition::{belief, nudge, percept, profile};
use serde_json::{json, Value};

pub(crate) fn handles(name: &str) -> bool {
    matches!(
        name,
        "guaxi_kb_write_note"
        | "guaxi_kb_add_tag"
        | "guaxi_kb_link"
        | "guaxi_percept_set_state"
        | "guaxi_percept_ingest"
        | "guaxi_belief_set_status"
        | "guaxi_nudge_decide"
        | "guaxi_profile_save"
        | "guaxi_review_feedback"
    )
}

pub(crate) async fn run(name: &str, a: &Value) -> Result<Value, String> {
    match name {
        "guaxi_kb_write_note" => {
            let kb = req_str(a, "kbName")?;
            let rel = req_str(a, "relPath")?;
            if !rel.to_lowercase().ends_with(".md") {
                return Err("relPath 必须以 .md 结尾".to_string());
            }
            let root = std::path::PathBuf::from(crate::commands::settingview::get_knowledge_bases_root())
                .join(&kb);
            let target = ensure_path_allowed(&root.join(&rel).to_string_lossy())?;
            if target.exists() && !opt_bool(a, "overwrite", false) {
                return Err(format!(
                    "目标已存在且未允许覆盖：{}。要覆盖请传 overwrite=true。",
                    target.to_string_lossy()
                ));
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let content = req_str(a, "content")?;
            std::fs::write(&target, &content).map_err(|e| format!("写入失败: {e}"))?;
            // 走已有钩子标脏索引，图谱与管理器会自动跟随。
            let _ = crate::commands::kgraph::after_write_file(&target, &content);
            Ok(json!({ "ok": true, "path": target.to_string_lossy(), "bytes": content.len() }))
        }
        "guaxi_kb_add_tag" => {
            crate::commands::kgraph::kg_add_tag(
                req_str(a, "kbName")?,
                req_str(a, "nodeId")?,
                req_str(a, "tag")?,
                Some(opt_str(a, "source").unwrap_or_else(|| "manual".into())),
                None,
            )?;
            Ok(json!({ "ok": true }))
        }
        "guaxi_kb_link" => {
            crate::commands::kgraph::kg_confirm_relation(
                req_str(a, "kbName")?,
                req_str(a, "fromId")?,
                req_str(a, "toId")?,
                Some(opt_str(a, "relation").unwrap_or_else(|| "related".into())),
                None,
            )?;
            Ok(json!({ "ok": true }))
        }
        "guaxi_percept_set_state" => {
            percept::set_state(
                &req_str(a, "id")?,
                &req_str(a, "state")?,
                opt_str(a, "reason").as_deref(),
            )?;
            Ok(json!({ "ok": true }))
        }
        "guaxi_percept_ingest" => {
            let r = percept::ingest(percept::PerceptInput {
                source_kind: req_str(a, "sourceKind")?,
                source_id: opt_str(a, "sourceId").unwrap_or_default(),
                uri: opt_str(a, "uri").unwrap_or_default(),
                title: opt_str(a, "title").unwrap_or_default(),
                text: req_str(a, "text")?,
                kb_name: opt_str(a, "kbName").unwrap_or_default(),
                trust: None,
                meta: None,
            })?;
            Ok(serde_json::to_value(r).map_err(|e| e.to_string())?)
        }
        "guaxi_belief_set_status" => {
            belief::set_status(&req_str(a, "id")?, &req_str(a, "status")?)?;
            Ok(json!({ "ok": true }))
        }
        "guaxi_nudge_decide" => {
            nudge::record_decision(&req_str(a, "id")?, opt_bool(a, "accepted", false))?;
            Ok(json!({ "ok": true }))
        }
        "guaxi_profile_save" => {
            let p = profile::save_profile(&req_str(a, "markdown")?)?;
            Ok(json!({ "ok": true, "interests": p.interests.len() }))
        }
        "guaxi_review_feedback" => {
            nudge::review_feedback(&req_str(a, "nodeId")?, opt_bool(a, "remembered", true))?;
            Ok(json!({ "ok": true }))
        }
        other => Err(format!("工具 {other} 尚未实现分发")),
    }
}
