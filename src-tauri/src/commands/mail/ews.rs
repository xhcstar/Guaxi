//! 本地 Exchange Server 的 EWS（Exchange Web Services）通道。
//!
//! 第一版覆盖 Basic / Bearer 认证、Autodiscover、文件夹发现、最近邮件同步、
//! MIME 正文/附件、已读/星标、移动/删除、发信与草稿。NTLM 需要连接级握手，
//! 放到下一小阶段，避免把不完整的 NTLM 伪装成可用。
//!
//! 与 Graph 一样：本地主键仍是 uid:u32，EWS 的字符串 ItemId 存在 `remote_ids`；
//! 正文和发信都走 MIME，复用现有 `mime::parse` / `mime::build`。
use super::model::{Account, FolderRole, MailAddress, MailFolder, MessageSummary};
use super::net;
use super::{oauth, store};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::blocking::{Client, Response};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const PAGE_SIZE: usize = 100;
const MAX_PAGES: usize = 5;
const DISTINGUISHED: &[(&str, FolderRole)] = &[
    ("inbox", FolderRole::Inbox),
    ("sentitems", FolderRole::Sent),
    ("drafts", FolderRole::Drafts),
    ("deleteditems", FolderRole::Trash),
    ("junkemail", FolderRole::Junk),
    ("archive", FolderRole::Archive),
];

pub fn is_ews(account: &Account) -> bool {
    account.protocol.eq_ignore_ascii_case("ews")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ---------------------------------------------------------------------------
// 极简 XML DOM（忽略命名空间）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct Node {
    name: String,
    attrs: HashMap<String, String>,
    text: String,
    children: Vec<Node>,
}

impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|node| node.name == name)
    }
    fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |node| node.name == name)
    }
    fn all<'a>(&'a self, name: &'a str, out: &mut Vec<&'a Node>) {
        for child in &self.children {
            if child.name == name {
                out.push(child);
            }
            child.all(name, out);
        }
    }
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(|value| value.as_str())
    }
    fn text_trim(&self) -> String {
        self.text.trim().to_string()
    }
}

fn local_name(bytes: &[u8]) -> String {
    let raw = String::from_utf8_lossy(bytes);
    raw.rsplit(':').next().unwrap_or(&raw).to_string()
}

fn parse_xml(xml: &str) -> Result<Node, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let decoder = reader.decoder();
    let mut stack: Vec<Node> = vec![Node {
        name: "#root".into(),
        ..Default::default()
    }];
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let mut node = Node {
                    name: local_name(event.name().as_ref()),
                    ..Default::default()
                };
                for attribute in event.attributes().flatten() {
                    let key = local_name(attribute.key.as_ref());
                    let value = attribute
                        .decode_and_unescape_value(decoder)
                        .map(|value| value.into_owned())
                        .unwrap_or_default();
                    node.attrs.insert(key, value);
                }
                stack.push(node);
            }
            Ok(Event::Empty(event)) => {
                let mut node = Node {
                    name: local_name(event.name().as_ref()),
                    ..Default::default()
                };
                for attribute in event.attributes().flatten() {
                    let key = local_name(attribute.key.as_ref());
                    let value = attribute
                        .decode_and_unescape_value(decoder)
                        .map(|value| value.into_owned())
                        .unwrap_or_default();
                    node.attrs.insert(key, value);
                }
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                }
            }
            Ok(Event::Text(event)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(
                        &event
                            .unescape()
                            .map(|value| value.into_owned())
                            .unwrap_or_default(),
                    );
                }
            }
            Ok(Event::CData(event)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&String::from_utf8_lossy(&event));
                }
            }
            Ok(Event::End(_)) => {
                if stack.len() > 1 {
                    let node = stack.pop().unwrap();
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("EWS 响应 XML 解析失败: {error}")),
            _ => {}
        }
    }
    Ok(stack.remove(0))
}

fn response_error(root: &Node) -> Option<String> {
    let mut codes = Vec::new();
    root.all("ResponseCode", &mut codes);
    let mut messages = Vec::new();
    root.all("MessageText", &mut messages);
    let detail = messages
        .first()
        .map(|item| item.text_trim())
        .unwrap_or_default();
    for code in codes {
        let text = code.text_trim();
        if !text.is_empty() && text != "NoError" && !text.starts_with("Warning") {
            return Some(if detail.is_empty() {
                format!("EWS 返回 {text}")
            } else {
                format!("EWS 返回 {text}：{detail}")
            });
        }
    }
    None
}

// ---------------------------------------------------------------------------
// HTTP / 认证
// ---------------------------------------------------------------------------

enum Auth {
    Basic(String, String),
    Bearer(String),
}

struct EwsClient {
    http: Client,
    base: String,
    auth: Auth,
}

fn http_client(account: &Account) -> Result<Client, String> {
    let settings = store::load_settings();
    let proxy = account.proxy.as_ref().or(Some(&settings.proxy));
    let proxy_host = account.email.split('@').nth(1).unwrap_or("localhost");
    let resolved = net::resolve_proxy(proxy, proxy_host)?;
    let mut builder = Client::builder()
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

fn ews_cache() -> &'static Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

impl EwsClient {
    fn new(account: &Account) -> Result<Self, String> {
        Self::new_with_secret(account, None)
    }

    fn new_with_secret(account: &Account, secret: Option<&str>) -> Result<Self, String> {
        let http = http_client(account)?;
        let auth = if account.auth_kind.eq_ignore_ascii_case("oauth2") {
            Auth::Bearer(oauth::access_token(account)?)
        } else {
            let user = if account.username.trim().is_empty() {
                account.email.clone()
            } else {
                account.username.clone()
            };
            let password = secret
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .map(Ok)
                .unwrap_or_else(|| super::account_password(account))?;
            Auth::Basic(user, password)
        };
        let base = resolve_ews_url(account, &http, &auth)?;
        Ok(Self { http, base, auth })
    }

    fn apply_auth(&self, request: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        match &self.auth {
            Auth::Basic(user, password) => request.basic_auth(user, Some(password)),
            Auth::Bearer(token) => request.header("Authorization", format!("Bearer {token}")),
        }
    }

    fn post_xml(&self, body: &str, content_type: &str) -> Result<Response, String> {
        let request = self
            .http
            .post(&self.base)
            .header("Content-Type", content_type)
            .header("SOAPAction", "\"\"")
            .body(body.to_string());
        self.apply_auth(request)
            .send()
            .map_err(|e| format!("EWS 请求失败: {e}"))
    }

    fn call(&self, body: &str) -> Result<Node, String> {
        let envelope = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"
 xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types"
 xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages">
 <soap:Header><t:RequestServerVersion Version="Exchange2013"/></soap:Header>
 <soap:Body>{body}</soap:Body>
</soap:Envelope>"#
        );
        let response = self.post_xml(&envelope, "text/xml; charset=utf-8")?;
        let status = response.status();
        let text = response
            .text()
            .map_err(|e| format!("读取 EWS 响应失败: {e}"))?;
        if !status.is_success() {
            return Err(format!(
                "EWS HTTP {status}: {}",
                text.chars().take(500).collect::<String>()
            ));
        }
        let root = parse_xml(&text)?;
        if let Some(error) = response_error(&root) {
            return Err(error);
        }
        Ok(root)
    }
}

fn resolve_ews_url(account: &Account, http: &Client, auth: &Auth) -> Result<String, String> {
    if !account.ews_url.trim().is_empty() {
        return Ok(account.ews_url.trim().to_string());
    }
    if let Ok(cache) = ews_cache().lock() {
        if let Some(url) = cache.get(&account.id) {
            return Ok(url.clone());
        }
    }
    let url = discover_ews_url(account, http, auth)?;
    if let Ok(mut cache) = ews_cache().lock() {
        cache.insert(account.id.clone(), url.clone());
    }
    Ok(url)
}

fn apply_auth_static(
    request: reqwest::blocking::RequestBuilder,
    auth: &Auth,
) -> reqwest::blocking::RequestBuilder {
    match auth {
        Auth::Basic(user, password) => request.basic_auth(user, Some(password)),
        Auth::Bearer(token) => request.header("Authorization", format!("Bearer {token}")),
    }
}

/// Autodiscover：先试 `autodiscover.<domain>`，再试 `<domain>`。
fn discover_ews_url(account: &Account, http: &Client, auth: &Auth) -> Result<String, String> {
    let domain = account
        .email
        .split('@')
        .nth(1)
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if domain.is_empty() {
        return Err("Exchange Autodiscover 需要完整邮箱地址".to_string());
    }
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Autodiscover xmlns="http://schemas.microsoft.com/exchange/autodiscover/outlook/requestschema/2006">
 <Request>
  <EMailAddress>{}</EMailAddress>
  <AcceptableResponseSchema>http://schemas.microsoft.com/exchange/autodiscover/outlook/responseschema/2006a</AcceptableResponseSchema>
 </Request>
</Autodiscover>"#,
        xml_escape(&account.email)
    );
    let urls = [
        format!("https://autodiscover.{domain}/autodiscover/autodiscover.xml"),
        format!("https://{domain}/autodiscover/autodiscover.xml"),
    ];
    let mut last_error = String::new();
    for url in urls {
        let request = http
            .post(&url)
            .header("Content-Type", "text/xml; charset=utf-8")
            .body(body.clone());
        let response = match apply_auth_static(request, auth).send() {
            Ok(response) => response,
            Err(error) => {
                last_error = format!("Autodiscover 请求失败: {error}");
                continue;
            }
        };
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            last_error = format!("Autodiscover HTTP {status}");
            continue;
        }
        if let Ok(root) = parse_xml(&text) {
            let mut nodes = Vec::new();
            root.all("EwsUrl", &mut nodes);
            if let Some(node) = nodes.first() {
                let url = node.text_trim();
                if !url.is_empty() {
                    return Ok(url);
                }
            }
        }
        last_error = "Autodiscover 响应里没有 EwsUrl".to_string();
    }
    Err(format!(
        "没有找到 EWS 地址（{last_error}）。请在账户里手动填写 EWS 地址，例如 https://server/EWS/Exchange.asmx"
    ))
}

// ---------------------------------------------------------------------------
// 文件夹
// ---------------------------------------------------------------------------

fn folder_from_node(account: &Account, node: &Node, role: FolderRole) -> Option<MailFolder> {
    let id = node
        .child("FolderId")
        .and_then(|item| item.attr("Id"))
        .or_else(|| node.attr("Id"))?;
    let display = node
        .child("DisplayName")
        .map(|item| item.text_trim())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| role.as_str().to_string());
    Some(MailFolder {
        account_id: account.id.clone(),
        name: id.to_string(),
        display_name: display,
        delimiter: Some("\u{1f}".to_string()),
        role,
        unread: node
            .child("UnreadCount")
            .and_then(|item| item.text_trim().parse::<u32>().ok())
            .unwrap_or(0),
        total: node
            .child("TotalCount")
            .and_then(|item| item.text_trim().parse::<u32>().ok())
            .unwrap_or(0),
        uid_validity: 0,
        uid_next: 0,
        selectable: true,
        local_only: false,
        user_created: false,
    })
}

fn get_folder(client: &EwsClient, account: &Account, distinguished: &str, role: FolderRole) -> Option<MailFolder> {
    let body = format!(
        r#"<m:GetFolder>
 <m:FolderShape><t:BaseShape>Default</t:BaseShape></m:FolderShape>
 <m:FolderIds><t:DistinguishedFolderId Id="{}"/></m:FolderIds>
</m:GetFolder>"#,
        xml_escape(distinguished)
    );
    let root = client.call(&body).ok()?;
    let mut folders = Vec::new();
    root.all("Folder", &mut folders);
    folders.first().and_then(|node| folder_from_node(account, node, role))
}

pub fn list_folders(account: &Account) -> Result<Vec<MailFolder>, String> {
    let client = EwsClient::new(account)?;
    let mut out = Vec::new();
    let mut known: HashSet<String> = HashSet::new();
    for (distinguished, role) in DISTINGUISHED {
        if let Some(folder) = get_folder(&client, account, distinguished, *role) {
            known.insert(folder.name.clone());
            out.push(folder);
        }
    }
    // 自定义文件夹：从根开始 Deep 遍历。标准文件夹已经通过 GetFolder 拿到，
    // FindFolder 里重复的那些按 FolderId 跳过。
    let body = r#"<m:FindFolder Traversal="Deep">
 <m:FolderShape><t:BaseShape>Default</t:BaseShape></m:FolderShape>
 <m:ParentFolderIds><t:DistinguishedFolderId Id="msgfolderroot"/></m:ParentFolderIds>
 <m:IndexedPageFolderView MaxEntriesReturned="1000" Offset="0" BasePoint="Beginning"/>
</m:FindFolder>"#;
    let root = client.call(body)?;
    let mut folders = Vec::new();
    root.all("Folder", &mut folders);
    for node in folders {
        let Some(folder) = folder_from_node(account, node, FolderRole::Custom) else {
            continue;
        };
        // 内部 Outbox 不在本地展示；发件箱是本地文件夹。
        if known.contains(&folder.name) {
            continue;
        }
        // 通过 DistinguishedFolderId 已经拿到的系统文件夹，FindFolder 也会返回；
        // 这里再用 displayName 做一次兜底（有些服务器 GetFolder 权限不足）。
        let lower = folder.display_name.to_ascii_lowercase();
        let role = if lower == "outbox" {
            continue;
        } else if lower.contains("inbox") || lower.contains("收件箱") {
            FolderRole::Inbox
        } else if lower.contains("sent item") || lower.contains("已发送") {
            FolderRole::Sent
        } else if lower.contains("draft") || lower.contains("草稿") {
            FolderRole::Drafts
        } else if lower.contains("deleted") || lower.contains("已删除") {
            FolderRole::Trash
        } else if lower.contains("junk") || lower.contains("垃圾邮件") {
            FolderRole::Junk
        } else if lower.contains("archive") || lower.contains("存档") || lower.contains("归档") {
            FolderRole::Archive
        } else {
            FolderRole::Custom
        };
        if known.insert(folder.name.clone()) {
            let mut folder = folder;
            folder.role = role;
            out.push(folder);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// 邮件
// ---------------------------------------------------------------------------

fn mailbox_address(node: Option<&Node>) -> MailAddress {
    let Some(node) = node else {
        return MailAddress::default();
    };
    MailAddress {
        name: node
            .child("Name")
            .map(|item| item.text_trim())
            .unwrap_or_default(),
        address: node
            .child("EmailAddress")
            .map(|item| item.text_trim())
            .unwrap_or_default(),
    }
}

fn mailbox_list(node: Option<&Node>, container: &str) -> Vec<MailAddress> {
    let Some(container) = node.and_then(|item| item.child(container)) else {
        return Vec::new();
    };
    container
        .children_named("Mailbox")
        .map(|item| mailbox_address(Some(item)))
        .collect()
}

fn summary_from_item(
    account: &Account,
    folder: &str,
    role: FolderRole,
    node: &Node,
    snippet_len: usize,
) -> Option<MessageSummary> {
    let remote_id = node
        .child("ItemId")
        .and_then(|item| item.attr("Id"))
        .or_else(|| node.attr("Id"))?;
    let uid = store::ensure_remote_uid(&account.id, folder, remote_id).ok()?;
    let from = mailbox_address(node.child("From").and_then(|item| item.child("Mailbox")));
    let to = mailbox_list(Some(node), "ToRecipients");
    let cc = mailbox_list(Some(node), "CcRecipients");
    let date = node
        .child("DateTimeReceived")
        .map(|item| item.text_trim())
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(&value).ok())
        .map(|value| value.timestamp())
        .unwrap_or(0);
    let subject = node
        .child("Subject")
        .map(|item| item.text_trim())
        .unwrap_or_default();
    let preview = node
        .child("Preview")
        .map(|item| item.text_trim())
        .unwrap_or_default();
    let flag = node
        .child("Flag")
        .and_then(|item| item.child("FlagStatus"))
        .map(|item| item.text_trim().eq_ignore_ascii_case("flagged"))
        .unwrap_or(false);
    let message_id = node
        .child("InternetMessageId")
        .map(|item| item.text_trim())
        .filter(|value| !value.is_empty());
    let conversation = node
        .child("ConversationId")
        .and_then(|item| item.attr("Id"))
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| message_id.clone());
    Some(MessageSummary {
        account_id: account.id.clone(),
        folder: folder.to_string(),
        uid,
        message_id,
        subject,
        from_name: from.name,
        from_addr: from.address,
        to_json: serde_json::to_string(&to).unwrap_or_else(|_| "[]".into()),
        cc_json: serde_json::to_string(&cc).unwrap_or_else(|_| "[]".into()),
        date,
        // EWS 的 DateTimeReceived 就是服务端收件时间。
        received_at: date,
        size: node
            .child("Size")
            .and_then(|item| item.text_trim().parse::<u32>().ok())
            .unwrap_or(0),
        snippet: preview.chars().take(snippet_len.max(20)).collect(),
        seen: node
            .child("IsRead")
            .map(|item| item.text_trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        flagged: flag,
        answered: false,
        draft: role == FolderRole::Drafts,
        deleted: false,
        has_attachments: node
            .child("HasAttachments")
            .map(|item| item.text_trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        thread_key: conversation.unwrap_or_default(),
        // Exchange 的 ConversationId 本身就是服务端算好的会话归属，比本地按
        // 引用链拼更准；引用头这里留空，免得两套规则互相打架。
        in_reply_to: String::new(),
        references: String::new(),
        labels: String::new(),
        outgoing: role == FolderRole::Sent,
    })
}

pub fn sync_folder(
    account: &Account,
    folder: &str,
    snippet_len: usize,
) -> Result<Vec<MessageSummary>, String> {
    let folders = store::list_folders(&account.id)?;
    let Some(folder_row) = folders.iter().find(|item| item.name == folder) else {
        return Err("本地没有这个文件夹，请先刷新文件夹".to_string());
    };
    if folder_row.local_only {
        return Ok(Vec::new());
    }
    let client = EwsClient::new(account)?;
    let mut summaries = Vec::new();
    let mut new_flags = Vec::new();
    for page in 0..MAX_PAGES {
        let offset = page * PAGE_SIZE;
        let body = format!(
            r#"<m:FindItem Traversal="Shallow">
 <m:ItemShape>
  <t:BaseShape>Default</t:BaseShape>
  <t:AdditionalProperties>
   <t:FieldURI FieldURI="item:Preview"/>
   <t:FieldURI FieldURI="item:ConversationId"/>
   <t:FieldURI FieldURI="item:InternetMessageId"/>
   <t:FieldURI FieldURI="item:Flag"/>
  </t:AdditionalProperties>
 </m:ItemShape>
 <m:IndexedPageItemView MaxEntriesReturned="{PAGE_SIZE}" Offset="{offset}" BasePoint="Beginning"/>
 <m:SortOrder><t:FieldOrder Order="Descending"><t:FieldURI FieldURI="item:DateTimeReceived"/></t:FieldOrder></m:SortOrder>
 <m:ParentFolderIds><t:FolderId Id="{}"/></m:ParentFolderIds>
</m:FindItem>"#,
            xml_escape(folder)
        );
        let root = client.call(&body)?;
        let mut items = Vec::new();
        root.all("Message", &mut items);
        root.all("Item", &mut items);
        if items.is_empty() {
            break;
        }
        let mut count = 0usize;
        for item in items {
            if let Some(summary) = summary_from_item(account, folder, folder_row.role, item, snippet_len)
            {
                new_flags.push(store::get_summary(&account.id, folder, summary.uid)?.is_none());
                summaries.push(summary);
                count += 1;
            }
        }
        if count < PAGE_SIZE {
            break;
        }
    }
    store::upsert_summaries(&summaries)?;
    if let Ok((unread, total)) = folder_counts(account, folder) {
        let _ = store::update_folder_counts(&account.id, folder, unread, total, 0, 0);
    }
    Ok(summaries
        .into_iter()
        .zip(new_flags)
        .filter_map(|(summary, new)| new.then_some(summary))
        .collect())
}

pub fn folder_counts(account: &Account, folder: &str) -> Result<(u32, u32), String> {
    let client = EwsClient::new(account)?;
    let body = format!(
        r#"<m:GetFolder>
 <m:FolderShape><t:BaseShape>Default</t:BaseShape></m:FolderShape>
 <m:FolderIds><t:FolderId Id="{}"/></m:FolderIds>
</m:GetFolder>"#,
        xml_escape(folder)
    );
    let root = client.call(&body)?;
    let mut folders = Vec::new();
    root.all("Folder", &mut folders);
    let folder_node = folders.first().ok_or_else(|| "EWS 没有返回该文件夹".to_string())?;
    Ok((
        folder_node
            .child("UnreadCount")
            .and_then(|item| item.text_trim().parse::<u32>().ok())
            .unwrap_or(0),
        folder_node
            .child("TotalCount")
            .and_then(|item| item.text_trim().parse::<u32>().ok())
            .unwrap_or(0),
    ))
}

pub fn fetch_mime(account: &Account, remote_id: &str) -> Result<Vec<u8>, String> {
    let client = EwsClient::new(account)?;
    let body = format!(
        r#"<m:GetItem>
 <m:ItemShape><t:BaseShape>IdOnly</t:BaseShape><t:IncludeMimeContent>true</t:IncludeMimeContent></m:ItemShape>
 <m:ItemIds><t:ItemId Id="{}"/></m:ItemIds>
</m:GetItem>"#,
        xml_escape(remote_id)
    );
    let root = client.call(&body)?;
    let mut contents = Vec::new();
    root.all("MimeContent", &mut contents);
    let encoded = contents
        .first()
        .map(|item| item.text_trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "EWS 没有返回 MimeContent".to_string())?;
    let compact: String = encoded.split_whitespace().collect();
    BASE64
        .decode(compact.as_bytes())
        .map_err(|e| format!("EWS MimeContent 解码失败: {e}"))
}

pub fn set_flags(
    account: &Account,
    folder: &str,
    uids: &[u32],
    seen: Option<bool>,
    flagged: Option<bool>,
) -> Result<(), String> {
    let mapping = store::remote_ids_for(&account.id, folder, uids);
    if mapping.is_empty() {
        return Ok(());
    }
    let client = EwsClient::new(account)?;
    let mut changes = String::new();
    for uid in uids {
        let Some(remote_id) = mapping.get(uid) else {
            continue;
        };
        let mut updates = String::new();
        if let Some(value) = seen {
            updates.push_str(&format!(
                r#"<t:SetItemField><t:FieldURI FieldURI="message:IsRead"/><t:Message><t:IsRead>{value}</t:IsRead></t:Message></t:SetItemField>"#
            ));
        }
        if let Some(value) = flagged {
            updates.push_str(&format!(
                r#"<t:SetItemField><t:FieldURI FieldURI="item:Flag"/><t:Message><t:Flag><t:FlagStatus>{}</t:FlagStatus></t:Flag></t:Message></t:SetItemField>"#,
                if value { "Flagged" } else { "NotFlagged" }
            ));
        }
        if updates.is_empty() {
            continue;
        }
        changes.push_str(&format!(
            r#"<t:ItemChange><t:ItemId Id="{}"/><t:Updates>{updates}</t:Updates></t:ItemChange>"#,
            xml_escape(remote_id)
        ));
    }
    if changes.is_empty() {
        return Ok(());
    }
    let body = format!(
        r#"<m:UpdateItem ConflictResolution="AlwaysOverwrite">
 <m:ItemChanges>{changes}</m:ItemChanges>
</m:UpdateItem>"#
    );
    client.call(&body).map(|_| ())
}

pub fn move_messages(
    account: &Account,
    folder: &str,
    uids: &[u32],
    target: &str,
) -> Result<(), String> {
    let mapping = store::remote_ids_for(&account.id, folder, uids);
    if mapping.is_empty() {
        return Ok(());
    }
    let client = EwsClient::new(account)?;
    let ids = uids
        .iter()
        .filter_map(|uid| mapping.get(uid))
        .map(|id| format!(r#"<t:ItemId Id="{}"/>"#, xml_escape(id)))
        .collect::<String>();
    if ids.is_empty() {
        return Ok(());
    }
    let body = format!(
        r#"<m:MoveItem>
 <m:ToFolderId><t:FolderId Id="{}"/></m:ToFolderId>
 <m:ItemIds>{ids}</m:ItemIds>
</m:MoveItem>"#,
        xml_escape(target)
    );
    client.call(&body).map(|_| ())
}

pub fn delete_messages(
    account: &Account,
    folder: &str,
    uids: &[u32],
    permanent: bool,
) -> Result<(), String> {
    let mapping = store::remote_ids_for(&account.id, folder, uids);
    if mapping.is_empty() {
        return Ok(());
    }
    let client = EwsClient::new(account)?;
    let ids = uids
        .iter()
        .filter_map(|uid| mapping.get(uid))
        .map(|id| format!(r#"<t:ItemId Id="{}"/>"#, xml_escape(id)))
        .collect::<String>();
    if ids.is_empty() {
        return Ok(());
    }
    let delete_type = if permanent { "HardDelete" } else { "MoveToDeletedItems" };
    let body = format!(
        r#"<m:DeleteItem DeleteType="{delete_type}"><m:ItemIds>{ids}</m:ItemIds></m:DeleteItem>"#
    );
    client.call(&body).map(|_| ())
}

pub fn create_folder(account: &Account, name: &str) -> Result<(), String> {
    let client = EwsClient::new(account)?;
    let body = format!(
        r#"<m:CreateFolder>
 <m:ParentFolderId><t:DistinguishedFolderId Id="msgfolderroot"/></m:ParentFolderId>
 <m:Folders><t:Folder><t:DisplayName>{}</t:DisplayName></t:Folder></m:Folders>
</m:CreateFolder>"#,
        xml_escape(name)
    );
    client.call(&body).map(|_| ())
}

pub fn delete_folder(account: &Account, folder: &str) -> Result<(), String> {
    let client = EwsClient::new(account)?;
    let body = format!(
        r#"<m:DeleteFolder DeleteType="MoveToDeletedItems">
 <m:FolderIds><t:FolderId Id="{}"/></m:FolderIds>
</m:DeleteFolder>"#,
        xml_escape(folder)
    );
    client.call(&body).map(|_| ())
}

fn create_mime_item(
    client: &EwsClient,
    mime: &[u8],
    disposition: &str,
    saved_folder: &str,
) -> Result<String, String> {
    let encoded = BASE64.encode(mime);
    let body = format!(
        r#"<m:CreateItem MessageDisposition="{disposition}">
 <m:SavedItemFolderId><t:FolderId Id="{}"/></m:SavedItemFolderId>
 <m:Items><t:Message><t:MimeContent CharacterSet="UTF-8">{encoded}</t:MimeContent></t:Message></m:Items>
</m:CreateItem>"#,
        xml_escape(saved_folder)
    );
    let root = client.call(&body)?;
    let mut items = Vec::new();
    root.all("ItemId", &mut items);
    items
        .iter()
        .find_map(|item| item.attr("Id").map(|id| id.to_string()))
        .ok_or_else(|| "EWS 没有返回新邮件的 ItemId".to_string())
}

pub fn send_mime(account: &Account, drafts: &str, sent: &str, raw: &[u8]) -> Result<(), String> {
    let client = EwsClient::new(account)?;
    let item_id = create_mime_item(&client, raw, "SaveOnly", drafts)?;
    let body = format!(
        r#"<m:SendItem SaveItemToFolder="true">
 <m:ItemIds><t:ItemId Id="{}"/></m:ItemIds>
 <m:SavedItemFolderId><t:FolderId Id="{}"/></m:SavedItemFolderId>
</m:SendItem>"#,
        xml_escape(&item_id),
        xml_escape(sent)
    );
    if let Err(error) = client.call(&body) {
        // 发送失败：把刚创建的草稿从服务器上删掉，避免草稿箱里留一封发了一半的信。
        let _ = client.call(&format!(
            r#"<m:DeleteItem DeleteType="HardDelete"><m:ItemIds><t:ItemId Id="{}"/></m:ItemIds></m:DeleteItem>"#,
            xml_escape(&item_id)
        ));
        return Err(error);
    }
    Ok(())
}

pub fn save_draft(
    account: &Account,
    drafts: &str,
    raw: &[u8],
) -> Result<serde_json::Value, String> {
    let client = EwsClient::new(account)?;
    let item_id = create_mime_item(&client, raw, "SaveOnly", drafts)?;
    let uid = store::ensure_remote_uid(&account.id, drafts, &item_id)?;
    if let Ok((unread, total)) = folder_counts(account, drafts) {
        let _ = store::update_folder_counts(&account.id, drafts, unread, total, 0, 0);
    }
    Ok(serde_json::json!({ "folder": drafts, "uid": uid, "remoteId": item_id }))
}

pub fn discard_draft(account: &Account, folder: &str, uid: u32) -> Result<(), String> {
    let Some(remote_id) = store::remote_id_for(&account.id, folder, uid) else {
        return Ok(());
    };
    let client = EwsClient::new(account)?;
    let body = format!(
        r#"<m:DeleteItem DeleteType="HardDelete"><m:ItemIds><t:ItemId Id="{}"/></m:ItemIds></m:DeleteItem>"#,
        xml_escape(&remote_id)
    );
    client.call(&body)?;
    let _ = store::delete_remote_ids(&account.id, folder, &[uid]);
    Ok(())
}

pub fn test_account_with_secret(account: &Account, secret: Option<&str>) -> Result<(), String> {
    let client = EwsClient::new_with_secret(account, secret)?;
    let body = r#"<m:GetFolder>
 <m:FolderShape><t:BaseShape>IdOnly</t:BaseShape></m:FolderShape>
 <m:FolderIds><t:DistinguishedFolderId Id="inbox"/></m:FolderIds>
</m:GetFolder>"#;
    client.call(body).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> Account {
        Account {
            id: "ews-acc".into(),
            email: "me@corp.example".into(),
            display_name: String::new(),
            protocol: "ews".into(),
            imap_host: String::new(),
            imap_port: 0,
            imap_security: super::super::model::Security::Ssl,
            ews_url: String::new(),
            pop3_host: String::new(),
            pop3_port: 0,
            pop3_security: super::super::model::Security::None,
            pop3_leave_on_server: true,
            smtp_host: String::new(),
            smtp_port: 0,
            smtp_security: super::super::model::Security::None,
            username: String::new(),
            auth_kind: "password".into(),
            oauth_provider: String::new(),
            oauth_expires: 0,
            proxy: None,
            accent: String::new(),
            signature: String::new(),
            folder_map: Default::default(),
            delegate: Vec::new(),
            sieve: None,
            created_at: 0,
            last_sync_at: 0,
        }
    }

    #[test]
    fn xml_escape_covers_markup() {
        assert_eq!(xml_escape("a&b<c>\"d\""), "a&amp;b&lt;c&gt;&quot;d&quot;");
    }

    #[test]
    fn parses_find_folder_response_into_mail_folder() {
        let xml = r#"<Envelope><Body><FindFolderResponse><ResponseMessages>
<FindFolderResponseMessage><ResponseCode>NoError</ResponseCode><RootFolder><Folders>
<Folder><FolderId Id="AAA=" ChangeKey="k"/><DisplayName>项目</DisplayName><TotalCount>12</TotalCount><UnreadCount>3</UnreadCount></Folder>
</Folders></RootFolder></FindFolderResponseMessage>
</ResponseMessages></FindFolderResponse></Body></Envelope>"#;
        let root = parse_xml(xml).unwrap();
        assert!(response_error(&root).is_none());
        let mut folders = Vec::new();
        root.all("Folder", &mut folders);
        let folder = folder_from_node(&account(), folders[0], FolderRole::Custom).unwrap();
        assert_eq!(folder.name, "AAA=");
        assert_eq!(folder.display_name, "项目");
        assert_eq!(folder.total, 12);
        assert_eq!(folder.unread, 3);
    }

    #[test]
    fn parses_mailbox_address() {
        let xml = r#"<Mailbox><Name>张三</Name><EmailAddress>zhangsan@corp.example</EmailAddress></Mailbox>"#;
        let root = parse_xml(xml).unwrap();
        let mailbox = root.child("Mailbox").unwrap();
        let address = mailbox_address(Some(mailbox));
        assert_eq!(address.name, "张三");
        assert_eq!(address.address, "zhangsan@corp.example");
    }

    #[test]
    fn parses_soap_error_message() {
        let xml = r#"<Envelope><Body><ResponseMessages>
<GetFolderResponseMessage><ResponseCode>ErrorFolderNotFound</ResponseCode><MessageText>找不到文件夹</MessageText></GetFolderResponseMessage>
</ResponseMessages></Body></Envelope>"#;
        let root = parse_xml(xml).unwrap();
        let error = response_error(&root).unwrap();
        assert!(error.contains("ErrorFolderNotFound"));
        assert!(error.contains("找不到文件夹"));
    }
}
