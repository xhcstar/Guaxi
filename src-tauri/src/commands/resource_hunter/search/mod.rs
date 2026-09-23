//! 搜索聚合层：全网发现（多引擎 + 嗅探）+ 富集站精确源，归一去重后按相关度排序。
use super::model::{ProxyConfig, ResourceItem, ResourceKind, SearchQuery};
use super::store;
use futures_util::future::join_all;
use sha2::{Digest, Sha256};

pub mod engines;
pub mod seed_sites;
pub mod sniffer;

/// 供子模块复用的代理配置读取。
pub(crate) fn load_proxy() -> ProxyConfig {
    store::get_setting("proxy")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 稳定 id：source_url 的 sha256 前 16 位，用于去重。
pub(crate) fn make_id(src: &str) -> String {
    let mut h = Sha256::new();
    h.update(src.as_bytes());
    let d = h.finalize();
    hex::encode(&d[..8])
}

fn all_kinds() -> Vec<ResourceKind> {
    vec![
        ResourceKind::Paper,
        ResourceKind::Book,
        ResourceKind::Video,
        ResourceKind::Audio,
        ResourceKind::Image,
    ]
}

/// 依资源类型构造搜索算子（dork）查询串。
fn build_queries(keyword: &str, kinds: &[ResourceKind]) -> Vec<String> {
    let mut qs: Vec<String> = Vec::new();
    let mut plain_needed = kinds.is_empty();
    for k in kinds {
        match k {
            ResourceKind::Paper => qs.push(format!("{keyword} filetype:pdf")),
            ResourceKind::Book => {
                qs.push(format!("{keyword} filetype:pdf"));
                qs.push(format!("{keyword} filetype:epub"));
            }
            ResourceKind::Audio => qs.push(format!("{keyword} filetype:mp3")),
            ResourceKind::Image => qs.push(format!("{keyword} filetype:jpg")),
            ResourceKind::Video | ResourceKind::Other => plain_needed = true,
        }
    }
    if plain_needed {
        qs.push(keyword.to_string());
    }
    qs.sort();
    qs.dedup();
    qs
}

/// 简单相关度：关键词分词在标题中的命中比例，与来源基准分融合。
fn score(item: &ResourceItem, keyword: &str) -> f32 {
    let title = item.title.to_lowercase();
    let toks: Vec<&str> = keyword
        .split_whitespace()
        .flat_map(|s| s.split(['，', ',', '、']))
        .filter(|s| !s.is_empty())
        .collect();
    if toks.is_empty() {
        return item.relevance;
    }
    let hit = toks.iter().filter(|t| title.contains(&t.to_lowercase())).count();
    let overlap = hit as f32 / toks.len() as f32;
    (item.relevance * 0.4 + overlap * 0.6).min(1.0)
}

/// 执行一次检索。
pub async fn run(query: SearchQuery) -> Vec<ResourceItem> {
    let keyword = query.keyword.trim().to_string();
    if keyword.is_empty() {
        return vec![];
    }
    let _ = store::record_query(&keyword);
    let kinds = if query.kinds.is_empty() {
        all_kinds()
    } else {
        query.kinds.clone()
    };
    let want_paper = kinds.contains(&ResourceKind::Paper) || query.kinds.is_empty();

    let mut items: Vec<ResourceItem> = Vec::new();

    // 富集站精确源（论文）。
    if query.seed_sites && want_paper {
        let (a, o) = tokio::join!(
            seed_sites::arxiv(&keyword, query.limit),
            seed_sites::openalex(&keyword, query.limit)
        );
        items.extend(a);
        items.extend(o);
    }

    // 通用全网发现：多引擎 × dork → 候选页 → 嗅探。
    if query.web_discovery {
        let engines = if query.engines.is_empty() {
            engines::known_engines().iter().map(|s| s.to_string()).collect()
        } else {
            query.engines.clone()
        };
        let queries = build_queries(&keyword, &kinds);
        let mut hit_futs = Vec::new();
        for eng in &engines {
            for q in &queries {
                hit_futs.push(engines::search(eng, q, 6));
            }
        }
        let hit_groups = join_all(hit_futs).await;
        // 汇总候选页 URL，去重后取前 N 个嗅探。
        let mut seen_url = std::collections::HashSet::new();
        let mut hits = Vec::new();
        for group in hit_groups {
            for h in group {
                if seen_url.insert(h.url.clone()) {
                    hits.push(h);
                }
            }
        }
        hits.truncate(16);
        let sniff_futs = hits
            .iter()
            .map(|h| sniffer::sniff(&h.url, &h.title, &kinds));
        for group in join_all(sniff_futs).await {
            items.extend(group);
        }
    }

    // 去重 + 打分排序。
    let mut seen = std::collections::HashSet::new();
    items.retain(|it| seen.insert(it.id.clone()));
    for it in items.iter_mut() {
        it.relevance = score(it, &keyword);
    }
    items.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
    items.truncate(query.limit.max(10) * kinds.len().max(1));
    items
}
