//! 任务接入：把任务管理里的待办按分类白名单接进认知层。
//!
//! 默认 work / life / study 三类全部接入，行为与接入前完全一致；用户可以在设置页
//! 或大脑的「任务」页签里逐类关闭（例如不希望工作待办进入大脑）。白名单存
//! `cog_settings.cognition_task_categories`，**空数组表示全部关闭**，与"未配置"
//! （缺省键 → 全开）严格区分。
//!
//! 只有分类白名单这一层过滤：任务本身仍以 `.org` 为真相源，这里只读。

use serde::Serialize;

use crate::commands::task_block::TaskBlock;
use crate::commands::taskmanager::{self, TaskQuery};
use crate::cognition::store::{now_iso, today};

/// 三个分类与中文标签。顺序即界面顺序。
pub const CATEGORIES: [(&str, &str); 3] = [("work", "工作"), ("life", "生活"), ("study", "学习")];
const SETTING_KEY: &str = "cognition_task_categories";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCategory {
    pub key: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub internal_id: String,
    pub id: String,
    pub title: String,
    pub deadline: String,
    pub category: String,
    pub category_label: String,
    pub status: String,
    pub priority: String,
    pub overdue: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOverview {
    pub categories: Vec<TaskCategory>,
    pub allowed: Vec<String>,
    pub items: Vec<TaskItem>,
    pub overdue: i64,
    pub due_today: i64,
    pub due_soon: i64,
    pub window_days: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCategories {
    pub categories: Vec<TaskCategory>,
    pub allowed: Vec<String>,
}

fn label_of(key: &str) -> String {
    CATEGORIES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_else(|| key.to_string())
}

fn normalize(list: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out = Vec::new();
    for item in list {
        let key = item.trim().to_lowercase();
        if CATEGORIES.iter().any(|(k, _)| *k == key) && !out.iter().any(|x| *x == key) {
            out.push(key);
        }
    }
    out
}

/// 当前允许接入的分类。设置缺省 → 全开；显式空数组 → 全关。
pub fn allowed_categories() -> Vec<String> {
    let raw = store_setting(SETTING_KEY);
    let Some(raw) = raw.filter(|v| !v.trim().is_empty()) else {
        return CATEGORIES.iter().map(|(k, _)| (*k).to_string()).collect();
    };
    match serde_json::from_str::<Vec<String>>(&raw) {
        Ok(list) => normalize(list),
        Err(_) => CATEGORIES.iter().map(|(k, _)| (*k).to_string()).collect(),
    }
}

fn store_setting(key: &str) -> Option<String> {
    crate::cognition::store::get_setting(key)
}

pub fn set_categories(list: Vec<String>) -> Result<Vec<String>, String> {
    let normalized = normalize(list);
    crate::cognition::store::set_setting(
        SETTING_KEY,
        &serde_json::to_string(&normalized).unwrap_or_else(|_| "[]".into()),
    )?;
    // 已有的到期提醒是按旧白名单生成的：让它们立即过期，下一轮审议按新白名单重算。
    // 否则关掉「工作」之后，看板上的工作待办提醒还会挂到失效为止。
    if let Ok(conn) = crate::cognition::store::ensure_db() {
        let _ = conn.execute(
            "UPDATE nudges SET state='expired', decided_at=?1 \
             WHERE kind='due' AND state IN ('pending','delivered')",
            rusqlite::params![now_iso()],
        );
    }
    Ok(normalized)
}

fn categories_with(allowed: &[String]) -> Vec<TaskCategory> {
    CATEGORIES
        .iter()
        .map(|(key, label)| TaskCategory {
            key: (*key).to_string(),
            label: (*label).to_string(),
            enabled: allowed.iter().any(|k| k == key),
        })
        .collect()
}

pub fn category_settings() -> TaskCategories {
    let allowed = allowed_categories();
    TaskCategories {
        categories: categories_with(&allowed),
        allowed: allowed.clone(),
    }
}

fn title_of(task: &TaskBlock) -> String {
    let description = task.description.trim();
    if !description.is_empty() {
        return description.to_string();
    }
    if !task.id.trim().is_empty() {
        return task.id.clone();
    }
    task.internal_id.clone()
}

/// 学习/工作/生活里已允许的分类，未来 `days` 天内到期或已过期的未完成任务。
///
/// 只返回带截止日期的条目：大脑关心的是"该做了"，无截止日期的收集箱条目留给
/// 任务管理本身。返回按截止日升序，最多 60 条。
pub fn overview(days: Option<i64>) -> Result<TaskOverview, String> {
    let allowed = allowed_categories();
    let window_days = days.unwrap_or(7).clamp(1, 60);
    let today_str = today();
    let end = (chrono::Local::now() + chrono::Duration::days(window_days))
        .format("%Y-%m-%d")
        .to_string();

    let mut items: Vec<TaskItem> = Vec::new();
    if !allowed.is_empty() {
        for category in &allowed {
            let tasks = taskmanager::query_blocks(&TaskQuery {
                category: Some(category.clone()),
                status: None,
                done: Some(false),
                date_from: None,
                date_to: None,
                keyword: None,
            })
            .unwrap_or_default();
            for task in tasks {
                let deadline = task.deadline.trim().to_string();
                if deadline.is_empty() || deadline.as_str() > end.as_str() {
                    continue;
                }
                items.push(TaskItem {
                    internal_id: task.internal_id.clone(),
                    id: task.id.clone(),
                    title: title_of(&task),
                    overdue: deadline.as_str() < today_str.as_str(),
                    deadline,
                    category: category.clone(),
                    category_label: label_of(category),
                    status: task.status.clone(),
                    priority: task.priority.clone(),
                });
            }
        }
    }
    items.sort_by(|a, b| {
        a.deadline
            .cmp(&b.deadline)
            .then_with(|| a.priority.cmp(&b.priority))
    });
    let overdue = items.iter().filter(|t| t.overdue).count() as i64;
    let due_today = items
        .iter()
        .filter(|t| t.deadline == today_str)
        .count() as i64;
    let due_soon = items.len() as i64 - overdue - due_today;
    items.truncate(60);

    Ok(TaskOverview {
        categories: categories_with(&allowed),
        allowed,
        items,
        overdue,
        due_today,
        due_soon,
        window_days,
        updated_at: now_iso(),
    })
}
