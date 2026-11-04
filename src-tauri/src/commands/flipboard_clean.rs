//! 块级清洗：从抽取出的 Markdown 正文里剔除"模板段落"。
//!
//! 典型问题：爱范儿的每篇文章都以
//! `#欢迎关注爱范儿官方微信公众号：爱范儿（微信号：ifanr），更多精彩内容第一时间为您奉上。`
//! 结尾；老式站点的每页都有"点击展开全文""相关阅读""版权所有 © 2026"。
//! 这些**不是**文章的正文，但通用抽取器（trafilatura/scraper）分不清——它们在
//! 单篇文章内部看，没有"这个站每篇都有这句"的信息。
//!
//! 本模块的核心思路是**跨文章统计**：同一来源抓到的文章里，某个段落若在
//! 超过阈值比例的文章中重复出现，它就是模板而不是内容。这样不用为每个站
//! 硬编码规则，新源也能自动适配。
//!
//! 两级策略：
//! 1. **通用规则**（本模块内）：明显不属于正文的段落，单篇即可判（版权行、
//!    订阅引导、涉政免责声明等）。这些不需要统计。
//! 2. **学习规则**（`TemplateIndex`）：需要跨文章频次统计，由调用方维护。
//!
//! 单独成模块而不是塞进 `flipboard_extract`：抽取是"从 HTML 得到候选正文"，
//! 清洗是"从候选正文里去掉非内容"，两者失败原因与调参手段都不同。

use std::collections::HashMap;

/// 段落哈希后的频次表。键是规范化后的段落文本。
#[derive(Debug, Default, Clone)]
pub struct TemplateIndex {
    /// 段落 → 出现该段落的文章数
    doc_freq: HashMap<u64, u32>,
    /// 段落**尾部签名** → 出现该签名的文章数。
    ///
    /// 为什么需要它：站点模板常常整段不同、结尾相同。实测少数派的每篇文章都有
    /// `> <某个产品推荐>[链接]，少数派为你呈现 🚀`——产品名与链接每次不同，
    /// 精确整段比对永远命中不了，但结尾签名完全一致。
    suffix_freq: HashMap<u64, u32>,
    /// 该来源已统计的文章总数
    total_docs: u32,
    /// 判定为模板所需的出现比例（0.0–1.0）
    threshold: f32,
    /// 段落至少要被这么多篇文章重复才可能是模板（防小样本误判）
    min_docs: u32,
}

/// 尾部签名的长度（字符）。
///
/// 取 8：实测少数派结尾固定句是"少数派为你呈现"（7 字），后面跟 emoji。
/// 取 7–8 能对齐到固定句；取 10 就会把变动的产品名（"软件""产品""指南"）
/// 圈进签名，反而永远匹配不上。
///
/// **注意**：短签名单独使用时会误伤——不同正文"碰巧结尾相似"很常见。
/// 所以签名判定必须与长度约束配合（见 `SUFFIX_MAX_BLOCK_CHARS`），
/// 且仅在段落本身就是"推销/订阅句式"时才有意义。
const SUFFIX_SIGNATURE_CHARS: usize = 8;

/// 只有短块才按后缀签名判模板。
/// 站点推销/订阅类模板块通常很短（一句话 + 一个链接）。
const SUFFIX_MAX_BLOCK_CHARS: usize = 80;

/// 判定"这像是站点推销/订阅文案"的语义标记。
///
/// 后缀签名只能说明"结尾一样"，不能说明"这段不是正文"。两者叠加才可靠：
/// 真实正文即使结尾相同，也几乎不会同时命中这些营销词。
const PROMO_MARKERS: &[&str] = &[
    "为你呈现",
    "少数派",
    "关注我们",
    "关注公众号",
    "扫码",
    "订阅",
    "下载 app",
    "下载app",
    "点击下载",
    "立即购买",
    "购买链接",
    "优惠",
    "discount",
    "subscribe",
];

/// 段落是否带明显的推销/订阅语汇。
fn has_promo_marker(p: &str) -> bool {
    let lower = p.to_lowercase();
    PROMO_MARKERS.iter().any(|m| lower.contains(m))
}

impl TemplateIndex {
    /// 默认阈值：在 30% 的文章里出现即视为模板；至少 3 篇才判定。
    ///
    /// 30% 不是拍脑袋：正文段落很少在 30% 的文章里一字不差重复，
    /// 而站点模板几乎每篇都有。3 篇下限是为了防止"抓了 2 篇、2 篇都有某句"就误判。
    pub fn new() -> Self {
        TemplateIndex {
            doc_freq: HashMap::new(),
            suffix_freq: HashMap::new(),
            total_docs: 0,
            threshold: 0.30,
            min_docs: 3,
        }
    }

    /// 喂一篇正文的段落集合，更新频次。
    /// 应在**剔除之前**调用（先把这篇的段落计入统计，再清洗）。
    pub fn observe(&mut self, paragraphs: &[String]) {
        if paragraphs.is_empty() {
            return;
        }
        self.total_docs = self.total_docs.saturating_add(1);
        // 同一篇里重复出现的段落只记一次（否则列表页式的重复会被放大）。
        let mut seen = std::collections::HashSet::new();
        let mut seen_suffix = std::collections::HashSet::new();
        for p in paragraphs {
            let h = hash_paragraph(p);
            if seen.insert(h) {
                *self.doc_freq.entry(h).or_insert(0) += 1;
            }
            if let Some(s) = suffix_signature(p) {
                let sh = hash_of(&s);
                if seen_suffix.insert(sh) {
                    *self.suffix_freq.entry(sh).or_insert(0) += 1;
                }
            }
        }
    }

    /// 这段是否已被判定为模板（整段命中，或**尾部签名**命中）。
    pub fn is_template(&self, paragraph: &str) -> bool {
        if self.total_docs < self.min_docs {
            return false;
        }
        if let Some(&df) = self.doc_freq.get(&hash_paragraph(paragraph)) {
            if df >= self.min_docs && ratio(df, self.total_docs) >= self.threshold {
                return true;
            }
        }
        // 尾部签名：必须同时满足"短块 + 营销语汇 + 签名高频"。
        // 三个条件缺一不可——单靠签名会误删碰巧同结尾的正文（已实测到这个误伤）。
        if paragraph.chars().count() <= SUFFIX_MAX_BLOCK_CHARS && has_promo_marker(paragraph) {
            if let Some(sig) = suffix_signature(paragraph) {
                if let Some(&df) = self.suffix_freq.get(&hash_of(&sig)) {
                    if df >= self.min_docs && ratio(df, self.total_docs) >= self.threshold {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 从序列化状态恢复（供跨进程持久化）。
    #[allow(dead_code)]
    pub fn from_parts(
        doc_freq: HashMap<u64, u32>,
        suffix_freq: HashMap<u64, u32>,
        total_docs: u32,
        threshold: f32,
        min_docs: u32,
    ) -> Self {
        TemplateIndex {
            doc_freq,
            suffix_freq,
            total_docs,
            threshold,
            min_docs,
        }
    }

    /// 序列化/诊断访问器。目前只有 `to_json` 与单测在用，
    /// 保留是为了让持久化与调试不必直接碰私有字段。
    #[allow(dead_code, clippy::type_complexity)]
    pub fn parts(
        &self,
    ) -> (
        &HashMap<u64, u32>,
        &HashMap<u64, u32>,
        u32,
        f32,
        u32,
    ) {
        (&self.doc_freq, &self.suffix_freq, self.total_docs, self.threshold, self.min_docs)
    }

    #[allow(dead_code)]
    pub fn total_docs(&self) -> u32 {
        self.total_docs
    }

    /// 内部访问器：持久化时用。
    #[allow(dead_code)]
    pub(crate) fn suffix_freq(&self) -> &HashMap<u64, u32> {
        &self.suffix_freq
    }

    #[allow(dead_code)]
    pub(crate) fn doc_freq(&self) -> &HashMap<u64, u32> {
        &self.doc_freq
    }
}

fn ratio(a: u32, b: u32) -> f32 {
    if b == 0 {
        0.0
    } else {
        a as f32 / b as f32
    }
}

/// 段落尾部签名：去掉链接目标后取最后 N 个字符。
///
/// 去掉 `](url)` 里的 URL 很关键——否则"同一个模板 + 不同链接"永远算作不同签名。
fn suffix_signature(p: &str) -> Option<String> {
    let stripped = strip_links(p);
    let normalized = normalize_for_hash(&stripped);
    let chars: Vec<char> = normalized.chars().collect();
    // 太短的段落做签名没有区分度，容易误伤。
    if chars.len() < SUFFIX_SIGNATURE_CHARS * 2 {
        return None;
    }
    let tail: String = chars[chars.len() - SUFFIX_SIGNATURE_CHARS..].iter().collect();
    Some(tail)
}

/// 把 Markdown 链接 `[文本](目标)` 换成只保留文本，并去掉裸 URL。
fn strip_links(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    let bytes: Vec<char> = p.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '[' {
            // 找 ](  ... )
            if let Some(close) = bytes[i..].iter().position(|c| *c == ']') {
                let text_end = i + close;
                if text_end + 1 < bytes.len() && bytes[text_end + 1] == '(' {
                    if let Some(paren) = bytes[text_end + 2..].iter().position(|c| *c == ')') {
                        // 保留链接文本
                        out.extend(&bytes[i + 1..text_end]);
                        i = text_end + 2 + paren + 1;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    // 去掉裸 URL
    out.split_whitespace()
        .filter(|w| !w.starts_with("http://") && !w.starts_with("https://"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn hash_of(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// 段落哈希：规范化（压空白、小写）后取 u64，避免存储长文本。
fn hash_paragraph(p: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let normalized = normalize_for_hash(p);
    let mut h = DefaultHasher::new();
    normalized.hash(&mut h);
    h.finish()
}

/// 用于比对的双向规范化：压空白 + 小写。
/// 不做更激进的变换（如去标点），因为模板段落的标点通常也是固定的。
fn normalize_for_hash(p: &str) -> String {
    p.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// 通用噪声规则：单篇即可判定，不需要统计。
///
/// 分两档，因为它们的误杀风险不同：
/// - `STRONG_NOISE`：几乎只出现在页面 UI 里，正文极少这么说。中等段落也可判。
/// - `WEAK_NOISE`：正文也可能出现（"来源：""编辑：""免责声明："），
///   只有在**极短段落**里才可信——长段落里出现这些词往往是在讲这些事本身。
///
/// 宁可漏掉一些噪声，也不要把真正的正文段落删掉（误删不可恢复）。
const STRONG_NOISE: &[&str] = &[
    "点击展开全文",
    "点击查看全文",
    "展开阅读全文",
    "相关阅读",
    "推荐阅读",
    "延伸阅读",
    "热门推荐",
    "猜你喜欢",
    "扫一扫",
    "扫描二维码",
    "长按识别",
    "分享到",
    "分享至",
    "广告合作",
    "投稿邮箱",
    "商务合作",
    "合作请联系",
];

/// 弱信号：正文里也常见，只在极短段落里判定。
const WEAK_NOISE: &[&str] = &[
    "版权所有",
    "all rights reserved",
    "copyright ©",
    "转载请注明",
    "本文来自",
    "关注我们的公众号",
    "责任编辑：",
    "编辑：",
    "来源：",
    "原标题：",
    "免责声明：",
    "本文仅代表作者",
    "如有版权问题",
];

/// 弱信号生效的段落长度上限（字符数）。超过就不按弱信号判。
/// 25 字大致是"编辑：张三"这类页面标注的长度；再长就该当成正文。
const WEAK_NOISE_MAX_CHARS: usize = 25;

/// 整段就是这些词（可带冒号、可被 `**` 包裹）时，判为页面标签而不是正文。
/// 实测：少数派用 `**关联阅读：**` 单独一行引出链接列表，这行本身不是内容；
/// Wired 的促销页尾部有 `## FAQs` / `## Comments` / `Back to top`。
///
/// 必须**整段匹配**才判——"相关阅读"出现在一句话中间（如在讲推荐算法）时不能删。
const STANDALONE_LABELS: &[&str] = &[
    "关联阅读",
    "相关阅读",
    "推荐阅读",
    "延伸阅读",
    "参考资料",
    "参考文献",
    "参考链接",
    "拓展阅读",
    "阅读原文",
    "往期推荐",
    "热门文章",
    "更多精彩",
    "faq",
    "faqs",
    "comments",
    "back to top",
];

/// 判断一段是否是"整段就是一个页面标签"。
/// 容忍：首尾的 `**`/`__` 加粗标记、结尾的冒号（全角/半角）。
fn is_standalone_label(paragraph: &str) -> bool {
    let mut p = paragraph.trim();
    // 去掉 Markdown 加粗包裹：**文字：** / __文字__
    p = p.trim_matches('*').trim_matches('_').trim();
    // 去掉结尾冒号
    let p = p.trim_end_matches([':', '：', '.', '。']).trim().to_lowercase();
    if p.is_empty() {
        return false;
    }
    STANDALONE_LABELS.iter().any(|l| *l == p)
}

/// 强信号生效的段落长度上限。更长的段落里出现"相关阅读"多半是在讨论这件事。
const STRONG_NOISE_MAX_CHARS: usize = 120;

/// 只知道这些词还不够的模糊匹配：必须在短段落里出现才判。
const SHORT_NOISE_PREFIXES: &[&str] = &[
    "扫码登录",
    "请登录",
    "登录后",
    "关注公众号",
    "订阅",
    "分享本文",
];

/// 判断一个段落是否是通用噪声。
/// `paragraph` 是 Markdown 文本，调用方需保证不是标题/列表/代码块（那些不该按此规则删）。
pub fn is_generic_noise(paragraph: &str) -> bool {
    let p = paragraph.trim();
    if p.is_empty() {
        return true;
    }
    let lower = p.to_lowercase();
    let chars = p.chars().count();

    // 整段就是一个页面标签（"关联阅读：""FAQs""Back to top"）。
    if is_standalone_label(p) {
        return true;
    }

    if chars <= STRONG_NOISE_MAX_CHARS && STRONG_NOISE.iter().any(|n| lower.contains(n)) {
        return true;
    }
    if chars <= WEAK_NOISE_MAX_CHARS {
        if WEAK_NOISE.iter().any(|n| lower.contains(n)) {
            return true;
        }
        if SHORT_NOISE_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
        {
            return true;
        }
    }

    false
}

/// 清洗结果：保留的段落 + 被剔除的段落（供审计/调试）。
#[derive(Debug, Default, Clone)]
pub struct CleanResult {
    pub kept: Vec<String>,
    pub removed: Vec<String>,
}

/// 清洗一段 Markdown 正文。
///
/// 规则：
/// - 空行分隔的块逐个判定
/// - **标题、代码块**永远保留（它们是文章结构本身）
/// - **列表、引用、图片**参与模板判定：站点常把"相关阅读""关注我们"写成列表或引用，
///   若这些块跨文章重复出现，就是模板；但它们是结构块，不适用"段落太短=噪声"规则，
///   只按跨文章频次判定
/// - 普通段落：通用噪声规则 + 跨文章模板
///
/// 返回保留与被删的块，调用方按需拼回 Markdown 或记录审计。
pub fn clean_markdown(markdown: &str, templates: Option<&TemplateIndex>) -> CleanResult {
    // 先修链接缺陷：抽取器的这个 bug 与清洗规则无关，
    // 但它会让"保留下来的链接"变成死链，必须在分块前处理。
    let repaired = fix_nested_links(markdown);
    let mut result = CleanResult::default();

    for block in split_blocks(&repaired) {
        let trimmed = block.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 标题与代码块是文章结构，永不删除。
        if is_heading(trimmed) || is_code_fence(trimmed) {
            result.kept.push(trimmed.to_string());
            continue;
        }

        // 列表/引用/图片：只按跨文章模板判定，不适用"短段落噪声"规则。
        // 原因：正文里的列表项本来就短，按长度判会大面积误删。
        if is_list_or_quote(trimmed) {
            if let Some(idx) = templates {
                if idx.is_template(trimmed) {
                    result.removed.push(trimmed.to_string());
                    continue;
                }
            }
            result.kept.push(trimmed.to_string());
            continue;
        }

        if is_generic_noise(trimmed) {
            result.removed.push(trimmed.to_string());
            continue;
        }

        if let Some(idx) = templates {
            if idx.is_template(trimmed) {
                result.removed.push(trimmed.to_string());
                continue;
            }
        }

        result.kept.push(trimmed.to_string());
    }

    result
}

/// 按空行切分 Markdown 块。代码围栏内的空行不切分。
fn split_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut in_fence = false;

    for line in markdown.lines() {
        let is_fence = line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~");
        if is_fence {
            in_fence = !in_fence;
            current.push_str(line);
            current.push('\n');
            continue;
        }
        if in_fence {
            current.push_str(line);
            current.push('\n');
            continue;
        }
        if line.trim().is_empty() {
            if !current.trim().is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    if !current.trim().is_empty() {
        blocks.push(current);
    }
    blocks
}

/// 是否是标题块。
fn is_heading(block: &str) -> bool {
    block.trim_start().starts_with('#')
}

/// 是否是代码围栏。
fn is_code_fence(block: &str) -> bool {
    let t = block.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// 是否是列表 / 引用 / 图片 / 表格这类"结构块"。
/// 它们不适用"短段落 = 噪声"的规则（正文列表项本来就短），
/// 但仍可被跨文章模板统计判定为模板。
fn is_list_or_quote(block: &str) -> bool {
    let t = block.trim_start();
    t.starts_with("> ")
        || t.starts_with("- ")
        || t.starts_with("* ")
        || t.starts_with("+ ")
        || t.starts_with('|')
        || t.contains("\n- ")
        || t.contains("\n* ")
        || t.contains("\n> ")
        || t.contains("\n|")
        || t.starts_with("![")
        || t.starts_with("<img")
        || is_ordered_list(t)
}

/// 是否是"与正文无关的结构"（标题/代码），清洗时永不删。
fn is_structural(block: &str) -> bool {
    is_heading(block) || is_code_fence(block) || is_list_or_quote(block)
}

fn is_ordered_list(t: &str) -> bool {
    // "1. " / "12) " 这类有序列表项
    let mut chars = t.chars();
    let mut saw_digit = false;
    for c in chars.by_ref() {
        if c.is_ascii_digit() {
            saw_digit = true;
        } else {
            return saw_digit && (c == '.' || c == ')') && chars.next() == Some(' ');
        }
    }
    false
}

/// 修复 trafilatura 生成的"嵌套 URL"链接。
///
/// 实测缺陷：trafilatura 2.3 处理文档内相对链接时会产出
/// `[text](<https://site/(https:/site/path>)` —— 把解析后的绝对 URL 和
/// 原始相对路径拼在一起。约 0.6% 的链接受影响，但会让链接完全失效。
///
/// 修复策略：识别 `(https://host/(` 这种模式，把括号内的重复路径丢掉，
/// 保留外层的绝对 URL。识别不出来就原样返回——宁可不修，也不要改成错的。
pub fn fix_nested_links(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let chars: Vec<char> = markdown.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // 只处理 `](` 之后紧跟 `<http` 或 `http` 的情形（链接目标）。
        if chars[i] == ']' && i + 1 < chars.len() && chars[i + 1] == '(' {
            let start = i + 2;
            // 跳过可选的 '<'
            let url_start = if start < chars.len() && chars[start] == '<' {
                start + 1
            } else {
                start
            };
            // 找链接目标的结束：')' 或 '>' 或空白
            let mut end = url_start;
            while end < chars.len()
                && chars[end] != ')'
                && chars[end] != '>'
                && !chars[end].is_whitespace()
            {
                end += 1;
            }
            let target: String = chars[url_start..end].iter().collect();

            // 匹配 `https://host/(...` 的嵌套形态
            if let Some(pos) = target.find("/(") {
                if target.starts_with("http") {
                    let outer = &target[..pos];
                    out.push_str("](");
                    out.push_str(outer);
                    // 嵌套 URL 里本身可能还含 ')'（如 `.../rl-overview/`），
                    // 所以要按括号深度扫描，直到找到真正闭合 Markdown 链接的那一个。
                    // 同时跳过 `<...>` 包裹带来的多余 '>'。
                    let mut j = i + 2; // 跳过 "]("
                    let mut depth = 1i32;
                    while j < chars.len() {
                        match chars[j] {
                            '(' => depth += 1,
                            ')' => {
                                depth -= 1;
                                if depth == 0 {
                                    j += 1;
                                    break;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    out.push(')');
                    i = j;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// 把清洗后的段落拼回 Markdown（段落之间空行）。
pub fn join_blocks(blocks: &[String]) -> String {
    blocks.join("\n\n")
}

// ===== 模板索引的持久化 =====
//
// 索引必须跨进程存活：模板的学习是"跨文章统计"，如果每次启动都从零开始，
// 刚抓第一篇时永远判不出模板（total_docs < min_docs），而这个特性最大的价值
// 恰恰是"下次抓取时自动去掉上次学到的模板"。
//
// 存储格式用 JSON（serde_json 已是依赖），文件很小：每个源只存
// 段落哈希 → 频次 的映射，外加文章数。

const TEMPLATE_INDEX_VERSION: u32 = 2;

#[derive(serde::Serialize, serde::Deserialize)]
struct TemplateIndexFile {
    version: u32,
    total_docs: u32,
    threshold: f32,
    min_docs: u32,
    /// "hash" 作为 JSON 的键必须是字符串，u64 以十进制串存。
    doc_freq: std::collections::BTreeMap<String, u32>,
    #[serde(default)]
    suffix_freq: std::collections::BTreeMap<String, u32>,
}

impl TemplateIndex {
    /// 序列化为 JSON 字符串。
    pub fn to_json(&self) -> String {
        let file = TemplateIndexFile {
            version: TEMPLATE_INDEX_VERSION,
            total_docs: self.total_docs,
            threshold: self.threshold,
            min_docs: self.min_docs,
            doc_freq: self
                .doc_freq
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
            suffix_freq: self
                .suffix_freq
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
        };
        serde_json::to_string(&file).unwrap_or_default()
    }

    /// 从 JSON 恢复。版本不符或解析失败时返回空索引（不影响抓取，只是这次没有模板知识）。
    pub fn from_json(raw: &str) -> Self {
        let Ok(file) = serde_json::from_str::<TemplateIndexFile>(raw) else {
            return TemplateIndex::new();
        };
        if file.version != TEMPLATE_INDEX_VERSION {
            return TemplateIndex::new();
        }
        let to_map = |m: std::collections::BTreeMap<String, u32>| -> HashMap<u64, u32> {
            m.into_iter()
                .filter_map(|(k, v)| k.parse::<u64>().ok().map(|k| (k, v)))
                .collect()
        };
        TemplateIndex {
            doc_freq: to_map(file.doc_freq),
            suffix_freq: to_map(file.suffix_freq),
            total_docs: file.total_docs,
            threshold: file.threshold,
            min_docs: file.min_docs,
        }
    }

    /// 上限保护：频次表不应无限增长。超过上限时丢弃最低频的条目
    /// （它们本来也不可能达到模板阈值）。返回是否发生了裁剪。
    pub fn prune(&mut self, max_entries: usize) -> bool {
        let pruned_df = prune_map(&mut self.doc_freq, max_entries);
        let pruned_sfx = prune_map(&mut self.suffix_freq, max_entries);
        pruned_df || pruned_sfx
    }
}

/// 按频次从高到低保留前 `max_entries` 个。返回是否裁剪过。
fn prune_map(map: &mut HashMap<u64, u32>, max_entries: usize) -> bool {
    if map.len() <= max_entries {
        return false;
    }
    let mut entries: Vec<(u64, u32)> = map.iter().map(|(k, v)| (*k, *v)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.truncate(max_entries);
    *map = entries.into_iter().collect();
    true
}

/// 提取 Markdown 里"可参与模板统计"的段落（去掉结构块与通用噪声）。
/// 用于 `TemplateIndex::observe`，保证统计的是"内容段落"而非列表/标题。
pub fn content_paragraphs(markdown: &str) -> Vec<String> {
    split_blocks(markdown)
        .into_iter()
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty() && !is_structural(b) && !is_generic_noise(b))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_generic_noise() {
        let md = "这是正文第一段，讲的是真正的技术内容，足够长不会被误判。\n\n\
                  点击展开全文\n\n\
                  这是正文第二段，继续讨论实现细节与取舍。";
        let r = clean_markdown(md, None);
        assert_eq!(r.kept.len(), 2);
        assert_eq!(r.removed.len(), 1);
        assert!(r.removed[0].contains("点击展开全文"));
    }

    #[test]
    fn keeps_structural_blocks() {
        let md = "# 标题\n\n- 列表项\n- 另一项\n\n```rust\nlet x = 1;\n\nlet y = 2;\n```\n\n正常段落。";
        let r = clean_markdown(md, None);
        let joined = join_blocks(&r.kept);
        assert!(joined.contains("# 标题"));
        assert!(joined.contains("- 列表项"));
        assert!(joined.contains("let y = 2;")); // 代码块内的空行没切开
        assert!(r.removed.is_empty());
    }

    #[test]
    fn learns_templates_across_articles() {
        let boiler = "欢迎关注爱范儿官方微信公众号：爱范儿（微信号：ifanr），更多精彩内容第一时间为您奉上。";
        let mut idx = TemplateIndex::new();
        // 5 篇文章都有这句；正文各不相同
        for i in 0..5 {
            idx.observe(&[format!("这是第 {} 篇文章的正文段落，内容互不相同。", i), boiler.to_string()]);
        }
        assert!(idx.is_template(boiler), "boilerplate appearing in 100% of docs must be a template");
        assert!(!idx.is_template("这是第 0 篇文章的正文段落，内容互不相同。"));

        let md = format!("正文段落。\n\n{}", boiler);
        let r = clean_markdown(&md, Some(&idx));
        assert_eq!(r.kept.len(), 1);
        assert_eq!(r.removed.len(), 1);
    }

    #[test]
    fn small_samples_do_not_trigger_template() {
        let mut idx = TemplateIndex::new();
        // 只有 2 篇（< min_docs=3），即使 100% 出现也不判模板
        idx.observe(&["共同出现的一句话，但其实可能是正常重复内容。".to_string()]);
        idx.observe(&["共同出现的一句话，但其实可能是正常重复内容。".to_string()]);
        assert!(!idx.is_template("共同出现的一句话，但其实可能是正常重复内容。"));
    }

    #[test]
    fn long_paragraph_with_keyword_is_kept() {
        // 长段落里出现"来源："不应整段删掉（那多半是在讲新闻来源的真实正文）
        let long = "本文讨论了数据来源：公开财报与第三方统计，并对这些数据的口径差异做了详细说明，\
                    这部分内容很长，属于真正的正文，不应因为含有'来源：'三个字就被删掉。";
        assert!(!is_generic_noise(long));
    }

    #[test]
    fn removes_structural_boilerplate() {
        // 实测场景：少数派把"关注我们"写成引用块、"相关阅读"写成列表。
        // 这些是结构块，不能按"短段落噪声"删，但跨文章重复就该被识别为模板。
        let quote_boiler = "> 关注 [少数派小红书](https://example.com/u)，感受精彩数字生活 🍃";
        let list_boiler = "- [把喜欢的图烧进 CD 里](https://example.com/x) ：想了解数学原理可以阅读。";
        let mut idx = TemplateIndex::new();
        for i in 0..4 {
            idx.observe(&[
                format!("第 {} 篇的正文段落，内容各不相同，足够长。", i),
                quote_boiler.to_string(),
                list_boiler.to_string(),
            ]);
        }
        assert!(idx.is_template(quote_boiler), "quote-style boilerplate must be detected");
        assert!(idx.is_template(list_boiler), "list-style boilerplate must be detected");

        let md = format!(
            "# 标题\n\n真正的正文段落，讲的是实际内容。\n\n{}\n\n{}",
            quote_boiler, list_boiler
        );
        let r = clean_markdown(&md, Some(&idx));
        assert_eq!(r.kept.len(), 2, "heading + real paragraph only");
        assert_eq!(r.removed.len(), 2, "both boilerplate blocks removed");
    }

    #[test]
    fn required_reading_list_is_kept_when_not_repeated() {
        // 反向保护：文章里真正的"参考链接"列表只出现一次，不能被当模板删掉。
        let unique_list = "- [Attention Is All You Need](https://arxiv.org/abs/1706.03762) — 原始论文";
        let mut idx = TemplateIndex::new();
        for i in 0..5 {
            idx.observe(&[
                format!("正文 {}", i),
                "> 固定的公众号尾巴，每篇都有".to_string(),
            ]);
        }
        let md = format!("文章正文。\n\n{}", unique_list);
        let r = clean_markdown(&md, Some(&idx));
        assert_eq!(r.kept.len(), 2, "unique reference list must survive");
        assert!(r.removed.is_empty());
    }

    #[test]
    fn headings_never_removed() {
        // 即使某个标题跨文章重复，也不该删——标题是文章结构。
        let mut idx = TemplateIndex::new();
        for i in 0..5 {
            idx.observe(&["## 相关阅读".to_string(), format!("正文 {}", i)]);
        }
        let md = "## 相关阅读\n\n正文。";
        let r = clean_markdown(md, Some(&idx));
        assert_eq!(r.kept.len(), 2, "heading must be kept");
        assert!(r.removed.is_empty());
    }

    #[test]
    fn suffix_signature_catches_varying_boilerplate() {
        // 实测场景：少数派每篇的推销块产品与链接都不同，只有结尾相同。
        let variants = [
            "> 简单、好用、[专注的写作软件](https://sspai.com/item/49?source=abc)，少数派为你呈现 🚀",
            "> 也许你拆快递时需要一个轻巧、便携的[安全开箱刀](https://sspai.com/product/46)，少数派为你呈现 🚀",
            "> 实用、好用的 [正版软件](https://sspai.com/mall)，少数派为你呈现 🚀",
            "> 游戏、选购、验机、改装……用得上、学得到的 [中古掌机指南](https://sspai.com/series/422)，少数派为你呈现 🚀",
        ];
        let mut idx = TemplateIndex::new();
        for (i, v) in variants.iter().enumerate() {
            idx.observe(&[format!("第 {} 篇的正文段落，内容互不相同。", i), v.to_string()]);
        }
        // 每个具体变体只出现一次，但后缀签名 + 营销语汇足以识别——
        // 这正是"整段不同、结尾相同"场景需要的效果。
        assert!(
            idx.is_template(variants[1]),
            "varying paragraph sharing suffix + promo markers should be detected"
        );
        // 新的一篇变体同样能被识别
        let new_variant = "> 一本正经的 [某个新产品](https://sspai.com/product/999)，少数派为你呈现 🚀";
        assert!(
            idx.is_template(new_variant),
            "a new variant sharing suffix and promo markers must be detected"
        );
        // 关键反向保护：同样是"少数派"结尾的正文讨论，不应被删——
        // 它带有营销词但不在短块范围内，或语义上是正文。
        let prose = "少数派曾在一篇文章里讨论过，这类产品呈现出的体验差异往往来自更底层的原因，\
                     而不只是表面功能的多寡，这才是需要仔细分辨的地方。";
        assert!(!idx.is_template(prose), "prose mentioning the brand must survive");
    }

    #[test]
    fn suffix_signature_does_not_remove_real_article_ending() {
        // 反向保护：真实文章结尾不能因为"碰巧和别的文章结尾像"被删。
        let mut idx = TemplateIndex::new();
        for i in 0..5 {
            idx.observe(&[format!(
                "第 {} 篇文章的正文，讨论了完全不同的主题，最后总结：这才是关键所在。",
                i
            )]);
        }
        // 一篇新的、主题不同的文章，结尾恰好也是"这才是关键所在。"
        let real = "一篇讲数据库索引的文章，结论需要权衡读写性能，这才是关键所在。";
        assert!(
            !idx.is_template(real),
            "ending-only similarity without promo markers must not be treated as template"
        );
    }

    #[test]
    fn short_block_without_promo_marker_is_kept() {
        // 短块 + 相同结尾，但没有营销语汇 → 不删。
        let mut idx = TemplateIndex::new();
        for i in 0..5 {
            idx.observe(&[format!("正文段落 {}", i), "以上就是全部的要点说明。".to_string()]);
        }
        assert!(!idx.is_template("以下是补充的要点说明。"), "no promo marker -> keep");
    }

    #[test]
    fn fixes_trafilatura_nested_links() {
        // 实测的真实缺陷：相对链接被解析成了 "绝对URL + 原始相对路径"
        let broken = "[RL](<https://lilianweng.github.io/(https:/lilianweng.github.io/posts/2018-02-19-rl-overview/)>)";
        let fixed = fix_nested_links(broken);
        assert_eq!(fixed, "[RL](https://lilianweng.github.io)");
        assert!(!fixed.contains("/(https:"), "nested url must be removed: {}", fixed);
    }

    #[test]
    fn leaves_normal_links_untouched() {
        for md in [
            "[原文](https://example.com/post)",
            "[a](https://example.com/a/b/c)",
            "普通文本没有链接",
            "![图](https://cdn.example.com/x.png)",
        ] {
            assert_eq!(fix_nested_links(md), md, "must not touch: {}", md);
        }
    }

    #[test]
    fn clean_markdown_repairs_links_as_part_of_pipeline() {
        let md = "正文。[RL](<https://x.com/(https:/x.com/a/)>) 后面还有内容。";
        let r = clean_markdown(md, None);
        let joined = join_blocks(&r.kept);
        assert!(joined.contains("[RL](https://x.com)"));
        assert!(!joined.contains("/(https:"));
    }

    #[test]
    fn template_index_roundtrips_through_json() {
        let mut idx = TemplateIndex::new();
        for i in 0..4 {
            idx.observe(&[format!("正文 {}", i), "固定模板句子".to_string()]);
        }
        let json = idx.to_json();
        let restored = TemplateIndex::from_json(&json);
        assert_eq!(restored.total_docs(), 4);
        assert!(restored.is_template("固定模板句子"));
        assert!(!restored.is_template("正文 1"));
    }

    #[test]
    fn corrupt_json_yields_empty_index() {
        let idx = TemplateIndex::from_json("{not valid json");
        assert_eq!(idx.total_docs(), 0);
        assert!(!idx.is_template("任何东西"));
    }

    #[test]
    fn prune_keeps_most_frequent() {
        // 直接构造频次表：observe 按"文章"计数，用它构造极端分布很别扭，
        // 而 prune 的语义只是"保留频次最高的 N 个"，所以直接喂数据更清晰。
        let mut freq: HashMap<u64, u32> = HashMap::new();
        for i in 0..50u64 {
            // 前 10 个高频，其余只出现 1 次
            freq.insert(i, if i < 10 { 100 } else { 1 });
        }
        let mut idx = TemplateIndex::from_parts(freq, HashMap::new(), 100, 0.30, 3);
        assert!(idx.prune(10), "should have pruned");
        let (kept, _, ..) = idx.parts();
        assert_eq!(kept.len(), 10, "should keep exactly the limit");
        // 保留下来的必须是高频那 10 个
        for i in 0..10u64 {
            assert_eq!(kept.get(&i), Some(&100), "high-frequency entry {} must survive", i);
        }
    }

    #[test]
    fn prune_noop_below_limit() {
        let mut idx = TemplateIndex::new();
        idx.observe(&["a".to_string()]);
        assert!(!idx.prune(100), "nothing to prune below the limit");
    }
}
