//! 通讯录的数据模型：联系人、分组（通讯组列表）、往来记录与导入导出结果。
//!
//! 这里只有「能跨 IPC 传、能进 SQLite」的扁平结构。vCard 的编解码在 `vcard` 子模块，
//! 自动收录的逻辑在 `contacts`。

use serde::{Deserialize, Serialize};

/// 一个联系人。一个人可以有多个邮箱与电话（工作 / 私人 / 其他），
/// 用 `Vec<LabeledValue>` 而不是固定字段：邮件客户端最常做错的事就是
/// 「一个字段只存一个地址」，于是同一个人换个邮箱发信就变成两个人。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub id: String,
    /// 显示名。空则前端回退到第一个邮箱的本地部分。
    #[serde(default)]
    pub display_name: String,
    /// 名 / 姓分开存：vCard 的 N 属性、Outlook 的排序都依赖这个。
    #[serde(default)]
    pub given_name: String,
    #[serde(default)]
    pub family_name: String,
    #[serde(default)]
    pub emails: Vec<LabeledValue>,
    #[serde(default)]
    pub phones: Vec<LabeledValue>,
    #[serde(default)]
    pub organization: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub note: String,
    /// 生日（`YYYY-MM-DD`），vCard 的 BDAY。
    #[serde(default)]
    pub birthday: String,
    /// 网址（vCard URL）。
    #[serde(default)]
    pub website: String,
    #[serde(default)]
    pub address: String,
    /// 头像：data URL 或 `asset:` 路径。
    #[serde(default)]
    pub avatar: String,
    /// 归属分组（通讯组列表）id。
    #[serde(default)]
    pub groups: Vec<String>,
    /// 星标 / 重要联系人。
    #[serde(default)]
    pub starred: bool,
    /// 来源：`manual`（手工建）/ `auto`（从往来邮件自动收录）/ `import`（vCard 导入）。
    #[serde(default = "default_source")]
    pub source: String,
    /// 首次出现 / 最后通信时间（Unix 秒）。自动收录的联系人靠它排序与清理。
    #[serde(default)]
    pub first_seen_at: i64,
    #[serde(default)]
    pub last_seen_at: i64,
    /// 与该联系人的往来邮件数 / 未读数，列表里直接展示，避免逐行查询。
    #[serde(default)]
    pub message_count: u32,
    #[serde(default)]
    pub unread_count: u32,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

fn default_source() -> String {
    "manual".to_string()
}

/// 带标签的值（邮箱 / 电话）。`label` 取 `work` | `home` | `other`，也允许用户自定义中文标签。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabeledValue {
    #[serde(default)]
    pub label: String,
    pub value: String,
}

impl Contact {
    /// 排序与占位字母用的名字。
    pub fn best_name(&self) -> String {
        let full = format!("{} {}", self.display_name.trim(), "").trim().to_string();
        if !full.is_empty() {
            return full;
        }
        let combined = format!("{}{}", self.family_name.trim(), self.given_name.trim());
        if !combined.trim().is_empty() {
            return combined.trim().to_string();
        }
        self.emails
            .first()
            .map(|e| e.value.clone())
            .unwrap_or_default()
    }

    /// 所有邮箱的小写集合，用于和其它模块做匹配。
    pub fn all_emails(&self) -> Vec<String> {
        self.emails
            .iter()
            .map(|e| e.value.trim().to_ascii_lowercase())
            .filter(|e| !e.is_empty())
            .collect()
    }

    pub fn primary_email(&self) -> String {
        self.emails.first().map(|e| e.value.clone()).unwrap_or_default()
    }
}

/// 通讯组列表（Outlook 的「联系人组」/ 邮件列表）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// 成员：联系人 id 与直接邮箱可以混用（有的组只是几个地址）。
    #[serde(default)]
    pub members: Vec<GroupMember>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMember {
    /// 指向联系人时为 `contact`，直接写在组里的地址为 `email`。
    #[serde(default = "default_member_kind")]
    pub kind: String,
    #[serde(default)]
    pub contact_id: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub name: String,
}

fn default_member_kind() -> String {
    "contact".to_string()
}

/// 通讯录查询条件。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactQuery {
    /// 关键词：匹配姓名、所有邮箱、组织、电话、备注。
    #[serde(default)]
    pub text: String,
    /// 只看这些分组。
    #[serde(default)]
    pub group_ids: Vec<String>,
    #[serde(default)]
    pub starred_only: bool,
    /// `manual` | `auto` | `import`；空表示全部。
    #[serde(default)]
    pub source: Option<String>,
    /// 只看「经常联系」：按往来次数排序并截断。
    #[serde(default)]
    pub frequent_only: bool,
    /// 是否包含从未往来过的手工联系人（默认包含）。
    #[serde(default = "default_true")]
    pub include_unused: bool,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

fn default_true() -> bool {
    true
}

/// 写信时地址栏自动补全的一条候选。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactSuggestion {
    pub display: String,
    pub email: String,
    #[serde(default)]
    pub contact_id: Option<String>,
    /// `contact` | `history` | `group`
    pub kind: String,
    /// 匹配度（0-100），前端据此排序。
    #[serde(default)]
    pub score: u32,
    /// 组候选展开后的成员数。
    #[serde(default)]
    pub member_count: u32,
}

/// 与某个联系人的一封往来邮件（联系人详情页的「邮件往来」）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactMessage {
    pub account_id: String,
    pub folder: String,
    pub uid: u32,
    pub subject: String,
    pub date: i64,
    pub snippet: String,
    /// `in`（对方发来）/ `out`（我发出）。
    pub direction: String,
}

/// vCard 导入结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: u32,
    pub merged: u32,
    pub skipped: u32,
    /// 前若干条失败原因，够用户判断是文件格式问题还是个别条目问题。
    #[serde(default)]
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn best_name_falls_back_through_fields() {
        let mut c = Contact::default();
        c.emails.push(LabeledValue {
            label: "work".into(),
            value: "a@b.c".into(),
        });
        assert_eq!(c.best_name(), "a@b.c");

        c.family_name = "张".into();
        c.given_name = "三".into();
        assert_eq!(c.best_name(), "张三");

        c.display_name = "张三（财务）".into();
        assert_eq!(c.best_name(), "张三（财务）");
    }

    #[test]
    fn all_emails_lowercases_and_drops_empty() {
        let c = Contact {
            emails: vec![
                LabeledValue { label: String::new(), value: " A@B.C ".into() },
                LabeledValue { label: String::new(), value: "  ".into() },
            ],
            ..Default::default()
        };
        assert_eq!(c.all_emails(), vec!["a@b.c".to_string()]);
    }
}
