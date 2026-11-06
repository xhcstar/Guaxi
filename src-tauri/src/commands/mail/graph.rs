//! Microsoft Graph（Exchange Online / Microsoft 365）原生通道。
//!
//! 设计取舍：
//! - **列表 / 标记 / 移动走 Graph JSON**，但**读正文与发信统一走 MIME**：
//!   `GET /me/messages/{id}/$value` 拿原始报文，交给现有的 `mime::parse` /
//!   `sanitize` / 附件 / 内联图 / 导出链路；发信也用 `mime::build` 组装后
//!   `POST /me/messages`（text/rfc822）→ `/send`。这样附件、内联图、签名、
//!   S/MIME、PGP 与 IMAP 账户共用同一套成熟代码。
//! - **本地主键仍是 uid:u32**：Graph 的字符串 ID 存在 `remote_ids` 映射表里，
//!   不把 `mail_*` 命令和前端全部改成字符串。
//! - 第一版同步「最近 500 封/文件夹」，不做 delta；先把可用性跑通，
//!   `sync_state` 表已经预留，后续接 deltaLink 不需要再改库。
use super::model::{Account, FolderRole, MailAddress, MailFolder, MessageSummary};
use super::net;
use super::{oauth, store};
use reqwest::Method;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::Duration;

const BASE: &str = "https://graph.microsoft.com/v1.0";
const MAX_PAGES_PER_FOLDER: usize = 5;
const PAGE_SIZE: usize = 100;

pub fn is_graph(account: &Account) -> bool {
    account.protocol.eq_ignore_ascii_case("graph")
}

fn encode(value: &str) -> String {
    urlencoding::encode(value).into_owned()
}

fn http_client(account: &Account) -> Result<reqwest::blocking::Client, String> {
    let settings = store::load_settings();
    let proxy = account.proxy.as_ref().or(Some(&settings.proxy));
    let resolved = net::resolve_proxy(proxy, "graph.microsoft.com")?;
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("Guaxi-Mail/1.0");
    match resolved {
        net::ResolvedProxy::Direct => {}
        net::ResolvedProxy::Http { host, port, auth } => {
            let mut proxy = reqwest::Proxy::http(format!("http://{host}:{port}"))
                .map_err(|e| format!("创建 HTTP 代理失败: {e}"))?;
            if let Some((user, pass)) = auth {
                proxy = proxy.basic_auth(&user, &pass);
            }
            builder = builder.proxy(proxy);
        }
        net::ResolvedProxy::Socks5 { host, port, auth } => {
            let mut proxy = reqwest::Proxy::all(format!("socks5h://{host}:{port}"))
                .map_err(|e| format!("创建 SOCKS5 代理失败: {e}"))?;
            if let Some((user, pass)) = auth {
                proxy = proxy.basic_auth(&user, &pass);
            }
            builder = builder.proxy(proxy);
        }
    }
    builder.build().map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}

struct GraphClient {
    http: reqwest::blocking::Client,
    token: String,
}

impl GraphClient {
    fn new(account: &Account) -> Result<Self, String> {
        Ok(Self {
            http: http_client(account)?,
            token: oauth::access_token(account)?,
        })
    }

    fn url(&self, path: &str) -> String {
        if path.starts_with("https://") || path.starts_with("http://") {
            path.to_string()
        } else {
            format!("{}{}", BASE, path)
        }
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
        content_type: Option<&str>,
        accept: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let url = self.url(path);
        let mut attempt = 0;
        loop {
            attempt += 1;
            let mut req = self
                .http
                .request(method.clone(), &url)
                .header("Authorization", format!("Bearer {}", self.token))
                .header("Prefer", "IdType=\"ImmutableId\"");
            if let Some(value) = content_type {
                req = req.header("Content-Type", value);
            }
            if let Some(value) = accept {
                req = req.header("Accept", value);
            }
            if let Some(bytes) = body.as_ref() {
                req = req.body(bytes.clone());
            }
            let response = req.send().map_err(|e| format!("Graph 请求失败: {e}"))?;
            let status = response.status();
            if (status.as_u16() == 429 || status.as_u16() == 503) && attempt <= 3 {
                let wait = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(2);
                std::thread::sleep(Duration::from_secs(wait.min(30)));
                continue;
            }
            let bytes = response
                .bytes()
                .map_err(|e| format!("读取 Graph 响应失败: {e}"))?
                .to_vec();
            if !status.is_success() {
                let text = String::from_utf8_lossy(&bytes);
                let text = text.chars().take(600).collect::<String>();
                return Err(format!("Graph 返回 HTTP {status}: {text}"));
            }
            return Ok(bytes);
        }
    }

    fn json(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value, String> {
        let bytes = self.request(
            method,
            path,
            body.map(|value| value.to_string().into_bytes()),
            Some("application/json"),
            Some("application/json"),
        )?;
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&bytes).map_err(|e| format!("Graph 响应不是有效 JSON: {e}"))
    }

    fn get_json(&self, path: &str) -> Result<Value, String> {
        self.json(Method::GET, path, None)
    }

    fn paged_values(&self, path: &str) -> Result<Vec<Value>, String> {
        let mut out = Vec::new();
        let mut next = path.to_string();
        let mut pages = 0;
        loop {
            let value = self.get_json(&next)?;
            if let Some(items) = value.get("value").and_then(|v| v.as_array()) {
                out.extend(items.iter().cloned());
            }
            pages += 1;
            match value.get("@odata.nextLink").and_then(|v| v.as_str()) {
                Some(link) if pages < MAX_PAGES_PER_FOLDER => next = link.to_string(),
                _ => break,
            }
        }
        Ok(out)
    }
}

fn role_for(well_known: &str, display: &str, path: &str) -> Option<FolderRole> {
    let known = well_known.trim().to_ascii_lowercase();
    let role = match known.as_str() {
        "inbox" => Some(FolderRole::Inbox),
        "drafts" => Some(FolderRole::Drafts),
        "sentitems" => Some(FolderRole::Sent),
        "deleteditems" => Some(FolderRole::Trash),
        "archive" => Some(FolderRole::Archive),
        "junkemail" => Some(FolderRole::Junk),
        // 这些是 Graph 的内部/系统目录，不是用户要看的邮件夹。
        "outbox" | "conversationhistory" | "recoverableitemsdeletions" | "searchfolders"
        | "msgfolderroot" | "root" | "conflicts" | "localfailures" | "serverfailures"
        | "syncissues" | "scheduled" => return None,
        _ => None,
    };
    if role.is_some() {
        return role;
    }
    let label = format!("{display} {path}").to_ascii_lowercase();
    if label.contains("inbox") || label.contains("收件箱") {
        Some(FolderRole::Inbox)
    } else if label.contains("sent item") || label.contains("已发送") {
        Some(FolderRole::Sent)
    } else if label.contains("draft") || label.contains("草稿") {
        Some(FolderRole::Drafts)
    } else if label.contains("deleted") || label.contains("已删除") || label.contains("垃圾箱") {
        Some(FolderRole::Trash)
    } else if label.contains("junk") || label.contains("垃圾邮件") {
        Some(FolderRole::Junk)
    } else if label.contains("archive") || label.contains("存档") || label.contains("归档") {
        Some(FolderRole::Archive)
    } else {
        Some(FolderRole::Custom)
    }
}

/// 文件夹树：Graph 的 childFolders 是分页的，用 BFS 展开并保留父子路径。
pub fn list_folders(account: &Account) -> Result<Vec<MailFolder>, String> {
    let client = GraphClient::new(account)?;
    let select = "$select=id,displayName,parentFolderId,childFolderCount,unreadItemCount,totalItemCount,wellKnownName";
    let mut out = Vec::new();
    let mut queue: Vec<(String, String, u32)> = vec![(String::new(), String::new(), 0)];
    let mut seen: HashSet<String> = HashSet::new();

    while let Some((parent_id, parent_path, depth)) = queue.pop() {
        if depth > 10 {
            continue;
        }
        let path = if parent_id.is_empty() {
            format!("/me/mailFolders?$top={}&{}", PAGE_SIZE, select)
        } else {
            format!(
                "/me/mailFolders/{}/childFolders?$top={}&{}",
                encode(&parent_id),
                PAGE_SIZE,
                select
            )
        };
        let items = client.paged_values(&path)?;
        for item in items {
            let Some(id) = item.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            if !seen.insert(id.to_string()) {
                continue;
            }
            let display = item
                .get("displayName")
                .and_then(|v| v.as_str())
                .unwrap_or(id)
                .to_string();
            let well_known = item
                .get("wellKnownName")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let full_path = if parent_path.is_empty() {
                display.clone()
            } else {
                format!("{parent_path}/{display}")
            };
            let Some(role) = role_for(well_known, &display, &full_path) else {
                continue;
            };
            out.push(MailFolder {
                account_id: account.id.clone(),
                name: id.to_string(),
                display_name: full_path.clone(),
                delimiter: Some("\u{1f}".to_string()),
                role,
                unread: item
                    .get("unreadItemCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                total: item
                    .get("totalItemCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                uid_validity: 0,
                uid_next: 0,
                selectable: true,
                local_only: false,
                user_created: false,
            });
            let children = item
                .get("childFolderCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            if children > 0 {
                queue.push((id.to_string(), full_path, depth + 1));
            }
        }
    }
    Ok(out)
}

fn email_address(value: &Value) -> MailAddress {
    let inner = value.get("emailAddress").unwrap_or(value);
    MailAddress {
        name: inner
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        address: inner
            .get("address")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

fn address_list(value: Option<&Value>) -> Vec<MailAddress> {
    value
        .and_then(|v| v.as_array())
        .map(|items| items.iter().map(email_address).collect())
        .unwrap_or_default()
}

fn trim_snippet(text: &str, limit: usize) -> String {
    text.chars().take(limit.max(20)).collect()
}

fn summary_from_graph(
    account: &Account,
    folder: &str,
    folder_role: FolderRole,
    item: &Value,
    snippet_len: usize,
) -> Result<MessageSummary, String> {
    let remote_id = item
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Graph 邮件缺少 id".to_string())?
        .to_string();
    let uid = store::ensure_remote_uid(&account.id, folder, &remote_id)?;
    let from = item.get("from").cloned().unwrap_or(Value::Null);
    let from = email_address(&from);
    let to = address_list(item.get("toRecipients"));
    let cc = address_list(item.get("ccRecipients"));
    let date = item
        .get("receivedDateTime")
        .and_then(|v| v.as_str())
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|value| value.timestamp())
        .unwrap_or(0);
    let labels = item
        .get("categories")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    let message_id = item
        .get("internetMessageId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let conversation = item
        .get("conversationId")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    Ok(MessageSummary {
        account_id: account.id.clone(),
        folder: folder.to_string(),
        uid,
        message_id: if message_id.is_empty() {
            None
        } else {
            Some(message_id.clone())
        },
        subject: item
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        from_name: from.name.clone(),
        from_addr: from.address.clone(),
        to_json: serde_json::to_string(&to).unwrap_or_else(|_| "[]".into()),
        cc_json: serde_json::to_string(&cc).unwrap_or_else(|_| "[]".into()),
        date,
        // Graph 的 receivedDateTime 就是服务端收件时间。
        received_at: date,
        size: item.get("size").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        snippet: trim_snippet(
            item.get("bodyPreview")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            snippet_len,
        ),
        seen: item.get("isRead").and_then(|v| v.as_bool()).unwrap_or(false),
        flagged: item
            .get("flag")
            .and_then(|v| v.get("flagStatus"))
            .and_then(|v| v.as_str())
            .map(|v| v.eq_ignore_ascii_case("flagged"))
            .unwrap_or(false),
        answered: false,
        draft: item.get("isDraft").and_then(|v| v.as_bool()).unwrap_or(false),
        deleted: false,
        has_attachments: item
            .get("hasAttachments")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        thread_key: if conversation.is_empty() {
            message_id
        } else {
            conversation.to_string()
        },
        // Graph 的 conversationId 已经由服务端归并好，本地不再按引用链重算。
        in_reply_to: String::new(),
        references: String::new(),
        labels,
        outgoing: folder_role == FolderRole::Sent,
    })
}

/// 同步一个文件夹的最近邮件。第一版不做 delta，但会把已读/星标的变化带回来。
pub fn sync_folder(
    account: &Account,
    folder: &str,
    snippet_len: usize,
) -> Result<Vec<MessageSummary>, String> {
    let folders = store::list_folders(&account.id)?;
    let Some(folder_row) = folders.iter().find(|f| f.name == folder) else {
        return Err("本地没有这个文件夹，请先刷新文件夹".to_string());
    };
    if folder_row.local_only {
        return Ok(Vec::new());
    }
    let role = folder_row.role;
    let client = GraphClient::new(account)?;
    let select = "$select=id,internetMessageId,subject,from,toRecipients,ccRecipients,receivedDateTime,isRead,flag,hasAttachments,bodyPreview,conversationId,size,isDraft,categories";
    let path = format!(
        "/me/mailFolders/{}/messages?$top={}&{}&$orderby=receivedDateTime%20desc",
        encode(folder),
        PAGE_SIZE,
        select
    );
    let items = client.paged_values(&path)?;

    let mut summaries = Vec::with_capacity(items.len());
    let mut is_new = Vec::with_capacity(items.len());
    for item in items {
        match summary_from_graph(account, folder, role, &item, snippet_len) {
            Ok(summary) => {
                is_new.push(store::get_summary(&account.id, folder, summary.uid)?.is_none());
                summaries.push(summary);
            }
            Err(error) => tracing::warn!(error = %error, "mail: 解析 Graph 邮件失败，已跳过"),
        }
    }
    store::upsert_summaries(&summaries)?;

    // 文件夹计数用 Graph 的权威值（本地只缓存了最近几百封，不能用本地行数冒充）。
    if let Ok((unread, total)) = folder_counts(account, folder) {
        let _ = store::update_folder_counts(&account.id, folder, unread, total, 0, 0);
    }

    Ok(summaries
        .into_iter()
        .zip(is_new)
        .filter_map(|(summary, new)| new.then_some(summary))
        .collect())
}

pub fn fetch_mime(account: &Account, remote_id: &str) -> Result<Vec<u8>, String> {
    let client = GraphClient::new(account)?;
    client.request(
        Method::GET,
        &format!("/me/messages/{}/$value", encode(remote_id)),
        None,
        None,
        Some("message/rfc822"),
    )
}

/// 用 Graph 发送已经组装好的 MIME：先创建草稿，再发送。
///
/// 这样附件、内联图、签名、S/MIME/PGP 全部复用现有 `mime::build`，不需要把
/// 同一套能力再用 Graph JSON 重写一遍。失败时把刚创建的草稿删掉，避免草稿箱里
/// 堆一封「发了一半」的信。
pub fn send_mime(account: &Account, raw: &[u8]) -> Result<(), String> {
    let client = GraphClient::new(account)?;
    let bytes = client.request(
        Method::POST,
        "/me/messages",
        Some(raw.to_vec()),
        Some("text/rfc822"),
        Some("application/json"),
    )?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("Graph 草稿响应无效: {e}"))?;
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Graph 没有返回草稿 ID".to_string())?
        .to_string();
    let send_result = client.request(
        Method::POST,
        &format!("/me/messages/{}/send", encode(&id)),
        None,
        None,
        None,
    );
    if send_result.is_err() {
        let _ = client.request(
            Method::DELETE,
            &format!("/me/messages/{}", encode(&id)),
            None,
            None,
            None,
        );
    }
    send_result.map(|_| ())
}

pub fn save_draft(
    account: &Account,
    folder: &str,
    raw: &[u8],
) -> Result<serde_json::Value, String> {
    let client = GraphClient::new(account)?;
    let bytes = client.request(
        Method::POST,
        "/me/messages",
        Some(raw.to_vec()),
        Some("text/rfc822"),
        Some("application/json"),
    )?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("Graph 草稿响应无效: {e}"))?;
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Graph 没有返回草稿 ID".to_string())?
        .to_string();
    let uid = match store::ensure_remote_uid(&account.id, folder, &id) {
        Ok(uid) => uid,
        Err(error) => {
            let _ = client.request(
                Method::DELETE,
                &format!("/me/messages/{}", encode(&id)),
                None,
                None,
                None,
            );
            return Err(error);
        }
    };
    // 草稿箱角标跟着动，侧栏保存后立刻能看到 +1。
    if let Ok((unread, total)) = folder_counts(account, folder) {
        let _ = store::update_folder_counts(&account.id, folder, unread, total, 0, 0);
    }
    Ok(json!({ "folder": folder, "uid": uid, "remoteId": id }))
}

pub fn discard_draft(account: &Account, folder: &str, uid: u32) -> Result<(), String> {
    let Some(remote_id) = store::remote_id_for(&account.id, folder, uid) else {
        return Ok(());
    };
    let client = GraphClient::new(account)?;
    client.request(
        Method::DELETE,
        &format!("/me/messages/{}", encode(&remote_id)),
        None,
        None,
        None,
    )?;
    let _ = store::delete_remote_ids(&account.id, folder, &[uid]);
    if let Ok((unread, total)) = folder_counts(account, folder) {
        let _ = store::update_folder_counts(&account.id, folder, unread, total, 0, 0);
    }
    Ok(())
}

pub fn set_flags(
    account: &Account,
    folder: &str,
    uids: &[u32],
    seen: Option<bool>,
    flagged: Option<bool>,
) -> Result<(), String> {
    let client = GraphClient::new(account)?;
    let mapping = store::remote_ids_for(&account.id, folder, uids);
    for uid in uids {
        let Some(remote_id) = mapping.get(uid) else {
            continue;
        };
        let mut body = serde_json::Map::new();
        if let Some(value) = seen {
            body.insert("isRead".into(), Value::Bool(value));
        }
        if let Some(value) = flagged {
            body.insert(
                "flag".into(),
                json!({ "flagStatus": if value { "flagged" } else { "notFlagged" } }),
            );
        }
        if body.is_empty() {
            continue;
        }
        client.json(
            Method::PATCH,
            &format!("/me/messages/{}", encode(remote_id)),
            Some(Value::Object(body)),
        )?;
    }
    Ok(())
}

pub fn move_messages(
    account: &Account,
    folder: &str,
    uids: &[u32],
    target: &str,
) -> Result<(), String> {
    let client = GraphClient::new(account)?;
    let mapping = store::remote_ids_for(&account.id, folder, uids);
    for uid in uids {
        let Some(remote_id) = mapping.get(uid) else {
            continue;
        };
        client.json(
            Method::POST,
            &format!("/me/messages/{}/move", encode(remote_id)),
            Some(json!({ "destinationId": target })),
        )?;
    }
    Ok(())
}

/// `permanent` 为 true 时走 permanentDelete；否则优先移动到垃圾箱。
pub fn delete_messages(
    account: &Account,
    folder: &str,
    uids: &[u32],
    permanent: bool,
    trash: Option<&str>,
) -> Result<(), String> {
    let client = GraphClient::new(account)?;
    let mapping = store::remote_ids_for(&account.id, folder, uids);
    for uid in uids {
        let Some(remote_id) = mapping.get(uid) else {
            continue;
        };
        if permanent {
            let result = client.request(
                Method::POST,
                &format!("/me/messages/{}/permanentDelete", encode(remote_id)),
                None,
                None,
                None,
            );
            if result.is_err() {
                // 个别租户 / Graph 版本没有 permanentDelete 动作，退回 DELETE。
                client.request(
                    Method::DELETE,
                    &format!("/me/messages/{}", encode(remote_id)),
                    None,
                    None,
                    None,
                )?;
            }
        } else if let Some(target) = trash {
            client.json(
                Method::POST,
                &format!("/me/messages/{}/move", encode(remote_id)),
                Some(json!({ "destinationId": target })),
            )?;
        } else {
            client.request(
                Method::DELETE,
                &format!("/me/messages/{}", encode(remote_id)),
                None,
                None,
                None,
            )?;
        }
    }
    Ok(())
}

pub fn create_folder(account: &Account, name: &str) -> Result<(), String> {
    let client = GraphClient::new(account)?;
    client.json(
        Method::POST,
        "/me/mailFolders",
        Some(json!({ "displayName": name })),
    )?;
    Ok(())
}

pub fn delete_folder(account: &Account, folder: &str) -> Result<(), String> {
    let client = GraphClient::new(account)?;
    client.request(
        Method::DELETE,
        &format!("/me/mailFolders/{}", encode(folder)),
        None,
        None,
        None,
    )?;
    Ok(())
}

pub fn test_account(account: &Account) -> Result<(), String> {
    let client = GraphClient::new(account)?;
    client
        .get_json("/me?$select=id,userPrincipalName,mail")
        .map(|_| ())
}

/// 文件夹的权威未读 / 总数。本地只缓存最近几百封，不能用本地行数算 Graph 的角标。
pub fn folder_counts(account: &Account, folder: &str) -> Result<(u32, u32), String> {
    let client = GraphClient::new(account)?;
    let value = client.get_json(&format!(
        "/me/mailFolders/{}?$select=unreadItemCount,totalItemCount",
        encode(folder)
    ))?;
    Ok((
        value
            .get("unreadItemCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        value
            .get("totalItemCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_graph_well_known_folders_to_roles() {
        assert_eq!(role_for("inbox", "Inbox", "Inbox"), Some(FolderRole::Inbox));
        assert_eq!(
            role_for("sentitems", "Sent Items", "Sent Items"),
            Some(FolderRole::Sent)
        );
        assert_eq!(
            role_for("deleteditems", "Deleted Items", "Deleted Items"),
            Some(FolderRole::Trash)
        );
        assert_eq!(
            role_for("junkemail", "Junk Email", "Junk Email"),
            Some(FolderRole::Junk)
        );
        assert_eq!(role_for("", "项目/发票", "项目/发票"), Some(FolderRole::Custom));
    }

    #[test]
    fn ignores_graph_internal_folders() {
        for name in [
            "outbox",
            "conversationhistory",
            "recoverableitemsdeletions",
            "searchfolders",
            "msgfolderroot",
        ] {
            assert_eq!(role_for(name, name, name), None, "{name} 不应出现在侧栏");
        }
    }

    #[test]
    fn parses_graph_email_address() {
        let value: Value = serde_json::from_str(
            r#"{"emailAddress":{"name":"张三","address":"zhangsan@example.com"}}"#,
        )
        .unwrap();
        let address = email_address(&value);
        assert_eq!(address.name, "张三");
        assert_eq!(address.address, "zhangsan@example.com");
    }

    #[test]
    fn trims_preview_by_chars_not_bytes() {
        assert_eq!(trim_snippet("中文预览内容", 40), "中文预览内容");
        assert_eq!(trim_snippet("0123456789", 20), "0123456789");
    }
}
