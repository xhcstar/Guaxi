//! OAuth2（XOAUTH2）认证。
//!
//! 为什么必须有：微软 2022 年起对个人与企业账户全面停用了 IMAP/SMTP 的基本身份验证，
//! Gmail 也要求「应用专用密码」（要求两个两步验证，很多人的账户开不了）。
//! 没有 OAuth2，「对标 Outlook」的邮箱在 Gmail / Outlook 账户上一个都接不进来。
//!
//! 设计取舍：
//! - **设备码流程（RFC 8628）而不是本机回环监听**：桌面应用起一个本地 HTTP 回调
//!   需要占用端口、处理浏览器回跳、还要面对「端口被占 / 杀毒软件拦」的现实问题。
//!   设备码流程只要求用户在一个网页里输入 8 位码，不需要注册回调地址，
//!   也天然适配「用户先从手机上点同意」的场景。
//! - **令牌存 keychain，不进数据库**：与口令同样的边界——备份 / 同步 `mail.db`
//!   不会把能读信的东西带出去。
//! - keychain 里存的是 JSON：`{refresh_token, access_token, expires_at}`，
//!   过期时用刷新令牌换新的，换完立刻写回。
//!
//! 客户端 id 是 OAuth 应用的公开标识（不是机密），因此可以放在源码里；
//! 机密（client_secret）只有「机密客户端」才需要，桌面 / 公共客户端不使用。

use super::model::Account;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

/// 各服务商的端点配置。
#[derive(Debug, Clone, Copy)]
pub struct Provider {
    pub id: &'static str,
    pub device_url: &'static str,
    pub token_url: &'static str,
    /// IMAP：`AUTHENTICATE XOAUTH2` 的 bearer 用户名。
    pub imap_scope: &'static str,
    /// SMTP：`AUTH XOAUTH2`（目前与 IMAP 用同一份 scope，保留字段以便服务商要求不同 scope 时区分）。
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub smtp_scope: &'static str,
    pub client_id: &'static str,
    /// Gmail 的设备码流程要求这个额外参数（离线访问 + 同意提示）。
    pub extra: &'static [(&'static str, &'static str)],
}

const GOOGLE: Provider = Provider {
    id: "gmail",
    device_url: "https://oauth2.googleapis.com/device/code",
    token_url: "https://oauth2.googleapis.com/token",
    imap_scope: "https://mail.google.com/",
    smtp_scope: "https://mail.google.com/",
    client_id: "764086051850-6qr4p6gpi6hn506pt8ejuq83di341hur.apps.googleusercontent.com",
    extra: &[],
};

const MICROSOFT: Provider = Provider {
    id: "outlook",
    device_url: "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode",
    token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    imap_scope: "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send offline_access",
    smtp_scope: "https://outlook.office.com/SMTP.Send offline_access",
    client_id: "9e5f94bc-e8a4-4e73-b8be-63364c29d753",
    extra: &[],
};

/// 用户自定义服务商（企业自建 IdP）：端点由账户配置给出。
const CUSTOM: Provider = Provider {
    id: "custom",
    device_url: "",
    token_url: "",
    imap_scope: "",
    smtp_scope: "",
    client_id: "",
    extra: &[],
};

pub fn provider_for(id: &str) -> Option<Provider> {
    match id.trim().to_ascii_lowercase().as_str() {
        "gmail" | "google" => Some(GOOGLE),
        "outlook" | "microsoft" | "office365" => Some(MICROSOFT),
        "custom" => Some(CUSTOM),
        _ => None,
    }
}

/// 设备码流程的第一步返回值（展示给用户的「请在网页输入这个码」）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    pub expires_in: u64,
    pub interval: u64,
    #[serde(default)]
    pub message: String,
}

/// keychain 里存的令牌 JSON。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenSet {
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub access_token: String,
    /// Unix 秒。
    #[serde(default)]
    pub expires_at: i64,
}

impl TokenSet {
    fn valid(&self) -> bool {
        !self.access_token.is_empty() && self.expires_at > chrono::Utc::now().timestamp() + 120
    }
}

fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}

/// 发起设备码授权。返回的 `user_code` / `verification_url` 要展示给用户。
pub fn start_device_flow(
    provider: &Provider,
    client_id_override: Option<&str>,
    scope_override: Option<&str>,
) -> Result<DeviceCode, String> {
    if provider.device_url.is_empty() {
        return Err("自定义 OAuth 服务商需要在账户设置里填写设备授权端点".to_string());
    }
    let client_id = client_id_override
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(provider.client_id);
    if client_id.is_empty() {
        return Err("缺少 OAuth 客户端 ID".to_string());
    }
    let scope = scope_override
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(provider.imap_scope);

    let mut form: Vec<(&str, &str)> = vec![("client_id", client_id), ("scope", scope)];
    // 微软要求带上 openid 才会返回 refresh_token（offline_access 已含在 scope 里）。
    if provider.id == "outlook" {
        form.push(("offline_access", "true"));
    }
    // Gmail 的设备码流程要求显式声明它支持设备授权。
    form.extend_from_slice(provider.extra);

    let response = http()?
        .post(provider.device_url)
        .form(&form)
        .send()
        .map_err(|e| format!("设备授权请求失败: {e}"))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|e| format!("读取设备授权响应失败: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "设备授权被拒绝（HTTP {status}）：{}",
            summarize_error(&body)
        ));
    }
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("设备授权响应格式无效: {e}"))?;
    let pick = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    Ok(DeviceCode {
        device_code: pick("device_code"),
        user_code: pick("user_code"),
        verification_url: {
            let url = pick("verification_url");
            if url.is_empty() {
                pick("verification_uri")
            } else {
                url
            }
        },
        expires_in: value.get("expires_in").and_then(|v| v.as_u64()).unwrap_or(900),
        interval: value
            .get("interval")
            .and_then(|v| v.as_u64())
            .unwrap_or(5),
        message: pick("message"),
    })
}

/// 轮询换取令牌。设备码流程要求客户端按 `interval` 轮询，遇到
/// `authorization_pending` 继续等，`slow_down` 则拉长间隔。
pub fn poll_device_token(
    provider: &Provider,
    client_id_override: Option<&str>,
    device_code: &str,
    interval_secs: u64,
) -> Result<TokenSet, String> {
    let client_id = client_id_override
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(provider.client_id);
    let client = http()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(900);
    let mut interval = interval_secs.clamp(2, 30);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_secs(interval));
        let response = client
            .post(provider.token_url)
            .form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                (
                    "grant_type",
                    "urn:ietf:params:oauth:grant-type:device_code",
                ),
            ])
            .send()
            .map_err(|e| format!("换取令牌失败: {e}"))?;
        let body = response
            .text()
            .map_err(|e| format!("读取令牌响应失败: {e}"))?;
        let value: serde_json::Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => return Err(format!("令牌响应格式无效: {}", summarize_error(&body))),
        };
        if let Some(token) = value.get("access_token").and_then(|v| v.as_str()) {
            let expires_in = value
                .get("expires_in")
                .and_then(|v| v.as_i64())
                .unwrap_or(3600);
            return Ok(TokenSet {
                refresh_token: value
                    .get("refresh_token")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                access_token: token.to_string(),
                expires_at: chrono::Utc::now().timestamp() + expires_in,
            });
        }
        match value.get("error").and_then(|v| v.as_str()).unwrap_or("") {
            "authorization_pending" => continue,
            "slow_down" => {
                interval = (interval + 5).min(60);
                continue;
            }
            "authorization_declined" => return Err("用户在授权页面上拒绝了访问".to_string()),
            "expired_token" => return Err("设备码已过期，请重新发起授权".to_string()),
            other => {
                return Err(format!(
                    "换取令牌失败: {}{}",
                    if other.is_empty() { "未知错误" } else { other },
                    summarize_error(&body)
                ))
            }
        }
    }
    Err("等待授权超时（15 分钟）".to_string())
}

/// 用刷新令牌换新的访问令牌。
pub fn refresh_access_token(
    provider: &Provider,
    client_id_override: Option<&str>,
    refresh_token: &str,
) -> Result<TokenSet, String> {
    let client_id = client_id_override
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(provider.client_id);
    let response = http()?
        .post(provider.token_url)
        .form(&[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .map_err(|e| format!("刷新令牌失败: {e}"))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|e| format!("读取刷新响应失败: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "刷新令牌被拒绝（HTTP {status}）：{}",
            summarize_error(&body)
        ));
    }
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("刷新响应格式无效: {e}"))?;
    let access = value
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("刷新响应缺少 access_token: {}", summarize_error(&body)))?;
    let expires_in = value
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);
    Ok(TokenSet {
        // 有的服务商刷新时不返回新的 refresh_token：沿用旧的。
        refresh_token: value
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(refresh_token)
            .to_string(),
        access_token: access.to_string(),
        expires_at: chrono::Utc::now().timestamp() + expires_in,
    })
}

fn summarize_error(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // OAuth 错误响应通常很小；仍然截断，避免把一大段 HTML 错误页糊到界面上。
    let short: String = trimmed.chars().take(300).collect();
    format!("（{short}）")
}

// ---------------------------------------------------------------------------
// keychain 存取
// ---------------------------------------------------------------------------

const TOKEN_SERVICE: &str = "guaxi-mail-oauth";

fn entry(account_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(TOKEN_SERVICE, account_id).map_err(|e| format!("访问系统凭据失败: {e}"))
}

pub fn load_tokens(account_id: &str) -> Option<TokenSet> {
    let raw = entry(account_id).ok()?.get_password().ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_tokens(account_id: &str, tokens: &TokenSet) -> Result<(), String> {
    let json = serde_json::to_string(tokens).map_err(|e| e.to_string())?;
    entry(account_id)?
        .set_password(&json)
        .map_err(|e| format!("写入系统凭据失败: {e}"))
}

pub fn delete_tokens(account_id: &str) {
    if let Ok(entry) = entry(account_id) {
        let _ = entry.delete_credential();
    }
}

pub fn has_tokens(account_id: &str) -> bool {
    load_tokens(account_id).is_some_and(|t| !t.refresh_token.is_empty() || !t.access_token.is_empty())
}

/// 取一个可用的访问令牌（必要时刷新）。刷新成功后立刻回写 keychain 与账户的过期时间。
pub fn access_token(account: &Account) -> Result<String, String> {
    let provider = provider_for(&account.oauth_provider)
        .ok_or_else(|| format!("未知的 OAuth 服务商: {}", account.oauth_provider))?;
    let mut tokens = load_tokens(&account.id).ok_or_else(|| {
        format!(
            "账户 {} 还没有完成 OAuth 授权，请在账户设置里重新登录",
            account.email
        )
    })?;
    if tokens.valid() {
        return Ok(tokens.access_token);
    }
    if tokens.refresh_token.trim().is_empty() {
        return Err(format!(
            "账户 {} 的授权已过期且没有刷新令牌，请重新授权",
            account.email
        ));
    }
    let refreshed = refresh_access_token(&provider, None, &tokens.refresh_token)?;
    tokens = refreshed;
    save_tokens(&account.id, &tokens)?;
    let _ = store::touch_oauth_expires(&account.id, tokens.expires_at);
    info!(account = %account.email, "mail: OAuth 访问令牌已刷新");
    Ok(tokens.access_token)
}

use super::store;

/// XOAUTH2 的 SASL 初始响应：`user=<email>\x01auth=Bearer <token>\x01\x01`。
///
/// IMAP (`AUTHENTICATE XOAUTH2`) 与 SMTP (`AUTH XOAUTH2`) 用的是同一份编码串，
/// 区别只在 SMTP 可以把它跟在命令行后面，IMAP 必须等 `+` 续行。
pub fn xoauth2_string(user: &str, token: &str) -> String {
    format!("user={user}\x01auth=Bearer {token}\x01\x01")
}

/// IMAP 的 `Authenticator` 实现：服务器发来 `+` 挑战时回 XOAUTH2 串。
///
/// 失败时服务器会再发一个 `+ <base64 error>` 挑战，我们回空串结束握手
/// （crate 的握手循环对空响应会把服务器的错误原文带出来）。
pub struct XOAuth2 {
    pub user: String,
    pub token: String,
    pub first: std::sync::atomic::AtomicBool,
}

impl XOAuth2 {
    pub fn new(user: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            user: user.into(),
            token: token.into(),
            first: std::sync::atomic::AtomicBool::new(true),
        }
    }
}

impl imap::Authenticator for XOAuth2 {
    type Response = String;

    fn process(&self, _challenge: &[u8]) -> Self::Response {
        if self
            .first
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            xoauth2_string(&self.user, &self.token)
        } else {
            // 第二次挑战意味着认证失败；空响应让服务器回最终的错误行。
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xoauth2_string_matches_rfc() {
        let s = xoauth2_string("me@example.com", "ya29.token");
        assert_eq!(s, "user=me@example.com\x01auth=Bearer ya29.token\x01\x01");
    }

    #[test]
    fn providers_resolve() {
        assert_eq!(provider_for("gmail").unwrap().id, "gmail");
        assert_eq!(provider_for("Outlook").unwrap().id, "outlook");
        assert_eq!(provider_for("microsoft").unwrap().id, "outlook");
        assert!(provider_for("nope").is_none());
    }

    #[test]
    fn token_validity_requires_headroom() {
        let mut t = TokenSet::default();
        t.access_token = "a".into();
        t.expires_at = chrono::Utc::now().timestamp() + 30;
        // 剩余不到 2 分钟就当作「即将过期」，避免请求到服务器时刚好失效。
        assert!(!t.valid());
        t.expires_at = chrono::Utc::now().timestamp() + 3600;
        assert!(t.valid());
    }
}
