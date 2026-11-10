//! 通讯录持久化：联系人、通讯组、与邮件的往来索引。
//!
//! 复用邮箱库（同一个 `mail.db`），不另开一个库：联系人详情页要按邮箱反查往来邮件，
//! 分库就得做跨库 join，而且事务边界会碎成两段。
//!
//! 去重规则（自动收录与导入都走它）：**同一个邮箱地址只能属于一个联系人**。
//! 邮件世界里「同一个人」的判定就是地址；姓名会被写成十几种（张三 / Zhang San / 张三（财务）），
//! 按姓名合并一定会把不同的人并掉。

use super::model::{
    AddressCard, Contact, ContactGroup, ContactMessage, ContactQuery, ContactSuggestion,
    ExpandedMember, GroupMember, ImportReport, LabeledValue,
};
use crate::commands::mail::store;
use rusqlite::{params, OptionalExtension, Row};
use std::collections::HashMap;
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
    // 旧库补列：嵌套通讯组建库时还没有 target_group_id。
    ensure_column(
        &conn,
        "contact_group_members",
        "target_group_id",
        "target_group_id TEXT NOT NULL DEFAULT ''",
    )?;
    Ok(())
}

/// `ALTER TABLE ... ADD COLUMN` 只在缺列时执行：SQLite 没有 `IF NOT EXISTS`。
fn ensure_column(
    conn: &rusqlite::Connection,
    table: &str,
    column: &str,
    ddl: &str,
) -> Result<(), String> {
    let present: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name='{column}'"),
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if present == 0 {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {ddl}"), [])
            .map_err(|e| format!("升级 {table} 表结构失败: {e}"))?;
    }
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
    let mut entries: Vec<(String, String, i64)> = Vec::new();
    if !from_addr.trim().is_empty() {
        entries.push((from_name.to_string(), from_addr.to_string(), when));
    }
    for list in [to, cc] {
        for item in list {
            if !item.value.trim().is_empty() {
                entries.push((item.label.clone(), item.value.clone(), when));
            }
        }
    }
    touch_addresses(own_addresses, &entries)
}

/// 批量收录：一次连接，同一批里按地址合并。
///
/// 一次同步可能收几十封、每封十几个收件人；以前一封一个连接、每个地址各查两次库，
/// 光通讯录就能占掉收信的一截时间。合并之后只查一次、每个地址只写一次。
pub fn touch_addresses(
    own_addresses: &[String],
    entries: &[(String, String, i64)],
) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }
    let own: Vec<String> = own_addresses
        .iter()
        .map(|addr| normalize_email(addr))
        .collect();
    let mut merged: std::collections::HashMap<String, (String, i64)> =
        std::collections::HashMap::new();
    for (name, addr, when) in entries {
        let email = normalize_email(addr);
        if email.is_empty() {
            continue;
        }
        let slot = merged
            .entry(email)
            .or_insert_with(|| (String::new(), 0));
        if slot.0.trim().is_empty() && !name.trim().is_empty() {
            slot.0 = name.trim().to_string();
        }
        slot.1 = slot.1.max(*when);
    }
    if merged.is_empty() {
        return Ok(());
    }
    let conn = store::connection()?;
    for (email, (name, when)) in merged {
        if own.iter().any(|own| own == &email) {
            continue;
        }
        if is_robot_address(&email) {
            continue;
        }
        touch_one(&conn, &name, &email, when)?;
    }
    Ok(())
}

fn touch_one(
    conn: &rusqlite::Connection,
    name: &str,
    email: &str,
    when: i64,
) -> Result<(), String> {
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
        return Ok(());
    }
    let contact = Contact {
        id: Uuid::new_v4().to_string(),
        display_name: name.trim().to_string(),
        emails: vec![LabeledValue {
            label: "work".into(),
            value: email.to_string(),
        }],
        source: "auto".into(),
        first_seen_at: when,
        last_seen_at: when,
        ..Default::default()
    };
    write_contact(conn, &contact)
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

    // 3) 通讯组：组名、说明、成员姓名/邮箱都能命中。
    {
        let mut stmt = conn
            .prepare(
                "SELECT g.id, g.name,
                        CASE WHEN LOWER(g.name) LIKE ?1 ESCAPE '\\'
                                  OR LOWER(g.description) LIKE ?1 ESCAPE '\\'
                             THEN 1 ELSE 0 END AS name_hit
                 FROM contact_groups g
                 WHERE LOWER(g.name) LIKE ?1 ESCAPE '\\'
                    OR LOWER(g.description) LIKE ?1 ESCAPE '\\'
                    OR EXISTS (
                        SELECT 1 FROM contact_group_members m
                        LEFT JOIN contacts c ON c.id = m.contact_id
                        WHERE m.group_id = g.id
                          AND (LOWER(c.display_name) LIKE ?1 ESCAPE '\\'
                               OR LOWER(m.email) LIKE ?1 ESCAPE '\\'
                               OR LOWER(m.name) LIKE ?1 ESCAPE '\\')
                    )
                 ORDER BY g.name LIMIT ?2",
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
            .map_err(|e| e.to_string())?
            .flatten()
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            // 只有真的命中通讯组时才整表读联系人来算人数：绝大多数按键输入
            // 只命中联系人，不该为此把几千条联系人读一遍。
            let groups_all = list_groups()?;
            let contacts_all = list(&ContactQuery::default())?;
            let by_id: HashMap<String, Contact> =
                contacts_all.into_iter().map(|c| (c.id.clone(), c)).collect();
            for (id, name, name_hit) in rows {
                let mut path = Vec::new();
                let mut members = Vec::new();
                let count = match expand_into(&groups_all, &by_id, &id, &mut path, &mut members) {
                    Ok(()) => members.len() as u32,
                    Err(_) => 0,
                };
                // 按成员命中时给一个固定的中档分：让「组里有这个人」的组仍能挤进前几条，
                // 但不要盖过直接命中联系人姓名/邮箱的结果。
                let score = if name_hit == 1 {
                    score_match(&name, &q) + 30
                } else {
                    55
                };
                out.push(ContactSuggestion {
                    display: name.clone(),
                    email: format!("group:{id}"),
                    contact_id: Some(id),
                    kind: "group".into(),
                    score,
                    member_count: count,
                });
            }
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

/// `messages_with` 的 SQL 与参数。
///
/// 拆成纯函数是为了能直接核对「占位符个数 == 参数个数」：`?1..?n` 这组编号在
/// `direction` 与 `WHERE` 里各出现一次（SQLite 允许复用编号，只算一个参数），
/// 收件人 LIKE 必须从 `?n+1` 往后排。老代码按「两遍地址」算了偏移，
/// 1 个地址时绑 4 个参数给 3 个占位符，rusqlite 直接报
/// 「Wrong number of parameters passed to query」，联系人详情整块打不开。
fn messages_with_sql(emails: &[String], limit: u32) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
    let n = emails.len();
    let placeholders = (1..=n)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT account_id, folder, uid, subject, date, snippet,
                CASE WHEN LOWER(from_addr) IN ({placeholders}) THEN 'in' ELSE 'out' END
         FROM messages
         WHERE LOWER(from_addr) IN ({placeholders}) OR LOWER(to_json) LIKE ?{}
         ORDER BY date DESC LIMIT ?{}",
        n + 1,
        n + 2
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    for email in emails {
        args.push(Box::new(email.clone()));
    }
    args.push(Box::new(format!("%{}%", emails[0])));
    args.push(Box::new(limit as i64));
    (sql, args)
}

/// 列出与某联系人的往来邮件。`direction` 由「发件人是否是他」推出。
pub fn messages_with(contact: &Contact, limit: u32) -> Result<Vec<ContactMessage>, String> {
    let emails = contact.all_emails();
    if emails.is_empty() {
        return Ok(Vec::new());
    }
    let conn = store::connection()?;
    let (sql, args) = messages_with_sql(&emails, limit);
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
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
// 地址悬停卡片
// ---------------------------------------------------------------------------

/// 批量取若干地址的「悬停卡片」信息。
///
/// 阅读区一次要展示发件人 + 全部收件人 + 抄送，逐个查会变成十几次 IPC；
/// 这里一次查完：通讯录条目（JOIN 邮箱索引）+ 本地邮件里的往来次数与最近时间。
/// 没有收录的地址也返回一条（`contact_id` 为空），前端据此显示「加进通讯录」按钮。
pub fn address_cards(addresses: &[String]) -> Result<Vec<AddressCard>, String> {
    let wanted: Vec<String> = addresses
        .iter()
        .map(|a| normalize_email(a))
        .filter(|a| !a.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let conn = store::connection()?;
    let mut out: Vec<AddressCard> = Vec::with_capacity(wanted.len());
    for email in wanted {
        let mut card = AddressCard {
            email: email.clone(),
            ..Default::default()
        };
        // 1) 通讯录：邮箱索引是唯一键，一个人只可能有一条。
        let row: Option<(String, String, String, String, i64, String)> = conn
            .query_row(
                "SELECT c.id, c.display_name, c.organization, c.title, c.starred, c.emails_json
                 FROM contact_emails e JOIN contacts c ON c.id = e.contact_id
                 WHERE e.email = ?1",
                params![email],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((id, name, org, title, starred, emails_json)) = row {
            card.contact_id = Some(id);
            card.display_name = name;
            card.organization = org;
            card.title = title;
            card.starred = starred != 0;
            // 通讯录里可能存了好几个地址：卡片上只展示主地址，其余在打开联系人详情时可见。
            let _ = emails_json;
        }
        // 2) 往来统计：收件（from）与发件（to/cc 里出现过）都算。
        let like = format!("%{email}%");
        let stats: Option<(i64, i64, String)> = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(MAX(date), 0),
                        COALESCE(MAX(CASE WHEN from_name <> '' THEN from_name END), '')
                 FROM messages
                 WHERE LOWER(from_addr) = ?1
                    OR LOWER(to_json) LIKE ?2 OR LOWER(cc_json) LIKE ?2",
                params![email, like],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((count, last, header_name)) = stats {
            card.message_count = count.max(0) as u32;
            card.last_seen_at = last;
            card.header_name = header_name;
        }
        out.push(card);
    }
    Ok(out)
}

/// 把一个地址收进通讯录（地址悬停卡片上的「加进通讯录」）。
///
/// 地址已经属于某个联系人时直接返回它，不重复建人——「同一个地址只能挂一个人」
/// 是库层的约束，这里只是把结果解释得对用户友好一点。
pub fn save_address_contact(email: &str, display_name: &str) -> Result<Contact, String> {
    let addr = normalize_email(email);
    if addr.is_empty() {
        return Err("邮箱地址为空".to_string());
    }
    if let Some(existing) = get_by_email(&addr)? {
        return Ok(existing);
    }
    let name = display_name.trim();
    let contact = Contact {
        id: String::new(),
        display_name: name.to_string(),
        emails: vec![LabeledValue {
            label: "work".into(),
            value: addr.clone(),
        }],
        source: "manual".into(),
        last_seen_at: chrono::Utc::now().timestamp(),
        first_seen_at: chrono::Utc::now().timestamp(),
        ..Default::default()
    };
    upsert(contact)
}

/// 按邮箱地址找联系人（规范化后精确匹配）。
pub fn get_by_email(email: &str) -> Result<Option<Contact>, String> {
    let addr = normalize_email(email);
    if addr.is_empty() {
        return Ok(None);
    }
    let conn = store::connection()?;
    let id: Option<String> = conn
        .query_row(
            "SELECT contact_id FROM contact_emails WHERE email=?1",
            params![addr],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    match id {
        Some(id) => get(&id),
        None => Ok(None),
    }
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
            "SELECT kind, contact_id, target_group_id, email, name FROM contact_group_members
             WHERE group_id=?1 ORDER BY position",
        )
        .map_err(|e| e.to_string())?;
    for group in groups.iter_mut() {
        let members = count_stmt
            .query_map(params![group.id], |r| {
                Ok(GroupMember {
                    kind: r.get(0)?,
                    contact_id: r.get(1)?,
                    group_id: r.get(2)?,
                    email: r.get(3)?,
                    name: r.get(4)?,
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
    if group.name.trim().is_empty() {
        return Err("通讯组名称不能为空".to_string());
    }
    for member in &group.members {
        if member.kind == "group" && member.group_id == group.id {
            return Err("通讯组不能包含自身".to_string());
        }
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
            "INSERT INTO contact_group_members
                (group_id, position, kind, contact_id, target_group_id, email, name)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                group.id,
                position as i64,
                if member.kind.is_empty() { "contact" } else { &member.kind },
                member.contact_id,
                member.group_id,
                member.email,
                member.name
            ],
        )
        .map_err(|e| format!("写入组成员失败: {e}"))?;
    }
    let cycle = group_cycle_names(&tx, &group.id).map_err(|e| e.to_string())?;
    if let Some(names) = cycle {
        return Err(format!("通讯组存在循环引用：{}", names.join(" → ")));
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(group)
}

/// 从 `start` 出发做一次 DFS；发现回到 `start` 就返回环上的组名。
fn group_cycle_names(
    tx: &rusqlite::Transaction<'_>,
    start: &str,
) -> rusqlite::Result<Option<Vec<String>>> {
    fn walk(
        tx: &rusqlite::Transaction<'_>,
        start: &str,
        current: &str,
        path: &mut Vec<String>,
        depth: usize,
    ) -> rusqlite::Result<Option<Vec<String>>> {
        if depth > 16 {
            return Ok(None);
        }
        let name: Option<String> = tx
            .query_row("SELECT name FROM contact_groups WHERE id=?1", params![current], |r| {
                r.get(0)
            })
            .optional()?;
        let Some(name) = name else { return Ok(None) };
        if current == start && !path.is_empty() {
            let mut names = path.clone();
            names.push(name);
            return Ok(Some(names));
        }
        path.push(name);
        let mut stmt = tx.prepare(
            "SELECT target_group_id FROM contact_group_members
             WHERE group_id=?1 AND kind='group' AND target_group_id <> ''",
        )?;
        let children = stmt
            .query_map(params![current], |r| r.get::<_, String>(0))?
            .flatten()
            .collect::<Vec<_>>();
        for child in children {
            if let Some(found) = walk(tx, start, &child, path, depth + 1)? {
                return Ok(Some(found));
            }
        }
        path.pop();
        Ok(None)
    }
    let mut path = Vec::new();
    walk(tx, start, start, &mut path, 0)
}

pub fn delete_group(id: &str) -> Result<(), String> {
    let mut conn = store::connection()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM contact_groups WHERE id=?1", params![id]).ok();
    tx.execute("DELETE FROM contact_group_members WHERE group_id=?1", params![id])
        .ok();
    // 其他组里指向这个组的嵌套成员一并去掉，避免展开时撞到「组不存在」。
    tx.execute(
        "DELETE FROM contact_group_members WHERE kind='group' AND target_group_id=?1",
        params![id],
    )
    .ok();
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// 把一个通讯组展开成成员列表（嵌套组递归解开、地址去重）。
///
/// 写信时地址栏只保留组名，真正投递前由这里展开；通讯录详情页也用它
/// 展示「这个组最终会发给哪些人」。
pub fn expand_group(id: &str) -> Result<Vec<ExpandedMember>, String> {
    let groups = list_groups()?;
    let contacts = list(&ContactQuery::default())?;
    let by_id: HashMap<String, Contact> = contacts.into_iter().map(|c| (c.id.clone(), c)).collect();
    let mut path = Vec::new();
    let mut out = Vec::new();
    expand_into(&groups, &by_id, id, &mut path, &mut out)?;
    Ok(out)
}

/// 一次展开多组收件人（收件人 / 抄送 / 密送）。只读一次通讯录，
/// 也保证三份名单看到的是同一时刻的组成员。
pub fn expand_recipient_lists(lists: &[Vec<String>]) -> Result<Vec<Vec<String>>, String> {
    let groups = list_groups()?;
    let contacts = list(&ContactQuery::default())?;
    let by_id: HashMap<String, Contact> = contacts.into_iter().map(|c| (c.id.clone(), c)).collect();
    let mut result: Vec<Vec<String>> = Vec::with_capacity(lists.len());
    for list in lists {
        let mut out: Vec<String> = Vec::new();
        for raw in list {
            match group_token_id(raw) {
                Some(id) => {
                    let mut path = Vec::new();
                    let mut members = Vec::new();
                    expand_into(&groups, &by_id, &id, &mut path, &mut members)
                        .map_err(|e| format!("展开通讯组失败：{e}"))?;
                    if members.is_empty() {
                        let name = groups
                            .iter()
                            .find(|g| g.id == id)
                            .map(|g| g.name.clone())
                            .unwrap_or_else(|| id.clone());
                        return Err(format!("通讯组「{name}」没有可用的收件人"));
                    }
                    for member in members {
                        push_formatted_address(&mut out, &member.name, &member.email);
                    }
                }
                None => {
                    let email = address_of(raw);
                    push_unique(&mut out, raw.trim(), &email);
                }
            }
        }
        result.push(out);
    }
    Ok(result)
}

/// 收件人里的通讯组标记：`名称 <group:ID>` 或裸 `group:ID`。
fn group_token_id(raw: &str) -> Option<String> {
    let value = match (raw.rfind('<'), raw.rfind('>')) {
        (Some(open), Some(close)) if close > open => &raw[open + 1..close],
        _ => raw.trim(),
    };
    value
        .trim()
        .strip_prefix("group:")
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
}

fn address_of(raw: &str) -> String {
    match (raw.rfind('<'), raw.rfind('>')) {
        (Some(open), Some(close)) if close > open => raw[open + 1..close].trim().to_string(),
        _ => raw.trim().to_string(),
    }
}

fn push_formatted_address(out: &mut Vec<String>, name: &str, email: &str) {
    let email = email.trim();
    if email.is_empty() {
        return;
    }
    let label = name.trim().replace(['"', '<', '>'], "");
    let formatted = if label.is_empty() {
        email.to_string()
    } else if label.contains(',')
        || label.contains(';')
        || label.contains(':')
        || label.contains('@')
    {
        // 显示名里有逗号/分号时按 RFC 5322 加引号，否则地址解析会把名字切成两个人。
        format!("\"{label}\" <{email}>")
    } else {
        format!("{label} <{email}>")
    };
    push_unique(out, &formatted, email);
}

fn push_unique(out: &mut Vec<String>, value: &str, email: &str) {
    if value.is_empty() {
        return;
    }
    let duplicate = out
        .iter()
        .any(|item| normalize_email(&address_of(item)) == normalize_email(email));
    if !duplicate {
        out.push(value.to_string());
    }
}

fn expand_into(
    groups: &[ContactGroup],
    contacts: &HashMap<String, Contact>,
    id: &str,
    path: &mut Vec<String>,
    out: &mut Vec<ExpandedMember>,
) -> Result<(), String> {
    let Some(group) = groups.iter().find(|g| g.id == id) else {
        return Err(format!("通讯组不存在（{id}）"));
    };
    if path.iter().any(|seen| seen == id) {
        let mut names: Vec<String> = path
            .iter()
            .filter_map(|seen| groups.iter().find(|g| &g.id == seen).map(|g| g.name.clone()))
            .collect();
        names.push(group.name.clone());
        return Err(format!("通讯组存在循环引用：{}", names.join(" → ")));
    }
    if path.len() >= 8 {
        return Err("通讯组嵌套层级过深（超过 8 层）".to_string());
    }
    path.push(id.to_string());
    let via_group = path[..path.len() - 1]
        .iter()
        .filter_map(|seen| groups.iter().find(|g| &g.id == seen).map(|g| g.name.clone()))
        .collect::<Vec<_>>()
        .join(" › ");

    for member in &group.members {
        match member.kind.as_str() {
            "group" => {
                let child = member.group_id.trim();
                if child.is_empty() {
                    continue;
                }
                expand_into(groups, contacts, child, path, out)?;
            }
            "email" => push_member(out, member.name.clone(), member.email.clone(), &via_group),
            _ => {
                let contact = contacts.get(&member.contact_id);
                let email = match contact {
                    Some(contact) => {
                        // 成员上记的是「选中的邮箱」；联系人后来改过地址时，
                        // 优先用它仍在用的那个，避免组里一直发到已经失效的旧地址。
                        let chosen = member.email.trim();
                        let still_valid = !chosen.is_empty()
                            && contact
                                .emails
                                .iter()
                                .any(|email| normalize_email(&email.value) == normalize_email(chosen));
                        if still_valid {
                            chosen.to_string()
                        } else {
                            contact.primary_email()
                        }
                    }
                    None => member.email.trim().to_string(),
                };
                let name = contact
                    .map(|c| c.display_name.trim().to_string())
                    .filter(|name| !name.is_empty())
                    .or_else(|| {
                        let fallback = member.name.trim().to_string();
                        (!fallback.is_empty()).then_some(fallback)
                    })
                    .unwrap_or_default();
                push_member(out, name, email, &via_group);
            }
        }
    }
    path.pop();
    Ok(())
}

fn push_member(out: &mut Vec<ExpandedMember>, name: String, email: String, via_group: &str) {
    let email = email.trim().to_string();
    if email.is_empty() {
        return;
    }
    if out
        .iter()
        .any(|member| normalize_email(&member.email) == normalize_email(&email))
    {
        return;
    }
    out.push(ExpandedMember {
        name,
        email,
        via_group: via_group.to_string(),
    });
}

// ---------------------------------------------------------------------------
// 导入 / 导出
// ---------------------------------------------------------------------------

/// 导入 vCard 文本。已存在的邮箱合并（补空字段、追加电话），不重复建人。
pub fn import_vcards(text: &str) -> Result<ImportReport, String> {
    let (contacts, imported_groups, errors) = super::vcard::parse(text);
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

    // 通讯组只有在联系人落库之后才能按名字/邮箱合并，所以放在同一个事务提交后处理。
    if !imported_groups.is_empty() {
        let mut existing = list_groups()?;
        let known_group_ids: Vec<String> = existing
            .iter()
            .map(|g| g.id.clone())
            .chain(imported_groups.iter().map(|g| g.id.clone()))
            .filter(|id| !id.is_empty())
            .collect();
        for group in imported_groups {
            let mut members: Vec<GroupMember> = Vec::new();
            for member in &group.members {
                match member.kind.as_str() {
                    "group" => {
                        let child = member.group_id.trim();
                        if !child.is_empty()
                            && child != group.id
                            && known_group_ids.iter().any(|id| id == child)
                        {
                            members.push(GroupMember {
                                kind: "group".into(),
                                group_id: child.to_string(),
                                ..Default::default()
                            });
                        }
                    }
                    _ => {
                        let email = member.email.trim();
                        if !email.is_empty() {
                            members.push(GroupMember {
                                kind: "email".into(),
                                email: email.to_string(),
                                name: member.name.clone(),
                                ..Default::default()
                            });
                        }
                    }
                }
            }

            // 有 UID 的组按 id 合并；没有 UID 的按同名组合并，避免导入两次变两组。
            // `existing` 会随每条导入更新，同一个文件里重复的组名也能合并到刚建的那一个。
            let target_index = existing
                .iter()
                .position(|g| !group.id.is_empty() && g.id == group.id)
                .or_else(|| {
                    existing
                        .iter()
                        .position(|g| g.name.eq_ignore_ascii_case(&group.name))
                });
            let mut merged = target_index
                .and_then(|index| existing.get(index).cloned())
                .unwrap_or_default();
            if let Some(target) = target_index.and_then(|index| existing.get(index)) {
                for member in &target.members {
                    if !members.iter().any(|m| same_group_member(m, member)) {
                        members.push(member.clone());
                    }
                }
            }
            merged.id = if merged.id.is_empty() { group.id.clone() } else { merged.id };
            merged.name = group.name.clone();
            if merged.description.trim().is_empty() {
                merged.description = group.description.clone();
            }
            merged.members = members;
            let saved = upsert_group(merged)?;
            match target_index {
                Some(index) => existing[index] = saved,
                None => existing.push(saved),
            }
            report.groups += 1;
        }
    }
    Ok(report)
}

fn same_group_member(a: &GroupMember, b: &GroupMember) -> bool {
    if a.kind == "group" || b.kind == "group" {
        return a.kind == b.kind && !a.group_id.is_empty() && a.group_id == b.group_id;
    }
    !a.email.trim().is_empty() && normalize_email(&a.email) == normalize_email(&b.email)
}

/// 导出一个通讯组（含嵌套组）或全部联系人为 .vcf 文本。
///
/// 通讯组写成「组卡片」：Apple 通讯录与多数客户端的 `X-ADDRESSBOOKSERVER-*`
/// 约定 + vCard 4.0 的 `KIND:group` / `MEMBER`，两边都认。
pub fn export_vcards(group_id: Option<&str>) -> Result<String, String> {
    let groups = list_groups()?;
    let contacts = list(&ContactQuery::default())?;
    let by_id: HashMap<&str, &Contact> = contacts.iter().map(|c| (c.id.as_str(), c)).collect();

    let (used_groups, used_contacts): (Vec<&ContactGroup>, Vec<&Contact>) = match group_id {
        Some(id) => {
            let mut group_ids = Vec::new();
            let mut contact_ids = Vec::new();
            collect_group_refs(&groups, id, &mut group_ids, &mut contact_ids);
            if group_ids.is_empty() {
                return Err("通讯组不存在".to_string());
            }
            let used_groups = group_ids
                .iter()
                .filter_map(|gid| groups.iter().find(|g| &g.id == gid))
                .collect();
            let used_contacts = contact_ids
                .iter()
                .filter_map(|cid| by_id.get(cid.as_str()).copied())
                .collect();
            (used_groups, used_contacts)
        }
        None => (groups.iter().collect(), contacts.iter().collect()),
    };

    let mut out = String::new();
    for contact in &used_contacts {
        out.push_str(&super::vcard::encode_contact(contact));
    }
    for group in &used_groups {
        let mut card = super::vcard::GroupCard {
            id: group.id.clone(),
            name: group.name.clone(),
            description: group.description.clone(),
            emails: Vec::new(),
            child_group_ids: Vec::new(),
        };
        for member in &group.members {
            match member.kind.as_str() {
                "group" => {
                    let child = member.group_id.trim();
                    if !child.is_empty() {
                        card.child_group_ids.push(child.to_string());
                    }
                }
                "email" => {
                    if !member.email.trim().is_empty() {
                        card.emails
                            .push((member.name.clone(), member.email.clone()));
                    }
                }
                _ => {
                    let contact = by_id.get(member.contact_id.as_str()).copied();
                    let email = if !member.email.trim().is_empty() {
                        member.email.clone()
                    } else {
                        contact.map(|c| c.primary_email()).unwrap_or_default()
                    };
                    let name = contact
                        .map(|c| c.display_name.clone())
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or_else(|| member.name.clone());
                    if !email.trim().is_empty() {
                        card.emails.push((name, email));
                    }
                }
            }
        }
        out.push_str(&super::vcard::encode_group_card(&card));
    }
    if out.is_empty() {
        return Err("没有可导出的联系人".to_string());
    }
    Ok(out)
}

/// 收集一个通讯组（含嵌套组）用到的组 id 与联系人 id，按出现顺序、去重。
fn collect_group_refs(
    groups: &[ContactGroup],
    id: &str,
    group_ids: &mut Vec<String>,
    contact_ids: &mut Vec<String>,
) {
    if group_ids.iter().any(|seen| seen == id) {
        return;
    }
    let Some(group) = groups.iter().find(|g| g.id == id) else {
        return;
    };
    group_ids.push(id.to_string());
    for member in &group.members {
        match member.kind.as_str() {
            "group" => collect_group_refs(groups, member.group_id.trim(), group_ids, contact_ids),
            "email" => {}
            _ => {
                if !member.contact_id.is_empty()
                    && !contact_ids.iter().any(|seen| seen == &member.contact_id)
                {
                    contact_ids.push(member.contact_id.clone());
                }
            }
        }
    }
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

    // 详情页打不开就是因为这里：占位符多了/少了，rusqlite 直接报
    // 「Wrong number of parameters passed to query」。
    #[test]
    fn messages_with_binds_exactly_the_placeholders_it_declares() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                account_id TEXT, folder TEXT, uid INTEGER, subject TEXT, date INTEGER,
                snippet TEXT, from_addr TEXT, to_json TEXT, cc_json TEXT
            );",
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO messages VALUES
                ('a','INBOX',1,'收到','2026-01-02','s','alice@x.com','[]','[]'),
                ('a','INBOX',2,'发出','2026-01-03','s','me@x.com',
                 '[{\"address\":\"alice@x.com\"}]','[]'),
                ('a','INBOX',3,'无关','2026-01-04','s','bob@x.com',
                 '[{\"address\":\"bob@x.com\"}]','[]');",
        )
        .unwrap();

        for n in 1..=3usize {
            let emails: Vec<String> = (0..n).map(|i| format!("alice{i}@x.com")).collect();
            let (sql, args) = messages_with_sql(&emails, 50);
            let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
            let mut stmt = conn.prepare(&sql).unwrap();
            let rows = stmt
                .query_map(refs.as_slice(), |r| r.get::<_, u32>(2))
                .unwrap();
            rows.collect::<rusqlite::Result<Vec<_>>>().unwrap_or_else(|e| {
                panic!("{n} 个地址时查询失败: {e}\nSQL: {sql}")
            });
        }

        let (sql, args) = messages_with_sql(&["alice@x.com".to_string()], 50);
        let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
        let uids: Vec<u32> = conn
            .prepare(&sql)
            .unwrap()
            .query_map(refs.as_slice(), |r| r.get(2))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(uids, vec![2, 1], "收发两个方向都要命中，且按日期倒序");
    }

    fn email_member(email: &str) -> GroupMember {
        GroupMember {
            kind: "email".into(),
            email: email.into(),
            ..Default::default()
        }
    }

    fn nested(group_id: &str) -> GroupMember {
        GroupMember {
            kind: "group".into(),
            group_id: group_id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn nested_groups_expand_and_dedupe() {
        let groups = vec![
            ContactGroup {
                id: "a".into(),
                name: "全公司".into(),
                members: vec![
                    email_member("boss@x.com"),
                    nested("b"),
                    nested("c"),
                ],
                ..Default::default()
            },
            ContactGroup {
                id: "b".into(),
                name: "财务".into(),
                members: vec![email_member("finance@x.com"), email_member("boss@x.com")],
                ..Default::default()
            },
            ContactGroup {
                id: "c".into(),
                name: "技术".into(),
                members: vec![nested("b")],
                ..Default::default()
            },
        ];
        let mut out = Vec::new();
        expand_into(&groups, &HashMap::new(), "a", &mut Vec::new(), &mut out).unwrap();
        let emails: Vec<&str> = out.iter().map(|m| m.email.as_str()).collect();
        assert_eq!(emails, vec!["boss@x.com", "finance@x.com"]);
        // finance 先经由直挂的「财务」组进来，后面「技术 → 财务」的重复地址被丢掉。
        assert_eq!(out[1].via_group, "全公司");
    }

    #[test]
    fn group_cycles_are_reported_instead_of_looping() {
        let groups = vec![
            ContactGroup {
                id: "a".into(),
                name: "A 组".into(),
                members: vec![nested("b")],
                ..Default::default()
            },
            ContactGroup {
                id: "b".into(),
                name: "B 组".into(),
                members: vec![nested("a")],
                ..Default::default()
            },
        ];
        let error = expand_into(&groups, &HashMap::new(), "a", &mut Vec::new(), &mut Vec::new())
            .unwrap_err();
        assert!(error.contains("循环引用"), "{error}");
    }

    #[test]
    fn group_tokens_are_recognized() {
        assert_eq!(
            group_token_id("财务组 <group:abc-123>").as_deref(),
            Some("abc-123")
        );
        assert_eq!(group_token_id("  group:bare  ").as_deref(), Some("bare"));
        assert_eq!(group_token_id("张三 <a@b.c>"), None);
        assert_eq!(group_token_id("a@b.c"), None);
        assert_eq!(address_of("张三 <a@b.c>"), "a@b.c");
    }
}
