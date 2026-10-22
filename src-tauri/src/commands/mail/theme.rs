//! 内置主题定义与 token 表。
//!
//! 两套默认主题：
//! - **墨**（`ink`，深色）：低饱和蓝灰，长时间看邮件不刺眼；
//! - **纸**（`paper`，浅色）：带一点暖灰的纸面，不是纯白——纯白配大量正文会发涩。
//!
//! 两者共用同一组 token 名，前端只认 token、不认主题，所以「自定义主题」就是
//! 抄一份 token 表改值。新增 token 时三处同步：`TOKENS`、`ink()`、`paper()`。

use super::model::CustomTheme;
use std::collections::BTreeMap;

/// 前端「自定义主题」编辑器里可编辑的字段，顺序即展示顺序。
pub const TOKENS: &[&str] = &[
    "bg",
    "bgElevated",
    "bgSunken",
    "panel",
    "panelHover",
    "sidebar",
    "sidebarActive",
    "border",
    "borderStrong",
    "text",
    "textSecondary",
    "textMuted",
    "accent",
    "accentText",
    "accentSoft",
    "unreadDot",
    "star",
    "danger",
    "warning",
    "success",
    "selection",
    "codeBg",
    "quoteBar",
    "shadow",
    "readerBg",
];

fn make(mode: &str, id: &str, name: &str, values: &[(&str, &str)]) -> CustomTheme {
    let mut tokens: BTreeMap<String, String> =
        TOKENS.iter().map(|t| (t.to_string(), String::new())).collect();
    for (key, value) in values {
        tokens.insert(key.to_string(), value.to_string());
    }
    CustomTheme {
        id: id.to_string(),
        name: name.to_string(),
        mode: mode.to_string(),
        tokens,
    }
}

/// 墨：深色。底色不是纯黑，层级靠「越靠上越亮」区分。
pub fn ink() -> CustomTheme {
    make(
        "dark",
        "ink",
        "墨",
        &[
            ("bg", "#14171c"),
            ("bgElevated", "#1a1e25"),
            ("bgSunken", "#101317"),
            ("panel", "#1a1e25"),
            ("panelHover", "#222730"),
            ("sidebar", "#171a20"),
            ("sidebarActive", "#262c36"),
            ("border", "#2a3038"),
            ("borderStrong", "#3a424d"),
            ("text", "#e2e6ec"),
            ("textSecondary", "#a4adba"),
            ("textMuted", "#6f7885"),
            ("accent", "#7aa2f7"),
            ("accentText", "#0d1014"),
            ("accentSoft", "rgba(122,162,247,0.14)"),
            ("unreadDot", "#7dcfff"),
            ("star", "#e0af68"),
            ("danger", "#f7768e"),
            ("warning", "#e0af68"),
            ("success", "#9ece6a"),
            ("selection", "rgba(122,162,247,0.22)"),
            ("codeBg", "#101317"),
            ("quoteBar", "#3d4654"),
            ("shadow", "0 12px 32px rgba(0,0,0,0.45)"),
            ("readerBg", "#171a20"),
        ],
    )
}

/// 纸：浅色。暖白底 + 冷灰文字，避免「白底黑字」的刺眼对比。
pub fn paper() -> CustomTheme {
    make(
        "light",
        "paper",
        "纸",
        &[
            ("bg", "#f7f6f3"),
            ("bgElevated", "#ffffff"),
            ("bgSunken", "#eeede9"),
            ("panel", "#ffffff"),
            ("panelHover", "#f2f1ed"),
            ("sidebar", "#f1f0ec"),
            ("sidebarActive", "#e6e4de"),
            ("border", "#dedbd4"),
            ("borderStrong", "#c6c2b9"),
            ("text", "#22262c"),
            ("textSecondary", "#5a616b"),
            ("textMuted", "#8a9099"),
            ("accent", "#2f6df6"),
            ("accentText", "#ffffff"),
            ("accentSoft", "rgba(47,109,246,0.10)"),
            ("unreadDot", "#2f6df6"),
            ("star", "#c98a18"),
            ("danger", "#cf3d4a"),
            ("warning", "#b8791a"),
            ("success", "#2f8a4a"),
            ("selection", "rgba(47,109,246,0.16)"),
            ("codeBg", "#f2f1ed"),
            ("quoteBar", "#c6c2b9"),
            ("shadow", "0 10px 28px rgba(30,34,40,0.10)"),
            ("readerBg", "#fbfaf7"),
        ],
    )
}

pub fn builtin_themes() -> Vec<CustomTheme> {
    vec![ink(), paper()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_themes_define_every_token() {
        for theme in builtin_themes() {
            for token in TOKENS {
                let value = theme.tokens.get(*token).cloned().unwrap_or_default();
                assert!(!value.is_empty(), "主题 {} 缺少 token {token}", theme.id);
            }
        }
    }

    #[test]
    fn modes_are_distinct() {
        let themes = builtin_themes();
        assert_eq!(themes.len(), 2);
        assert_ne!(themes[0].mode, themes[1].mode);
    }
}
