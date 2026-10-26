//! 召回、知识图谱、感知/记忆/任务等只读工具。
//!
//! 这一组是 Agent 回答"用户自己的资料"的基础，全部只读、恒可用。

use super::*;
use crate::cognition::{belief, brief, nudge, percept, profile, recall};
use serde_json::{json, Value};

pub(crate) fn handles(name: &str) -> bool {
    matches!(
        name,
        "guaxi_recall"
        | "guaxi_kb_list"
        | "guaxi_kb_semantic_search"
        | "guaxi_kb_node_text"
        | "guaxi_kb_neighbors"
        | "guaxi_kb_path"
        | "guaxi_kb_stats"
        | "guaxi_kb_tags"
        | "guaxi_kb_files_by_tags"
        | "guaxi_percept_list"
        | "guaxi_percept_get"
        | "guaxi_belief_list"
        | "guaxi_nudge_list"
        | "guaxi_profile_get"
        | "guaxi_brief_get"
        | "guaxi_episode_recent"
        | "guaxi_task_query"
        | "guaxi_search_text"
    )
}

pub(crate) async fn run(name: &str, a: &Value) -> Result<Value, String> {
    match name {
        "guaxi_recall" => {
            let q = req_str(a, "query")?;
            let cites = recall::recall_brief(&q, opt_str(a, "kbName"), opt_usize(a, "k", 8).min(20)).await?;
            Ok(json!({ "citations": cites }))
        }
        "guaxi_kb_list" => Ok(json!({ "knowledgeBases": crate::commands::kgraph::list_kb_names() })),
        "guaxi_kb_semantic_search" => {
            let hits = crate::commands::kgraph::kg_semantic_search(
                req_str(a, "kbName")?,
                req_str(a, "query")?,
                Some(opt_usize(a, "topK", 20)),
                opt_strs(a, "nodeTypes"),
            )
            .await?;
            Ok(serde_json::to_value(hits).map_err(|e| e.to_string())?)
        }
        "guaxi_kb_node_text" => {
            let kb = req_str(a, "kbName")?;
            let id = req_str(a, "nodeId")?;
            let node = crate::commands::kgraph::kg_node_of(&kb, &id)
                .ok_or_else(|| format!("节点不存在: {id}"))?;
            let text = crate::commands::kgraph::kg_node_text(&node, opt_usize(a, "maxChars", 3000));
            Ok(json!({
                "id": node.id, "name": node.name, "type": node.node_type,
                "path": node.file_path.clone().or(node.path.clone()),
                "startLine": node.start_line, "endLine": node.end_line, "text": text
            }))
        }
        "guaxi_kb_neighbors" => {
            let sub = crate::commands::kgraph::kg_node_neighbors(
                req_str(a, "kbName")?,
                req_str(a, "nodeId")?,
                Some(opt_i64(a, "hops", 1).clamp(1, 3)),
                Some(opt_usize(a, "limit", 30)),
                None,
            )?;
            Ok(serde_json::to_value(sub).map_err(|e| e.to_string())?)
        }
        "guaxi_kb_path" => {
            let r = crate::commands::kgraph::kg_shortest_path(
                req_str(a, "kbName")?,
                req_str(a, "fromId")?,
                req_str(a, "toId")?,
            )?;
            Ok(serde_json::to_value(r).map_err(|e| e.to_string())?)
        }
        "guaxi_kb_stats" => {
            let r = crate::commands::kgraph::kg_stats(
                req_str(a, "kbName")?,
                Some(opt_usize(a, "topN", 20)),
            )?;
            Ok(serde_json::to_value(r).map_err(|e| e.to_string())?)
        }
        "guaxi_kb_tags" => {
            let r = crate::commands::kgraph::kg_all_tags(req_str(a, "kbName")?)?;
            Ok(serde_json::to_value(r).map_err(|e| e.to_string())?)
        }
        "guaxi_kb_files_by_tags" => {
            let tags = opt_strs(a, "tags").unwrap_or_default();
            let r = crate::commands::kgraph::kg_files_by_tags(
                req_str(a, "kbName")?,
                tags,
                Some(opt_bool(a, "matchAll", false)),
            )?;
            Ok(json!({ "files": r }))
        }
        "guaxi_percept_list" => {
            let list = percept::list_percepts(&percept::PerceptQuery {
                state: opt_str(a, "state"),
                source_kind: opt_str(a, "sourceKind"),
                kb_name: opt_str(a, "kbName"),
                min_salience: a.get("minSalience").and_then(|v| v.as_f64()),
                limit: Some(opt_usize(a, "limit", 30)),
            })?;
            Ok(json!({ "percepts": list }))
        }
        "guaxi_percept_get" => {
            let id = req_str(a, "id")?;
            let (title, text, uri) =
                percept::percept_text(&id).ok_or_else(|| format!("感知不存在: {id}"))?;
            // 交给模型时仍然包一层不可信声明——即便调用方是可信的。
            Ok(json!({
                "id": id, "title": title, "uri": uri,
                "content": percept::untrusted_block(&title, &text)
            }))
        }
        "guaxi_belief_list" => {
            let list = belief::list_beliefs(
                opt_str(a, "kbName").as_deref(),
                opt_str(a, "status").as_deref(),
                opt_usize(a, "limit", 50),
            )?;
            Ok(json!({ "beliefs": list }))
        }
        "guaxi_nudge_list" => {
            let list = nudge::list_nudges(
                opt_str(a, "state").as_deref(),
                opt_str(a, "kind").as_deref(),
                opt_usize(a, "limit", 30),
            )?;
            Ok(json!({ "nudges": list }))
        }
        "guaxi_profile_get" => {
            let p = profile::load_profile()?;
            Ok(serde_json::to_value(p).map_err(|e| e.to_string())?)
        }
        "guaxi_brief_get" => {
            let day = opt_str(a, "day").unwrap_or_else(store::today);
            let kind = opt_str(a, "kind").unwrap_or_else(|| "morning".into());
            match brief::get(&day, &kind)? {
                Some(b) => Ok(serde_json::to_value(b).map_err(|e| e.to_string())?),
                None => Ok(json!({ "found": false, "day": day, "kind": kind })),
            }
        }
        "guaxi_episode_recent" => {
            let list = crate::cognition::episode::recent(
                opt_usize(a, "limit", 20),
                opt_str(a, "kind").as_deref(),
            )?;
            Ok(json!({ "episodes": list }))
        }
        "guaxi_task_query" => {
            let r = crate::commands::taskmanager::query_blocks(
                &crate::commands::taskmanager::TaskQuery {
                    category: opt_str(a, "category"),
                    status: opt_str(a, "status"),
                    done: a.get("done").and_then(|v| v.as_bool()),
                    date_from: opt_str(a, "dateFrom"),
                    date_to: opt_str(a, "dateTo"),
                    keyword: opt_str(a, "keyword"),
                },
            )?;
            Ok(serde_json::to_value(r).map_err(|e| e.to_string())?)
        }
        "guaxi_search_text" => {
            let path = ensure_path_allowed(&req_str(a, "path")?)?;
            let root = path.to_string_lossy().to_string();
            let pattern = req_str(a, "pattern")?;
            let limit = opt_usize(a, "limit", 50);
            let extensions = opt_strs(a, "extensions");
            // 模型给的 pattern 默认按正则理解，但允许显式关掉。
            let regex = opt_bool(a, "regex", true);
            let opts = crate::commands::searcher::MatchOptions {
                regex: Some(regex),
                ..Default::default()
            };

            // 非正则查询先走倒排索引（tantivy + jieba）：它按词匹配，
            // 「增强 搜索」能找到同一篇里分散的两个词，而不是像 grep 那样要求字面连续。
            // 索引没建好时 `fulltext_search` 会自己退回 rg_search，这里不需要判断。
            if !regex {
                let Some(app) = crate::cognition::runtime::app_handle() else {
                    return Err("AppHandle 未就绪，暂时无法使用全文索引".to_string());
                };
                let (hits, truncated) = crate::commands::fulltext::fulltext_search(
                    app,
                    pattern,
                    root,
                    Some(limit),
                    None,
                    None,
                    extensions,
                    None,
                    None,
                    Some(opts),
                )
                .await?;
                return Ok(json!({ "matches": hits, "truncated": truncated, "engine": "fulltext" }));
            }

            let (hits, truncated) = crate::commands::searcher::rg_search(
                pattern,
                root,
                Some(limit),
                None,
                None,
                extensions,
                None,
                None,
                Some(opts),
            )
            .await?;
            Ok(json!({ "matches": hits, "truncated": truncated, "engine": "grep" }))
        }
        other => Err(format!("工具 {other} 尚未实现分发")),
    }
}
