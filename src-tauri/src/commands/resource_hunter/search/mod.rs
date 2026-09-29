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

/// 来源权威度：结构化 API > 富集站 > 通用 SERP 抓取。
fn source_authority(source: &str) -> f32 {
    match source {
        "arXiv" | "OpenAlex" | "Crossref" | "DOAJ" | "SemanticScholar"
        | "Unpaywall" | "EuropePMC" | "CORE" => 0.95,
        "GitHub" | "GitLab" | "crates.io" | "npm" => 0.92,
        "Gutenberg" | "OpenLibrary" | "DOAB" => 0.86,
        "Wikimedia" | "Openverse" | "InternetArchive" | "MetMuseum" | "NASA"
        | "LibriVox" | "Pexels" | "Jamendo" => 0.82,
        "LibGen" => 0.78,
        "百度图片" | "YouTube" | "Bilibili" => 0.74,
        "web" => 0.48,
        _ => 0.6,
    }
}

/// 元数据丰富度：作者/年份/摘要/大小/缩略图齐全的条目更可信、更值得排前。
fn metadata_richness(item: &ResourceItem) -> f32 {
    let mut n = 0.0f32;
    if item.author.is_some() { n += 1.0; }
    if item.published_at.is_some() { n += 1.0; }
    if item.summary.is_some() { n += 1.0; }
    if item.size_hint.is_some() { n += 1.0; }
    if item.thumbnail.is_some() { n += 1.0; }
    (n / 5.0).min(1.0)
}

/// 工业级多特征相关度：标题匹配 × 来源权威 × 可下载性 × 元数据 × 来源先验。
/// 各特征独立归一后加权融合，比单一子串/模糊匹配稳得多。
fn score(matcher: &SkimMatcherV2, item: &ResourceItem, keyword: &str) -> f32 {
    // 标题匹配：优先精确子串命中，其次模糊匹配。
    let title_lc = item.title.to_lowercase();
    let kw_lc = keyword.to_lowercase();
    let title = if keyword.is_empty() {
        0.6
    } else if title_lc.contains(&kw_lc) {
        1.0
    } else {
        matcher
            .fuzzy_match(&item.title, keyword)
            .map(|s| (s as f32 / (keyword.len() as f32 * 12.0)).min(1.0))
            .unwrap_or(0.0)
    };
    // 可下载性：有直链/磁力/可解析视频页 > 仅有详情页。
    let concrete = if item.download_url.is_some() || item.magnet.is_some() || item.needs_ytdlp {
        1.0
    } else {
        0.35
    };
    let auth = source_authority(&item.source);
    let meta = metadata_richness(item);
    let prior = item.relevance; // 各源给出的先验分
    (title * 0.42 + auth * 0.20 + concrete * 0.18 + meta * 0.08 + prior * 0.12).min(1.0)
}

/// 内建双语/同义词种子表：LLM 不可用时也能扩召回（非穷举，覆盖常见检索词）。
fn offline_expand(keyword: &str) -> Vec<String> {
    const SEED: &[(&str, &[&str])] = &[
        ("机器学习", &["machine learning"]),
        ("深度学习", &["deep learning"]),
        ("神经网络", &["neural network"]),
        ("强化学习", &["reinforcement learning"]),
        ("自然语言处理", &["natural language processing", "NLP"]),
        ("计算机视觉", &["computer vision"]),
        ("大语言模型", &["large language model", "LLM"]),
        ("数据结构", &["data structures"]),
        ("算法", &["algorithms"]),
        ("操作系统", &["operating systems"]),
        ("编译原理", &["compiler"]),
        ("量子计算", &["quantum computing"]),
        ("区块链", &["blockchain"]),
        ("密码学", &["cryptography"]),
        ("生物信息学", &["bioinformatics"]),
        ("数据库", &["database systems"]),
        ("计算机网络", &["computer networks"]),
        ("分布式系统", &["distributed systems"]),
        ("图神经网络", &["graph neural network", "GNN"]),
        ("生成对抗网络", &["generative adversarial network", "GAN"]),
        ("扩散模型", &["diffusion model"]),
        ("Transformer", &["transformer 模型", "注意力机制"]),
        ("知识图谱", &["knowledge graph"]),
        ("推荐系统", &["recommender system"]),
        ("信息检索", &["information retrieval"]),
        ("统计学习", &["statistical learning"]),
        ("概率论", &["probability theory"]),
        ("线性代数", &["linear algebra"]),
        ("微积分", &["calculus"]),
        ("经济学", &["economics"]),
        ("心理学", &["psychology"]),
        ("哲学", &["philosophy"]),
        ("历史", &["history"]),
    ];
    let kw = keyword.trim();
    let lc = kw.to_lowercase();
    let mut out = Vec::new();
    for (zh, ens) in SEED {
        if kw.contains(zh) {
            for e in *ens { out.push(e.to_string()); }
        }
        for e in *ens {
            if lc.contains(&e.to_lowercase()) { out.push(zh.to_string()); }
        }
    }
    out.sort();
    out.dedup();
    out.retain(|s| s != kw);
    out.truncate(3);
    out
}

/// 关键词意图识别：从查询里的显式信号推断用户更可能想要的资源类型。
/// 仅在用户未显式勾选类型时用于聚焦扇出，避免图片/网页淹没论文等。
fn detect_intent(keyword: &str) -> Vec<ResourceKind> {
    let lc = keyword.to_lowercase();
    let hit = |pats: &[&str]| pats.iter().any(|p| lc.contains(p));
    let mut kinds = Vec::new();
    if hit(&["论文", "文献", "paper", "arxiv", "doi", "journal", "研究", "综述", "survey"]) {
        kinds.push(ResourceKind::Paper);
    }
    if hit(&["电子书", "书籍", "book", "epub", "pdf 书", "教材", "小说", "novel", "textbook"]) {
        kinds.push(ResourceKind::Book);
    }
    if hit(&["电影", "视频", "movie", "video", "film", "纪录片", "剧集", "动画", "mv"]) {
        kinds.push(ResourceKind::Video);
    }
    if hit(&["音乐", "歌曲", "音频", "song", "music", "audio", "mp3", "有声书", "podcast", "播客"]) {
        kinds.push(ResourceKind::Audio);
    }
    if hit(&["图片", "壁纸", "照片", "image", "photo", "wallpaper", "picture", "插画", "素材"]) {
        kinds.push(ResourceKind::Image);
    }
    if hit(&["代码", "源码", "库", "框架", "github", "code", "library", "sdk", "开源", "repo"]) {
        kinds.push(ResourceKind::Code);
    }
    kinds
}

/// 标题归一化：小写、去标点/空白，用于跨源近似去重（不同源同一资源合并）。
fn norm_title(t: &str) -> String {
    t.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
        .chars()
        .take(60)
        .collect()
}

/// 统一超时包装：慢源(镜像/反爬/卡死候选页)不拖垮整体聚合。
pub(crate) async fn timed<T, F>(secs: u64, fut: F) -> T
where
    T: Default,
    F: std::future::Future<Output = T>,
{
    tokio::time::timeout(std::time::Duration::from_secs(secs), fut)
        .await
        .unwrap_or_default()
}
// PLACEHOLDER_RUN

/// 领域 API 精确源：按需要的类型并发查结构化接口。每个源独立超时，慢源不拖累其它源。
async fn api_sources(app: &tauri::AppHandle, kw: &str, kinds: &[ResourceKind], limit: usize) -> Vec<ResourceItem> {
    type BoxFut = std::pin::Pin<Box<dyn std::future::Future<Output = Vec<ResourceItem>> + Send>>;
    let mut futs: Vec<BoxFut> = Vec::new();
    let has = |k: ResourceKind| kinds.contains(&k);
    macro_rules! src {
        ($secs:expr, $body:expr) => {{
            futs.push(Box::pin(async move { timed($secs, $body).await }));
        }};
    }
    if has(ResourceKind::Paper) {
        let k = kw.to_string(); src!(12, seed_sites::arxiv(&k, limit));
        let k = kw.to_string(); src!(12, seed_sites::openalex(&k, limit));
        let k = kw.to_string(); src!(12, apis::semantic_scholar(&k, limit));
        let k = kw.to_string(); src!(12, apis::crossref(&k, limit));
        let k = kw.to_string(); src!(12, apis::doaj(&k, limit));
        let k = kw.to_string(); src!(14, apis::unpaywall(&k, limit));
        let k = kw.to_string(); src!(14, apis::europepmc(&k, limit));
        let k = kw.to_string(); src!(14, apis::core_api(&k, limit));
    }
    if has(ResourceKind::Code) {
        let k = kw.to_string(); src!(12, apis::github(&k, limit));
        let k = kw.to_string(); src!(12, apis::gitlab(&k, limit));
        let k = kw.to_string(); src!(12, apis::crates_io(&k, limit));
        let k = kw.to_string(); src!(12, apis::npm_pkg(&k, limit));
    }
    if has(ResourceKind::Book) {
        let k = kw.to_string(); src!(12, apis::gutendex(&k, limit));
        let k = kw.to_string(); src!(12, apis::libgen(&k, limit));
        let k = kw.to_string(); src!(14, apis::open_library(&k, limit));
        let k = kw.to_string(); src!(14, apis::doab(&k, limit));
    }
    if has(ResourceKind::Image) {
        let k = kw.to_string(); src!(12, apis::baidu_image(&k, limit));
        let k = kw.to_string(); src!(12, apis::openverse(&k, "images", limit));
        let k = kw.to_string(); src!(14, apis::wikimedia_commons(&k, limit));
        let k = kw.to_string(); src!(16, apis::met_museum(&k, limit));
        let k = kw.to_string(); src!(14, apis::nasa_images(&k, limit));
        let k = kw.to_string(); src!(14, apis::pexels(&k, "image", limit));
    }
    if has(ResourceKind::Audio) {
        let k = kw.to_string(); src!(12, apis::openverse(&k, "audio", limit));
        let k = kw.to_string(); src!(18, apis::internet_archive(&k, "audio", ResourceKind::Audio, limit));
        let k = kw.to_string(); src!(14, apis::librivox(&k, limit));
        let k = kw.to_string(); src!(14, apis::jamendo(&k, limit));
        let k = kw.to_string(); let a = app.clone(); src!(28, apis::ytsearch(&a, &k, "audio", limit));
    }
    if has(ResourceKind::Video) {
        let k = kw.to_string(); src!(18, apis::internet_archive(&k, "movies", ResourceKind::Video, limit));
        let k = kw.to_string(); src!(14, apis::pexels(&k, "video", limit));
        let k = kw.to_string(); let a = app.clone(); src!(28, apis::ytsearch(&a, &k, "video", limit));
    }
    join_all(futs).await.into_iter().flatten().collect()
}

/// 执行一次检索。
pub async fn run(app: &tauri::AppHandle, query: SearchQuery) -> Vec<ResourceItem> {
    let keyword = query.keyword.trim().to_string();
    if keyword.is_empty() {
        return vec![];
    }
    let _ = store::record_query(&keyword);
    let cfg = load_proxy();
    // 用户显式勾选优先；否则按关键词意图聚焦，意图不明时才全类型铺开。
    let kinds = if !query.kinds.is_empty() {
        query.kinds.clone()
    } else {
        let intent = detect_intent(&keyword);
        if intent.is_empty() { all_kinds() } else { intent }
    };

    let mut items: Vec<ResourceItem> = Vec::new();

    // 1) 领域 API 精确源（结构化直链，精度最高）。
    if query.seed_sites {
        items.extend(api_sources(app, &keyword, &kinds, query.limit).await);
    }

    // 2) 通用全网发现：多引擎 ×（dork + LLM 扩展）→ 候选页 → 嗅探。
    if query.web_discovery {
        let engines: Vec<String> = if query.engines.is_empty() {
            engines::known_engines().iter().map(|s| s.to_string()).collect()
        } else {
            query.engines.clone()
        };
        // 查询扩展：内建双语种子表(离线可用) + 可选 LLM 扩展(同义/中英/相关术语)。
        let mut keywords = vec![keyword.clone()];
        for ex in offline_expand(&keyword) {
            keywords.push(ex);
        }
        if query.use_llm {
            for ex in llm::expand_query(&keyword).await.into_iter().take(3) {
                keywords.push(ex);
            }
        }
        keywords.sort();
        keywords.dedup();
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
                hit_futs.push(timed(10, engines::search(&cfg, eng, q, 8)));
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
        let sniff_futs = hits.iter().map(|h| timed(9, sniffer::sniff(&h.url, &h.title, &kinds)));
        for group in join_all(sniff_futs).await {
            items.extend(group);
        }
    }

    // 3) 去重 → HEAD 死链校验 + Content-Type 定类 → 多特征打分 → 按类分组截断排序。
    let matcher = SkimMatcherV2::default();
    let mut seen = std::collections::HashSet::new();
    items.retain(|it| seen.insert(it.id.clone()));
    let mut items = sniffer::verify(items, &kinds).await;
    for it in items.iter_mut() {
        it.relevance = score(&matcher, it, &keyword);
    }
    // 跨源近似去重：同类型 + 归一标题相同的条目只保留相关度最高的一条
    // （合并「不同源命中同一资源」，权威直链自然胜出）。
    items.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
    {
        let mut seen_title = std::collections::HashSet::new();
        items.retain(|it| {
            let key = format!("{:?}|{}", it.kind, norm_title(&it.title));
            norm_title(&it.title).is_empty() || seen_title.insert(key)
        });
    }
    // 按类分组：每类独立排序并限量，避免图片/网页把论文等挤出结果。
    let per_kind = query.limit.max(20);
    let mut groups: std::collections::HashMap<String, Vec<ResourceItem>> = std::collections::HashMap::new();
    for it in items {
        groups.entry(format!("{:?}", it.kind)).or_default().push(it);
    }
    let mut items: Vec<ResourceItem> = Vec::new();
    for (_, mut g) in groups {
        g.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
        g.truncate(per_kind);
        items.extend(g);
    }
    items.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
    items
}
