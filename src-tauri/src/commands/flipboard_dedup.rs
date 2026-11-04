//! 去重与合并：识别"同一篇文章被多个源/多种 URL 抓到"。
//!
//! 现在的 `dedup_articles_by_identity` 只做 `url|title` 全等比对，
//! 跨源转载（标题稍改、URL 不同）完全合并不了，导致同一篇内容在列表里
//! 出现多次、在训练语料里重复计数。
//!
//! 两级判定：
//! 1. **URL 规范化后相等** → 同一篇（便宜、无歧义）。
//! 2. **正文 simhash 汉明距离 ≤ 阈值** → 判为同文（转载、镜像站）。
//!
//! 为什么用 simhash 而不是精确哈希：转载常改标题、加导语、换图片，
//! 精确哈希完全失效；simhash 对少量文本改动稳健。

/// 64 位 simhash。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SimHash(pub u64);

/// 汉明距离阈值。64 位下：
/// - ≤ 2：几乎必然同文（同源重复抓取、纯 URL 变体）
/// - 3–4：高度相似（转载 + 极少量改动，如加个"【转载】"前缀）
/// - 5–8：结构相似但内容不同的文章
/// - > 10：无关
///
/// 取 4 是实测校准的结果：阈值为 6 时，把两篇**不同的**「早报」聚合贴判成了
/// 重复（它们的排版、栏目名高度相似，但报道的新闻完全不同）。聚合贴在
/// Phase 0 已被索引规则拒收，但这个案例说明"结构模板相似"会被 simhash 放大。
/// 收紧到 4 后，只有真正的转载（正文几乎逐字相同）才会合并。
pub const HAMMING_THRESHOLD: u32 = 4;

impl SimHash {
    /// 两个 simhash 的汉明距离。
    pub fn distance(self, other: SimHash) -> u32 {
        (self.0 ^ other.0).count_ones()
    }

    /// 是否判为同一内容。
    pub fn is_duplicate_of(self, other: SimHash) -> bool {
        self.distance(other) <= HAMMING_THRESHOLD
    }
}

/// 计算正文的 simhash。
///
/// 做法：把文本切成 token（中文按二元组，英文按词），统计频次，
/// 每个 token 取哈希，按位加权求和，最后取符号位。
pub fn simhash(text: &str) -> SimHash {
    let tokens = tokenize(text);
    if tokens.is_empty() {
        return SimHash(0);
    }

    let mut counts: std::collections::HashMap<u64, i32> = std::collections::HashMap::new();
    for t in tokens {
        let h = hash_token(&t);
        *counts.entry(h).or_insert(0) += 1;
    }

    let mut weights = [0i64; 64];
    for (h, count) in counts {
        for (bit, w) in weights.iter_mut().enumerate() {
            if (h >> bit) & 1 == 1 {
                *w += count as i64;
            } else {
                *w -= count as i64;
            }
        }
    }

    let mut out = 0u64;
    for (bit, w) in weights.iter().enumerate() {
        if *w > 0 {
            out |= 1u64 << bit;
        }
    }
    SimHash(out)
}

/// 切 token：中文二元组、英文/数字词、去停用词与标点。
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cjk = String::new();
    let mut ascii = String::new();

    let flush_cjk = |run: &mut String, out: &mut Vec<String>| {
        let chars: Vec<char> = run.chars().collect();
        if chars.len() == 1 {
            out.push(chars[0].to_string());
        } else {
            for w in chars.windows(2) {
                out.push(w.iter().collect());
            }
        }
        run.clear();
    };
    let flush_ascii = |run: &mut String, out: &mut Vec<String>| {
        if run.chars().count() >= 2 {
            out.push(std::mem::take(run));
        } else {
            run.clear();
        }
    };

    for c in text.chars() {
        if is_cjk(c) {
            if !ascii.is_empty() {
                flush_ascii(&mut ascii, &mut out);
            }
            cjk.push(c);
        } else if c.is_ascii_alphanumeric() {
            if !cjk.is_empty() {
                flush_cjk(&mut cjk, &mut out);
            }
            ascii.push(c.to_ascii_lowercase());
        } else {
            if !cjk.is_empty() {
                flush_cjk(&mut cjk, &mut out);
            }
            if !ascii.is_empty() {
                flush_ascii(&mut ascii, &mut out);
            }
        }
    }
    if !cjk.is_empty() {
        flush_cjk(&mut cjk, &mut out);
    }
    if !ascii.is_empty() {
        flush_ascii(&mut ascii, &mut out);
    }
    out
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF)
}

fn hash_token(t: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// 一批内容 + 它们的 simhash，用于合并。
#[derive(Debug, Clone)]
pub struct DedupEntry {
    /// 规范化 URL（合并后的唯一标识由它决定）
    pub url_norm: String,
    pub title: String,
    pub simhash: SimHash,
}

/// 合并结果：每个簇的代表下标 + 被合并的下标。
#[derive(Debug, Clone)]
pub struct DedupCluster {
    /// 保留的代表条目下标（在输入数组里）
    pub keep: usize,
    /// 被合并掉的下标
    pub merged: Vec<usize>,
}

/// 对一批条目做去重分组。
///
/// 规则（按优先级）：
/// 1. 规范化 URL 相同 → 同簇
/// 2. 正文 simhash 距离 ≤ 阈值 → 同簇
///
/// 簇的代表选**正文最长的**（通常是完整版而非摘要版）。
pub fn cluster(entries: &[DedupEntry]) -> Vec<DedupCluster> {
    let mut used = vec![false; entries.len()];
    let mut clusters = Vec::new();

    for i in 0..entries.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut members = vec![i];

        for j in (i + 1)..entries.len() {
            if used[j] {
                continue;
            }
            let same_url = !entries[i].url_norm.is_empty()
                && entries[i].url_norm == entries[j].url_norm;
            let same_body = entries[i].simhash.0 != 0
                && entries[j].simhash.0 != 0
                && entries[i].simhash.is_duplicate_of(entries[j].simhash);
            if same_url || same_body {
                used[j] = true;
                members.push(j);
            }
        }

        // 代表：正文最长的那个。这里没有正文长度字段，用"simhash 非零 + 标题更长"近似。
        // 调用方可以在 entry 顺序上把完整版排在前面来影响结果，但更稳的是显式给长度。
        let keep = *members
            .iter()
            .max_by_key(|&&idx| entries[idx].title.chars().count())
            .unwrap_or(&i);
        let merged = members.iter().copied().filter(|&m| m != keep).collect();
        clusters.push(DedupCluster { keep, merged });
    }
    clusters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_has_zero_distance() {
        let a = simhash("这是一篇关于分布式系统的文章，讨论了共识算法与容错。");
        let b = simhash("这是一篇关于分布式系统的文章，讨论了共识算法与容错。");
        assert_eq!(a.distance(b), 0);
        assert!(a.is_duplicate_of(b));
    }

    #[test]
    fn slightly_modified_repost_is_duplicate() {
        let original = "这是一篇关于分布式系统的文章，讨论了共识算法、容错机制以及实际工程中的取舍。\
                        我们首先回顾 Paxos，然后讨论 Raft 的可理解性改进。".repeat(3);
        let repost = format!("【转载】{}", original);
        let a = simhash(&original);
        let b = simhash(&repost);
        assert!(
            a.is_duplicate_of(b),
            "repost with a prefix must still be duplicate, distance={}",
            a.distance(b)
        );
    }

    #[test]
    fn different_articles_are_not_duplicate() {
        let a = simhash(&"讨论数据库索引的结构，B+ 树与 LSM 树的差异，以及查询优化器的选择。".repeat(3));
        let b = simhash(&"介绍 Rust 的所有权模型，借用检查器如何在不使用垃圾回收的情况下保证内存安全。".repeat(3));
        assert!(!a.is_duplicate_of(b), "unrelated articles must not merge");
    }

    #[test]
    fn empty_text_yields_zero_hash() {
        assert_eq!(simhash("").0, 0);
        assert_eq!(simhash("   ").0, 0);
    }

    #[test]
    fn same_url_clusters_together() {
        let entries = vec![
            DedupEntry { url_norm: "https://x.com/a".into(), title: "标题".into(), simhash: SimHash(1) },
            DedupEntry { url_norm: "https://x.com/a".into(), title: "另一个标题".into(), simhash: SimHash(2) },
        ];
        let clusters = cluster(&entries);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].merged.len(), 1);
    }

    #[test]
    fn similar_body_clusters_across_sources() {
        let text = "这是一篇关于共识算法的长文，讨论 Paxos 与 Raft 的差异与工程取舍。".repeat(4);
        let h = simhash(&text);
        let entries = vec![
            DedupEntry { url_norm: "https://a.com/p1".into(), title: "共识算法的取舍".into(), simhash: h },
            DedupEntry { url_norm: "https://b.com/p9".into(), title: "共识算法的取舍（转载）".into(), simhash: h },
        ];
        let clusters = cluster(&entries);
        assert_eq!(clusters.len(), 1, "cross-source repost must merge");
    }

    #[test]
    fn structurally_similar_but_different_content_must_not_merge() {
        // 实测回归：两篇不同的「早报」聚合贴，栏目排版高度相似、报道的新闻不同，
        // 在阈值 6 时被误判为重复。收紧到 4 后必须分开。
        let a = "早报｜苹果第四财季营收增长12%，服务收入再创新高。曝下一代触屏 MacBook Pro 将换新模具。\
                 OpenAI 推出电网保护计划，拟让未来园区在用电紧张时降载。三星 Fold9 回归屏下摄像头。"
            .repeat(4);
        let b = "早报｜iPhone 国行 AI 官宣，千问接入 Siri。DeepSeek 千亿融资曝光。\
                 于东来建议支持「消费入座」。奇瑞回应智界独立，希望团队更像新势力。"
            .repeat(4);
        let ha = simhash(&a);
        let hb = simhash(&b);
        assert!(
            !ha.is_duplicate_of(hb),
            "different newsletters must not merge, distance={}",
            ha.distance(hb)
        );
    }

    #[test]
    fn short_different_text_is_not_merged() {
        let a = simhash("今天天气不错。");
        let b = simhash("明天可能下雨。");
        assert!(!a.is_duplicate_of(b), "short unrelated text must stay separate");
    }

    #[test]
    fn distinct_urls_and_bodies_stay_separate() {
        let entries = vec![
            DedupEntry { url_norm: "https://a.com/1".into(), title: "A".into(), simhash: simhash("完全不同的内容甲甲甲甲甲甲甲甲") },
            DedupEntry { url_norm: "https://a.com/2".into(), title: "B".into(), simhash: simhash("毫无关系的主题乙乙乙乙乙乙乙乙") },
        ];
        let clusters = cluster(&entries);
        assert_eq!(clusters.len(), 2);
    }
}
