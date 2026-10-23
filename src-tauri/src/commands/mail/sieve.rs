//! Sieve 服务器端规则（RFC 5228 + 管理协议 ManageSieve，RFC 5804）。
//!
//! 为什么需要：本地规则只在「这台机器开着客户端并同步」时生效。手机收到信、
//! 关掉电脑，规则就停了——这正是 Outlook 的服务器端规则要解决的问题。
//!
//! 但现实约束很硬：**国内邮箱（QQ/163/阿里/腾讯企业邮）基本不开放 ManageSieve**，
//! Exchange 用的是它自己的传输规则（EWS/Graph），不是 Sieve。
//! 所以这里的定位是「在有条件的服务器上把本地规则镜像上去」：
//! - 服务器不声明支持时，功能整个灰显，错误信息要明确说「这台服务器不支持」；
//! - 只生成 RFC 5228 的**基础子集**（`require` 只写真正用到的 capability），
//!   不使用任何厂商扩展，避免写出一个只有我们能读的脚本；
//! - 上传是 UPSERT：先 `PUTSCRIPT`，再 `SETACTIVE`；失败时保留上一次的脚本不动。

use super::model::{Account, MailRule, SieveConfig};
use std::io::{BufRead, BufReader, Write};

/// 生成 Sieve 脚本。
///
/// 只映射「服务器能独立判定」的动作：文件名 / 收件人 / 主题 / 大小 / 是否带附件。
/// 规则里依赖正文内容（`body_contains`、正则）的条件无法在服务器上表达——
/// 服务器在收信时不做全文匹配（`body` 扩展需要 `body` capability 且性能极差），
/// 这些规则会被跳过并在返回值里报告，而不是静默生成一条行不通的规则。
pub fn generate(account: &Account, rules: &[MailRule], trash_folder: Option<&str>) -> (String, Vec<String>) {
    let mut lines: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut requires: Vec<&str> = vec!["fileinto"];

    let applicable: Vec<&MailRule> = rules
        .iter()
        .filter(|r| r.enabled)
        .filter(|r| {
            // 只处理明确作用于本账户（或未限定账户）的规则。
            r.action.move_to.is_some() || r.action.delete || r.action.mark_read || r.action.star
        })
        .collect();

    for rule in &applicable {
        let mut conditions: Vec<String> = Vec::new();
        let cond = &rule.condition;
        if !cond.folder_roles.is_empty()
            && !cond.folder_roles.iter().any(|r| r == "inbox")
            && !cond.folder_roles.is_empty()
        {
            // 服务器端只能作用在投递时（收件箱），非收件箱规则无法表达。
            warnings.push(format!("「{}」：服务器端规则只能作用于新到邮件（收件箱），已跳过", rule.name));
            continue;
        }
        if !cond.account_ids.is_empty() && !cond.account_ids.iter().any(|id| id == &account.id) {
            continue;
        }
        if !cond.body_contains.is_empty()
            || cond
                .any_matches_regex
                .as_ref()
                .is_some_and(|p| !p.trim().is_empty())
        {
            warnings.push(format!("「{}」：依赖正文内容，无法在服务器端表达，已跳过", rule.name));
            continue;
        }

        for value in &cond.from_contains {
            if !value.trim().is_empty() {
                conditions.push(format!("address :contains \"From\" \"{}\"", escape(value)));
            }
        }
        for value in &cond.to_contains {
            if !value.trim().is_empty() {
                conditions.push(format!("address :contains [\"To\",\"Cc\"] \"{}\"", escape(value)));
            }
        }
        for value in &cond.subject_contains {
            if !value.trim().is_empty() {
                conditions.push(format!("header :contains \"Subject\" \"{}\"", escape(value)));
            }
        }
        for value in &cond.subject_not_contains {
            if !value.trim().is_empty() {
                conditions.push(format!("not header :contains \"Subject\" \"{}\"", escape(value)));
            }
        }
        if let Some(true) = cond.has_attachment {
            requires.push("foreverypart");
            // 简化：用 Content-Type 是 multipart/mixed 近似「有附件」。
            // 精确判定需要解析 multipart 结构，Sieve 的基础语法做不到。
            conditions.push("header :contains \"Content-Type\" \"multipart/mixed\"".to_string());
        }
        if let Some(min) = cond.min_size {
            requires.push("size");
            let kb = (min.max(1024) as f64 / 1024.0).ceil() as u64;
            conditions.push(format!("size :over {kb}K"));
        }

        let mut actions: Vec<String> = Vec::new();
        let move_target = rule
            .action
            .move_to
            .clone()
            .or_else(|| rule.action.delete.then(|| trash_folder.map(|s| s.to_string())).flatten());
        if let Some(target) = move_target {
            // 角色名 → 服务器真实文件夹名由调用方给好（`move_to` 存的就是服务器名）。
            actions.push(format!("fileinto \"{}\";", escape(&target)));
        }
        if rule.action.mark_read {
            requires.push("imap4flags");
            actions.push("addflag \"\\\\Seen\";".to_string());
        }
        if rule.action.star {
            requires.push("imap4flags");
            actions.push("addflag \"\\\\Flagged\";".to_string());
        }
        if actions.is_empty() {
            continue;
        }

        let condition = if conditions.is_empty() {
            "anyof (true, false)".to_string()
        } else {
            format!("allof ({})", conditions.join(", "))
        };
        lines.push(format!(
            "# {} (优先级 {})\nif {condition} {{\n    {}\n}}",
            sanitize_comment(&rule.name),
            rule.priority,
            actions.join("\n    ")
        ));
    }

    requires.sort_unstable();
    requires.dedup();
    let mut script = String::new();
    script.push_str(&format!("require [{}];\n", requires
        .iter()
        .map(|r| format!("\"{r}\""))
        .collect::<Vec<_>>()
        .join(", ")));
    script.push_str(&format!("# 由 Guaxi 邮箱生成 —— 服务器端规则镜像\n# 账户: {}\n\n", account.email));
    if lines.is_empty() {
        script.push_str("# 当前没有可映射到服务器端的规则。\n");
    } else {
        script.push_str(&lines.join("\n\n"));
        script.push('\n');
    }
    (script, warnings)
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn sanitize_comment(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

/// 一次 ManageSieve 会话。协议是文本行 + 字面量（`{n}` / `{n+}`）。
pub struct SieveSession {
    stream: BufReader<super::net::Stream>,
}

impl SieveSession {
    pub fn connect(account: &Account, config: &SieveConfig) -> Result<Self, String> {
        let proxy = super::effective_proxy(account);
        let host = if config.host.trim().is_empty() {
            account.imap_host.clone()
        } else {
            config.host.trim().to_string()
        };
        let port = if config.port == 0 { 4190 } else { config.port };
        let mut stream = super::net::open_stream(&host, port, config.security, proxy)?;
        if config.security == super::model::Security::Starttls {
            let mut session = SieveSession {
                stream: BufReader::new(stream),
            };
            session.expect_greeting()?;
            let reply = session.command("STARTTLS")?;
            if !reply.starts_with("OK") {
                return Err(format!("Sieve 服务器拒绝 STARTTLS: {reply}"));
            }
            let plain = session.stream.into_inner();
            stream = plain.upgrade_tls(&host)?;
            let mut upgraded = SieveSession {
                stream: BufReader::new(stream),
            };
            upgraded.expect_greeting()?;
            return Ok(upgraded);
        }
        let mut session = SieveSession {
            stream: BufReader::new(stream),
        };
        session.expect_greeting()?;
        Ok(session)
    }

    fn expect_greeting(&mut self) -> Result<(), String> {
        let line = self.read_line()?;
        // 成功时是 `OK ...`，失败是 `NO ...`。
        if line.starts_with("OK") {
            Ok(())
        } else {
            Err(format!("Sieve 服务器拒绝连接: {line}"))
        }
    }

    fn read_line(&mut self) -> Result<String, String> {
        let mut line = String::new();
        let n = self
            .stream
            .read_line(&mut line)
            .map_err(|e| format!("读取 Sieve 响应失败: {e}"))?;
        if n == 0 {
            return Err("Sieve 服务器关闭了连接".to_string());
        }
        Ok(line.trim_end_matches(['\r', '\n']).to_string())
    }

    fn command(&mut self, line: &str) -> Result<String, String> {
        let out = self.stream.get_mut();
        out.write_all(line.as_bytes())
            .and_then(|_| out.write_all(b"\r\n"))
            .and_then(|_| out.flush())
            .map_err(|e| format!("发送 Sieve 命令失败: {e}"))?;
        // 读取完整响应：处理多行（每行以 `OK` / `NO` / `BYE` 结尾标志结束）。
        loop {
            let last = self.read_line()?;
            if last.starts_with("OK")
                || last.starts_with("NO")
                || last.starts_with("BYE")
                || last.starts_with("BYE")
            {
                return Ok(last);
            }
        }
    }

    /// 使用 SASL PLAIN 认证。
    pub fn authenticate(&mut self, username: &str, secret: &str) -> Result<(), String> {
        use base64::Engine;
        let token =
            base64::engine::general_purpose::STANDARD.encode(format!("\0{username}\0{secret}"));
        // 字面量形式：`AUTHENTICATE "PLAIN" {n+}\r\n<token>\r\n`
        let request = format!("AUTHENTICATE \"PLAIN\" {{{}+}}", token.len());
        let out = self.stream.get_mut();
        out.write_all(request.as_bytes())
            .and_then(|_| out.write_all(b"\r\n"))
            .and_then(|_| out.write_all(token.as_bytes()))
            .and_then(|_| out.write_all(b"\r\n"))
            .and_then(|_| out.flush())
            .map_err(|e| format!("Sieve 认证写入失败: {e}"))?;
        loop {
            let last = self.read_line()?;
            if last.starts_with("OK") {
                return Ok(());
            }
            if last.starts_with("NO") || last.starts_with("BYE") {
                return Err(format!("Sieve 认证失败: {last}"));
            }
        }
    }

    /// 上传脚本（`PUTSCRIPT`）。脚本内容用字面量传输。
    pub fn put_script(&mut self, name: &str, script: &str) -> Result<(), String> {
        let bytes = script.as_bytes();
        let header = format!("PUTSCRIPT \"{}\" {{{}+}}", escape(name), bytes.len());
        let out = self.stream.get_mut();
        out.write_all(header.as_bytes())
            .and_then(|_| out.write_all(b"\r\n"))
            .and_then(|_| out.write_all(bytes))
            .and_then(|_| out.write_all(b"\r\n"))
            .and_then(|_| out.flush())
            .map_err(|e| format!("上传 Sieve 脚本失败: {e}"))?;
        let reply = self.read_line_status()?;
        if !reply.starts_with("OK") {
            return Err(format!("服务器拒绝保存脚本: {reply}"));
        }
        Ok(())
    }

    /// 设为活动脚本（`SETACTIVE`）。
    pub fn set_active(&mut self, name: &str) -> Result<(), String> {
        let reply = self.command(&format!("SETACTIVE \"{}\"", escape(name)))?;
        if !reply.starts_with("OK") {
            return Err(format!("服务器拒绝启用脚本: {reply}"));
        }
        Ok(())
    }


    fn read_line_status(&mut self) -> Result<String, String> {
        loop {
            let last = self.read_line()?;
            if last.starts_with("OK") || last.starts_with("NO") || last.starts_with("BYE") {
                return Ok(last);
            }
        }
    }

    pub fn logout(mut self) {
        let _ = self.command("LOGOUT");
    }
}

/// 把本地规则镜像到服务器。
pub fn sync_rules(
    account: &Account,
    config: &SieveConfig,
    rules: &[MailRule],
    trash_folder: Option<&str>,
    secret: &str,
) -> Result<Vec<String>, String> {
    let (script, warnings) = generate(account, rules, trash_folder);
    let mut session = SieveSession::connect(account, config)?;
    session.authenticate(&account.login_name(), secret)?;
    let name = if config.script.trim().is_empty() {
        "guaxi"
    } else {
        config.script.trim()
    };
    session.put_script(name, &script)?;
    session.set_active(name)?;
    session.logout();
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::mail::model::*;
    use std::collections::BTreeMap;

    fn account() -> Account {
        Account {
            id: "acc".into(),
            email: "me@example.com".into(),
            display_name: String::new(),
            imap_host: String::new(),
            imap_port: 993,
            imap_security: Security::Ssl,
            smtp_host: String::new(),
            smtp_port: 465,
            smtp_security: Security::Ssl,
            username: String::new(),
            auth_kind: "password".into(),
            oauth_provider: String::new(),
            oauth_expires: 0,
            proxy: None,
            accent: String::new(),
            signature: String::new(),
            folder_map: BTreeMap::new(),
            delegate: Vec::new(),
            sieve: None,
            created_at: 0,
            last_sync_at: 0,
        }
    }

    fn rule(name: &str, cond: RuleCondition, action: RuleAction) -> MailRule {
        MailRule {
            id: name.into(),
            name: name.into(),
            enabled: true,
            priority: 0,
            match_mode: "all".into(),
            condition: cond,
            action,
            stop_processing: false,
            created_at: 0,
        }
    }

    #[test]
    fn generates_fileinto_rule() {
        let r = rule(
            "发票归档",
            RuleCondition {
                subject_contains: vec!["发票".into()],
                ..Default::default()
            },
            RuleAction {
                move_to: Some("Finance".into()),
                ..Default::default()
            },
        );
        let (script, warnings) = generate(&account(), &[r], None);
        assert!(script.contains("require [\"fileinto\"]"));
        assert!(script.contains("header :contains \"Subject\" \"发票\""));
        assert!(script.contains("fileinto \"Finance\";"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn body_rules_are_skipped_with_warning() {
        let r = rule(
            "正文规则",
            RuleCondition {
                body_contains: vec!["secret".into()],
                ..Default::default()
            },
            RuleAction {
                move_to: Some("X".into()),
                ..Default::default()
            },
        );
        let (script, warnings) = generate(&account(), &[r], None);
        assert_eq!(warnings.len(), 1);
        assert!(!script.contains("secret"));
    }

    #[test]
    fn delete_maps_to_trash_folder() {
        let r = rule(
            "垃圾",
            RuleCondition {
                from_contains: vec!["spam.example".into()],
                ..Default::default()
            },
            RuleAction {
                delete: true,
                ..Default::default()
            },
        );
        let (script, _) = generate(&account(), &[r], Some("Trash"));
        assert!(script.contains("fileinto \"Trash\";"));
    }

    #[test]
    fn escaping_quotes_in_values() {
        let r = rule(
            "q",
            RuleCondition {
                subject_contains: vec!["a\"b".into()],
                ..Default::default()
            },
            RuleAction {
                mark_read: true,
                ..Default::default()
            },
        );
        let (script, _) = generate(&account(), &[r], None);
        assert!(script.contains("a\\\"b"));
        assert!(script.contains("imap4flags"));
    }
}
