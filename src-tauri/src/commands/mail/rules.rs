//! 收信规则引擎与「撤回发送」。
//!
//! 规则模型对齐 Outlook 的规则向导：条件按「与 / 或」组合（`match_mode` = `all` / `any`），
//! 每一项还能选「不包含」（非）；命中后执行一组动作。
//! 差别在于这里**不依赖服务器端 Sieve**——国内邮箱基本不开放 Sieve，
//! 所以规则在本地同步到新邮件后执行：打标签、自动归档、标已读、转发、删除。
//! 其中「移动 / 删除 / 转发」需要服务器操作，由调用方（commands 层）拿动作清单去执行；
//! 规则引擎本身只负责**判定**，保持纯函数、可单测。

use super::model::{
    Account, MailRule, MessageSummary, RuleAction, RuleClause, RuleCondition, RuleGroup,
};
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
/// 这批规则里有没有一条要用到正文。
///
/// 同步时对每封新邮件都去读一次正文是纯浪费：绝大多数规则只看发件人 / 主题，
/// 而正文是密文，读一次就要解密一次——1000 封新邮件就是 1000 次 AES 解密。
/// 调用方先问一句，不需要就完全不碰正文。
pub fn needs_body(rules: &[MailRule]) -> bool {
    rules.iter().any(|rule| {
        if !rule.enabled {
            return false;
        }
        let cond = &rule.condition;
        if !cond.groups.is_empty() {
            return groups_need_body(&cond.groups);
        }
        !cond.body_contains.is_empty()
            || !cond.body_not_contains.is_empty()
            || cond.any_matches_regex.is_some()
    })
}

pub fn evaluate(rules: &[MailRule], ctx: &RuleContext<'_>) -> Vec<(MailRule, RuleAction)> {
    let mut hits: Vec<(MailRule, RuleAction)> = Vec::new();
    for rule in rules.iter().filter(|r| r.enabled) {
        if !conditions_match(&rule.condition, &rule.match_mode, ctx) {
            continue;
        }
        hits.push((rule.clone(), rule.action.clone()));
        if rule.stop_processing {
            break;
        }
    }
    hits
}

/// 只按「不需要正文」的条件先过一遍：都不可能是命中的邮件，就不必解密正文。
///
/// 「对已有邮件执行规则」要扫几千封：先按发件人 / 收件人 / 主题筛掉绝大多数，
/// 真正要解的正文从几 GB 降到几十 MB（真实反馈：按钮一直卡在「正在对已有邮件执行…」）。
pub fn body_relevant(rules: &[MailRule], ctx: &RuleContext<'_>) -> bool {
    rules
        .iter()
        .filter(|rule| rule.enabled)
        .any(|rule| {
            let cond = &rule.condition;
            if !scope_matches(cond, ctx) {
                return false;
            }
            // 含正文的条件组不能用「正文占位」预筛：`非(正文含…)` 或组内「或」
            // 都会让占位结果与真实结果相反 / 不同，直接按「需要正文」处理。
            if !cond.groups.is_empty() && groups_need_body(&cond.groups) {
                return true;
            }
            conditions_match_inner(cond, &rule.match_mode, ctx, false)
        })
}

fn conditions_match(cond: &RuleCondition, match_mode: &str, ctx: &RuleContext<'_>) -> bool {
    conditions_match_inner(cond, match_mode, ctx, true)
}

/// `all` / `any`（也认 `and` / `or`）；认不出来的按「全部满足」处理，保持老规则行为。
fn is_any_mode(match_mode: &str) -> bool {
    matches!(
        match_mode.trim().to_ascii_lowercase().as_str(),
        "any" | "or"
    )
}

/// 一项「包含 / 不包含」条件的结果；这一项没填就返回 None（不参与与或）。
fn list_term(needles: &[String], haystack_lower: &str, negate: bool) -> Option<bool> {
    let needles: Vec<String> = needles
        .iter()
        .map(|item| item.trim().to_lowercase())
        .filter(|item| !item.is_empty())
        .collect();
    if needles.is_empty() {
        return None;
    }
    let hit = needles.iter().any(|item| haystack_lower.contains(item));
    Some(if negate { !hit } else { hit })
}

// ---------------------------------------------------------------------------
// 嵌套条件组（Outlook 风格：组内与/或、组间与/或、整组取反）
// ---------------------------------------------------------------------------

fn groups_need_body(groups: &[RuleGroup]) -> bool {
    groups.iter().any(|group| {
        group
            .clauses
            .iter()
            .any(|clause| matches!(clause.field.trim().to_ascii_lowercase().as_str(), "body" | "any"))
            || groups_need_body(&group.groups)
    })
}

fn group_is_empty(group: &RuleGroup) -> bool {
    group
        .clauses
        .iter()
        .all(|clause| clause.value.trim().is_empty())
        && group.groups.iter().all(group_is_empty)
}

/// 组内各项按 `mode` 组合，最后按 `not` 取反。
///
/// 空组（没有填任何值）视为「没有约束」，直接跳过——用户在界面里新建了一个
/// 条件组还没填内容时，不该因为它是「任一满足」就把整条规则判为不命中。
fn group_match(
    group: &RuleGroup,
    ctx: &RuleContext<'_>,
    with_body: bool,
) -> bool {
    let mut results: Vec<bool> = Vec::new();
    for clause in &group.clauses {
        if clause.value.trim().is_empty() {
            continue;
        }
        results.push(clause_match(clause, ctx, with_body));
    }
    for child in &group.groups {
        if group_is_empty(child) {
            continue;
        }
        results.push(group_match(child, ctx, with_body));
    }
    let base = if results.is_empty() {
        true
    } else if is_any_mode(&group.mode) {
        results.iter().any(|hit| *hit)
    } else {
        results.iter().all(|hit| *hit)
    };
    if group.not {
        !base
    } else {
        base
    }
}

fn clause_match(clause: &RuleClause, ctx: &RuleContext<'_>, with_body: bool) -> bool {
    let RuleClause { field, op, value } = clause;
    let value = value.trim();
    if value.is_empty() {
        return true;
    }
    let field = field.trim().to_ascii_lowercase();
    let op = op.trim().to_ascii_lowercase().replace(['_', '-'], "");

    if field == "size" {
        let Ok(kb) = value.parse::<f64>() else {
            return false;
        };
        let size_kb = ctx.message.size as f64 / 1024.0;
        return match op.as_str() {
            "greaterthan" | "gt" => size_kb > kb,
            "lessthan" | "lt" => size_kb < kb,
            "equals" => (size_kb - kb).abs() < 0.5,
            "notequals" => (size_kb - kb).abs() >= 0.5,
            _ => false,
        };
    }

    let body_dependent = matches!(field.as_str(), "body" | "any");
    if body_dependent && !with_body {
        // 预筛阶段：正文条件按「可能命中」处理，宁可多解一次正文也不能漏。
        return true;
    }

    let msg = ctx.message;
    let from_raw = format!("{} {}", msg.from_addr, msg.from_name);
    let hay_raw = match field.as_str() {
        "from" => from_raw.clone(),
        "to" => format!("{} {}", msg.to_json, msg.cc_json),
        "subject" => msg.subject.clone(),
        "body" => ctx.body_preview.to_string(),
        // `any` 与旧的「高级正则」一致：主题 + 正文 + 发件人。
        _ => format!("{} {} {}", msg.subject, ctx.body_preview, from_raw),
    };
    let hay = hay_raw.to_lowercase();
    let needle = value.to_lowercase();
    let values = clause_values(&field, ctx);

    match op.as_str() {
        "contains" => hay.contains(&needle),
        "notcontains" => !hay.contains(&needle),
        "equals" => values.iter().any(|item| item.eq_ignore_ascii_case(value))
            || hay.trim().eq_ignore_ascii_case(value),
        "notequals" => {
            !(values.iter().any(|item| item.eq_ignore_ascii_case(value))
                || hay.trim().eq_ignore_ascii_case(value))
        }
        "startswith" => values
            .iter()
            .any(|item| item.to_lowercase().starts_with(&needle))
            || hay.starts_with(&needle),
        "endswith" => values
            .iter()
            .any(|item| item.to_lowercase().ends_with(&needle))
            || hay.ends_with(&needle),
        // 正则按用户写的大小写原样匹配；把正文预先小写会让 `[A-Z]` 这类表达式失效。
        "regex" => match Regex::new(value) {
            Ok(re) => re.is_match(&hay_raw),
            // 用户写的正则可能不合法；不合法时这一项判为不命中，而不是让同步线程崩掉。
            Err(_) => false,
        },
        _ => false,
    }
}

/// 叶子条件比较「等于 / 开头 / 结尾」时用的值：地址、显示名、主题、正文。
///
/// 直接拿 `to_json` 这种序列化文本做 `equals` 永远匹配不上，所以收件人要从
/// JSON 里拆出姓名与地址；发件人同时比较地址与显示名。
fn clause_values(field: &str, ctx: &RuleContext<'_>) -> Vec<String> {
    let msg = ctx.message;
    match field {
        "from" => vec![msg.from_addr.clone(), msg.from_name.clone()],
        "to" => {
            let mut out = Vec::new();
            for raw in [&msg.to_json, &msg.cc_json] {
                if let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(raw) {
                    for item in items {
                        for key in ["address", "name"] {
                            if let Some(value) = item.get(key).and_then(|v| v.as_str()) {
                                if !value.trim().is_empty() {
                                    out.push(value.to_string());
                                }
                            }
                        }
                    }
                }
            }
            out
        }
        "subject" => vec![msg.subject.clone()],
        "body" => vec![ctx.body_preview.to_string()],
        _ => vec![
            msg.subject.clone(),
            ctx.body_preview.to_string(),
            msg.from_addr.clone(),
            msg.from_name.clone(),
        ],
    }
}

/// 账户 / 文件夹 / 附件 / 大小这类「范围」判定。
///
/// 作用域不是筛选条件：它们决定这条规则管哪一批邮件，永远按「与」处理，
/// 不参与条件组的与或非。
fn scope_matches(cond: &RuleCondition, ctx: &RuleContext<'_>) -> bool {
    let msg = ctx.message;
    if !cond.account_ids.is_empty() && !cond.account_ids.iter().any(|id| id == &msg.account_id) {
        return false;
    }
    // 文件夹范围：空表示「收件箱」（Outlook 默认）；`*` 表示所有文件夹。
    if cond.folder_roles.is_empty() {
        if ctx.folder_role != "inbox" {
            return false;
        }
    } else if !cond
        .folder_roles
        .iter()
        .any(|role| role == "*" || role == ctx.folder_role)
    {
        return false;
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
    true
}

fn conditions_match_inner(
    cond: &RuleCondition,
    match_mode: &str,
    ctx: &RuleContext<'_>,
    with_body: bool,
) -> bool {
    if !scope_matches(cond, ctx) {
        return false;
    }
    let msg = ctx.message;

    // 新版规则：条件组是可嵌套的树，组内 / 组间都能选与或非。
    if !cond.groups.is_empty() {
        let effective: Vec<&RuleGroup> = cond
            .groups
            .iter()
            .filter(|group| !group_is_empty(group))
            .collect();
        if effective.is_empty() {
            return true;
        }
        let hits = effective
            .iter()
            .map(|group| group_match(group, ctx, with_body));
        return if is_any_mode(match_mode) {
            hits.into_iter().any(|hit| hit)
        } else {
            hits.into_iter().all(|hit| hit)
        };
    }

    let subject_lower = msg.subject.to_lowercase();
    let from_lower = format!("{} {}", msg.from_addr, msg.from_name).to_lowercase();
    let to_lower = format!("{} {}", msg.to_json, msg.cc_json).to_lowercase();

    // 每一项是一个「词」：发件人 / 收件人 / 主题 / 正文，各自可正向或反向。
    let mut terms: Vec<bool> = Vec::new();
    if let Some(hit) = list_term(&cond.from_contains, &from_lower, false) {
        terms.push(hit);
    }
    if let Some(hit) = list_term(&cond.from_not_contains, &from_lower, true) {
        terms.push(hit);
    }
    if let Some(hit) = list_term(&cond.to_contains, &to_lower, false) {
        terms.push(hit);
    }
    if let Some(hit) = list_term(&cond.to_not_contains, &to_lower, true) {
        terms.push(hit);
    }
    if let Some(hit) = list_term(&cond.subject_contains, &subject_lower, false) {
        terms.push(hit);
    }
    if let Some(hit) = list_term(&cond.subject_not_contains, &subject_lower, true) {
        terms.push(hit);
    }

    let body_dependent = !cond.body_contains.is_empty()
        || !cond.body_not_contains.is_empty()
        || cond
            .any_matches_regex
            .as_ref()
            .is_some_and(|pattern| !pattern.trim().is_empty());

    if with_body {
        let body_lower = ctx.body_preview.to_lowercase();
        if let Some(hit) = list_term(&cond.body_contains, &body_lower, false) {
            terms.push(hit);
        }
        if let Some(hit) = list_term(&cond.body_not_contains, &body_lower, true) {
            terms.push(hit);
        }
        if let Some(pattern) = cond
            .any_matches_regex
            .as_ref()
            .filter(|pattern| !pattern.trim().is_empty())
        {
            // 用户写的正则可能不合法；不合法时这一项判为不命中，而不是让同步线程崩掉。
            match Regex::new(pattern) {
                Ok(re) => {
                    let hay = format!("{} {} {}", msg.subject, ctx.body_preview, from_lower);
                    terms.push(re.is_match(&hay));
                }
                Err(_) => terms.push(false),
            }
        }
    } else if body_dependent {
        // 预筛阶段：正文相关条件按「可能命中」处理——宁可多解一次正文，也不能漏。
        terms.push(true);
    }

    let _ = ctx.account;
    if terms.is_empty() {
        return true;
    }
    if is_any_mode(match_mode) {
        terms.iter().any(|hit| *hit)
    } else {
        terms.iter().all(|hit| *hit)
    }
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
            protocol: "imap".into(),
            pop3_host: String::new(),
            pop3_port: 995,
            pop3_security: super::super::model::Security::None,
            pop3_leave_on_server: true,
            smtp_host: String::new(),
            smtp_port: 465,
            smtp_security: super::super::model::Security::Ssl,
            ews_url: String::new(),
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
            in_reply_to: String::new(),
            references: String::new(),
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

#[cfg(test)]
mod logic_tests {
    use super::*;
    use crate::commands::mail::model::{Account, MessageSummary, Security};
    use std::collections::BTreeMap;

    fn account() -> Account {
        Account {
            id: "acc".into(),
            email: "me@example.com".into(),
            display_name: String::new(),
            imap_host: String::new(),
            imap_port: 993,
            imap_security: Security::Ssl,
            protocol: "imap".into(),
            pop3_host: String::new(),
            pop3_port: 995,
            pop3_security: Security::None,
            pop3_leave_on_server: true,
            smtp_host: String::new(),
            smtp_port: 465,
            smtp_security: Security::Ssl,
            ews_url: String::new(),
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

    fn message(subject: &str, from: &str) -> MessageSummary {
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
            has_attachments: false,
            thread_key: String::new(),
            in_reply_to: String::new(),
            references: String::new(),
            labels: String::new(),
            outgoing: false,
        }
    }

    fn ctx<'a>(msg: &'a MessageSummary, acc: &'a Account) -> RuleContext<'a> {
        RuleContext {
            message: msg,
            body_preview: "",
            account: acc,
            folder_role: "inbox",
        }
    }

    fn rule(mode: &str, cond: RuleCondition) -> MailRule {
        MailRule {
            id: "r".into(),
            name: "t".into(),
            enabled: true,
            priority: 0,
            match_mode: mode.into(),
            condition: cond,
            action: RuleAction::default(),
            stop_processing: false,
            created_at: 0,
        }
    }

    #[test]
    fn any_mode_needs_only_one_condition() {
        let acc = account();
        let msg = message("季度报告", "boss@corp.com");
        let ctx = ctx(&msg, &acc);
        let cond = RuleCondition {
            from_contains: vec!["nobody@corp.com".into()],
            subject_contains: vec!["报告".into()],
            ..Default::default()
        };
        assert!(evaluate(&[rule("all", cond.clone())], &ctx).is_empty(), "与：一条不满足就不命中");
        assert_eq!(evaluate(&[rule("any", cond)], &ctx).len(), 1, "或：任一满足即命中");
    }

    #[test]
    fn not_contains_acts_as_negation() {
        let acc = account();
        let cond = RuleCondition {
            from_not_contains: vec!["bad.example".into()],
            ..Default::default()
        };
        let spam = message("hi", "someone@bad.example");
        assert!(evaluate(&[rule("all", cond.clone())], &ctx(&spam, &acc)).is_empty());
        let friend = message("hi", "someone@good.example");
        assert_eq!(evaluate(&[rule("all", cond)], &ctx(&friend, &acc)).len(), 1);
    }

    #[test]
    fn scope_conditions_do_not_participate_in_any() {
        let acc = account();
        let msg = message("hi", "a@b.c");
        let mut r = rule(
            "any",
            RuleCondition {
                subject_contains: vec!["hi".into()],
                ..Default::default()
            },
        );
        r.condition.has_attachment = Some(true);
        assert!(evaluate(&[r], &ctx(&msg, &acc)).is_empty(), "附件是范围，不能被「或」绕过");
    }

    #[test]
    fn body_prefilter_only_skips_when_other_terms_already_fail() {
        let acc = account();
        let cond = RuleCondition {
            from_contains: vec!["boss@corp.com".into()],
            body_contains: vec!["发票".into()],
            ..Default::default()
        };
        let other = message("hi", "other@corp.com");
        assert!(!body_relevant(&[rule("all", cond.clone())], &ctx(&other, &acc)), "与模式下不可能命中，不必解正文");
        let boss = message("hi", "boss@corp.com");
        assert!(body_relevant(&[rule("all", cond)], &ctx(&boss, &acc)), "还可能命中，需要正文");
    }

    #[test]
    fn recipient_matching_reads_to_and_cc_json() {
        let acc = account();
        let mut msg = message("hi", "a@b.c");
        msg.cc_json = r#"[{"name":"R&D Department","address":"team@example.com"}]"#.into();
        let cond = RuleCondition {
            to_contains: vec!["team@example.com".into()],
            ..Default::default()
        };
        assert_eq!(evaluate(&[rule("all", cond)], &ctx(&msg, &acc)).len(), 1);
    }

    fn clause(field: &str, op: &str, value: &str) -> RuleClause {
        RuleClause {
            field: field.into(),
            op: op.into(),
            value: value.into(),
        }
    }

    fn group(mode: &str, not: bool, clauses: Vec<RuleClause>) -> RuleGroup {
        RuleGroup {
            mode: mode.into(),
            not,
            clauses,
            groups: Vec::new(),
        }
    }

    fn grouped(cond_groups: Vec<RuleGroup>, mode: &str) -> MailRule {
        rule(
            mode,
            RuleCondition {
                groups: cond_groups,
                ..Default::default()
            },
        )
    }

    #[test]
    fn nested_groups_express_mixed_and_or() {
        let acc = account();
        let msg = message("季度报告", "boss@corp.com");
        let ctx = ctx(&msg, &acc);
        // (发件人含 boss 或 发件人含 cfo) 且 主题含 报告
        let cond = vec![RuleGroup {
            mode: "all".into(),
            not: false,
            clauses: vec![clause("subject", "contains", "报告")],
            groups: vec![group(
                "any",
                false,
                vec![
                    clause("from", "contains", "boss"),
                    clause("from", "contains", "cfo"),
                ],
            )],
        }];
        assert_eq!(evaluate(&[grouped(cond, "all")], &ctx).len(), 1);

        let cond2 = vec![RuleGroup {
            mode: "all".into(),
            not: false,
            clauses: vec![clause("subject", "contains", "发票")],
            groups: vec![group("any", false, vec![clause("from", "contains", "boss")])],
        }];
        assert!(evaluate(&[grouped(cond2, "all")], &ctx).is_empty());
    }

    #[test]
    fn group_not_inverts_the_whole_group() {
        let acc = account();
        let spam = message("广告：限时优惠", "noreply@spam.example");
        let spam_ctx = ctx(&spam, &acc);
        // 非(发件人含 spam 且 主题含 优惠)
        let cond = vec![RuleGroup {
            mode: "all".into(),
            not: true,
            clauses: vec![
                clause("from", "contains", "spam.example"),
                clause("subject", "contains", "优惠"),
            ],
            groups: Vec::new(),
        }];
        assert!(evaluate(&[grouped(cond, "all")], &spam_ctx).is_empty());

        let friend = message("会议纪要", "boss@corp.com");
        let friend_ctx = ctx(&friend, &acc);
        let cond2 = vec![RuleGroup {
            mode: "all".into(),
            not: true,
            clauses: vec![
                clause("from", "contains", "spam.example"),
                clause("subject", "contains", "优惠"),
            ],
            groups: Vec::new(),
        }];
        assert_eq!(evaluate(&[grouped(cond2, "all")], &friend_ctx).len(), 1);
    }

    #[test]
    fn equals_matches_recipient_address_not_json_text() {
        let acc = account();
        let mut msg = message("hi", "a@b.c");
        msg.to_json = r#"[{"name":"张三","address":"zhangsan@corp.com"}]"#.into();
        let cond = RuleCondition {
            groups: vec![group("all", false, vec![clause("to", "equals", "zhangsan@corp.com")])],
            ..Default::default()
        };
        assert_eq!(evaluate(&[rule("all", cond)], &ctx(&msg, &acc)).len(), 1);
    }

    #[test]
    fn body_group_prefilter_is_conservative_under_negation() {
        let acc = account();
        let msg = message("hi", "a@b.c");
        let cond = RuleCondition {
            groups: vec![group("all", true, vec![clause("body", "contains", "secret")])],
            ..Default::default()
        };
        assert!(
            body_relevant(&[rule("all", cond)], &ctx(&msg, &acc)),
            "非(正文含…) 不能靠正文占位预筛，必须解正文"
        );
    }

    #[test]
    fn empty_groups_are_ignored() {
        let acc = account();
        let msg = message("hi", "a@b.c");
        let cond = RuleCondition {
            groups: vec![RuleGroup {
                mode: "any".into(),
                not: false,
                clauses: vec![clause("subject", "contains", "")],
                groups: Vec::new(),
            }],
            ..Default::default()
        };
        assert_eq!(
            evaluate(&[rule("all", cond)], &ctx(&msg, &acc)).len(),
            1,
            "没有填值的空条件组不应把规则判为不命中"
        );
    }
}
