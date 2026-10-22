//! 通讯录：联系人、通讯组、自动收录、vCard 导入导出、地址补全。
//!
//! - `model`  数据结构
//! - `store`  SQLite（与邮箱共用一个库）与业务规则
//! - `vcard`  vCard 2.1 / 3.0 解析与 3.0 编码
//!
//! 三条设计约束，都来自真实邮件的形态：
//! 1. **一个地址一个人**：去重键是邮箱而不是姓名，姓名在邮件里会被写成十几种；
//! 2. **自动收录要克制**：跳过自己的地址与 `noreply@` 这类机器人，否则补全列表会被污染；
//! 3. **导入要能读旧格式**：Outlook / 手机导出的往往是 3.0 甚至 2.1，且带 GBK 编码的中文名。

pub mod model;
pub mod store;
pub mod vcard;

use model::{
    Contact, ContactGroup, ContactMessage, ContactQuery, ContactSuggestion, ImportReport,
};
use tauri::command;

#[command]
pub async fn mail_list_contacts(query: ContactQuery) -> Result<Vec<Contact>, String> {
    store::list(&query)
}

#[command]
pub async fn mail_get_contact(id: String) -> Result<Option<Contact>, String> {
    store::get(&id)
}

#[command]
pub async fn mail_save_contact(contact: Contact) -> Result<Contact, String> {
    store::upsert(contact)
}

#[command]
pub async fn mail_delete_contact(id: String) -> Result<(), String> {
    store::delete(&id)
}

/// 通讯录统计：总数 / 自动收录数 / 星标数，侧栏徽标用。
#[command]
pub async fn mail_contact_stats() -> Result<serde_json::Value, String> {
    let all = store::list(&ContactQuery::default())?;
    Ok(serde_json::json!({
        "total": all.len(),
        "auto": all.iter().filter(|c| c.source == "auto").count(),
        "starred": all.iter().filter(|c| c.starred).count(),
        "withUnread": all.iter().filter(|c| c.unread_count > 0).count(),
    }))
}

/// 写信地址栏补全。
#[command]
pub async fn mail_contact_suggest(prefix: String) -> Result<Vec<ContactSuggestion>, String> {
    store::suggest(&prefix, 8)
}

/// 与某联系人的往来邮件。
#[command]
pub async fn mail_contact_messages(id: String, limit: Option<u32>) -> Result<Vec<ContactMessage>, String> {
    let Some(contact) = store::get(&id)? else {
        return Ok(Vec::new());
    };
    store::messages_with(&contact, limit.unwrap_or(50))
}

// ---------------- 通讯组 ----------------

#[command]
pub async fn mail_list_contact_groups() -> Result<Vec<ContactGroup>, String> {
    store::list_groups()
}

#[command]
pub async fn mail_save_contact_group(group: ContactGroup) -> Result<ContactGroup, String> {
    store::upsert_group(group)
}

#[command]
pub async fn mail_delete_contact_group(id: String) -> Result<(), String> {
    store::delete_group(&id)
}

/// 展开通讯组为地址列表（写信时点选组）。
#[command]
pub async fn mail_expand_contact_group(id: String) -> Result<Vec<String>, String> {
    store::expand_group(&id)
}

// ---------------- 导入 / 导出 ----------------

/// 从 vCard 文件导入。返回导入 / 合并 / 跳过数量。
#[command]
pub async fn mail_import_contacts(path: String) -> Result<ImportReport, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("读取文件失败（{path}）: {e}"))?;
    // .vcf 的编码不统一：优先按 UTF-8，失败再按 GBK（Outlook 中文导出常见）。
    let text = match String::from_utf8(bytes.clone()) {
        Ok(text) => text,
        Err(_) => {
            let (decoded, _, _) = encoding_rs::GBK.decode(&bytes);
            decoded.to_string()
        }
    };
    store::import_vcards(&text)
}

/// 导出联系人为 .vcf。`groupId` 为空表示导出全部。
#[command]
pub async fn mail_export_contacts(path: String, group_id: Option<String>) -> Result<u32, String> {
    let text = store::export_vcards(group_id.as_deref())?;
    let count = text.matches("BEGIN:VCARD").count() as u32;
    std::fs::write(&path, text).map_err(|e| format!("写入文件失败（{path}）: {e}"))?;
    Ok(count)
}

/// 从已有邮件里一次性重建通讯录（升级 / 换了账户后补数据用）。
#[command]
pub async fn mail_contacts_rebuild_from_mail(app: tauri::AppHandle) -> Result<u32, String> {
    let _ = app;
    let conn = crate::commands::mail::store::connection()?;
    let accounts = crate::commands::mail::store::list_accounts()?;
    let own: Vec<String> = accounts
        .iter()
        .flat_map(|a| vec![a.email.clone(), a.login_name()])
        .collect();

    let mut stmt = conn
        .prepare(
            "SELECT from_name, from_addr, to_json, cc_json, date FROM messages
             WHERE folder NOT IN (SELECT name FROM folders WHERE role IN ('sent','drafts'))",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut touched = 0u32;
    for row in rows.flatten() {
        let (from_name, from_addr, to_json, cc_json, date) = row;
        let to: Vec<model::LabeledValue> = serde_json::from_str(&to_json).unwrap_or_default();
        let cc: Vec<model::LabeledValue> = serde_json::from_str(&cc_json).unwrap_or_default();
        if store::touch_from_message(&own, &from_name, &from_addr, &to, &cc, date).is_ok() {
            touched += 1;
        }
    }
    Ok(touched)
}

