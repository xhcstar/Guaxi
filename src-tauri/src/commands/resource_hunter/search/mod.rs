//! 搜索聚合层：领域 API + 全网多引擎嗅探 + 富集站，归一去重后按相关度排序。
use super::model::{ProxyConfig, ResourceItem, ResourceKind, SearchQuery};
use super::store;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use futures_util::future::join_all;
use sha2::{Digest, Sha256};

pub mod apis;
pub mod engines;
pub mod llm;
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
        ResourceKind::Code,
    ]
}

/// 各类型的资源富集站清单：喂给通用引擎做 `site:` 偏置，即便无专用 API 也能提高命中。
fn seed_domains(kind: ResourceKind) -> &'static [&'static str] {
    match kind {
        ResourceKind::Book => &["annas-archive.org", "libgen.is", "z-lib.io", "standardebooks.org"],
        ResourceKind::Paper => &["arxiv.org", "openalex.org", "sci-hub.se", "researchgate.net", "core.ac.uk"],
        ResourceKind::Video => &["youtube.com", "bilibili.com", "vimeo.com", "archive.org"],
        ResourceKind::Audio => &["bandcamp.com", "soundcloud.com", "freemusicarchive.org", "archive.org"],
        ResourceKind::Image => &["commons.wikimedia.org", "unsplash.com", "flickr.com", "pexels.com"],
        ResourceKind::Code => &["github.com", "gitlab.com", "gitee.com", "sourceforge.net"],
        ResourceKind::Other => &[],
    }
}

/// 依资源类型构造搜索算子（dork）查询串：filetype + site 偏置。
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
            ResourceKind::Video | ResourceKind::Code | ResourceKind::Other => plain_needed = true,
        }
        // site: 偏置（取每类前 2 个高产站，控制扇出）。
        for d in seed_domains(*k).iter().take(2) {
            qs.push(format!("{keyword} site:{d}"));
        }
    }
    if plain_needed {
        qs.push(keyword.to_string());
    }
    qs.sort();
    qs.dedup();
    qs
}

/// 相关度：fuzzy 匹配得分 + 来源基准分融合，比子串命中更稳。
fn score(matcher: &SkimMatcherV2, item: &ResourceItem, keyword: &str) -> f32 {
    if keyword.is_empty() {
        return item.relevance;
    }
    let fuzzy = matcher
        .fuzzy_match(&item.title, keyword)
        .map(|s| (s as f32 / (keyword.len() as f32 * 12.0)).min(1.0))
        .unwrap_or(0.0);
    (item.relevance * 0.45 + fuzzy * 0.55).min(1.0)
}
// PLACEHOLDER_RUN

/// 领域 API 精确源：按需要的类型并发查结构化接口。
async fn api_sources(kw: &str, kinds: &[ResourceKind], limit: usize) -> Vec<ResourceItem> {
    type BoxFut = std::pin::Pin<Box<dyn std::future::Future<Output = Vec<ResourceItem>> + Send>>;
    let mut futs: Vec<BoxFut> = Vec::new();
    let has = |k: ResourceKind| kinds.contains(&k);
    if has(ResourceKind::Paper) {
        let k = kw.to_string();
        futs.push(Box::pin(async move { seed_sites::arxiv(&k, limit).await }));
        let k = kw.to_string();
        futs.push(Box::pin(async move { seed_sites::openalex(&k, limit).await }));
        let k = kw.to_string();
        futs.push(Box::pin(async move { apis::semantic_scholar(&k, limit).await }));
    }
    if has(ResourceKind::Code) {
        let k = kw.to_string();
        futs.push(Box::pin(async move { apis::github(&k, limit).await }));
    }
    if has(ResourceKind::Book) {
        let k = kw.to_string();
        futs.push(Box::pin(async move { apis::gutendex(&k, limit).await }));
        let k = kw.to_string();
        futs.push(Box::pin(async move { apis::libgen(&k, limit).await }));
    }
    if has(ResourceKind::Image) {
        let k = kw.to_string();
        futs.push(Box::pin(async move { apis::openverse(&k, "images", limit).await }));
    }
    if has(ResourceKind::Audio) {
        let k = kw.to_string();
        futs.push(Box::pin(async move { apis::openverse(&k, "audio", limit).await }));
        let k = kw.to_string();
        futs.push(Box::pin(async move { apis::ytsearch(&k, "audio", limit).await }));
    }
    if has(ResourceKind::Video) {
        let k = kw.to_string();
        futs.push(Box::pin(async move { apis::ytsearch(&k, "video", limit).await }));
    }
    join_all(futs).await.into_iter().flatten().collect()
}

/// 执行一次检索。
pub async fn run(query: SearchQuery) -> Vec<ResourceItem> {
    let keyword = query.keyword.trim().to_string();
    if keyword.is_empty() {
        return vec![];
    }
    let _ = store::record_query(&keyword);
    let cfg = load_proxy();
    let kinds = if query.kinds.is_empty() { all_kinds() } else { query.kinds.clone() };

    let mut items: Vec<ResourceItem> = Vec::new();

    // 1) 领域 API 精确源（结构化直链，精度最高）。
    if query.seed_sites {
        items.extend(api_sources(&keyword, &kinds, query.limit).await);
    }

    // 2) 通用全网发现：多引擎 ×（dork + LLM 扩展）→ 候选页 → 嗅探。
    if query.web_discovery {
        let engines: Vec<String> = if query.engines.is_empty() {
            engines::known_engines().iter().map(|s| s.to_string()).collect()
        } else {
            query.engines.clone()
        };
        // LLM 查询扩展（可选）：把同义/中英/相关术语也拿去搜，提升召回。
        let mut keywords = vec![keyword.clone()];
        if query.use_llm {
            for ex in llm::expand_query(&keyword).await.into_iter().take(3) {
                keywords.push(ex);
            }
        }
        let mut queries = Vec::new();
        for kw in &keywords {
            queries.extend(build_queries(kw, &kinds));
        }
        queries.sort();
        queries.dedup();
        queries.truncate(24);

        let mut hit_futs = Vec::new();
        for eng in &engines {
            for q in &queries {
                hit_futs.push(engines::search(&cfg, eng, q, 8));
            }
        }
        let mut seen_url = std::collections::HashSet::new();
        let mut hits = Vec::new();
        for group in join_all(hit_futs).await {
            for h in group {
                if seen_url.insert(h.url.clone()) {
                    hits.push(h);
                }
            }
        }
        hits.truncate(48);
        let sniff_futs = hits.iter().map(|h| sniffer::sniff(&h.url, &h.title, &kinds));
        for group in join_all(sniff_futs).await {
            items.extend(group);
        }
    }

    // 3) 去重 → HEAD 死链校验 → fuzzy 打分排序。
    let matcher = SkimMatcherV2::default();
    let mut seen = std::collections::HashSet::new();
    items.retain(|it| seen.insert(it.id.clone()));
    let mut items = sniffer::verify(items).await;
    for it in items.iter_mut() {
        it.relevance = score(&matcher, it, &keyword);
    }
    items.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
    items.truncate(query.limit.max(20) * kinds.len().max(1));
    items
}
