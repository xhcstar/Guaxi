//! vCard 编解码。
//!
//! 为什么不用现成的 `vcard` crate：0.6 只接受 vCard 4.0，而 Outlook、Windows 通讯录、
//! 多数手机导出的都是 3.0，不少国产客户端还是 2.1；它还会拉进 rkyv / borsh / phonenumber
//! 等二十来个传递依赖。通讯录的 vCard 子集很小，自己写反而能**按需宽容**：
//! - 解析：2.1 / 3.0 / 4.0 都吃，缺 FN 时按 N 拼，缺 N 时按 FN 拆；
//! - 编码：输出 3.0（兼容面最广），必要时用 QUOTED-PRINTABLE 处理非 ASCII。
//!
//! 三个必须正确处理、否则导入会「静默丢字」的点：
//! 1. **行折叠**：RFC 规定长行在 75 字节处折行，续行以空格或制表符开头；
//! 2. **转义**：`\n` `\,` `\;` `\\`，2.1/3.0 里逗号和分号都要转义；
//! 3. **编码**：3.0 的 `ENCODING=QUOTED-PRINTABLE`（中文名字常见）与 `CHARSET=GBK`。

use super::model::{Contact, ContactGroup, GroupMember, LabeledValue};
use uuid::Uuid;

/// 逻辑行（已展开折叠）。
fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        if raw.starts_with(' ') || raw.starts_with('\t') {
            // 续行：接着上一条。
            if let Some(last) = out.last_mut() {
                last.push_str(&raw[1..]);
                continue;
            }
        }
        if !raw.trim().is_empty() {
            out.push(raw.to_string());
        }
    }
    out
}

/// 拆分 `NAME;PARAM=V;PARAM="v":value`。返回 (属性名大写, 参数表, 值)。
fn split_line(line: &str) -> Option<(String, Vec<(String, String)>, String)> {
    // 冒号必须找「不在引号里」的那个。
    let mut in_quote = false;
    let mut colon = None;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' => in_quote = !in_quote,
            ':' if !in_quote => {
                colon = Some(i);
                break;
            }
            _ => {}
        }
    }
    let colon = colon?;
    let head = &line[..colon];
    let value = line[colon + 1..].to_string();

    let mut parts = split_semicolon(head);
    if parts.is_empty() {
        return None;
    }
    let name = parts.remove(0).to_ascii_uppercase();
    // 去掉可能的组前缀 `group.NAME`。
    let name = name.rsplit('.').next().unwrap_or(&name).to_string();

    let mut params = Vec::new();
    for part in parts {
        if let Some((k, v)) = part.split_once('=') {
            params.push((
                k.trim().to_ascii_uppercase(),
                v.trim().trim_matches('"').to_string(),
            ));
        } else if !part.trim().is_empty() {
            // 2.1 里裸的 `PREF` / `WORK` 这种位置参数。
            params.push((String::new(), part.trim().to_ascii_uppercase()));
        }
    }
    Some((name, params, value))
}

/// 按分号切分，尊重被引号包住的参数值。
fn split_semicolon(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    for ch in input.chars() {
        match ch {
            '"' => {
                in_quote = !in_quote;
                current.push(ch);
            }
            ';' if !in_quote => {
                out.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    out.push(current);
    out
}

/// 按未转义的逗号切分（多值属性：EMAIL / TEL / N）。
fn split_escaped(input: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            current.push('\\');
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            c if c == sep => {
                out.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if escaped {
        current.push('\\');
    }
    out.push(current);
    out
}

/// 处理 vCard 的字符串转义。
fn unescape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') | Some('N') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn escape_text(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(';', "\\;")
        .replace(',', "\\,")
}

/// QUOTED-PRINTABLE 解码（2.1/3.0 的中文名常见）。`soft_break` 是行尾的 `=` 续行。
fn decode_quoted_printable(input: &str) -> Vec<u8> {
    let joined = input.replace("=\r\n", "").replace("=\n", "");
    let bytes = joined.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' && i + 2 < bytes.len() {
            // 按字节切片：非 ASCII 字符在 QP 里就是字节，直接 &str 切片会切断 UTF-8 边界。
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn decode_bytes(bytes: &[u8], charset: &str) -> String {
    let cs = charset.trim().trim_matches('"').to_ascii_lowercase();
    if cs.is_empty() || cs == "utf-8" || cs == "utf8" {
        return String::from_utf8_lossy(bytes).to_string();
    }
    // encoding_rs 是项目既有依赖，覆盖 GBK / GB2312 / BIG5 / SHIFT_JIS 等。
    let encoding = encoding_rs::Encoding::for_label(cs.as_bytes());
    match encoding {
        Some(enc) => {
            let (text, _, _) = enc.decode(bytes);
            text.to_string()
        }
        None => String::from_utf8_lossy(bytes).to_string(),
    }
}

fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn has_positional(params: &[(String, String)], value: &str) -> bool {
    params
        .iter()
        .any(|(k, v)| k.is_empty() && v.eq_ignore_ascii_case(value))
}

/// 解码属性值（处理 QUOTED-PRINTABLE / BASE64 / CHARSET）。
fn decode_value(params: &[(String, String)], raw: &str) -> String {
    let encoding = param(params, "ENCODING")
        .unwrap_or("")
        .to_ascii_uppercase();
    if encoding.starts_with("QUOTED-PRINTABLE") {
        let charset = param(params, "CHARSET").unwrap_or("utf-8");
        return unescape(&decode_bytes(&decode_quoted_printable(raw), charset));
    }
    // BASE64 只出现在 PHOTO / LOGO 之类，通讯录导入用不上，直接丢弃。
    if encoding == "B" || encoding == "BASE64" {
        return String::new();
    }
    let charset = param(params, "CHARSET");
    if let Some(cs) = charset {
        let lower = cs.to_ascii_lowercase();
        if lower != "utf-8" && lower != "utf8" {
            return unescape(&decode_bytes(raw.as_bytes(), cs));
        }
    }
    unescape(raw)
}

/// 邮箱标签归一：vCard 的 `WORK` / `HOME` 或 `TYPE=work`。
fn mail_label(params: &[(String, String)]) -> String {
    let typed = param(params, "TYPE").unwrap_or("").to_ascii_uppercase();
    let has = |needle: &str| {
        typed.split(',').any(|t| t.trim() == needle) || has_positional(params, needle)
    };
    if has("WORK") {
        "work".into()
    } else if has("HOME") {
        "home".into()
    } else if has("CELL") || has("VOICE") {
        "phone".into()
    } else {
        "other".into()
    }
}

/// 解析一个 vCard 文件。返回 (联系人列表, 通讯组列表)。
///
/// 容错策略：单张卡片解析失败只跳过它并记一行错误，不影响整个文件——
/// 从 Outlook 导出的 .vcf 里混进半张损坏的卡片是常态。
pub fn parse(text: &str) -> (Vec<Contact>, Vec<ContactGroup>, Vec<String>) {
    let lines = unfold(text);
    let mut contacts: Vec<Contact> = Vec::new();
    let mut groups: Vec<ContactGroup> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    // 苹果导出的组卡片用 `urn:uuid:<联系人 UID>` 引用成员，而联系人本身在另一张卡片里。
    // 先记下 UID → (姓名, 邮箱)，等整份文件解析完再把 URN 换成邮箱。
    let mut uid_to_email: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();

    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        let upper = line.to_ascii_uppercase();
        if !upper.starts_with("BEGIN:VCARD") {
            i += 1;
            continue;
        }
        i += 1;
        let mut contact = Contact {
            id: Uuid::new_v4().to_string(),
            source: "import".into(),
            ..Default::default()
        };
        let mut group_names: Vec<String> = Vec::new();
        let mut is_group = false;
        let mut uid = String::new();
        let mut group_members: Vec<GroupMember> = Vec::new();
        let mut ended = false;

        while i < lines.len() {
            let raw = lines[i].clone();
            i += 1;
            let upper = raw.to_ascii_uppercase();
            if upper.starts_with("END:VCARD") {
                ended = true;
                break;
            }
            // 卡片内部又出现 BEGIN:VCARD，说明上一张忘了 END。
            // 不能把它当普通属性忽略——那样两张卡会被静默合并成一条（姓名被后者覆盖、
            // 邮箱混在一起），导入结果看起来「成功」实则丢人。这里记录错误并从这张重新开始。
            if upper.starts_with("BEGIN:VCARD") {
                if errors.len() < 20 {
                    errors.push("有一张卡片缺少 END:VCARD，已从下一张重新开始".to_string());
                }
                // 回退一行，让外层循环从新的 BEGIN 开始解析。
                i -= 1;
                break;
            }
            let Some((name, params, value)) = split_line(&raw) else {
                continue;
            };
            let decoded = |v: &str| decode_value(&params, v);

            match name.as_str() {
                "FN" => contact.display_name = decoded(&value).trim().to_string(),
                "N" => {
                    let parts = split_escaped(&value, ';');
                    let get = |idx: usize| {
                        parts
                            .get(idx)
                            .map(|s| unescape(s).trim().to_string())
                            .unwrap_or_default()
                    };
                    contact.family_name = get(0);
                    contact.given_name = get(1);
                }
                "EMAIL" | "X-EMAIL" => {
                    let addr = decoded(&value).trim().to_string();
                    if !addr.is_empty() {
                        contact.emails.push(LabeledValue {
                            label: mail_label(&params),
                            value: addr,
                        });
                    }
                }
                "TEL" | "X-TEL" => {
                    let number = decoded(&value).trim().to_string();
                    if !number.is_empty() {
                        contact.phones.push(LabeledValue {
                            label: mail_label(&params),
                            value: number,
                        });
                    }
                }
                "ORG" => {
                    contact.organization = split_escaped(&value, ';')
                        .first()
                        .map(|s| unescape(s).trim().to_string())
                        .unwrap_or_default();
                }
                "TITLE" => contact.title = decoded(&value).trim().to_string(),
                "NOTE" => contact.note = decoded(&value).trim().to_string(),
                "BDAY" => contact.birthday = decoded(&value).trim().to_string(),
                "URL" => contact.website = decoded(&value).trim().to_string(),
                "ADR" => {
                    let parts: Vec<String> = split_escaped(&value, ';')
                        .iter()
                        .map(|s| unescape(s).trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if !parts.is_empty() {
                        contact.address = parts.join(" ");
                    }
                }
                "CATEGORIES" => {
                    for item in split_escaped(&value, ',') {
                        let label = unescape(&item).trim().to_string();
                        if !label.is_empty() {
                            group_names.push(label);
                        }
                    }
                }
                "KIND" | "X-ADDRESSBOOKSERVER-KIND" => {
                    // vCard 4.0 的 KIND:group，或苹果 3.0 扩展的同义字段。
                    if decoded(&value).trim().eq_ignore_ascii_case("group") {
                        is_group = true;
                    }
                }
                "UID" => uid = decoded(&value).trim().to_string(),
                "MEMBER" | "X-ADDRESSBOOKSERVER-MEMBER" | "X-MEMBER" => {
                    if let Some(member) = parse_group_member(&params, &decoded(&value)) {
                        group_members.push(member);
                    }
                }
                _ => {}
            }
        }

        if !ended {
            if errors.len() < 20 {
                errors.push("有一张卡片缺少 END:VCARD（已跳过）".to_string());
            }
            continue;
        }

        // 通讯组卡片：成员是「组引用」或「直接邮箱」，不是联系人本身。
        if is_group {
            let name = contact.display_name.trim().to_string();
            if name.is_empty() {
                if errors.len() < 20 {
                    errors.push("有一张通讯组卡片没有组名（已跳过）".to_string());
                }
                continue;
            }
            let mut members: Vec<GroupMember> = Vec::new();
            for member in group_members {
                let duplicate = members.iter().any(|existing| {
                    if existing.kind == "group" || member.kind == "group" {
                        existing.kind == member.kind && existing.group_id == member.group_id
                    } else {
                        existing.email.to_lowercase() == member.email.to_lowercase()
                    }
                });
                if !duplicate {
                    members.push(member);
                }
            }
            groups.push(ContactGroup {
                id: uid,
                name,
                description: contact.note.trim().to_string(),
                members,
                ..Default::default()
            });
            continue;
        }

        if contact.display_name.is_empty() {
            let name = contact.best_name();
            contact.display_name = name;
        }
        if contact.display_name.is_empty() && contact.emails.is_empty() {
            errors.push("有一张卡片既没有姓名也没有邮箱（已跳过）".to_string());
            continue;
        }
        // CATEGORIES 是 3.0/4.0 的分组写法：转成「分组名」标记，由调用方决定是否建组。
        if !group_names.is_empty() {
            contact.groups = group_names;
        }
        if !uid.is_empty() {
            let email = contact.primary_email();
            if !email.is_empty() {
                uid_to_email.insert(uid.clone(), (contact.display_name.clone(), email));
            }
        }
        contacts.push(contact);
    }

    // 组卡片里的 `urn:uuid:` 成员：能对上联系人 UID 的换成邮箱；
    // 对不上但确实是本文件里的另一个组，就保留成嵌套组；其余丢弃并说明原因。
    let group_ids: Vec<String> = groups
        .iter()
        .map(|group| group.id.clone())
        .filter(|id| !id.is_empty())
        .collect();
    for group in groups.iter_mut() {
        let mut kept: Vec<GroupMember> = Vec::new();
        let mut dropped = 0usize;
        for member in group.members.drain(..) {
            if member.kind == "group" {
                if let Some((name, email)) = uid_to_email.get(&member.group_id) {
                    kept.push(GroupMember {
                        kind: "email".into(),
                        email: email.clone(),
                        name: name.clone(),
                        ..Default::default()
                    });
                } else if group_ids.iter().any(|id| id == &member.group_id) {
                    kept.push(member);
                } else {
                    dropped += 1;
                }
            } else {
                kept.push(member);
            }
        }
        group.members = kept;
        if dropped > 0 && errors.len() < 20 {
            errors.push(format!(
                "通讯组「{}」有 {dropped} 个成员在文件里找不到对应联系人（已跳过）",
                group.name
            ));
        }
    }

    (contacts, groups, errors)
}

/// 解析 `MEMBER` / `X-ADDRESSBOOKSERVER-MEMBER` 的值。
///
/// 三种形态都要认：
/// - `mailto:a@b.c`（vCard 4.0 / 苹果）；
/// - `urn:uuid:...`（指向另一张卡片：通讯组嵌套，或苹果导出的联系人 UUID）；
/// - 裸邮箱（部分国产客户端）。
fn parse_group_member(params: &[(String, String)], value: &str) -> Option<GroupMember> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return None;
    }
    let name = param(params, "CN")
        .map(|cn| unescape(cn).trim().to_string())
        .unwrap_or_default();
    if let Some(rest) = value
        .strip_prefix("urn:uuid:")
        .or_else(|| value.strip_prefix("URN:UUID:"))
    {
        let group_id = rest.trim().to_string();
        return (!group_id.is_empty()).then_some(GroupMember {
            kind: "group".into(),
            group_id,
            name,
            ..Default::default()
        });
    }
    let address = value
        .strip_prefix("mailto:")
        .or_else(|| value.strip_prefix("MAILTO:"))
        .unwrap_or(&value)
        .trim();
    let (name, address) = match (address.rfind('<'), address.rfind('>')) {
        (Some(open), Some(close)) if close > open => (
            if name.is_empty() {
                address[..open].trim().trim_matches('"').to_string()
            } else {
                name
            },
            address[open + 1..close].trim().to_string(),
        ),
        _ => (name, address.trim_matches(['<', '>']).to_string()),
    };
    if address.is_empty() {
        return None;
    }
    Some(GroupMember {
        kind: "email".into(),
        email: address,
        name,
        ..Default::default()
    })
}

/// 通讯组导出卡片的内容（邮箱已经解析成具体地址）。
#[derive(Debug, Clone, Default)]
pub struct GroupCard {
    pub id: String,
    pub name: String,
    pub description: String,
    /// (显示名, 邮箱)。收件端看到的名字越完整，回信时越不容易选错人。
    pub emails: Vec<(String, String)>,
    /// 嵌套的子通讯组 id。
    pub child_group_ids: Vec<String>,
}

/// 把通讯组编码成 vCard。同时写苹果的 `X-ADDRESSBOOKSERVER-*` 与
/// vCard 4.0 的 `KIND:group` / `MEMBER`，让 Windows、苹果、安卓都能读回成员。
pub fn encode_group_card(card: &GroupCard) -> String {
    let mut out = String::with_capacity(256);
    out.push_str("BEGIN:VCARD\r\n");
    out.push_str("VERSION:3.0\r\n");
    out.push_str("PRODID:-//Guaxi//Contacts//CN\r\n");
    if !card.id.trim().is_empty() {
        out.push_str(&format!("UID:{}\r\n", escape_text(card.id.trim())));
    }
    // 组卡片用组名占住 N，部分客户端只读 N 不读 FN。
    out.push_str(&format!("N:{};;;;\r\n", escape_text(card.name.trim())));
    out.push_str(&format!("FN:{}\r\n", escape_text(card.name.trim())));
    if !card.description.trim().is_empty() {
        out.push_str(&format!("NOTE:{}\r\n", escape_text(card.description.trim())));
    }
    out.push_str("KIND:group\r\n");
    out.push_str("X-ADDRESSBOOKSERVER-KIND:group\r\n");
    for (name, email) in &card.emails {
        let email = email.trim();
        if email.is_empty() {
            continue;
        }
        let uri = format!("mailto:{email}");
        if name.trim().is_empty() {
            out.push_str(&format!("MEMBER:{uri}\r\n"));
            out.push_str(&format!("X-ADDRESSBOOKSERVER-MEMBER:{uri}\r\n"));
        } else {
            let label = name.trim().replace('"', "");
            out.push_str(&format!("MEMBER;CN=\"{label}\":{uri}\r\n"));
            out.push_str(&format!("X-ADDRESSBOOKSERVER-MEMBER;CN=\"{label}\":{uri}\r\n"));
        }
    }
    for child in &card.child_group_ids {
        let child = child.trim();
        if child.is_empty() {
            continue;
        }
        let uri = format!("urn:uuid:{child}");
        out.push_str(&format!("MEMBER:{uri}\r\n"));
        out.push_str(&format!("X-ADDRESSBOOKSERVER-MEMBER:{uri}\r\n"));
    }
    out.push_str("END:VCARD\r\n");
    out
}

/// 把联系人编码成 vCard 3.0。
///
/// 选 3.0 而不是 4.0：Windows 通讯录、Outlook 2016 以前的版本、大量安卓机都只认 3.0，
/// 而 4.0 的收益（参数语法更干净）对通讯录这种小数据没有实际意义。
pub fn encode_contact(contact: &Contact) -> String {
    let mut out = String::with_capacity(256);
    out.push_str("BEGIN:VCARD\r\n");
    out.push_str("VERSION:3.0\r\n");

    let given = if contact.given_name.trim().is_empty() && !contact.display_name.is_empty() {
        contact.display_name.trim().to_string()
    } else {
        contact.given_name.trim().to_string()
    };
    out.push_str(&format!(
        "N:{};{};;;\r\n",
        escape_text(&contact.family_name),
        escape_text(&given)
    ));
    let fn_value = if contact.display_name.trim().is_empty() {
        contact.best_name()
    } else {
        contact.display_name.clone()
    };
    out.push_str(&format!("FN:{}\r\n", escape_text(&fn_value)));

    for email in &contact.emails {
        if email.value.trim().is_empty() {
            continue;
        }
        let kind = match email.label.as_str() {
            "work" => "WORK",
            "home" => "HOME",
            _ => "OTHER",
        };
        out.push_str(&format!(
            "EMAIL;TYPE=INTERNET,{kind}:{}\r\n",
            escape_text(email.value.trim())
        ));
    }
    for phone in &contact.phones {
        if phone.value.trim().is_empty() {
            continue;
        }
        let kind = match phone.label.as_str() {
            "work" => "WORK,VOICE",
            "home" => "HOME,VOICE",
            "phone" => "CELL",
            _ => "VOICE",
        };
        out.push_str(&format!(
            "TEL;TYPE={kind}:{}\r\n",
            escape_text(phone.value.trim())
        ));
    }
    if !contact.organization.trim().is_empty() {
        out.push_str(&format!("ORG:{}\r\n", escape_text(contact.organization.trim())));
    }
    if !contact.title.trim().is_empty() {
        out.push_str(&format!("TITLE:{}\r\n", escape_text(contact.title.trim())));
    }
    if !contact.address.trim().is_empty() {
        out.push_str(&format!("ADR:;;{};;;;\r\n", escape_text(contact.address.trim())));
    }
    if !contact.birthday.trim().is_empty() {
        out.push_str(&format!("BDAY:{}\r\n", contact.birthday.trim()));
    }
    if !contact.website.trim().is_empty() {
        out.push_str(&format!("URL:{}\r\n", escape_text(contact.website.trim())));
    }
    if !contact.note.trim().is_empty() {
        out.push_str(&format!("NOTE:{}\r\n", escape_text(contact.note.trim())));
    }
    for group in &contact.groups {
        if group.trim().is_empty() {
            continue;
        }
        out.push_str(&format!("CATEGORIES:{}\r\n", escape_text(group.trim())));
    }
    out.push_str("END:VCARD\r\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vcard_3_0() {
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nN:张;三;;;\r\nFN:张三\r\n\
ORG:某某公司\r\nTITLE:财务经理\r\n\
EMAIL;TYPE=WORK:zhangsan@corp.com\r\nEMAIL;TYPE=HOME:zs@qq.com\r\n\
TEL;TYPE=CELL:13800000000\r\nEND:VCARD\r\n";
        let (contacts, _, errors) = parse(text);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(contacts.len(), 1);
        let c = &contacts[0];
        assert_eq!(c.display_name, "张三");
        assert_eq!(c.family_name, "张");
        assert_eq!(c.given_name, "三");
        assert_eq!(c.organization, "某某公司");
        assert_eq!(c.emails.len(), 2);
        assert_eq!(c.emails[0].value, "zhangsan@corp.com");
        assert_eq!(c.emails[0].label, "work");
        assert_eq!(c.phones[0].value, "13800000000");
        assert_eq!(c.source, "import");
    }

    #[test]
    fn parse_folded_lines() {
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:一个非常非常长的\r\n 名字被折行了\r\n\
EMAIL:a@b.c\r\nEND:VCARD\r\n";
        let (contacts, _, _) = parse(text);
        assert_eq!(contacts[0].display_name, "一个非常非常长的名字被折行了");
    }

    #[test]
    fn parse_quoted_printable_gbk_name() {
        // "张三" 的 GBK 编码是 D5 C5 C8 FD，QP 写作 =D5=C5=C8=FD。
        let text = "BEGIN:VCARD\r\nVERSION:2.1\r\nFN;CHARSET=GBK;ENCODING=QUOTED-PRINTABLE:=D5=C5=C8=FD\r\n\
EMAIL:a@b.c\r\nEND:VCARD\r\n";
        let (contacts, _, errors) = parse(text);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(contacts[0].display_name, "张三");
    }

    #[test]
    fn parse_escaped_commas_and_newlines() {
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Smith\\, John\r\n\
NOTE:第一行\\n第二行\r\nEMAIL:a@b.c\r\nEND:VCARD\r\n";
        let (contacts, _, _) = parse(text);
        assert_eq!(contacts[0].display_name, "Smith, John");
        assert_eq!(contacts[0].note, "第一行\n第二行");
    }

    #[test]
    fn missing_fn_falls_back_to_n() {
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nN:王;小明;;;\r\nEMAIL:x@y.z\r\nEND:VCARD\r\n";
        let (contacts, _, _) = parse(text);
        assert_eq!(contacts[0].display_name, "王小明");
    }

    #[test]
    fn skips_broken_card_but_keeps_others() {
        let text = "BEGIN:VCARD\r\nFN:坏的\r\nBEGIN:VCARD\r\nFN:好的\r\nEMAIL:g@h.i\r\nEND:VCARD\r\n";
        let (contacts, _, errors) = parse(text);
        assert_eq!(contacts.len(), 1, "损坏的卡片应被跳过");
        assert_eq!(contacts[0].display_name, "好的");
        assert!(!errors.is_empty());
    }

    #[test]
    fn encodes_3_0_and_roundtrips() {
        let contact = Contact {
            id: "c1".into(),
            display_name: "张三, 财务".into(),
            family_name: "张".into(),
            given_name: "三".into(),
            emails: vec![LabeledValue {
                label: "work".into(),
                value: "zs@corp.com".into(),
            }],
            phones: vec![LabeledValue {
                label: "phone".into(),
                value: "13800000000".into(),
            }],
            organization: "某某公司".into(),
            note: "第一行\n第二行".into(),
            ..Default::default()
        };
        let encoded = encode_contact(&contact);
        assert!(encoded.contains("VERSION:3.0"));
        assert!(encoded.contains("FN:张三\\, 财务"));

        let (parsed, _, errors) = parse(&encoded);
        assert!(errors.is_empty(), "{errors:?}");
        let back = &parsed[0];
        assert_eq!(back.display_name, "张三, 财务");
        assert_eq!(back.emails[0].value, "zs@corp.com");
        assert_eq!(back.note, "第一行\n第二行");
    }

    #[test]
    fn split_semicolon_respects_quotes() {
        let parts = split_semicolon("EMAIL;TYPE=\"a;b\";X:y");
        assert_eq!(parts[0], "EMAIL");
        assert_eq!(parts[1], "TYPE=\"a;b\"");
    }

    #[test]
    fn group_card_round_trip() {
        let card = GroupCard {
            id: "g-1".into(),
            name: "研发组".into(),
            description: "研发同事".into(),
            emails: vec![("张三".into(), "zs@corp.com".into())],
            child_group_ids: vec!["g-2".into()],
        };
        let child = GroupCard {
            id: "g-2".into(),
            name: "前端".into(),
            description: String::new(),
            emails: vec![("李四".into(), "ls@corp.com".into())],
            child_group_ids: Vec::new(),
        };
        let text = format!("{}{}", encode_group_card(&card), encode_group_card(&child));
        let (contacts, groups, errors) = parse(&text);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(contacts.is_empty());
        assert_eq!(groups.len(), 2);
        let group = groups.iter().find(|g| g.id == "g-1").expect("缺少 g-1");
        assert_eq!(group.name, "研发组");
        assert_eq!(group.description, "研发同事");
        assert_eq!(group.members.len(), 2, "{:?}", group.members);
        assert_eq!(group.members[0].email, "zs@corp.com");
        assert_eq!(group.members[0].name, "张三");
        assert_eq!(group.members[1].kind, "group");
        assert_eq!(group.members[1].group_id, "g-2");
    }

    #[test]
    fn apple_group_urns_resolve_to_contact_emails() {
        // 苹果通讯录：联系人卡片带 UID，组卡片用 urn:uuid 引用它。
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:person-1\r\n\
FN:王五\r\nEMAIL:ww@corp.com\r\nEND:VCARD\r\n\
BEGIN:VCARD\r\nVERSION:3.0\r\nUID:group-1\r\nFN:项目组\r\n\
X-ADDRESSBOOKSERVER-KIND:group\r\n\
X-ADDRESSBOOKSERVER-MEMBER:urn:uuid:person-1\r\nEND:VCARD\r\n";
        let (contacts, groups, errors) = parse(text);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(contacts.len(), 1);
        assert_eq!(groups[0].name, "项目组");
        assert_eq!(groups[0].members[0].kind, "email");
        assert_eq!(groups[0].members[0].email, "ww@corp.com");
        assert_eq!(groups[0].members[0].name, "王五");
    }

    #[test]
    fn vcard4_group_card_is_recognized() {
        let text = "BEGIN:VCARD\r\nVERSION:4.0\r\nKIND:group\r\nFN:财务组\r\n\
UID:fin\r\nMEMBER:mailto:a@corp.com\r\nMEMBER:mailto:b@corp.com\r\nEND:VCARD\r\n";
        let (contacts, groups, errors) = parse(text);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(contacts.is_empty());
        assert_eq!(groups[0].name, "财务组");
        assert_eq!(groups[0].id, "fin");
        assert_eq!(groups[0].members.len(), 2);
        assert_eq!(groups[0].members[1].email, "b@corp.com");
    }
}
