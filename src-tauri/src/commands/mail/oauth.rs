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
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read as IoRead, Write as IoWrite};
use std::net::TcpListener;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::info;
use uuid::Uuid;

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
    imap_scope: "https://mail.google.com/ openid email",
    smtp_scope: "https://mail.google.com/ openid email",
    client_id: "764086051850-6qr4p6gpi6hn506pt8ejuq83di341hur.apps.googleusercontent.com",
    extra: &[],
};

const MICROSOFT: Provider = Provider {
    id: "outlook",
    device_url: "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode",
    token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    // openid/profile/email 用来在授权完成后直接拿到账户邮箱，用户就不必手填地址。
    imap_scope: "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send openid profile email offline_access",
    smtp_scope: "https://outlook.office.com/SMTP.Send openid email offline_access",
    client_id: "9e5f94bc-e8a4-4e73-b8be-63364c29d753",
    extra: &[],
};

/// Microsoft Graph（Exchange Online / Microsoft 365 的原生通道）。
///
/// 与 `outlook` 共用同一个公开客户端 ID，但 scope 换成 Graph 的邮件读写与发送权限。
/// IMAP/SMTP 的 XOAUTH2 scope 不能用于 Graph 端点，反之亦然，所以必须是独立 provider。
const GRAPH: Provider = Provider {
    id: "graph",
    device_url: "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode",
    token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    imap_scope: "https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send openid profile email offline_access User.Read",
    smtp_scope: "https://graph.microsoft.com/Mail.Send openid profile email offline_access",
    client_id: "9e5f94bc-e8a4-4e73-b8be-63364c29d753",
    extra: &[],
};


/// Exchange Online 的 EWS OAuth（本地 Exchange 通常用 Basic；混合部署可用它）。
const EWS: Provider = Provider {
    id: "ews",
    device_url: "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode",
    token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    imap_scope: "https://outlook.office.com/EWS.AccessAsUser.All openid profile email offline_access",
    smtp_scope: "https://outlook.office.com/EWS.AccessAsUser.All openid profile email offline_access",
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
        "graph" | "exchange" | "m365" => Some(GRAPH),
        "ews" => Some(EWS),
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
    if provider.id == "outlook" || provider.id == "graph" || provider.id == "ews" {
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

/// 从令牌响应里的 id_token（JWT）提取账户邮箱。
///
/// 只在服务商返回 id_token 时可用（scope 里带 openid + email）。签名不需要校验：
/// 这是本客户端刚从 TLS 端点拿到的响应，不涉及跨方转发。
fn email_from_id_token(value: &serde_json::Value) -> Option<String> {
    let token = value.get("id_token")?.as_str()?;
    let payload = token.split('.').nth(1)?.trim_end_matches('=');
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    for key in ["email", "preferred_username", "upn", "unique_name"] {
        if let Some(raw) = claims.get(key).and_then(|v| v.as_str()) {
            let email = raw.trim();
            if email.contains('@') {
                return Some(email.to_string());
            }
        }
    }
    None
}

/// 轮询换取令牌。设备码流程要求客户端按 `interval` 轮询，遇到
/// `authorization_pending` 继续等，`slow_down` 则拉长间隔。
pub fn poll_device_token(
    provider: &Provider,
    client_id_override: Option<&str>,
    device_code: &str,
    interval_secs: u64,
) -> Result<(TokenSet, Option<String>), String> {
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
            let tokens = TokenSet {
                refresh_token: value
                    .get("refresh_token")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                access_token: token.to_string(),
                expires_at: chrono::Utc::now().timestamp() + expires_in,
            };
            return Ok((tokens, email_from_id_token(&value)));
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

// ---------------------------------------------------------------------------
// Outlook：授权码 + 本机回环回调
// ---------------------------------------------------------------------------
//
// 微软服务商的设备码页面在部分浏览器会话下会把登录重定向到
// `http://127.0.0.1/?error=invalid_request...`（缺少 response_type），
// 用户卡在错误页、令牌永远拿不到。Thunderbird 对同一个客户端 ID 用的是
// 授权码流程 + `http://localhost:<随机端口>` 回调（该地址已在应用注册里），
// 不依赖设备码页面，也就绕开了这个问题。
//
// 这里复刻同一条路径：起一个只监听 127.0.0.1 的一次性 HTTP 服务，浏览器登录
// 完成后带着 `?code=` 回到我们自己的端口，再用授权码换令牌。state 参数用于
// 防止别的本机进程伪造回调。

type FlowSlot = Arc<(Mutex<Option<Result<(), String>>>, Condvar)>;

static LOOPBACK_FLOWS: OnceLock<Mutex<HashMap<String, FlowSlot>>> = OnceLock::new();

fn flows() -> &'static Mutex<HashMap<String, FlowSlot>> {
    LOOPBACK_FLOWS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 启动 Outlook 的 localhost 回调授权，返回需要打开的授权页地址。
///
/// 返回的 `device_code` 字段其实是这次授权会话的 id；后续 `wait_loopback_flow`
/// 用它等待回调线程的结果。令牌由回调线程直接写进 keychain 与账户记录。
pub fn start_loopback_flow(
    provider: &Provider,
    account_id: &str,
    client_id_override: Option<&str>,
    scope_override: Option<&str>,
) -> Result<DeviceCode, String> {
    if provider.id != "outlook" {
        return Err("该服务商不支持本机回调授权".to_string());
    }
    let account_id = account_id.trim();
    if account_id.is_empty() {
        return Err("账户还没有保存完成，无法开始授权".to_string());
    }
    let client_id = client_id_override
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(provider.client_id);
    let scope = scope_override
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(provider.imap_scope);

    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("启动本机授权回调失败: {e}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("设置授权回调超时失败: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("读取授权回调端口失败: {e}"))?
        .port();
    // 必须写 localhost：Thunderbird 的客户端注册的就是 http://localhost（端口不限），
    // 用 127.0.0.1 反而可能被微软当成不匹配的回调地址。
    let redirect_uri = format!("http://localhost:{port}/");

    let flow_id = Uuid::new_v4().to_string();
    let authorize_url = build_authorize_url(client_id, scope, &redirect_uri, &flow_id)?;
    info!(
        account = %account_id,
        redirect = %redirect_uri,
        "mail: Outlook 授权已启动（localhost 回调）"
    );

    let slot: FlowSlot = Arc::new((Mutex::new(None), Condvar::new()));
    flows()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(flow_id.clone(), slot.clone());

    let state = flow_id.clone();
    let provider = *provider;
    let client_id = client_id.to_string();
    let scope = scope.to_string();
    let account_id = account_id.to_string();
    let thread_slot = slot.clone();
    std::thread::spawn(move || {
        let result = serve_loopback(
            listener,
            &state,
            &redirect_uri,
            &provider,
            &client_id,
            &scope,
            &account_id,
        );
        let (lock, ready) = &*thread_slot;
        let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        // 用户可能已经通过「手动粘贴回调地址」完成了授权：先到的结果说了算，
        // 别让这个线程随后的超时把成功覆盖成失败。
        if guard.is_none() {
            *guard = Some(result);
        }
        ready.notify_all();
    });

    Ok(DeviceCode {
        device_code: flow_id,
        user_code: String::new(),
        verification_url: authorize_url,
        expires_in: 300,
        interval: 1,
        message: "在浏览器里完成 Microsoft 登录，页面提示授权完成后回到应用。".to_string(),
    })
}

fn build_authorize_url(
    client_id: &str,
    scope: &str,
    redirect_uri: &str,
    state: &str,
) -> Result<String, String> {
    let mut url = url::Url::parse(
        "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
    )
    .map_err(|e| format!("授权地址无效: {e}"))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_mode", "query")
        .append_pair("scope", scope)
        .append_pair("state", state)
        // 强制选账户：避免浏览器复用上一次未完成的登录会话直接跳走。
        .append_pair("prompt", "select_account");
    Ok(url.into())
}

/// 等待浏览器回调，收到授权码后立刻换令牌并写回账户。
fn serve_loopback(
    listener: TcpListener,
    expected_state: &str,
    redirect_uri: &str,
    provider: &Provider,
    client_id: &str,
    scope: &str,
    account_id: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        if Instant::now() > deadline {
            return Err("等待浏览器授权超时（5 分钟）".to_string());
        }
        let (mut stream, _) = match listener.accept() {
            Ok(pair) => pair,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(120));
                continue;
            }
            Err(error) => return Err(format!("等待授权回调失败: {error}")),
        };
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let mut buf = [0u8; 8192];
        let read = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..read]);
        let target = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");
        let parsed = url::Url::parse(&format!("http://localhost{target}")).ok();

        let mut code = None;
        let mut returned_state = None;
        let mut error = None;
        let mut error_description = None;
        if let Some(parsed) = parsed.as_ref() {
            for (key, value) in parsed.query_pairs() {
                match key.as_ref() {
                    "code" => code = Some(value.into_owned()),
                    "state" => returned_state = Some(value.into_owned()),
                    "error" => error = Some(value.into_owned()),
                    "error_description" => error_description = Some(value.into_owned()),
                    _ => {}
                }
            }
        }

        let matched = returned_state.as_deref() == Some(expected_state);
        let succeeded = matched && error.is_none() && code.is_some();
        let body = if succeeded {
            "<!doctype html><meta charset=\"utf-8\"><title>授权完成</title>\
             <p style=\"font:16px system-ui,-apple-system,sans-serif;padding:48px\">\
             授权已完成，可以关闭此页面并回到 Guaxi。</p>"
        } else if error.is_some() {
            "<!doctype html><meta charset=\"utf-8\"><title>授权未完成</title>\
             <p style=\"font:16px system-ui,-apple-system,sans-serif;padding:48px\">\
             授权没有完成，请关闭此页面回到 Guaxi 重新发起授权。</p>"
        } else {
            "<!doctype html><meta charset=\"utf-8\"><title>等待授权</title>\
             <p style=\"font:16px system-ui,-apple-system,sans-serif;padding:48px\">\
             正在等待授权回调，请回到 Guaxi 查看状态。</p>"
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).ok();
        stream.flush().ok();

        // favicon / 其他杂请求：已经回了一个页面，继续等真正的回调。
        if returned_state.is_none() {
            continue;
        }
        if let Some(error) = error {
            if matched {
                return Err(format!(
                    "授权被拒绝：{error}{}",
                    error_description
                        .map(|d| format!("（{d}）"))
                        .unwrap_or_default()
                ));
            }
            continue;
        }
        let Some(code) = code else { continue };
        if !matched {
            continue;
        }
        return exchange_code(provider, client_id, scope, &code, redirect_uri, account_id);
    }
}

fn exchange_code(
    provider: &Provider,
    client_id: &str,
    scope: &str,
    code: &str,
    redirect_uri: &str,
    account_id: &str,
) -> Result<(), String> {
    let response = http()?
        .post(provider.token_url)
        .form(&[
            ("client_id", client_id),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("scope", scope),
        ])
        .send()
        .map_err(|e| format!("换取令牌失败: {e}"))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|e| format!("读取令牌响应失败: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "换取令牌被拒绝（HTTP {status}）：{}",
            summarize_error(&body)
        ));
    }
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("令牌响应格式无效: {e}"))?;
    let access = value
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("令牌响应缺少 access_token: {}", summarize_error(&body)))?;
    let expires_in = value
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);
    let tokens = TokenSet {
        refresh_token: value
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        access_token: access.to_string(),
        expires_at: chrono::Utc::now().timestamp() + expires_in,
    };
    let oauth_email = email_from_id_token(&value);

    // 用户可能在浏览器授权期间删掉了账户：别把令牌写成孤儿条目。
    let Some(mut account) = super::store::get_account(account_id).map_err(|e| e.to_string())?
    else {
        return Err("账户已被删除，授权结果未保存".to_string());
    };
    save_tokens(account_id, &tokens)?;
    account.auth_kind = "oauth2".to_string();
    account.oauth_provider = provider.id.to_string();
    account.oauth_expires = tokens.expires_at;
    // 用户没填邮箱（或填了半截）时，用授权结果里的真实地址补上；
    // 这样「添加账户」只需要点授权，不必先知道自己的地址。
    if (account.email.trim().is_empty() || !account.email.contains('@')) && oauth_email.is_some() {
        account.email = oauth_email.unwrap_or_default();
    }
    super::store::upsert_account(&account)?;
    super::pool::invalidate(account_id);
    info!(account = %account.email, "mail: Outlook 授权完成，令牌已保存");
    Ok(())
}

/// 等待某个 localhost 回调授权会话结束（令牌已由回调线程或手动完成写入）。
pub fn wait_loopback_flow(flow_id: &str) -> Result<(), String> {
    // 先拿到槽位再等：手动完成授权的命令也要能找到同一个槽位。
    let slot = flows()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(flow_id)
        .cloned()
        .ok_or_else(|| "授权会话不存在或已过期，请重新发起授权".to_string())?;
    let (lock, ready) = &*slot;
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    let deadline = Instant::now() + Duration::from_secs(330);
    while guard.is_none() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let (next, timeout) = ready
            .wait_timeout(guard, remaining)
            .unwrap_or_else(|e| e.into_inner());
        guard = next;
        if timeout.timed_out() && guard.is_none() {
            break;
        }
    }
    let result = guard
        .take()
        .unwrap_or_else(|| Err("等待浏览器授权超时".to_string()));
    drop(guard);
    // 超时不删会话：用户可能随后才把回调地址粘贴进来，仍应能完成授权。
    if result.is_ok() {
        flows()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(flow_id);
    }
    result
}

/// 手动完成授权：浏览器没能自动回到本机（被拦截 / 在另一台机器上打开）时，
/// 用户把地址栏里 `http://localhost:<端口>/?code=...&state=...` 整条粘进来，
/// 由后端完成同样的一次「授权码换令牌」。`state` 由前端与 flow_id 比对。
pub fn complete_loopback_flow(
    flow_id: &str,
    account_id: &str,
    provider: &Provider,
    redirect_uri: &str,
    code: &str,
) -> Result<(), String> {
    let code = code.trim();
    if code.is_empty() {
        return Err("回调地址里没有授权码 code".to_string());
    }
    if redirect_uri.trim().is_empty() {
        return Err("回调地址无效".to_string());
    }
    let slot = flows()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(flow_id)
        .cloned()
        .ok_or_else(|| "授权会话不存在或已过期，请重新发起授权".to_string())?;
    // 浏览器已经自动回调成功时，粘贴只是重复动作：直接当作成功，别再拿
    // 用过的 code 去换令牌（那只会得到 invalid_grant）。
    {
        let (lock, _) = &*slot;
        let guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(&*guard, Some(Ok(()))) {
            return Ok(());
        }
    }
    let result = exchange_code(
        provider,
        provider.client_id,
        provider.imap_scope,
        code,
        redirect_uri,
        account_id,
    );
    let (lock, ready) = &*slot;
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    // 成功结果优先；失败不要覆盖掉浏览器已经完成的成功。
    if guard.is_none() || result.is_ok() {
        *guard = Some(result.clone());
    }
    ready.notify_all();
    result
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
/// Windows 凭据管理器单条密码上限是 2560 字节（UTF-16 编码后约 1280 字符），
/// 微软的 refresh_token + access_token JSON 轻松超过这个长度。按字符切块，
/// 每块留在安全线以内，读取时再拼回去。
const TOKEN_CHUNK_CHARS: usize = 1000;
const MAX_TOKEN_CHUNKS: usize = 16;

fn entry(account_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(TOKEN_SERVICE, account_id).map_err(|e| format!("访问系统凭据失败: {e}"))
}

fn chunk_entry(account_id: &str, index: usize) -> Result<keyring::Entry, String> {
    keyring::Entry::new(
        TOKEN_SERVICE,
        &format!("{account_id}#chunk{index}"),
    )
    .map_err(|e| format!("访问系统凭据失败: {e}"))
}

pub fn load_tokens(account_id: &str) -> Option<TokenSet> {
    // 旧格式（整串存在账户名下）：先试一次，方便老数据升级。
    if let Ok(entry) = entry(account_id) {
        if let Ok(raw) = entry.get_password() {
            if let Ok(tokens) = serde_json::from_str(&raw) {
                return Some(tokens);
            }
        }
    }
    let mut raw = String::new();
    for index in 0..MAX_TOKEN_CHUNKS {
        let entry = chunk_entry(account_id, index).ok()?;
        match entry.get_password() {
            Ok(chunk) => raw.push_str(&chunk),
            Err(keyring::Error::NoEntry) => break,
            Err(_) => return None,
        }
    }
    if raw.is_empty() {
        return None;
    }
    serde_json::from_str(&raw).ok()
}

pub fn save_tokens(account_id: &str, tokens: &TokenSet) -> Result<(), String> {
    let json = serde_json::to_string(tokens).map_err(|e| e.to_string())?;
    // 先清掉旧格式与旧分片：新令牌可能比旧的短，不清理会残留尾部导致 JSON 损坏。
    if let Ok(entry) = entry(account_id) {
        let _ = entry.delete_credential();
    }
    for index in 0..MAX_TOKEN_CHUNKS {
        if let Ok(entry) = chunk_entry(account_id, index) {
            let _ = entry.delete_credential();
        }
    }
    let chars: Vec<char> = json.chars().collect();
    for (index, chunk) in chars.chunks(TOKEN_CHUNK_CHARS).enumerate() {
        if index >= MAX_TOKEN_CHUNKS {
            return Err("授权令牌过大，无法写入系统凭据".to_string());
        }
        let text: String = chunk.iter().collect();
        chunk_entry(account_id, index)?
            .set_password(&text)
            .map_err(|e| format!("写入系统凭据失败: {e}"))?;
    }
    Ok(())
}

pub fn delete_tokens(account_id: &str) {
    if let Ok(entry) = entry(account_id) {
        let _ = entry.delete_credential();
    }
    for index in 0..MAX_TOKEN_CHUNKS {
        if let Ok(entry) = chunk_entry(account_id, index) {
            let _ = entry.delete_credential();
        }
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

    #[test]
    fn outlook_authorize_url_has_required_params() {
        let url = build_authorize_url(
            "client-id",
            "https://outlook.office.com/IMAP.AccessAsUser.All",
            "http://localhost:53682/",
            "state-token",
        )
        .expect("authorize url");
        let parsed = url::Url::parse(&url).expect("parse");
        assert_eq!(parsed.host_str(), Some("login.microsoftonline.com"));
        let pairs: std::collections::HashMap<String, String> =
            parsed.query_pairs().into_owned().collect();
        // response_type 缺失正是用户在浏览器里看到的报错；这里锁住它一定要有。
        assert_eq!(pairs.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(
            pairs.get("redirect_uri").map(String::as_str),
            Some("http://localhost:53682/")
        );
        assert_eq!(pairs.get("state").map(String::as_str), Some("state-token"));
        assert_eq!(pairs.get("prompt").map(String::as_str), Some("select_account"));
    }

    #[test]
    fn reads_email_from_id_token() {
        let encode = |claims: serde_json::Value| {
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        };
        let email = encode(serde_json::json!({
            "email": "me@outlook.com",
            "preferred_username": "me@outlook.com"
        }));
        let value = serde_json::json!({ "id_token": format!("header.{email}.sig") });
        assert_eq!(
            email_from_id_token(&value).as_deref(),
            Some("me@outlook.com")
        );

        // 没有 email 声明时退回 preferred_username / upn。
        let upn = encode(serde_json::json!({ "upn": "work@corp.com" }));
        let value = serde_json::json!({ "id_token": format!("header.{upn}.sig") });
        assert_eq!(email_from_id_token(&value).as_deref(), Some("work@corp.com"));

        // 没有 id_token 或声明里没有邮箱：返回 None，前端会要求手动填写。
        assert!(email_from_id_token(&serde_json::json!({})).is_none());
        let empty = encode(serde_json::json!({ "sub": "abc" }));
        let value = serde_json::json!({ "id_token": format!("h.{empty}.s") });
        assert!(email_from_id_token(&value).is_none());
    }
}
