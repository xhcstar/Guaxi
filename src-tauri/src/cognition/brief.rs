//! 晨间简报：绝大多数提醒的归宿。
//!
//! 设计要点：
//! - **骨架完全确定性**。不调模型也能出一份完整简报，LLM 只负责开头两句话的串联，
//!   且预算不足时直接省略。任何"必须联网才有输出"的主动功能都不可靠。
//! - **它是投递通道，不是又一个列表**。进了简报的提醒状态转为 delivered(channel=brief)，
//!   用户在简报里的采纳/忽略同样喂给策略模型。
//! - **产物是 markdown 文件**，落在 `config/cognition/briefs/`，可 git、可导入知识库。

use crate::cognition::store::{ensure_db, now_iso, today};
use crate::cognition::{episode, nudge, percept, profile};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::fs;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Brief {
    pub id: String,
    pub day: String,
    pub kind: String,
    pub path: Option<String>,
    pub markdown: String,
    pub created_at: String,
    pub opened: bool,
}

fn briefs_dir() -> Result<std::path::PathBuf, String> {
    let d = crate::cognition::store::cog_data_dir()?.join("briefs");
    if !d.exists() {
        fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    }
    Ok(d)
}

/// 生成（或重生成）某天的简报。已存在则覆盖，便于用户手动"刷新简报"。
pub async fn generate(
    app: Option<&tauri::AppHandle>,
    modelid: i32,
    kind: &str,
) -> Result<Brief, String> {
    let day = today();
    let kind = if kind.trim().is_empty() { "morning" } else { kind.trim() };

    // 1) 昨天以来的新感知（只看显著的，最多 5 条）
    let since = (chrono::Local::now() - chrono::Duration::hours(30)).to_rfc3339();
    let fresh = percept::list_percepts(&percept::PerceptQuery {
        state: Some("new".into()),
        source_kind: None,
        kb_name: None,
        min_salience: Some(0.4),
        limit: Some(5),
    })
    .unwrap_or_default();

    // 2) 未处理的提醒，按类型分组
    let open = nudge::list_nudges(Some("open"), None, 40).unwrap_or_default();
    let pick = |k: &str, n: usize| -> Vec<nudge::Nudge> {
        open.iter().filter(|x| x.kind == k).take(n).cloned().collect()
    };
    let contradictions = pick("contradiction", 3);
    let gaps = pick("gap", 2);
    let dues = pick("due", 2);
    let reviews = pick("review", 1);
    let emerging = pick("emerging", 1);
    let stales = pick("stale", 2);

    // 3) 统计
    let pstats = percept::stats().unwrap_or_default();
    let estats = episode::stats().unwrap_or_default();
    let nstats = nudge::stats().unwrap_or_default();

    // 4) 开场两句：只有预算充足且确实有料时才调模型；失败即省略。
    let mut opening = String::new();
    let budget_ok = crate::cognition::store::tokens_spent_today(None)
        < crate::cognition::store::get_i64("daily_token_budget", 60_000);
    if budget_ok && (!contradictions.is_empty() || fresh.len() >= 2) {
        let mut material = String::new();
        for c in contradictions.iter() {
            material.push_str(&format!("- 冲突：{}\n", c.title));
        }
        for f in fresh.iter() {
            material.push_str(&format!(
                "- 新材料：{}\n",
                f.title.clone().unwrap_or_else(|| f.excerpt.clone())
            ));
        }
        let prompt = format!(
            "下面是某人今天的知识动态清单。用**两句话**中文概括今天最值得他注意的是什么，\
             直接说结论，不要客套、不要重复清单、不要用「今天」开头。\n\n{material}"
        );
        if let Ok((ans, _)) =
            crate::commands::chatview::kg_llm_complete(modelid, &prompt, "cog-brief").await
        {
            crate::cognition::store::record_tokens("brief", (ans.chars().count() / 3) as i64);
            opening = ans.trim().chars().take(400).collect();
        }
    }

    let markdown = render(
        &day,
        kind,
        &opening,
        &fresh,
        &contradictions,
        &gaps,
        &dues,
        &reviews,
        &emerging,
        &stales,
        &pstats,
        &estats,
        &nstats,
        &since,
    );

    // 5) 落盘 + 落库
    let path = briefs_dir()?.join(format!("{day}-{kind}.md"));
    fs::write(&path, &markdown).map_err(|e| format!("写简报失败: {e}"))?;
    let conn = ensure_db()?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO briefs(id,day,kind,path,markdown,created_at,opened) \
         VALUES(?1,?2,?3,?4,?5,?6,0) \
         ON CONFLICT(day,kind) DO UPDATE SET markdown=excluded.markdown, path=excluded.path, \
           created_at=excluded.created_at, opened=0",
        params![id, day, kind, path.to_string_lossy().to_string(), markdown, now_iso()],
    )
    .map_err(|e| e.to_string())?;

    // 6) 进了简报的提醒，通道置为 brief（原状态是 pending 时才动）
    for n in contradictions
        .iter()
        .chain(gaps.iter())
        .chain(dues.iter())
        .chain(reviews.iter())
        .chain(emerging.iter())
        .chain(stales.iter())
    {
        let _ = conn.execute(
            "UPDATE nudges SET state='delivered', channel='brief', delivered_at=?2 \
             WHERE id=?1 AND state='pending'",
            params![n.id, now_iso()],
        );
    }

    if let Some(handle) = app {
        use tauri::Emitter;
        let _ = handle.emit(
            "cognition_brief",
            serde_json::json!({ "day": day, "kind": kind, "path": path.to_string_lossy() }),
        );
    }
    info!("[cognition] 简报已生成 {}", path.to_string_lossy());

    Ok(Brief {
        id,
        day,
        kind: kind.to_string(),
        path: Some(path.to_string_lossy().to_string()),
        markdown,
        created_at: now_iso(),
        opened: false,
    })
}

/// 渲染简报正文。**完全确定性**：无网络、无模型，只读已有数据。
#[allow(clippy::too_many_arguments)]
fn render(
    day: &str,
    kind: &str,
    opening: &str,
    fresh: &[percept::Percept],
    contradictions: &[nudge::Nudge],
    gaps: &[nudge::Nudge],
    dues: &[nudge::Nudge],
    reviews: &[nudge::Nudge],
    emerging: &[nudge::Nudge],
    stales: &[nudge::Nudge],
    pstats: &percept::PerceptStats,
    estats: &episode::EpisodeStats,
    nstats: &nudge::NudgeStats,
    since: &str,
) -> String {
    let title = match kind {
        "morning" => "晨间简报",
        "evening" => "晚间简报",
        other => other,
    };
    let mut md = String::with_capacity(4096);
    md.push_str(&format!("# {title} · {day}\n\n"));
    if !opening.is_empty() {
        md.push_str(&format!("> {}\n\n", opening.replace('\n', " ")));
    }

    let nothing = contradictions.is_empty()
        && gaps.is_empty()
        && dues.is_empty()
        && reviews.is_empty()
        && emerging.is_empty()
        && stales.is_empty()
        && fresh.is_empty();
    if nothing {
        md.push_str("今天没有需要你处理的事。感知收件箱是空的，也没有到期任务或结构缺口。\n\n");
    }

    if !contradictions.is_empty() {
        md.push_str("## ⚠ 需要你裁决的冲突\n\n");
        md.push_str("这些是新材料与你既有结论对不上的地方。不处理的话，知识库会一直错下去。\n\n");
        for n in contradictions {
            md.push_str(&format!("### {}\n\n{}\n\n", n.title, n.body));
            if !n.why.is_empty() {
                md.push_str("依据：\n");
                for w in &n.why {
                    md.push_str(&format!("- {w}\n"));
                }
                md.push('\n');
            }
            md.push_str(&refs_block(&n.refs));
        }
    }

    if !dues.is_empty() {
        md.push_str("## 今天到期\n\n");
        for n in dues {
            md.push_str(&format!("**{}**\n\n{}\n\n", n.title, n.body));
            md.push_str(&refs_block(&n.refs));
        }
    }

    if !fresh.is_empty() {
        md.push_str("## 值得一读\n\n");
        md.push_str("按显著性排序（兼顾新颖度、与你关注的贴合度、来源可信度）。\n\n");
        for (i, p) in fresh.iter().enumerate() {
            let label = p.title.clone().unwrap_or_else(|| "（无标题）".into());
            md.push_str(&format!(
                "{}. **{}**  \n   显著性 {:.2}｜新颖度 {:.2}｜来源 {}\n",
                i + 1,
                label,
                p.salience,
                p.novelty,
                p.source_kind
            ));
            if let Some(u) = &p.uri {
                if !u.is_empty() {
                    md.push_str(&format!("   {u}\n"));
                }
            }
            md.push_str(&format!("   > {}\n\n", p.excerpt.chars().take(180).collect::<String>()));
            if p.neutralized > 0 {
                md.push_str(&format!(
                    "   （注意：这条材料里有 {} 处试图指挥 AI 的内容，已被中和）\n\n",
                    p.neutralized
                ));
            }
        }
    }

    if !gaps.is_empty() || !emerging.is_empty() {
        md.push_str("## 知识结构\n\n");
        for n in gaps.iter().chain(emerging.iter()) {
            md.push_str(&format!("### {}\n\n{}\n\n", n.title, n.body));
            md.push_str(&refs_block(&n.refs));
        }
    }

    if !stales.is_empty() {
        md.push_str("## 停下来的项目\n\n");
        for n in stales {
            md.push_str(&format!("### {}\n\n{}\n\n", n.title, n.body));
            md.push_str(&refs_block(&n.refs));
        }
    }

    if !reviews.is_empty() {
        md.push_str("## 该回看的笔记\n\n");
        for n in reviews {
            md.push_str(&format!("{}\n\n", n.body));
            md.push_str(&refs_block(&n.refs));
        }
    }

    if let Ok(prof) = profile::load_profile() {
        if !prof.projects.is_empty() {
            md.push_str(&format!(
                "## 在做的项目\n\n{}\n\n",
                prof.projects
                    .iter()
                    .map(|p| format!("- {p}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
    }

    md.push_str("---\n\n## 系统状态\n\n");
    md.push_str(&format!(
        "- 感知：共 {} 条，待打分 {}，未处理 {}，已中和的指令样片段 {} 处\n",
        pstats.total, pstats.pending_score, pstats.new_count, pstats.neutralized_total
    ));
    md.push_str(&format!(
        "- 记忆：情节 {} 段（{} 段带召回），引证 {} 条其中被真正用上 {} 条\n",
        estats.total, estats.with_recall, estats.citation_total, estats.citation_used
    ));
    md.push_str(&format!(
        "- 主动：未处理提醒 {}，今日已打断 {} 次（上限内剩 {}），近 14 天采纳率 {:.0}%，当前阈值 {:.2}\n",
        nstats.open,
        nstats.delivered_today,
        nstats.interrupts_left_today,
        nstats.accept_rate_14d * 100.0,
        nstats.threshold
    ));
    md.push_str(&format!(
        "- 预算：今日 token {} / {}\n",
        nstats.tokens_today, nstats.token_budget
    ));
    md.push_str(&format!("\n<!-- 统计窗口起点 {since} -->\n"));
    md
}

/// 引用块：文件路径写成 `path:line`，前端与编辑器都能直接跳。
fn refs_block(refs: &[nudge::NudgeRef]) -> String {
    if refs.is_empty() {
        return String::new();
    }
    let mut s = String::from("相关：\n");
    for r in refs {
        let loc = match (&r.path, r.line) {
            (Some(p), Some(l)) if !p.is_empty() => format!("`{p}:{l}`"),
            (Some(p), None) if !p.is_empty() => format!("`{p}`"),
            _ => r.uri.clone().unwrap_or_default(),
        };
        if loc.is_empty() {
            s.push_str(&format!("- {}\n", r.label));
        } else {
            s.push_str(&format!("- {} — {}\n", r.label, loc));
        }
    }
    s.push('\n');
    s
}

/// 取某天的简报（不存在返回 None）。
pub fn get(day: &str, kind: &str) -> Result<Option<Brief>, String> {
    let conn = ensure_db()?;
    let row = conn
        .query_row(
            "SELECT id,day,kind,path,markdown,created_at,opened FROM briefs WHERE day=?1 AND kind=?2",
            params![day, kind],
            |r| {
                Ok(Brief {
                    id: r.get(0)?,
                    day: r.get(1)?,
                    kind: r.get(2)?,
                    path: r.get(3)?,
                    markdown: r.get(4)?,
                    created_at: r.get(5)?,
                    opened: r.get::<_, i64>(6)? == 1,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(row)
}

/// 列出历史简报（不带正文，省流量）。
pub fn list(limit: usize) -> Result<Vec<serde_json::Value>, String> {
    let conn = ensure_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT day,kind,COALESCE(path,''),created_at,opened,LENGTH(markdown) \
             FROM briefs ORDER BY day DESC, kind LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit.clamp(1, 200) as i64], |r| {
            Ok(serde_json::json!({
                "day": r.get::<_, String>(0)?,
                "kind": r.get::<_, String>(1)?,
                "path": r.get::<_, String>(2)?,
                "createdAt": r.get::<_, String>(3)?,
                "opened": r.get::<_, i64>(4)? == 1,
                "size": r.get::<_, i64>(5)?,
            }))
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|x| x.ok()).collect())
}

pub fn mark_opened(day: &str, kind: &str) -> Result<(), String> {
    let conn = ensure_db()?;
    conn.execute(
        "UPDATE briefs SET opened=1 WHERE day=?1 AND kind=?2",
        params![day, kind],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 今天该不该出简报：到了画像里设定的时刻、且今天还没出过。
pub fn due_now() -> bool {
    let Ok(prof) = profile::load_profile() else { return false };
    let hour: u32 = chrono::Local::now()
        .format("%H")
        .to_string()
        .parse()
        .unwrap_or(0);
    if hour < prof.brief_hour {
        return false;
    }
    match get(&today(), "morning") {
        Ok(Some(_)) => false,
        Ok(None) => true,
        Err(e) => {
            debug!("[cognition] 简报状态查询失败: {}", e);
            false
        }
    }
}
