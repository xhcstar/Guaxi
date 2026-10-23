//! 系统通知、诊断导出与危险附件提示。
//!
//! 三件事看似无关，但都属于「用户感知 / 排障」这一层：
//! - 通知：收信没提示等于没收信；
//! - 诊断导出：出问题时用户能给出一份可排查的东西，而不是「用不了」；
//! - 危险附件：`.exe` / `.scr` 双击即执行，至少要拦一下。

use super::model::NewMailNotice;
use tauri::AppHandle;

/// 发一条「新邮件」系统通知。失败只记日志——通知发不出去不该影响收信。
pub fn notify_new_mail(app: &AppHandle, notices: &[NewMailNotice]) {
    use tauri_plugin_notification::NotificationExt;
    if notices.is_empty() {
        return;
    }
    let (title, body) = if notices.len() == 1 {
        let item = &notices[0];
        let from = if item.from_name.trim().is_empty() {
            item.from_addr.clone()
        } else {
            item.from_name.clone()
        };
        let subject = if item.subject.trim().is_empty() {
            "（无主题）".to_string()
        } else {
            item.subject.clone()
        };
        (format!("{from}"), format!("{subject}\n{}", item.account_email))
    } else {
        let first = &notices[0];
        let from = if first.from_name.trim().is_empty() {
            first.from_addr.clone()
        } else {
            first.from_name.clone()
        };
        (
            format!("{} 封新邮件", notices.len()),
            format!(
                "最新来自 {from}：{}",
                if first.subject.trim().is_empty() {
                    "（无主题）"
                } else {
                    &first.subject
                }
            ),
        )
    };
    if let Err(e) = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show()
    {
        tracing::warn!(error = %e, "mail: 系统通知发送失败");
    }
}

/// 诊断导出：把所有「能与不能」收集成一个 JSON 文本。
///
/// 刻意不包含口令 / 令牌 / 邮件正文：用户把这份东西发给别人排查时，
/// 不该顺带把收件箱也交出去。
pub fn diagnostics(app: &AppHandle) -> Result<String, String> {
    let settings = super::store::load_settings();
    let accounts = super::store::list_accounts()?;
    let mut account_rows = Vec::new();
    for account in &accounts {
        let folders = super::store::list_folders(&account.id).unwrap_or_default();
        let has_secret = super::read_secret(&account.id).is_some();
        let oauth_ok = super::oauth::has_tokens(&account.id);
        account_rows.push(serde_json::json!({
            "email": account.email,
            "authKind": account.auth_kind,
            "oauthProvider": account.oauth_provider,
            "imap": format!("{}:{} ({:?})", account.imap_host, account.imap_port, account.imap_security),
            "smtp": format!("{}:{} ({:?})", account.smtp_host, account.smtp_port, account.smtp_security),
            "hasPassword": has_secret,
            "hasOAuthTokens": oauth_ok,
            "delegates": account.delegate.len(),
            "sieve": account.sieve.as_ref().map(|s| serde_json::json!({
                "mode": s.mode,
                "script": s.script,
                "lastError": s.last_error,
            })),
            "lastSyncAt": account.last_sync_at,
            "folders": folders.iter().map(|f| serde_json::json!({
                "name": f.name,
                "role": f.role.as_str(),
                "unread": f.unread,
                "total": f.total,
                "uidValidity": f.uid_validity,
                "localCount": super::store::uids_in_folder(&account.id, &f.name).map(|v| v.len()).unwrap_or(0),
            })).collect::<Vec<_>>(),
        }));
    }

    let outbox = super::store::list_outbox().unwrap_or_default();
    let offline = super::offline::list(None).unwrap_or_default();
    let db_path = super::store::db_path(app)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    let report = serde_json::json!({
        "generatedAt": chrono::Local::now().to_rfc3339(),
        "appVersion": env!("CARGO_PKG_VERSION"),
        "database": {
            "path": db_path,
            "bytes": db_size,
            "schemaVersion": super::store::schema_version(),
        },
        "settings": {
            "fetchMode": settings.fetch_mode,
            "pollSeconds": settings.poll_seconds,
            "flagSyncMinutes": settings.flag_sync_minutes,
            "offlineQueue": settings.offline_queue,
            "retentionDays": settings.retention_days,
            "encryptLocal": super::store::encryption_enabled(),
            "proxyMode": settings.proxy.mode,
            "proxyHost": settings.proxy.url.as_ref().map(|_| "<已配置>"),
            "attachmentLimitMb": settings.attachment_limit_mb,
        },
        "accounts": account_rows,
        "pendingOutbox": outbox.iter().map(|o| serde_json::json!({
            "subject": o.subject,
            "to": o.to,
            "attempts": o.attempts,
            "nextAttemptAt": o.next_attempt_at,
            "lastError": o.last_error,
        })).collect::<Vec<_>>(),
        "offlineQueue": offline.iter().map(|o| serde_json::json!({
            "kind": o.kind,
            "folder": o.folder,
            "uidCount": o.uids.len(),
            "attempts": o.attempts,
            "lastError": o.last_error,
        })).collect::<Vec<_>>(),
        "idle": super::pool::pooled_accounts(),
        "syncBusy": super::sync::busy_accounts(),
    });
    serde_json::to_string_pretty(&report).map_err(|e| format!("生成诊断报告失败: {e}"))
}

/// 危险附件过滤：把 .exe / .scr 这类改成 `.exe.txt` 之后才允许直接落盘？
///
/// 这里**不做自动改名**（用户明确点了「保存附件」，改名会让文件看起来损坏）。
/// 这个函数只负责回答「这个文件名是不是危险类型」，由前端决定要不要弹确认。
pub fn is_dangerous_filename(name: &str) -> bool {
    const DANGEROUS: &[&str] = &[
        "exe", "scr", "com", "bat", "cmd", "pif", "msi", "vbs", "vbe", "js", "jse", "wsf",
        "wsh", "ps1", "psm1", "hta", "cpl", "reg", "lnk", "jar",
    ];
    std::path::Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| DANGEROUS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangerous_extensions_are_flagged() {
        assert!(is_dangerous_filename("invoice.exe"));
        assert!(is_dangerous_filename("setup.SCR"));
        assert!(!is_dangerous_filename("report.pdf"));
        // 双扩展名（`a.pdf.exe`）按最后一段判断——这正是攻击者常用的写法。
        assert!(is_dangerous_filename("report.pdf.exe"));
    }
}
