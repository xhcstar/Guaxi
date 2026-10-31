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
    AddressCard, Contact, ContactGroup, ContactMessage, ContactQuery, ContactSuggestion,
    ExpandedMember, ImportReport,
};
use rusqlite::OptionalExtension;
use tauri::command;

#[command]
pub async fn mail_list_contacts(query: ContactQuery) -> Result<Vec<Contact>, String> {
    store::list(&query)
}

#[command]
pub async fn mail_get_contact(id: String) -> Result<Option<Contact>, String> {
    store::get(&id)
}

/// 阅读区地址悬停卡片：一次批量查完发件人 / 收件人 / 抄送。
#[command]
pub async fn mail_address_cards(addresses: Vec<String>) -> Result<Vec<AddressCard>, String> {
    store::address_cards(&addresses)
}

/// 把一个地址收进通讯录（地址卡片上的「加进通讯录」）。
#[command]
pub async fn mail_save_address_contact(
    email: String,
    display_name: String,
) -> Result<Contact, String> {
    store::save_address_contact(&email, &display_name)
}

/// 按邮箱地址查联系人：正文里的地址被点开时看「有没有存过」。
#[command]
pub async fn mail_contact_by_email(email: String) -> Result<Option<Contact>, String> {
    store::get_by_email(&email)
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
    store::suggest(&prefix, 12)
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

/// 展开通讯组为成员列表（嵌套组递归解开、地址去重）。
///
/// 写信页不再用它把组拆成一串地址——地址栏只保留组名，发送时才在 Rust 侧展开；
/// 这里主要供通讯录详情页展示「这个组最终包含哪些人」。
#[command]
pub async fn mail_expand_contact_group(id: String) -> Result<Vec<ExpandedMember>, String> {
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
    let group_cards = text.matches("X-ADDRESSBOOKSERVER-KIND:group").count() as u32;
    let count = (text.matches("BEGIN:VCARD").count() as u32).saturating_sub(group_cards);
    std::fs::write(&path, text).map_err(|e| format!("写入文件失败（{path}）: {e}"))?;
    Ok(count)
}

/// 从已有邮件里一次性重建通讯录（升级 / 换了账户后补数据用）。
#[command]
pub async fn mail_contacts_rebuild_from_mail(app: tauri::AppHandle) -> Result<u32, String> {
    let _ = app;
    rebuild_from_mail_sync()
}

/// 一次性补齐通讯录里的「收件人」。
///
/// 老版本把 `to_json` 当成通讯录的 `{label,value}` 解析，必然失败——结果通讯录里
/// 只有发件人，邮件组 / 分发列表地址（常常只出现在收件人里）永远收不进来
/// （真实反馈）。解析修好后，升级时自动扫一遍，用户不用自己去找「从邮件重建」。
pub fn backfill_recipients_once() -> Result<u32, String> {
    let conn = crate::commands::mail::store::connection()?;
    let done: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key='contacts_recipients_v1'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if done.as_deref() == Some("1") {
        return Ok(0);
    }
    // 先落标记再扫：中途失败不会每次启动都全量重扫；用户仍可手动「从邮件重建」。
    let _ = conn.execute(
        "INSERT INTO meta (key, value) VALUES ('contacts_recipients_v1', '1')
         ON CONFLICT(key) DO UPDATE SET value='1'",
        [],
    );
    drop(conn);
    rebuild_from_mail_sync()
}

fn rebuild_from_mail_sync() -> Result<u32, String> {
    let conn = crate::commands::mail::store::connection()?;
    let accounts = crate::commands::mail::store::list_accounts()?;
    let own: Vec<String> = accounts
        .iter()
        .flat_map(|a| vec![a.email.clone(), a.login_name()])
        .collect();

    let mut stmt = conn
        .prepare(
            // 已发送也要扫：邮件组 / 分发列表地址常常只出现在发出的邮件里。
            // 草稿不算（还没发出去的收件人不是「往来过」）。
            "SELECT from_name, from_addr, to_json, cc_json, date FROM messages
             WHERE folder NOT IN (SELECT name FROM folders WHERE role = 'drafts')",
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

    let mut entries: Vec<(String, String, i64)> = Vec::new();
    let mut touched = 0u32;
    for row in rows.flatten() {
        let (from_name, from_addr, to_json, cc_json, date) = row;
        // to_json / cc_json 是 `[{"name","address"}]`（MailAddress），不是
        // 通讯录的 {label,value}：按 LabeledValue 解析会整段失败，收件人全丢。
        if !from_addr.trim().is_empty() {
            entries.push((from_name, from_addr, date));
        }
        for item in recipients_of(&to_json) {
            entries.push((item.label, item.value, date));
        }
        for item in recipients_of(&cc_json) {
            entries.push((item.label, item.value, date));
        }
        touched += 1;
    }
    // 一次性写库：重建要扫上万封，一封一个连接会明显拖慢。
    store::touch_addresses(&own, &entries)?;
    Ok(touched)
}

fn recipients_of(json: &str) -> Vec<model::LabeledValue> {
    serde_json::from_str::<Vec<crate::commands::mail::model::MailAddress>>(json)
        .unwrap_or_default()
        .into_iter()
        .filter(|item| !item.address.trim().is_empty())
        .map(|item| model::LabeledValue {
            label: item.name,
            value: item.address,
        })
        .collect()
}


#[cfg(test)]
mod recipients_tests {
    use super::recipients_of;

    #[test]
    fn reads_recipients_from_message_json() {
        // messages.to_json 存的是解析 MIME 时的 MailAddress（name/address），
        // 不是通讯录的 {label,value}：以前按后者解析整段失败，收件人全丢。
        let json = r#"[{"name":"R&D Department","address":"team@example.com"},{"name":"","address":"a@b.cn"}]"#;
        let items = recipients_of(json);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "R&D Department");
        assert_eq!(items[0].value, "team@example.com");
        assert_eq!(items[1].value, "a@b.cn");
    }

    #[test]
    fn skips_empty_and_broken_json() {
        assert!(recipients_of("[]").is_empty());
        assert!(recipients_of("not json").is_empty());
        assert!(recipients_of(r#"[{"name":"x","address":"  "}]"#).is_empty());
    }
}
