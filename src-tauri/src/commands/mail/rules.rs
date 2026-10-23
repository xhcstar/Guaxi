//! 收信规则引擎与「撤回发送」。
//!
//! 规则模型对齐 Outlook 的规则向导：条件全满足（AND）后执行一组动作。
//! 差别在于这里**不依赖服务器端 Sieve**——国内邮箱基本不开放 Sieve，
//! 所以规则在本地同步到新邮件后执行：打标签、自动归档、标已读、转发、删除。
//! 其中「移动 / 删除 / 转发」需要服务器操作，由调用方（commands 层）拿动作清单去执行；
//! 规则引擎本身只负责**判定**，保持纯函数、可单测。

use super::model::{Account, MailRule, MessageSummary, RuleAction, RuleCondition};
use regex::Regex;

/// 判定所需的邮件上下文。列表摘要之外还需要正文，因为「正文包含」条件
/// 不能只查摘要（摘要只有一百多字）。
pub struct RuleContext<'a> {
    pub message: &'a MessageSummary,
    pub body_preview: &'a str,
    pub account: &'a Account,
    pub folder_role: &'a str,
}

/// 一封邮件跑完整条规则链：返回命中的规则（按优先级）与动作。命中 `stop_processing` 时截断。
pub fn evaluate(rules: &[MailRule], ctx: &RuleContext<'_>) -> Vec<(MailRule, RuleAction)> {
    let mut hits: Vec<(MailRule, RuleAction)> = Vec::new();
    for rule in rules.iter().filter(|r| r.enabled) {
        if !conditions_match(&rule.condition, ctx) {
            continue;
        }
        hits.push((rule.clone(), rule.action.clone()));
        if rule.stop_processing {
            break;
        }
    }
    hits
}

fn conditions_match(cond: &RuleCondition, ctx: &RuleContext<'_>) -> bool {
    let msg = ctx.message;

    if !cond.account_ids.is_empty() && !cond.account_ids.iter().any(|id| id == &msg.account_id) {
        return false;
    }
    // 文件夹范围：空表示「收件箱」（Outlook 默认）。
    if !cond.folder_roles.is_empty() {
        if !cond.folder_roles.iter().any(|r| r == ctx.folder_role) {
            return false;
        }
    } else if ctx.folder_role != "inbox" {
        // 只有明确指定了文件夹角色的规则才处理非收件箱邮件；
        // 否则「把所有来自 X 的邮件归档」会把已发送的回复也挪走。
        return false;
    }

    let subject_lower = msg.subject.to_lowercase();
    let from_lower = format!("{} {}", msg.from_addr, msg.from_name).to_lowercase();
    let to_lower = format!("{} {}", msg.to_json, msg.cc_json).to_lowercase();

    let any_contains = |haystack: &str, needles: &[String]| {
        needles.iter().any(|n| {
            let n = n.trim().to_lowercase();
            !n.is_empty() && haystack.contains(&n)
        })
    };

    if !cond.from_contains.is_empty() && !any_contains(&from_lower, &cond.from_contains) {
        return false;
    }
    if !cond.to_contains.is_empty() && !any_contains(&to_lower, &cond.to_contains) {
        return false;
    }
    if !cond.subject_contains.is_empty() && !any_contains(&subject_lower, &cond.subject_contains) {
        return false;
    }
    if cond
        .subject_not_contains
        .iter()
        .any(|n| !n.trim().is_empty() && subject_lower.contains(&n.trim().to_lowercase()))
    {
        return false;
    }
    if !cond.body_contains.is_empty()
        && !any_contains(&ctx.body_preview.to_lowercase(), &cond.body_contains)
    {
        return false;
    }
    if let Some(pattern) = cond.any_matches_regex.as_ref().filter(|p| !p.trim().is_empty()) {
        // 用户写的正则可能不合法；不合法时整条规则判为不匹配，而不是让同步线程崩掉。
        match Regex::new(pattern) {
            Ok(re) => {
                let hay = format!("{} {} {}", msg.subject, ctx.body_preview, from_lower);
                if !re.is_match(&hay) {
                    return false;
                }
            }
            Err(_) => return false,
        }
    }
    if let Some(want) = cond.has_attachment {
        if msg.has_attachments != want {
            return false;
        }
    }
    if let Some(min) = cond.min_size {
        if msg.size < min {
            return false;
        }
    }
    let _ = ctx.account;
    true
}

/// 合并多个命中规则的动作：标签取并集，移动取第一条，布尔动作取或。
pub fn merge_actions(actions: &[RuleAction]) -> RuleAction {
    let mut merged = RuleAction::default();
    for action in actions {
        if merged.move_to.is_none() {
            merged.move_to = action.move_to.clone();
        }
        if merged.copy_to.is_none() {
            merged.copy_to = action.copy_to.clone();
        }
        for label in &action.add_labels {
            if !merged.add_labels.contains(label) {
                merged.add_labels.push(label.clone());
            }
        }
        for label in &action.remove_labels {
            if !merged.remove_labels.contains(label) {
                merged.remove_labels.push(label.clone());
            }
        }
        for addr in &action.forward_to {
            if !merged.forward_to.contains(addr) {
                merged.forward_to.push(addr.clone());
            }
        }
        merged.mark_read |= action.mark_read;
        merged.star |= action.star;
        merged.delete |= action.delete;
        merged.skip_inbox |= action.skip_inbox;
    }
    merged
}

/// 收发件人名单的判定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenderVerdict {
    Blocked,
    Allowed,
}

/// 按名单判定发件人。
///
/// 匹配规则（由宽到窄）：
/// - `*` 通配符（`@*.spam.example`、`noreply-*@x.com`）；
/// - 以 `@` 开头：匹配整个域名（`@spam.example`）；
/// - 含 `@`：精确匹配地址；
/// - 否则：匹配地址或显示名里出现该子串。
///
/// **黑名单优先**：同时出现在两个名单里时按拉黑处理——用户把某人加进黑名单
/// 的意图更强烈，一条误放行的代价远大于误删（后者还能从垃圾箱找回）。
pub fn sender_list_verdict(
    rules: &[super::model::SenderRule],
    from_addr: &str,
    from_name: &str,
) -> Option<SenderVerdict> {
    if rules.is_empty() {
        return None;
    }
    let addr = from_addr.trim().to_ascii_lowercase();
    let name = from_name.trim().to_ascii_lowercase();
    let matches = |pattern: &str| -> bool {
        let pattern = pattern.trim().to_ascii_lowercase();
        if pattern.is_empty() {
            return false;
        }
        if pattern.contains('*') {
            return wildcard_match(&pattern, &addr) || wildcard_match(&pattern, &name);
        }
        if let Some(domain) = pattern.strip_prefix('@') {
            return addr.ends_with(&format!("@{domain}"));
        }
        if pattern.contains('@') {
            return addr == pattern;
        }
        addr.contains(&pattern) || name.contains(&pattern)
    };

    let mut allowed = false;
    for rule in rules {
        if !matches(&rule.pattern) {
            continue;
        }
        match rule.kind.as_str() {
            "block" => return Some(SenderVerdict::Blocked),
            "allow" => allowed = true,
            _ => {}
        }
    }
    if allowed {
        Some(SenderVerdict::Allowed)
    } else {
        None
    }
}

/// 极简通配符匹配（只支持 `*`）。地址串很短，回溯风险可以忽略。
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut mark = 0usize;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// 标签串的增删（逗号分隔，去重保序）。
pub fn apply_label_delta(current: &str, add: &[String], remove: &[String]) -> String {
    let mut labels: Vec<String> = current
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    for label in remove {
        let label = label.trim();
        labels.retain(|l| l != label);
    }
    for label in add {
        let label = label.trim();
        if !label.is_empty() && !labels.iter().any(|l| l == label) {
            labels.push(label.to_string());
        }
    }
    labels.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn account() -> Account {
        Account {
            id: "acc".into(),
            email: "me@example.com".into(),
            display_name: String::new(),
            imap_host: String::new(),
            imap_port: 993,
            imap_security: super::super::model::Security::Ssl,
            smtp_host: String::new(),
            smtp_port: 465,
            smtp_security: super::super::model::Security::Ssl,
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

    fn message(subject: &str, from: &str, has_att: bool) -> MessageSummary {
        MessageSummary {
            account_id: "acc".into(),
            folder: "INBOX".into(),
            uid: 1,
            message_id: None,
            subject: subject.into(),
            from_name: String::new(),
            from_addr: from.into(),
            to_json: "[]".into(),
            cc_json: "[]".into(),
            date: 0,
            size: 1000,
            snippet: String::new(),
            seen: false,
            flagged: false,
            answered: false,
            draft: false,
            deleted: false,
            has_attachments: has_att,
            thread_key: String::new(),
            labels: String::new(),
            outgoing: false,
        }
    }

    fn rule(cond: RuleCondition, action: RuleAction) -> MailRule {
        MailRule {
            id: "r1".into(),
            name: "test".into(),
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
    fn from_and_subject_conditions_are_anded() {
        let acc = account();
        let msg = message("季度报告", "boss@corp.com", false);
        let ctx = RuleContext {
            message: &msg,
            body_preview: "",
            account: &acc,
            folder_role: "inbox",
        };
        let cond = RuleCondition {
            from_contains: vec!["corp.com".into()],
            subject_contains: vec!["报告".into()],
            ..Default::default()
        };
        assert_eq!(evaluate(&[rule(cond.clone(), RuleAction::default())], &ctx).len(), 1);
        let miss = rule(
            RuleCondition {
                subject_contains: vec!["发票".into()],
                ..cond
            },
            RuleAction::default(),
        );
        assert!(evaluate(&[miss], &ctx).is_empty());
    }

    #[test]
    fn rules_without_folder_scope_only_touch_inbox() {
        let acc = account();
        let msg = message("hi", "a@b.c", false);
        let r = rule(
            RuleCondition {
                from_contains: vec!["a@b.c".into()],
                ..Default::default()
            },
            RuleAction::default(),
        );
        let sent_ctx = RuleContext {
            message: &msg,
            body_preview: "",
            account: &acc,
            folder_role: "sent",
        };
        assert!(evaluate(&[r.clone()], &sent_ctx).is_empty());
        let inbox_ctx = RuleContext {
            message: &msg,
            body_preview: "",
            account: &acc,
            folder_role: "inbox",
        };
        assert_eq!(evaluate(&[r], &inbox_ctx).len(), 1);
    }

    #[test]
    fn disabled_rule_is_skipped() {
        let acc = account();
        let msg = message("hi", "a@b.c", false);
        let mut r = rule(RuleCondition::default(), RuleAction::default());
        r.enabled = false;
        let ctx = RuleContext {
            message: &msg,
            body_preview: "",
            account: &acc,
            folder_role: "inbox",
        };
        assert!(evaluate(&[r], &ctx).is_empty());
    }

    #[test]
    fn stop_processing_halts_chain() {
        let acc = account();
        let msg = message("hi", "a@b.c", false);
        let mut first = rule(RuleCondition::default(), RuleAction::default());
        first.stop_processing = true;
        let second = rule(RuleCondition::default(), RuleAction::default());
        let ctx = RuleContext {
            message: &msg,
            body_preview: "",
            account: &acc,
            folder_role: "inbox",
        };
        assert_eq!(evaluate(&[first, second], &ctx).len(), 1);
    }

    #[test]
    fn invalid_regex_does_not_match_instead_of_panicking() {
        let acc = account();
        let msg = message("hi", "a@b.c", false);
        let r = rule(
            RuleCondition {
                any_matches_regex: Some("([unclosed".into()),
                ..Default::default()
            },
            RuleAction::default(),
        );
        let ctx = RuleContext {
            message: &msg,
            body_preview: "",
            account: &acc,
            folder_role: "inbox",
        };
        assert!(evaluate(&[r], &ctx).is_empty());
    }

    #[test]
    fn merge_actions_unions_labels_and_or_flags() {
        let a = RuleAction {
            add_labels: vec!["财务".into()],
            mark_read: true,
            move_to: Some("Finance".into()),
            ..Default::default()
        };
        let b = RuleAction {
            add_labels: vec!["财务".into(), "重要".into()],
            star: true,
            move_to: Some("Other".into()),
            ..Default::default()
        };
        let merged = merge_actions(&[a, b]);
        assert_eq!(
            merged.add_labels,
            vec!["财务".to_string(), "重要".to_string()]
        );
        assert_eq!(merged.move_to.as_deref(), Some("Finance"));
        assert!(merged.mark_read && merged.star);
    }

    #[test]
    fn label_delta_adds_and_removes() {
        assert_eq!(
            apply_label_delta("a,b", &["c".into(), "a".into()], &["b".into()]),
            "a,c"
        );
        assert_eq!(apply_label_delta("", &["x".into()], &[]), "x");
    }

}
