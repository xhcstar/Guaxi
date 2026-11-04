//! 内容质量分：把"这篇值不值得看/导出"变成一个可解释的数字。
//!
//! 为什么需要：现在的 `relevance_score` 只有一个笼统的数字，而且因为没有
//! 兴趣画像永远是 -1。真正需要的是一组分项，既能排序，又能在调参时
//! 直接看出"是正文太短，还是链接太多，还是元数据缺失"。
//!
//! 设计原则：
//! - **分项可解释**：每项 0–100，按权重加权；同时保留原始分项供诊断。
//! - **不追求语义理解**：这是廉价的结构性打分，用来做初步排序与分流；
//!   语义相关性仍然交给 `local_relevance_score` / LLM。
//! - **宁可低估也不要高估**：质量分驱动"要不要展示/导出"，高估让垃圾混进来，
//!   低估只是暂时埋没好文。

/// 质量分各分项的原始值（已归一到 0–100）。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct QualityParts {
    /// 正文完整度：字符数、段落数
    pub completeness: u8,
    /// 噪声比：链接密度、剔除比例（100 = 极干净）
    pub cleanliness: u8,
    /// 元数据完整：标题/作者/时间/语言
    pub metadata: u8,
    /// 标题与正文的一致度
    pub title_body: u8,
}

/// 打分结果。
#[derive(Debug, Clone)]
pub struct QualityScore {
    /// 加权总分 0–100
    pub total: u8,
    pub parts: QualityParts,
}

/// 打分输入。刻意做成纯数据，便于单测与后续调整。
#[derive(Debug, Clone, Default)]
pub struct QualityInput<'a> {
    pub title: &'a str,
    pub body_text: &'a str,
    pub author: Option<&'a str>,
    pub published: Option<&'a str>,
    pub lang: Option<&'a str>,
    /// 正文里的 Markdown 链接数
    pub link_count: usize,
    /// 清洗阶段剔除的块数
    pub removed_blocks: usize,
}

/// 各分项权重，合计 100。
const W_COMPLETENESS: f32 = 40.0;
const W_CLEANLINESS: f32 = 30.0;
const W_METADATA: f32 = 15.0;
const W_TITLE_BODY: f32 = 15.0;

/// 计算质量分。
pub fn score(input: &QualityInput) -> QualityScore {
    let parts = QualityParts {
        completeness: score_completeness(input.body_text),
        cleanliness: score_cleanliness(input),
        metadata: score_metadata(input),
        title_body: score_title_body(input.title, input.body_text),
    };

    let total = (parts.completeness as f32 * W_COMPLETENESS
        + parts.cleanliness as f32 * W_CLEANLINESS
        + parts.metadata as f32 * W_METADATA
        + parts.title_body as f32 * W_TITLE_BODY)
        / 100.0;

    QualityScore {
        total: total.round().clamp(0.0, 100.0) as u8,
        parts,
    }
}

/// 结合页面类型调整质量分。
///
/// 为什么必须分开：质量分是**结构**打分——正文长、链接少、元数据全，
/// 促销页完全满足这些条件（实测 Wired 的 Promo Code 页正文 1 万+ 字，
/// 结构分 96，和深度文章并列第一）。识别"这是不是一篇文章"是
/// [`crate::commands::flipboard_pagekind`] 的职责，不是结构分的。
///
/// 所以两处结果必须联动：非文章类页面，无论结构多"完整"，都不该得到高分。
pub fn adjust_for_page_kind(
    score: &QualityScore,
    kind: crate::commands::flipboard_pagekind::PageKind,
) -> QualityScore {
    use crate::commands::flipboard_pagekind::PageKind;

    // 非正文类型：直接压到低分区，避免进入"高质量"筛选。
    let cap = match kind {
        PageKind::Article | PageKind::Forum => {
            return score.clone(); // 正常文章不调整
        }
        PageKind::Unknown => {
            // 无法判定：轻微折价，因为它至少可能是个普通页面。
            85
        }
        PageKind::Index => 25,
        PageKind::Product => 15,
        PageKind::Video => 35,
        PageKind::LoginWall => 5,
        PageKind::Error => 0,
    };

    QualityScore {
        total: score.total.min(cap),
        parts: score.parts.clone(),
    }
}

/// 正文完整度：以 800 字为"满分区"，但跳过了阶梯惩罚——
/// 400 字和 790 字的差别没那么重要。
fn score_completeness(text: &str) -> u8 {
    let chars = text.chars().count();
    if chars == 0 {
        return 0;
    }
    // 200 字以下线性到 40 分，之后到 800 字线性到 100 分。
    let score = if chars < 200 {
        (chars as f32 / 200.0) * 40.0
    } else if chars < 800 {
        40.0 + ((chars - 200) as f32 / 600.0) * 60.0
    } else {
        100.0
    };
    score.round() as u8
}

/// 干净度：链接密度 + 剔除块占比。
fn score_cleanliness(input: &QualityInput) -> u8 {
    let chars = input.body_text.chars().count().max(1);
    // 每 100 字里的链接数。正常文章通常 < 1。
    let links_per_100 = input.link_count as f32 / (chars as f32 / 100.0);
    let link_score = if links_per_100 <= 0.5 {
        100.0
    } else if links_per_100 <= 3.0 {
        100.0 - ((links_per_100 - 0.5) / 2.5) * 60.0
    } else {
        40.0_f32.max(0.0)
    };

    // 剔除比例：如果清洗掉了超过一半的块，说明原页面很可能不是文章。
    let total_blocks = input.removed_blocks + count_blocks(input.body_text);
    let removed_ratio = if total_blocks == 0 {
        0.0
    } else {
        input.removed_blocks as f32 / total_blocks as f32
    };
    let removal_score = if removed_ratio <= 0.2 {
        100.0
    } else if removed_ratio <= 0.6 {
        100.0 - ((removed_ratio - 0.2) / 0.4) * 50.0
    } else {
        50.0 - ((removed_ratio - 0.6) / 0.4) * 50.0
    };

    // 取两者较小值：任何一个信号很差都不该给高分。
    link_score.min(removal_score).round().clamp(0.0, 100.0) as u8
}

/// 元数据完整度：4 项各 25 分。
fn score_metadata(input: &QualityInput) -> u8 {
    let mut score = 0u32;
    if !input.title.trim().is_empty() && input.title.trim() != "无标题" {
        score += 25;
    }
    if input.author.map(|a| !a.trim().is_empty()).unwrap_or(false) {
        score += 25;
    }
    if input.published.map(|p| !p.trim().is_empty()).unwrap_or(false) {
        score += 25;
    }
    if input.lang.map(|l| !l.trim().is_empty()).unwrap_or(false) {
        score += 25;
    }
    score as u8
}

/// 标题与正文一致度：标题里的实词有多少出现在正文中。
///
/// 目的：识别"标题说一件事、正文说另一件事"的页面（错配页、被劫持的 URL、
/// 或者正文抽取抽错了容器）。
fn score_title_body(title: &str, body: &str) -> u8 {
    let tokens = title_tokens(title);
    if tokens.is_empty() {
        return 50; // 标题太短/无实词，给中性分而非 0
    }
    let lower_body = body.to_lowercase();
    let hits = tokens.iter().filter(|t| lower_body.contains(*t)).count();
    let ratio = hits as f32 / tokens.len() as f32;
    // 覆盖率越高越好；但 100% 也不现实（正文用同义改写很常见），
    // 所以 0.6 以上就接近满分。
    (ratio / 0.6).min(1.0).mul_add(100.0, 0.0).round() as u8
}

/// 从标题里抽出可比的实词。
///
/// 中文没有空格，不能按分隔符切词——否则整个标题会变成一个 token，
/// 和正文永远匹配不上。这里对中文取**二元组（bigram）**：
/// "量子计算与纠错码" → 量子/子计/计算/算与/与纠/纠错/错码。
/// 这是无分词器依赖时最稳的做法（不必引入 jieba）。
fn title_tokens(title: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "a", "an", "of", "and", "or", "in", "on", "at", "to", "for", "with", "is", "are",
        "的", "了", "和", "与", "在", "是", "及",
    ];

    let mut tokens: Vec<String> = Vec::new();
    // 连续的中文串与连续的 ASCII 词分别累积。
    let mut cjk = String::new();
    let mut ascii = String::new();

    let flush_cjk = |run: &mut String, out: &mut Vec<String>| {
        let chars: Vec<char> = run.chars().collect();
        match chars.len() {
            0 => {}
            1 => out.push(chars[0].to_string()),
            _ => {
                for w in chars.windows(2) {
                    out.push(w.iter().collect());
                }
                out.push(run.clone());
            }
        }
        run.clear();
    };

    for c in title.chars() {
        if is_cjk(c) {
            if !ascii.is_empty() {
                if ascii.chars().count() >= 3 {
                    tokens.push(std::mem::take(&mut ascii));
                } else {
                    ascii.clear();
                }
            }
            cjk.push(c);
        } else if c.is_ascii_alphanumeric() {
            if !cjk.is_empty() {
                flush_cjk(&mut cjk, &mut tokens);
            }
            ascii.push(c.to_ascii_lowercase());
        } else {
            // 标点/空白：结束两侧的词
            if !cjk.is_empty() {
                flush_cjk(&mut cjk, &mut tokens);
            }
            if ascii.chars().count() >= 3 {
                tokens.push(std::mem::take(&mut ascii));
            } else {
                ascii.clear();
            }
        }
    }
    if !cjk.is_empty() {
        flush_cjk(&mut cjk, &mut tokens);
    }
    if ascii.chars().count() >= 3 {
        tokens.push(ascii);
    }

    tokens
        .into_iter()
        .filter(|s| !STOP.contains(&s.as_str()))
        .collect()
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF)
}

fn count_blocks(text: &str) -> usize {
    text.split("\n\n").filter(|b| !b.trim().is_empty()).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(chars: usize) -> String {
        "文".repeat(chars)
    }

    #[test]
    fn short_body_scores_low() {
        let input = QualityInput {
            title: "某篇文章",
            body_text: &body(50),
            ..Default::default()
        };
        assert!(score(&input).total < 40, "50-char body must score low");
    }

    #[test]
    fn full_body_with_metadata_scores_high() {
        let b = body(1200);
        let input = QualityInput {
            title: "深入理解数据库索引",
            body_text: &b,
            author: Some("张三"),
            published: Some("2026-01-01"),
            lang: Some("zh"),
            link_count: 2,
            removed_blocks: 0,
        };
        let s = score(&input);
        assert!(s.total >= 70, "well-formed article should score >= 70, got {}", s.total);
        assert_eq!(s.parts.metadata, 100);
    }

    #[test]
    fn link_farm_scores_low_cleanliness() {
        let mut b = body(1000);
        // 100 个字 5 个链接 = 链接密度 5/100，超过阈值
        for _ in 0..50 {
            b.push_str(" [链接](https://x)");
        }
        let input = QualityInput {
            title: "聚合页",
            body_text: &b,
            link_count: 50,
            ..Default::default()
        };
        assert!(score(&input).parts.cleanliness < 50, "link-heavy body must be penalized");
    }

    #[test]
    fn heavy_cleaning_lowers_cleanliness() {
        let input = QualityInput {
            title: "标题",
            body_text: "只有一段正文。",
            removed_blocks: 9,
            ..Default::default()
        };
        // 剔除 9 块、留 1 块 → 剔除比例 0.9
        assert!(score(&input).parts.cleanliness < 50);
    }

    #[test]
    fn mismatched_title_scores_low() {
        let b = "这篇正文完全在讨论另一个话题，与标题毫无关系。".repeat(20);
        let input = QualityInput {
            title: "量子计算与纠错码",
            body_text: &b,
            ..Default::default()
        };
        assert!(score(&input).parts.title_body < 50, "title-body mismatch must be detected");
    }

    #[test]
    fn matching_title_scores_high() {
        let b = "量子计算的关键挑战是纠错码。纠错码让量子计算可用。".repeat(20);
        let input = QualityInput {
            title: "量子计算与纠错码",
            body_text: &b,
            ..Default::default()
        };
        assert!(score(&input).parts.title_body > 80);
    }

    #[test]
    fn empty_body_scores_zero() {
        let input = QualityInput { title: "x", body_text: "", ..Default::default() };
        assert_eq!(score(&input).parts.completeness, 0);
    }

    #[test]
    fn promo_page_with_long_body_is_capped_by_page_kind() {
        // 实测问题：Wired 的 Promo Code 页正文 1 万+ 字，结构分 96，
        // 与真正的深度文章并列第一。页面类型判定必须把它压下去。
        use crate::commands::flipboard_pagekind::PageKind;
        let b = "Uber Eats 促销内容与优惠码说明。".repeat(2000);
        let input = QualityInput {
            title: "Uber Eats Promo Codes: $15 Off",
            body_text: &b,
            author: Some("x"),
            published: Some("2026-01-01"),
            lang: Some("en"),
            link_count: 2,
            removed_blocks: 0,
        };
        let raw = score(&input);
        assert!(raw.total >= 70, "structure score alone is high, got {}", raw.total);

        let adjusted = adjust_for_page_kind(&raw, PageKind::Product);
        assert!(
            adjusted.total <= 15,
            "product page must be capped, got {}",
            adjusted.total
        );
    }

    #[test]
    fn article_page_is_not_adjusted() {
        use crate::commands::flipboard_pagekind::PageKind;
        let b = "正文内容".repeat(2000);
        let input = QualityInput {
            title: "深入理解分布式共识",
            body_text: &b,
            author: Some("x"),
            published: Some("2026-01-01"),
            lang: Some("zh"),
            link_count: 2,
            removed_blocks: 0,
        };
        let raw = score(&input);
        let adjusted = adjust_for_page_kind(&raw, PageKind::Article);
        assert_eq!(adjusted.total, raw.total, "normal article must not be adjusted");
    }

    #[test]
    fn unknown_kind_is_slightly_discounted() {
        use crate::commands::flipboard_pagekind::PageKind;
        let input = QualityInput {
            title: "某个页面",
            body_text: &"内容".repeat(2000),
            author: Some("x"),
            published: Some("2026-01-01"),
            lang: Some("zh"),
            link_count: 1,
            removed_blocks: 0,
        };
        let raw = score(&input);
        let adjusted = adjust_for_page_kind(&raw, PageKind::Unknown);
        assert!(adjusted.total <= 85);
        assert!(adjusted.total >= raw.total.min(85));
    }
}
