//! 邮箱客户端的数据模型：账户、文件夹、邮件、规则与设置。
//!
//! 这里只有「能跨 IPC 传、能进 SQLite」的扁平结构。连接到 IMAP/SMTP 的句柄、
//! 未解析的 MIME 原文都不属于模型层，见 `imap` 与 `mime`。

use serde::{Deserialize, Serialize};

/// 传输层安全模式。三种都要支持：国内企业邮常见的 143 + STARTTLS 与 587 + STARTTLS，
/// 以及 993/465 的隐式 TLS。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Security {
    /// 直接明文（本地 relay / 内网测试用，界面上会给出告警）。
    None,
    /// 先连明文端口，再用 STARTTLS 升级。
    Starttls,
    /// 建连即 TLS（993 / 465）。
    Ssl,
}

impl Default for Security {
    fn default() -> Self {
        Security::Ssl
    }
}

impl Security {
    pub fn as_str(self) -> &'static str {
        match self {
            Security::None => "none",
            Security::Starttls => "starttls",
            Security::Ssl => "ssl",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Security::None,
            "starttls" => Security::Starttls,
            _ => Security::Ssl,
        }
    }
}

/// 代理模式。与资源嗅探模块保持同样的语义，但单独定义：两者序列化形态不同，共用会互相牵制。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    /// 直连，忽略系统代理。
    None,
    /// 跟随系统代理（优先环境变量，其次系统设置）。
    #[default]
    System,
    /// 使用下面 `url` 指定的代理。
    Custom,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    #[serde(default)]
    pub mode: ProxyMode,
    /// `http://user:pass@host:port` / `socks5://host:port`。仅 `custom` 模式使用。
    #[serde(default)]
    pub url: Option<String>,
    /// 不套代理的主机后缀，例如 `internal.corp`、`localhost`。
    #[serde(default)]
    pub bypass: Vec<String>,
}

/// 一个邮箱账户。口令不进这张表，只存 keyring 的服务名（`account.id`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub email: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub imap_host: String,
    #[serde(default = "default_imap_port")]
    pub imap_port: u16,
    #[serde(default)]
    pub imap_security: Security,
    #[serde(default)]
    pub smtp_host: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_security: Security,
    /// 登录名。留空则回退到邮箱地址（Gmail/QQ/163 都是地址本身，企业邮常需要短用户名）。
    #[serde(default)]
    pub username: String,
    /// `password` | `oauth2`（当前只落地密码/授权码；OAuth2 预留字段，界面灰显）。
    #[serde(default = "default_auth_kind")]
    pub auth_kind: String,
    /// 代理配置（账户级覆盖全局设置）。`None` 表示跟随全局。
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    /// 账户配色，用于左侧账户条与邮件行的小色点。
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default)]
    pub signature: String,
    /// 服务器文件夹名 → 语义角色（`sent` / `drafts` / `trash` …）。
    /// 有的服务器用 `Sent Items`，有的用 `已发送`，用户可在这里覆盖自动识别。
    #[serde(default)]
    pub folder_map: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub last_sync_at: i64,
}

fn default_imap_port() -> u16 {
    993
}
fn default_smtp_port() -> u16 {
    465
}
fn default_auth_kind() -> String {
    "password".to_string()
}
fn default_accent() -> String {
    "#5b7cfa".to_string()
}

impl Account {
    pub fn login_name(&self) -> String {
        let u = self.username.trim();
        if u.is_empty() {
            self.email.trim().to_string()
        } else {
            u.to_string()
        }
    }
}

/// 文件夹的语义分类。搜索「未读」「已发送」这类快捷入口按它过滤，不依赖服务器上的显示名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderRole {
    Inbox,
    Sent,
    Drafts,
    Trash,
    Junk,
    Archive,
    Custom,
}

impl FolderRole {
    pub fn as_str(self) -> &'static str {
        match self {
            FolderRole::Inbox => "inbox",
            FolderRole::Sent => "sent",
            FolderRole::Drafts => "drafts",
            FolderRole::Trash => "trash",
            FolderRole::Junk => "junk",
            FolderRole::Archive => "archive",
            FolderRole::Custom => "custom",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "inbox" => FolderRole::Inbox,
            "sent" => FolderRole::Sent,
            "drafts" => FolderRole::Drafts,
            "trash" => FolderRole::Trash,
            "junk" => FolderRole::Junk,
            "archive" => FolderRole::Archive,
            _ => FolderRole::Custom,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailFolder {
    pub account_id: String,
    /// 服务器上的真实名字（`INBOX`、`Sent Items`、`&XfJT0ZAB-` 这类 IMAP 修饰 UTF-7）。
    pub name: String,
    /// 已解码的显示名（`已发送`）。
    pub display_name: String,
    #[serde(default)]
    pub delimiter: Option<String>,
    pub role: FolderRole,
    pub unread: u32,
    pub total: u32,
    #[serde(default)]
    pub uid_validity: u32,
    #[serde(default)]
    pub uid_next: u32,
    #[serde(default)]
    pub selectable: bool,
}

/// 邮件主题行（列表用）。正文与附件单独按需拉取，列表页不碰大字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSummary {
    pub account_id: String,
    pub folder: String,
    pub uid: u32,
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub from_name: String,
    #[serde(default)]
    pub from_addr: String,
    #[serde(default)]
    pub to_json: String,
    #[serde(default)]
    pub cc_json: String,
    /// Unix 秒。优先取 Date 头，缺失时退回服务器 INTERNALDATE。
    pub date: i64,
    #[serde(default)]
    pub size: u32,
    #[serde(default)]
    pub snippet: String,
    #[serde(default)]
    pub seen: bool,
    #[serde(default)]
    pub flagged: bool,
    #[serde(default)]
    pub answered: bool,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub has_attachments: bool,
    /// 归一化后的线程键（去掉 Re:/答复: 前缀后的主题）。
    #[serde(default)]
    pub thread_key: String,
    /// 本地分类（规则打上的标签），逗号分隔。
    #[serde(default)]
    pub labels: String,
    /// 服务器文件夹是「已发送 / 草稿」时为 true：规则与未读徽标都不应作用于它。
    #[serde(default)]
    pub outgoing: bool,
}

impl MessageSummary {
    pub fn to_addrs(&self) -> Vec<MailAddress> {
        serde_json::from_str(&self.to_json).unwrap_or_default()
    }
    pub fn cc_addrs(&self) -> Vec<MailAddress> {
        serde_json::from_str(&self.cc_json).unwrap_or_default()
    }
}

/// 邮件正文与附件清单。HTML 已在后端做过一次安全清洗。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDetail {
    pub account_id: String,
    pub folder: String,
    pub uid: u32,
    #[serde(default)]
    pub message_id: Option<String>,
    pub subject: String,
    pub from_name: String,
    pub from_addr: String,
    pub to: Vec<MailAddress>,
    pub cc: Vec<MailAddress>,
    pub bcc: Vec<MailAddress>,
    pub reply_to: Vec<MailAddress>,
    pub date: i64,
    /// `Message-Id` 链，用于拼会话视图与回复头。
    #[serde(default)]
    pub in_reply_to: String,
    #[serde(default)]
    pub references: String,
    /// sanitize 后的 HTML；没有 HTML 部分时为 None，前端用 `text` 渲染。
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub text: String,
    pub attachments: Vec<AttachmentMeta>,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub labels: String,
    #[serde(default)]
    pub seen: bool,
    #[serde(default)]
    pub flagged: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAddress {
    #[serde(default)]
    pub name: String,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMeta {
    /// MIME 部件的正文字节偏移（mail-parser 的 part id，用于 `extract_part`）。
    pub part: u32,
    pub filename: String,
    pub content_type: String,
    pub size: u32,
    #[serde(default)]
    pub content_id: Option<String>,
}

/// 搜索条件。所有字段都参与 AND；`text` 走 FTS5，其余走列过滤。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    #[serde(default)]
    pub account_ids: Vec<String>,
    #[serde(default)]
    pub folder: Option<String>,
    /// 语义：`all` | `unread` | `flagged` | `attachments` | `drafts`。空等同 `all`。
    #[serde(default)]
    pub scope: Option<String>,
    /// 全文 / 主题 / 发件人 / 收件人 / 正文的组合关键词。
    #[serde(default)]
    pub text: String,
    /// 搜索字段：`all` | `subject` | `from` | `to` | `body`。
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    /// Unix 秒区间（含端点）。
    #[serde(default)]
    pub after: Option<i64>,
    #[serde(default)]
    pub before: Option<i64>,
    #[serde(default)]
    pub has_attachment: Option<bool>,
    #[serde(default)]
    pub unread_only: Option<bool>,
    #[serde(default)]
    pub starred_only: Option<bool>,
    /// 标签（规则分类结果）包含其中之一。
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub min_size: Option<u32>,
    #[serde(default)]
    pub max_size: Option<u32>,
    /// 排序：`date` | `size` | `sender` | `subject`。
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub descending: Option<bool>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

/// 规则条件：全部满足才命中（AND）。与 Outlook 的规则向导同一套心智模型。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCondition {
    /// 发件人地址或显示名包含（不区分大小写）。
    #[serde(default)]
    pub from_contains: Vec<String>,
    /// 收件人（To/Cc）包含。
    #[serde(default)]
    pub to_contains: Vec<String>,
    #[serde(default)]
    pub subject_contains: Vec<String>,
    #[serde(default)]
    pub subject_not_contains: Vec<String>,
    #[serde(default)]
    pub body_contains: Vec<String>,
    /// 主题 / 正文 / 发件人合一的正则（Rust regex 语法）。
    #[serde(default)]
    pub any_matches_regex: Option<String>,
    #[serde(default)]
    pub has_attachment: Option<bool>,
    /// 仅作用于这些账户；空表示全部账户。
    #[serde(default)]
    pub account_ids: Vec<String>,
    /// 仅作用于这些文件夹角色；空表示收件箱。
    #[serde(default)]
    pub folder_roles: Vec<String>,
    #[serde(default)]
    pub min_size: Option<u32>,
}

/// 规则动作。一条规则可以同时做多件事（Outlook 同样如此）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleAction {
    /// 归档到指定文件夹（服务器名）；`None` 表示不移动。
    #[serde(default)]
    pub move_to: Option<String>,
    /// 复制到指定文件夹。
    #[serde(default)]
    pub copy_to: Option<String>,
    /// 打标签（写入 labels 列，同时参与搜索）。
    #[serde(default)]
    pub add_labels: Vec<String>,
    #[serde(default)]
    pub remove_labels: Vec<String>,
    #[serde(default)]
    pub mark_read: bool,
    #[serde(default)]
    pub star: bool,
    /// 转发到这些地址。
    #[serde(default)]
    pub forward_to: Vec<String>,
    #[serde(default)]
    pub delete: bool,
    /// 跳过收件箱。
    #[serde(default)]
    pub skip_inbox: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailRule {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    /// 数值越小越先执行。
    #[serde(default)]
    pub priority: i32,
    /// 预留字段：`all`（全部条件）。
    #[serde(default = "default_match")]
    pub match_mode: String,
    pub condition: RuleCondition,
    pub action: RuleAction,
    #[serde(default)]
    pub stop_processing: bool,
    #[serde(default)]
    pub created_at: i64,
}

fn default_match() -> String {
    "all".to_string()
}

/// 全局设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailSettings {
    /// 当前主题 id（内置：`ink` / `paper`，或用户自定义 id）。
    #[serde(default = "default_theme")]
    pub theme: String,
    /// 界面密度：`compact` | `comfortable` | `spacious`。
    #[serde(default = "default_density")]
    pub density: String,
    /// 阅读字体（CSS font-family 串）；`system` 表示跟随系统。
    #[serde(default = "default_reading_font")]
    pub reading_font: String,
    /// 等宽字体（源码视图 / 编辑器）。
    #[serde(default = "default_mono_font")]
    pub mono_font: String,
    /// 阅读字号（px）。
    #[serde(default = "default_reading_size")]
    pub reading_size: u32,
    /// 主题色覆盖：留空则用主题自带。
    #[serde(default)]
    pub accent_override: String,
    /// 是否在阅读区显示原始头部。
    #[serde(default)]
    pub show_headers: bool,
    /// 拉取策略：`idle` | `poll`。当前实现为定时轮询，字段保留给 IDLE。
    #[serde(default = "default_fetch_mode")]
    pub fetch_mode: String,
    #[serde(default = "default_poll_seconds")]
    pub poll_seconds: u32,
    /// 同步时每封邮件的预览字符数。
    #[serde(default = "default_snippet_len")]
    pub snippet_len: u32,
    /// 发信后延迟 N 秒才真正投递（撤回窗口）。
    #[serde(default = "default_undo_seconds")]
    pub undo_send_seconds: u32,
    /// 代理（全局默认；账户可覆盖）。
    #[serde(default)]
    pub proxy: ProxyConfig,
    /// 收件箱是否把同一 thread_key 合成一行。
    #[serde(default = "default_true")]
    pub group_threads: bool,
    /// 左侧账户/文件夹栏是否收起（给列表腾宽度）。
    #[serde(default)]
    pub sidebar_collapsed: bool,
    /// 中间邮件列表是否收起（只读长邮件时用）。
    #[serde(default)]
    pub list_collapsed: bool,
    /// 左侧账户栏宽度（px）。0 表示用默认值，由前端兜底。
    #[serde(default)]
    pub sidebar_width: u32,
    /// 中间邮件列表宽度（px）。
    #[serde(default)]
    pub list_width: u32,
    #[serde(default = "default_true")]
    pub rules_enabled: bool,
    /// 本地保留天数；0 表示不清理。
    #[serde(default)]
    pub retention_days: u32,
    /// 新建邮件默认使用 HTML。
    #[serde(default = "default_true")]
    pub compose_html: bool,
    /// 远端图片是否直接加载（false 时由用户点「显示图片」）。
    #[serde(default)]
    pub inline_images: bool,
    /// 自定义主题。
    #[serde(default)]
    pub custom_themes: Vec<CustomTheme>,
}

fn default_theme() -> String {
    "ink".to_string()
}
fn default_density() -> String {
    "comfortable".to_string()
}
fn default_reading_font() -> String {
    "system".to_string()
}
fn default_mono_font() -> String {
    "Maple Mono NF CN".to_string()
}
fn default_reading_size() -> u32 {
    15
}
fn default_fetch_mode() -> String {
    "poll".to_string()
}
fn default_poll_seconds() -> u32 {
    90
}
fn default_snippet_len() -> u32 {
    140
}
fn default_undo_seconds() -> u32 {
    10
}
fn default_true() -> bool {
    true
}

impl Default for MailSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            density: default_density(),
            reading_font: default_reading_font(),
            mono_font: default_mono_font(),
            reading_size: default_reading_size(),
            accent_override: String::new(),
            show_headers: false,
            fetch_mode: default_fetch_mode(),
            poll_seconds: default_poll_seconds(),
            snippet_len: default_snippet_len(),
            undo_send_seconds: default_undo_seconds(),
            proxy: ProxyConfig::default(),
            group_threads: true,
            sidebar_collapsed: false,
            list_collapsed: false,
            sidebar_width: 232,
            list_width: 340,
            rules_enabled: true,
            retention_days: 0,
            compose_html: true,
            inline_images: false,
            custom_themes: Vec::new(),
        }
    }
}

/// 用户自定义主题：一组 CSS 变量。`ink` / `paper` 两套内置主题同样用这套 token 表达。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomTheme {
    pub id: String,
    pub name: String,
    /// `light` | `dark`，用于让组件（源码编辑器、滚动条）跟随。
    pub mode: String,
    /// token 名 -> 值。键见 `theme::TOKENS`。
    #[serde(default)]
    pub tokens: std::collections::BTreeMap<String, String>,
}

/// 发信请求（由前端编辑器发起）。需要 Serialize：延迟发送时会整份序列化进 outbox。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRequest {
    pub account_id: String,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub subject: String,
    /// 纯文本正文（始终存在，作为 multipart/alternative 的第一段）。
    #[serde(default)]
    pub text: String,
    /// HTML 正文；为空则只发纯文本。
    #[serde(default)]
    pub html: Option<String>,
    /// 回复头，用于让收信端线程归并正确。
    #[serde(default)]
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub references: Option<String>,
    /// 附件本地路径。
    #[serde(default)]
    pub attachments: Vec<String>,
    /// 是否保存到「已发送」。
    #[serde(default)]
    pub save_to_sent: Option<bool>,
}

/// 待发送队列中的一项（延迟发送 / 撤回）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxItem {
    pub id: String,
    pub account_id: String,
    pub subject: String,
    pub to: String,
    pub due_at: i64,
    pub created_at: i64,
}

/// 通信测试结果（连接诊断页用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProbe {
    pub ok: bool,
    #[serde(default)]
    pub stage: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
}
