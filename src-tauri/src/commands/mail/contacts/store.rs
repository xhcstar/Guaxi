//! 通讯录持久化：联系人、通讯组、与邮件的往来索引。
//!
//! 复用邮箱库（同一个 `mail.db`），不另开一个库：联系人详情页要按邮箱反查往来邮件，
//! 分库就得做跨库 join，而且事务边界会碎成两段。
//!
//! 去重规则（自动收录与导入都走它）：**同一个邮箱地址只能属于一个联系人**。
//! 邮件世界里「同一个人」的判定就是地址；姓名会被写成十几种（张三 / Zhang San / 张三（财务）），
//! 按姓名合并一定会把不同的人并掉。

use super::model::{
    Contact, ContactGroup, ContactMessage, ContactQuery, ContactSuggestion, GroupMember,
    ImportReport, LabeledValue,
};
use crate::commands::mail::store;
use rusqlite::{params, OptionalExtension, Row};
use uuid::Uuid;

pub fn init() -> Result<(), String> {
    let conn = store::connection()?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS contacts (
            id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL DEFAULT '',
            given_name TEXT NOT NULL DEFAULT '',
            family_name TEXT NOT NULL DEFAULT '',
            emails_json TEXT NOT NULL DEFAULT '[]',
            phones_json TEXT NOT NULL DEFAULT '[]',
            organization TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT '',
            birthday TEXT NOT NULL DEFAULT '',
            website TEXT NOT NULL DEFAULT '',
            address TEXT NOT NULL DEFAULT '',
            avatar TEXT NOT NULL DEFAULT '',
            starred INTEGER NOT NULL DEFAULT 0,
            source TEXT NOT NULL DEFAULT 'manual',
            first_seen_at INTEGER NOT NULL DEFAULT 0,
            last_seen_at INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_contacts_name ON contacts (display_name);
        CREATE INDEX IF NOT EXISTS idx_contacts_last ON contacts (last_seen_at DESC);

        -- 邮箱 → 联系人 的规范索引。单列唯一：一个地址只能挂在一个人身上。
        CREATE TABLE IF NOT EXISTS contact_emails (
            email TEXT PRIMARY KEY,
            contact_id TEXT NOT NULL,
            label TEXT NOT NULL DEFAULT '',
            position INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_contact_emails_contact ON contact_emails (contact_id);

        CREATE TABLE IF NOT EXISTS contact_groups (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            description TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS contact_group_members (
            group_id TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            kind TEXT NOT NULL DEFAULT 'contact',
            contact_id TEXT NOT NULL DEFAULT '',
            email TEXT NOT NULL DEFAULT '',
            name TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (group_id, position)
        );
        CREATE INDEX IF NOT EXISTS idx_group_members_contact ON contact_group_members (contact_id);
        "#,
    )
    .map_err(|e| format!("创建通讯录表失败: {e}"))?;
    Ok(())
}

fn normalize_email(email: &str) -> String {
    email.trim().trim_matches(['<', '>']).to_ascii_lowercase()
}

fn row_to_contact(row: &Row<'_>) -> rusqlite::Result<Contact> {
    let emails_json: String = row.get(4)?;
    let phones_json: String = row.get(5)?;
    Ok(Contact {
        id: row.get(0)?,
        display_name: row.get(1)?,
        given_name: row.get(2)?,
        family_name: row.get(3)?,
        emails: serde_json::from_str(&emails_json).unwrap_or_default(),
        phones: serde_json::from_str(&phones_json).unwrap_or_default(),
        organization: row.get(6)?,
        title: row.get(7)?,
        note: row.get(8)?,
        birthday: row.get(9)?,
        website: row.get(10)?,
        address: row.get(11)?,
        avatar: row.get(12)?,
        groups: Vec::new(),
        starred: row.get::<_, i32>(13)? != 0,
        source: row.get(14)?,
        first_seen_at: row.get(15)?,
        last_seen_at: row.get(16)?,
        message_count: 0,
        unread_count: 0,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

const CONTACT_COLS: &str = "id, display_name, given_name, family_name, emails_json, phones_json, \
    organization, title, note, birthday, website, address, avatar, starred, source, \
    first_seen_at, last_seen_at, created_at, updated_at";

/// 写联系人并把邮箱索引重建。`conn` 由调用方提供，便于导入时包在一个事务里。
fn write_contact(conn: &rusqlite::Connection, contact: &Contact) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        r#"INSERT INTO contacts (id, display_name, given_name, family_name, emails_json, phones_json,
                organization, title, note, birthday, website, address, avatar, starred, source,
                first_seen_at, last_seen_at, created_at, updated_at)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)
           ON CONFLICT(id) DO UPDATE SET
                display_name=excluded.display_name, given_name=excluded.given_name,
                family_name=excluded.family_name, emails_json=excluded.emails_json,
                phones_json=excluded.phones_json, organization=excluded.organization,
                title=excluded.title, note=excluded.note, birthday=excluded.birthday,
                website=excluded.website, address=excluded.address, avatar=excluded.avatar,
                starred=excluded.starred, source=excluded.source,
                first_seen_at=excluded.first_seen_at, last_seen_at=excluded.last_seen_at,
                updated_at=excluded.updated_at"#,
        params![
            contact.id,
            contact.display_name,
            contact.given_name,
            contact.family_name,
            serde_json::to_string(&contact.emails).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&contact.phones).unwrap_or_else(|_| "[]".into()),
            contact.organization,
            contact.title,
            contact.note,
            contact.birthday,
            contact.website,
            contact.address,
            contact.avatar,
            contact.starred as i32,
            contact.source,
            contact.first_seen_at,
            contact.last_seen_at,
            if contact.created_at == 0 { now } else { contact.created_at },
            now,
        ],
    )
    .map_err(|e| format!("保存联系人失败: {e}"))?;

    // 邮箱索引：先删该联系人的旧行，再写新行；若某个地址已属于别人，就把它从对方那里摘掉
    // （「同一个地址只属于一个人」这条规则必须由库层保证，否则自动收录会把同一个人拆成两条）。
    conn.execute(
        "DELETE FROM contact_emails WHERE contact_id=?1",
        params![contact.id],
    )
    .ok();
    for (position, email) in contact.emails.iter().enumerate() {
        let addr = normalize_email(&email.value);
        if addr.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT INTO contact_emails (email, contact_id, label, position) VALUES (?1,?2,?3,?4)
             ON CONFLICT(email) DO UPDATE SET contact_id=excluded.contact_id,
                 label=excluded.label, position=excluded.position",
            params![addr, contact.id, email.label, position as i64],
        )
        .map_err(|e| format!("写入邮箱索引失败: {e}"))?;
    }
    Ok(())
}

pub fn upsert(contact: Contact) -> Result<Contact, String> {
    let mut contact = contact;
    if contact.id.trim().is_empty() {
        contact.id = Uuid::new_v4().to_string();
    }
    if contact.source.trim().is_empty() {
        contact.source = "manual".into();
    }
    let conn = store::connection()?;
    write_contact(&conn, &contact)?;
    Ok(contact)
}

pub fn delete(id: &str) -> Result<(), String> {
    let mut conn = store::connection()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM contacts WHERE id=?1", params![id]).ok();
    tx.execute("DELETE FROM contact_emails WHERE contact_id=?1", params![id])
        .ok();
    // 组成员里的引用一起清掉，避免组里留下「空成员」。
    tx.execute(
        "DELETE FROM contact_group_members WHERE kind='contact' AND contact_id=?1",
        params![id],
    )
    .ok();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get(id: &str) -> Result<Option<Contact>, String> {
    let conn = store::connection()?;
    let mut contact = conn
        .query_row(
            &format!("SELECT {CONTACT_COLS} FROM contacts WHERE id=?1"),
            params![id],
            row_to_contact,
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(c) = contact.as_mut() {
        attach_groups(&conn, std::slice::from_mut(c))?;
        attach_counts(&conn, std::slice::from_mut(c))?;
    }
    Ok(contact)
}

/// 批量补分组与往来计数：列表页一次查完，避免逐行 N+1。
fn attach_groups(conn: &rusqlite::Connection, contacts: &mut [Contact]) -> Result<(), String> {
    if contacts.is_empty() {
        return Ok(());
    }
    let mut stmt = conn
        .prepare(
            "SELECT m.contact_id, g.id FROM contact_group_members m
             JOIN contact_groups g ON g.id = m.group_id
             WHERE m.kind='contact'",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for row in rows.flatten() {
        map.entry(row.0).or_default().push(row.1);
    }
    for contact in contacts.iter_mut() {
        contact.groups = map.get(&contact.id).cloned().unwrap_or_default();
    }
    Ok(())
}

/// 往来次数：按该联系人的所有邮箱去 messages 里数。
fn attach_counts(conn: &rusqlite::Connection, contacts: &mut [Contact]) -> Result<(), String> {
    if contacts.is_empty() {
        return Ok(());
    }
    let mut stmt = conn
        .prepare(
            "SELECT COUNT(*), SUM(CASE WHEN seen=0 AND outgoing=0 THEN 1 ELSE 0 END)
             FROM messages WHERE LOWER(from_addr) = ?1",
        )
        .map_err(|e| e.to_string())?;
    for contact in contacts.iter_mut() {
        let mut total = 0u32;
        let mut unread = 0u32;
        for email in contact.all_emails() {
            if let Ok((count, unseen)) = stmt.query_row(params![email], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?))
            }) {
                total += count as u32;
                unread += unseen.unwrap_or(0) as u32;
            }
        }
        contact.message_count = total;
        contact.unread_count = unread;
      }
    Ok(())
}

pub fn list(query: &ContactQuery) -> Result<Vec<Contact>, String> {
    let conn = store::connection()?;
    let limit = query.limit.unwrap_or(500).min(5000);
    let offset = query.offset.unwrap_or(0);

    let mut sql = format!("SELECT {CONTACT_COLS} FROM contacts WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if !query.text.trim().is_empty() {
        // 关键词同时匹配姓名、邮箱、组织、电话、备注。邮箱走索引表，其余走 LIKE。
        sql.push_str(
            " AND (display_name LIKE ?1 ESCAPE '\\' OR organization LIKE ?1 ESCAPE '\\' \
             OR phones_json LIKE ?1 ESCAPE '\\' OR note LIKE ?1 ESCAPE '\\' \
             OR id IN (SELECT contact_id FROM contact_emails WHERE email LIKE ?1 ESCAPE '\\'))",
        );
        args.push(Box::new(format!(
            "%{}%",
            query.text.trim().replace('%', "\\%").replace('_', "\\_")
        )));
    }
    if let Some(source) = query.source.as_ref().filter(|s| !s.trim().is_empty()) {
        sql.push_str(&format!(" AND source = ?{}", args.len() + 1));
        args.push(Box::new(source.clone()));
    }
    if query.starred_only {
        sql.push_str(" AND starred = 1");
    }
    if !query.group_ids.is_empty() {
        let placeholders = (0..query.group_ids.len())
            .map(|i| format!("?{}", args.len() + i + 1))
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(
            " AND id IN (SELECT contact_id FROM contact_group_members WHERE group_id IN ({placeholders}))"
        ));
        for id in &query.group_ids {
            args.push(Box::new(id.clone()));
        }
    }
    if query.frequent_only {
        sql.push_str(" AND last_seen_at > 0");
    }

    sql.push_str(if query.frequent_only {
        " ORDER BY last_seen_at DESC, display_name ASC"
    } else {
        " ORDER BY display_name ASC, last_seen_at DESC"
    });
    sql.push_str(&format!(" LIMIT ?{} OFFSET ?{}", args.len() + 1, args.len() + 2));
    args.push(Box::new(limit as i64));
    args.push(Box::new(offset as i64));

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("查询联系人失败: {e}"))?;
    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let mut contacts = stmt
        .query_map(refs.as_slice(), row_to_contact)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;

    attach_groups(&conn, &mut contacts)?;
    attach_counts(&conn, &mut contacts)?;

    if !query.include_unused {
        contacts.retain(|c| c.message_count > 0);
    }
    Ok(contacts)
}

// ---------------------------------------------------------------------------
// 自动收录
// ---------------------------------------------------------------------------

/// 从一封邮件的发件人 / 收件人里自动收录联系人。
///
/// 规则：
/// - 跳过自己账户的地址（不然通讯录里全是自己）；
/// - 跳过明显的机器人地址（`noreply@`、`no-reply@`、`mailer-daemon@`、`postmaster@`）——
///   这些永远不需要回信，收录进来只会污染补全列表；
/// - 已存在则只更新 `last_seen_at` 与（空的话）姓名，不覆盖用户改过的字段。
pub fn touch_from_message(
    own_addresses: &[String],
    from_name: &str,
    from_addr: &str,
    to: &[LabeledValue],
    cc: &[LabeledValue],
    when: i64,
) -> Result<(), String> {
    let mut seen: Vec<(String, String)> = Vec::new();
    if !from_addr.trim().is_empty() {
        seen.push((from_name.to_string(), from_addr.to_string()));
    }
    for list in [to, cc] {
        for item in list {
            if !item.value.trim().is_empty() {
                seen.push((item.label.clone(), item.value.clone()));
            }
        }
    }

    let conn = store::connection()?;
    for (name, addr) in seen {
        let email = normalize_email(&addr);
        if email.is_empty() {
            continue;
        }
        if own_addresses.iter().any(|own| normalize_email(own) == email) {
            continue;
        }
        if is_robot_address(&email) {
            continue;
        }
        let existing: Option<String> = conn
            .query_row(
                "SELECT contact_id FROM contact_emails WHERE email=?1",
                params![email],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(id) = existing {
            conn.execute(
                "UPDATE contacts SET last_seen_at=MAX(last_seen_at, ?1),
                     display_name=CASE WHEN display_name='' THEN ?2 ELSE display_name END,
                     first_seen_at=CASE WHEN first_seen_at=0 THEN ?1 ELSE first_seen_at END
                 WHERE id=?3",
                params![when, name.trim(), id],
            )
            .ok();
            continue;
        }
        let contact = Contact {
            id: Uuid::new_v4().to_string(),
            display_name: name.trim().to_string(),
            emails: vec![LabeledValue {
                label: "work".into(),
                value: email.clone(),
            }],
            source: "auto".into(),
            first_seen_at: when,
            last_seen_at: when,
            ..Default::default()
        };
        write_contact(&conn, &contact)?;
    }
    Ok(())
}

fn is_robot_address(email: &str) -> bool {
    const ROBOT_PREFIXES: [&str; 12] = [
        "noreply",
        "no-reply",
        "no_reply",
        "donotreply",
        "do-not-reply",
        "mailer-daemon",
        "postmaster",
        "bounce",
        "notifications",
        "notification",
        "automated",
        "system",
    ];
    let local = email.split('@').next().unwrap_or("");
    ROBOT_PREFIXES.iter().any(|p| local == *p || local.starts_with(&format!("{p}+")))
}

// ---------------------------------------------------------------------------
// 自动补全
// ---------------------------------------------------------------------------

/// 写信地址栏的补全。三路来源合并排序：
/// 1. 通讯录（命中姓名或邮箱）；
/// 2. 历史往来地址（不在通讯录里的，从 messages 里捞）；
/// 3. 通讯组（按名字命中，选中后展开成成员）。
pub fn suggest(prefix: &str, limit: u32) -> Result<Vec<ContactSuggestion>, String> {
    let q = prefix.trim().to_lowercase();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let conn = store::connection()?;
    let like = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));
    let mut out: Vec<ContactSuggestion> = Vec::new();

    // 1) 通讯录
    {
        let mut stmt = conn
            .prepare(
                "SELECT c.id, c.display_name, e.email, e.label FROM contact_emails e
                 JOIN contacts c ON c.id = e.contact_id
                 WHERE LOWER(c.display_name) LIKE ?1 ESCAPE '\\' OR e.email LIKE ?1 ESCAPE '\\'
                 ORDER BY c.starred DESC, c.last_seen_at DESC
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![like, limit], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for (id, name, email) in rows.flatten() {
            let display = if name.trim().is_empty() {
                email.clone()
            } else {
                format!("{name} <{email}>")
            };
            out.push(ContactSuggestion {
                display,
                email,
                contact_id: Some(id),
                kind: "contact".into(),
                score: score_match(&name, &q) + 20,
                member_count: 0,
            });
        }
    }

    // 2) 历史地址（去重：已在通讯录结果里的不重复出现）
    {
        let known: std::collections::HashSet<String> =
            out.iter().map(|s| normalize_email(&s.email)).collect();
        let mut stmt = conn
            .prepare(
                "SELECT from_addr, from_name, MAX(date) AS last FROM messages
                 WHERE from_addr <> '' AND (LOWER(from_addr) LIKE ?1 ESCAPE '\\'
                       OR LOWER(from_name) LIKE ?1 ESCAPE '\\')
                 GROUP BY from_addr ORDER BY last DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![like, limit], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for (email, name) in rows.flatten() {
            if known.contains(&normalize_email(&email)) {
                continue;
            }
            let display = if name.trim().is_empty() {
                email.clone()
            } else {
                format!("{name} <{email}>")
            };
            out.push(ContactSuggestion {
                display,
                email,
                contact_id: None,
                kind: "history".into(),
                score: score_match(&name, &q) + 10,
                member_count: 0,
            });
        }
    }

    // 3) 通讯组
    {
        let mut stmt = conn
            .prepare(
                "SELECT g.id, g.name, COUNT(m.contact_id) FROM contact_groups g
                 LEFT JOIN contact_group_members m ON m.group_id = g.id
                 WHERE LOWER(g.name) LIKE ?1 ESCAPE '\\'
                 GROUP BY g.id ORDER BY g.name LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![like, limit], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for (id, name, count) in rows.flatten() {
            out.push(ContactSuggestion {
                display: format!("{name}（通讯组 · {count} 人）"),
                email: format!("group:{id}"),
                contact_id: Some(id),
                kind: "group".into(),
                score: score_match(&name, &q) + 30,
                member_count: count as u32,
            });
        }
    }

    out.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.display.cmp(&b.display)));
    out.truncate(limit as usize);
    Ok(out)
}

/// 简单的匹配打分：前缀命中 > 词首命中 > 包含。
fn score_match(text: &str, q: &str) -> u32 {
    let lower = text.to_lowercase();
    if lower.is_empty() {
        return 0;
    }
    if lower.starts_with(q) {
        70
    } else if lower.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(q)) {
        45
    } else {
        20
    }
}

// ---------------------------------------------------------------------------
// 与邮件的往来
// ---------------------------------------------------------------------------

/// 列出与某联系人的往来邮件。`direction` 由「发件人是否是他」推出。
pub fn messages_with(contact: &Contact, limit: u32) -> Result<Vec<ContactMessage>, String> {
    let emails = contact.all_emails();
    if emails.is_empty() {
        return Ok(Vec::new());
    }
    let conn = store::connection()?;
    let placeholders = (0..emails.len())
        .map(|i| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT account_id, folder, uid, subject, date, snippet,
                CASE WHEN LOWER(from_addr) IN ({placeholders}) THEN 'in' ELSE 'out' END
         FROM messages
         WHERE LOWER(from_addr) IN ({placeholders}) OR LOWER(to_json) LIKE ?{}
         ORDER BY date DESC LIMIT ?{}",
        emails.len() + 1,
        emails.len() + 2
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    for email in &emails {
        args.push(Box::new(email.clone()));
    }
    for email in &emails {
        args.push(Box::new(email.clone()));
    }
    args.push(Box::new(format!("%{}%", emails[0])));
    args.push(Box::new(limit as i64));

    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt
        .query_map(refs.as_slice(), |r| {
            Ok(ContactMessage {
                account_id: r.get(0)?,
                folder: r.get(1)?,
                uid: r.get(2)?,
                subject: r.get(3)?,
                date: r.get(4)?,
                snippet: r.get(5)?,
                direction: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// 通讯组
// ---------------------------------------------------------------------------

pub fn list_groups() -> Result<Vec<ContactGroup>, String> {
    let conn = store::connection()?;
    let mut stmt = conn
        .prepare("SELECT id, name, description, created_at, updated_at FROM contact_groups ORDER BY name")
        .map_err(|e| e.to_string())?;
    let mut groups = stmt
        .query_map([], |r| {
            Ok(ContactGroup {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                members: Vec::new(),
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;

    let mut count_stmt = conn
        .prepare(
            "SELECT kind, contact_id, email, name FROM contact_group_members
             WHERE group_id=?1 ORDER BY position",
        )
        .map_err(|e| e.to_string())?;
    for group in groups.iter_mut() {
        let members = count_stmt
            .query_map(params![group.id], |r| {
                Ok(GroupMember {
                    kind: r.get(0)?,
                    contact_id: r.get(1)?,
                    email: r.get(2)?,
                    name: r.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?
            .flatten()
            .collect();
        group.members = members;
    }
    Ok(groups)
}

pub fn upsert_group(group: ContactGroup) -> Result<ContactGroup, String> {
    let mut group = group;
    if group.id.trim().is_empty() {
        group.id = Uuid::new_v4().to_string();
    }
    let now = chrono::Utc::now().timestamp();
    let mut conn = store::connection()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO contact_groups (id, name, description, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(id) DO UPDATE SET name=excluded.name, description=excluded.description,
             updated_at=excluded.updated_at",
        params![
            group.id,
            group.name,
            group.description,
            if group.created_at == 0 { now } else { group.created_at },
            now
        ],
    )
    .map_err(|e| format!("保存通讯组失败: {e}"))?;
    tx.execute(
        "DELETE FROM contact_group_members WHERE group_id=?1",
        params![group.id],
    )
    .ok();
    for (position, member) in group.members.iter().enumerate() {
        tx.execute(
            "INSERT INTO contact_group_members (group_id, position, kind, contact_id, email, name)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                group.id,
                position as i64,
                if member.kind.is_empty() { "contact" } else { &member.kind },
                member.contact_id,
                member.email,
                member.name
            ],
        )
        .map_err(|e| format!("写入组成员失败: {e}"))?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(group)
}

pub fn delete_group(id: &str) -> Result<(), String> {
    let mut conn = store::connection()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM contact_groups WHERE id=?1", params![id]).ok();
    tx.execute("DELETE FROM contact_group_members WHERE group_id=?1", params![id])
        .ok();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 把一个通讯组展开成地址列表（写信时点选组、或规则转发到组）。
pub fn expand_group(id: &str) -> Result<Vec<String>, String> {
    let groups = list_groups()?;
    let Some(group) = groups.into_iter().find(|g| g.id == id) else {
        return Ok(Vec::new());
    };
    let mut out: Vec<String> = Vec::new();
    for member in &group.members {
        let addr = if member.kind == "email" {
            member.email.trim().to_string()
        } else {
            get(&member.contact_id)
                .ok()
                .flatten()
                .map(|c| c.primary_email())
                .unwrap_or_default()
        };
        if !addr.trim().is_empty() && !out.iter().any(|e| normalize_email(e) == normalize_email(&addr)) {
            out.push(addr);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// 导入 / 导出
// ---------------------------------------------------------------------------

/// 导入 vCard 文本。已存在的邮箱合并（补空字段、追加电话），不重复建人。
pub fn import_vcards(text: &str) -> Result<ImportReport, String> {
    let (contacts, _, errors) = super::vcard::parse(text);
    let mut report = ImportReport {
        skipped: errors.len() as u32,
        errors,
        ..Default::default()
    };
    let mut conn = store::connection()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for contact in contacts {
        // 用第一个邮箱判断是否已存在。
        let primary = contact.primary_email().to_ascii_lowercase();
        let existing_id: Option<String> = if primary.is_empty() {
            None
        } else {
            tx.query_row(
                "SELECT contact_id FROM contact_emails WHERE email=?1",
                params![primary],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
        };

        match existing_id {
            Some(id) => {
                let existing = tx
                    .query_row(
                        &format!("SELECT {CONTACT_COLS} FROM contacts WHERE id=?1"),
                        params![id],
                        row_to_contact,
                    )
                    .optional()
                    .map_err(|e| e.to_string())?;
                if let Some(mut merged) = existing {
                    // 只补空字段：用户手工改过的名字不能被导入包里的旧名字覆盖。
                    if merged.display_name.trim().is_empty() {
                        merged.display_name = contact.display_name.clone();
                    }
                    if merged.organization.trim().is_empty() {
                        merged.organization = contact.organization.clone();
                    }
                    if merged.title.trim().is_empty() {
                        merged.title = contact.title.clone();
                    }
                    if merged.note.trim().is_empty() {
                        merged.note = contact.note.clone();
                    }
                    for email in contact.emails {
                        if !merged
                            .all_emails()
                            .iter()
                            .any(|e| *e == normalize_email(&email.value))
                        {
                            merged.emails.push(email);
                        }
                    }
                    for phone in contact.phones {
                        if !merged.phones.iter().any(|p| p.value == phone.value) {
                            merged.phones.push(phone);
                        }
                    }
                    write_contact(&tx, &merged)?;
                    report.merged += 1;
                }
            }
            None => {
                write_contact(&tx, &contact)?;
                report.imported += 1;
            }
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(report)
}

/// 导出全部联系人为一个 .vcf 文本。
pub fn export_vcards(group_id: Option<&str>) -> Result<String, String> {
    let contacts = list(&ContactQuery::default())?;
    let selected: Vec<Contact> = match group_id {
        Some(id) => contacts
            .into_iter()
            .filter(|c| c.groups.iter().any(|g| g == id))
            .collect(),
        None => contacts,
    };
    let mut out = String::new();
    for contact in &selected {
        out.push_str(&super::vcard::encode_contact(contact));
    }
    if out.is_empty() {
        return Err("没有可导出的联系人".to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robot_addresses_are_skipped() {
        assert!(is_robot_address("noreply@jd.com"));
        assert!(is_robot_address("no-reply+123@x.com"));
        assert!(is_robot_address("mailer-daemon@x.com"));
        assert!(!is_robot_address("zhangsan@corp.com"));
    }

    #[test]
    fn normalize_strips_brackets_and_case() {
        assert_eq!(normalize_email(" <Alice@Example.COM> "), "alice@example.com");
    }

    #[test]
    fn score_prefers_prefix() {
        assert!(score_match("张三", "张") > score_match("我是张三", "张"));
        assert!(score_match("Zhang San", "zh") > score_match("San Zhang", "zh"));
    }
}
