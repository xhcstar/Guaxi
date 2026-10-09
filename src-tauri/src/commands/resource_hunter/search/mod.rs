//! 搜索聚合层：领域 API + 全网多引擎嗅探 + 富集站，归一去重后按相关度排序。
use super::model::{ProxyConfig, ResourceItem, ResourceKind, SearchQuery};
use super::store;
use futures_util::future::join_all;
use sha2::{Digest, Sha256};

pub mod apis;
pub mod engines;
pub mod llm;
pub mod resolve;
pub mod seed_sites;
pub mod sniffer;
pub mod translate;

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
        ResourceKind::Dataset,
    ]
}

/// 各类型的资源富集站清单：喂给通用引擎做 `site:` 偏置，即便无专用 API 也能提高命中。
fn seed_domains(kind: ResourceKind) -> &'static [&'static str] {
    match kind {
        ResourceKind::Book => &["annas-archive.org", "libgen.is", "libgen.rs", "standardebooks.org"],
        ResourceKind::Paper => &["arxiv.org", "openalex.org", "sci-hub.se", "researchgate.net", "core.ac.uk"],
        ResourceKind::Video => &[
            "youtube.com", "bilibili.com", "vimeo.com", "archive.org",
            "youku.com", "v.qq.com", "iqiyi.com", "douyin.com", "ixigua.com",
        ],
        ResourceKind::Audio => &["bandcamp.com", "soundcloud.com", "freemusicarchive.org", "archive.org"],
        ResourceKind::Image => &["commons.wikimedia.org", "unsplash.com", "flickr.com", "pexels.com"],
        ResourceKind::Code => &["github.com", "gitlab.com", "gitee.com", "sourceforge.net"],
        ResourceKind::Dataset => &["zenodo.org", "huggingface.co", "kaggle.com", "data.gov"],
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
                qs.push(format!("{keyword} 电子书 filetype:pdf"));
                qs.push(format!("{keyword} 教材 pdf 下载"));
            }
            ResourceKind::Audio => qs.push(format!("{keyword} filetype:mp3")),
            ResourceKind::Image => qs.push(format!("{keyword} filetype:jpg")),
            ResourceKind::Dataset => {
                qs.push(format!("{keyword} dataset filetype:csv"));
                plain_needed = true;
            }
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
        | "Unpaywall" | "EuropePMC" | "CORE" | "PubMed" | "DBLP" | "HAL" => 0.95,
        "GitHub" | "GitLab" | "crates.io" | "npm" | "HuggingFace" | "Codeberg" => 0.92,
        "Zenodo" | "Dataverse" | "Figshare" => 0.9,
        "Gutenberg" | "OpenLibrary" | "DOAB" => 0.86,
        "Wikimedia" | "Openverse" | "InternetArchive" | "MetMuseum" | "NASA"
        | "LibriVox" | "Pexels" | "Jamendo" | "ArtInstitute" | "ClevelandArt"
        | "Unsplash" | "Pixabay" | "ccMixter" => 0.82,
        "PeerTube" | "Dailymotion" => 0.78,
        "LibGen" | "AnnasArchive" => 0.78,
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

/// 匹配用分词：拉丁按单词（≥2 字符，整词边界）、CJK 按相邻 bigram（单字时保留单字）。
/// 全小写。相比子序列模糊匹配，词元匹配不会把 "rust" 命中到 "RUBSON...vaporisaTeur"。
fn tokenize(s: &str) -> Vec<String> {
    fn is_cjk(c: char) -> bool {
        ('\u{4e00}'..='\u{9fff}').contains(&c)
            || ('\u{3040}'..='\u{30ff}').contains(&c)
            || ('\u{ac00}'..='\u{d7af}').contains(&c)
    }
    fn flush_latin(buf: &mut String, out: &mut Vec<String>) {
        if buf.chars().count() >= 2 {
            out.push(buf.to_lowercase());
        }
        buf.clear();
    }
    fn flush_cjk(buf: &mut Vec<char>, out: &mut Vec<String>) {
        if buf.len() == 1 {
            out.push(buf[0].to_string());
        } else if buf.len() >= 2 {
            for w in buf.windows(2) {
                out.push(w.iter().collect());
            }
        }
        buf.clear();
    }
    let mut out = Vec::new();
    let mut latin = String::new();
    let mut cjk: Vec<char> = Vec::new();
    for ch in s.chars() {
        if is_cjk(ch) {
            flush_latin(&mut latin, &mut out);
            cjk.push(ch);
        } else if ch.is_alphanumeric() {
            flush_cjk(&mut cjk, &mut out);
            latin.push(ch);
        } else {
            flush_latin(&mut latin, &mut out);
            flush_cjk(&mut cjk, &mut out);
        }
    }
    flush_latin(&mut latin, &mut out);
    flush_cjk(&mut cjk, &mut out);
    out.sort();
    out.dedup();
    out
}

/// 单个查询词的词元覆盖率 ∈ [0,1]：命中词元数 / 查询词元数。
fn term_coverage(title_toks: &[String], query_toks: &[String]) -> f32 {
    if query_toks.is_empty() {
        return 0.0;
    }
    let hit = query_toks.iter().filter(|q| title_toks.contains(q)).count();
    hit as f32 / query_toks.len() as f32
}

/// 标题对“任一检索词”的最佳词元覆盖率（任意语种的检索词命中都算数）。
fn best_lexical(title: &str, terms: &[Vec<String>]) -> f32 {
    let tt = tokenize(title);
    terms.iter().map(|qt| term_coverage(&tt, qt)).fold(0.0, f32::max)
}

/// 工业级多特征相关度：标题词元命中 × 来源权威 × 可下载性 × 元数据 × 来源先验。
/// 标题以“整串精确命中”为最强信号，退而求其次用词元覆盖率；不再做子序列模糊匹配
/// （子序列匹配对短查询几乎全命中，是旧版噪声结果的主因）。
fn score(item: &ResourceItem, terms: &[Vec<String>], terms_lc: &[String]) -> f32 {
    let has_query = terms.iter().any(|t| !t.is_empty());
    let title = if !has_query {
        0.6
    } else {
        let title_lc = item.title.to_lowercase();
        let exact = terms_lc.iter().any(|t| !t.is_empty() && title_lc.contains(t));
        if exact {
            1.0
        } else {
            best_lexical(&item.title, terms)
        }
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
    (title * 0.46 + auth * 0.18 + concrete * 0.14 + meta * 0.10 + prior * 0.12).min(1.0)
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
    if hit(&["数据集", "数据", "dataset", "corpus", "语料", "benchmark", "基准", "csv", "kaggle"]) {
        kinds.push(ResourceKind::Dataset);
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
        let k = kw.to_string(); src!(14, apis::pubmed(&k, limit));
        let k = kw.to_string(); src!(12, apis::dblp(&k, limit));
        let k = kw.to_string(); src!(14, apis::hal(&k, limit));
    }
    if has(ResourceKind::Code) {
        let k = kw.to_string(); src!(12, apis::github(&k, limit));
        let k = kw.to_string(); src!(12, apis::gitlab(&k, limit));
        let k = kw.to_string(); src!(12, apis::crates_io(&k, limit));
        let k = kw.to_string(); src!(12, apis::npm_pkg(&k, limit));
        let k = kw.to_string(); src!(12, apis::codeberg(&k, limit));
        let k = kw.to_string(); src!(12, apis::huggingface(&k, "models", ResourceKind::Code, limit));
    }
    if has(ResourceKind::Dataset) {
        let k = kw.to_string(); src!(12, apis::zenodo(&k, limit));
        let k = kw.to_string(); src!(12, apis::huggingface(&k, "datasets", ResourceKind::Dataset, limit));
        let k = kw.to_string(); src!(14, apis::dataverse(&k, limit));
    }
    if has(ResourceKind::Book) {
        let k = kw.to_string(); src!(12, apis::gutendex(&k, limit));
        let k = kw.to_string(); src!(16, apis::libgen(&k, limit));
        let k = kw.to_string(); src!(16, apis::annas_archive(&k, limit));
        let k = kw.to_string(); src!(14, apis::open_library(&k, limit));
        let k = kw.to_string(); src!(14, apis::doab(&k, limit));
        // Internet Archive 文本馆藏：keyless、CJK 友好、非 CF 门禁，且给真实 pdf/epub 直链。
        // 是中文书籍召回的可靠主力（英文向的 Gutendex/DOAB/OpenLibrary 对中文几乎无命中）。
        let k = kw.to_string(); src!(18, apis::internet_archive(&k, "texts", ResourceKind::Book, limit));
    }
    if has(ResourceKind::Image) {
        let k = kw.to_string(); src!(12, apis::baidu_image(&k, limit));
        let k = kw.to_string(); src!(12, apis::openverse(&k, "images", limit));
        let k = kw.to_string(); src!(14, apis::wikimedia_commons(&k, limit));
        let k = kw.to_string(); src!(16, apis::met_museum(&k, limit));
        let k = kw.to_string(); src!(14, apis::nasa_images(&k, limit));
        let k = kw.to_string(); src!(14, apis::pexels(&k, "image", limit));
        let k = kw.to_string(); src!(12, apis::art_institute(&k, limit));
        let k = kw.to_string(); src!(12, apis::cleveland_art(&k, limit));
        let k = kw.to_string(); src!(12, apis::unsplash(&k, limit));
        let k = kw.to_string(); src!(12, apis::pixabay(&k, "image", limit));
    }
    if has(ResourceKind::Audio) {
        let k = kw.to_string(); src!(12, apis::openverse(&k, "audio", limit));
        let k = kw.to_string(); src!(18, apis::internet_archive(&k, "audio", ResourceKind::Audio, limit));
        let k = kw.to_string(); src!(14, apis::librivox(&k, limit));
        let k = kw.to_string(); src!(14, apis::jamendo(&k, limit));
        let k = kw.to_string(); src!(14, apis::ccmixter(&k, limit));
        let k = kw.to_string(); let a = app.clone(); src!(28, apis::ytsearch(&a, &k, "audio", limit));
    }
    if has(ResourceKind::Video) {
        let k = kw.to_string(); src!(18, apis::internet_archive(&k, "movies", ResourceKind::Video, limit));
        let k = kw.to_string(); src!(14, apis::pexels(&k, "video", limit));
        let k = kw.to_string(); src!(14, apis::invidious(&k, limit));
        let k = kw.to_string(); src!(12, apis::bilibili_search(&k, limit));
        let k = kw.to_string(); src!(14, apis::peertube(&k, limit));
        let k = kw.to_string(); src!(14, apis::dailymotion(&k, limit));
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

    // 多语翻译扩展：中英互译始终执行（保证“中英双语召回必含”）；其余语种仅在用户
    // 开启“多语检索”时追加（跨语种放大召回，但可能引入他国语言结果，故设为可选）。
    let mut translations = translate::cross_cn_en(&keyword).await;
    if query.multilingual {
        for t in translate::translate_multi(&keyword, &translate::EXTRA_TARGETS).await {
            if !translations.contains(&t) {
                translations.push(t);
            }
        }
    }
    // 可选 LLM 查询扩展（同义/中英/相关术语），仅在用户开启时执行。
    let llm_terms: Vec<String> = if query.use_llm {
        llm::expand_query(&keyword).await.into_iter().take(3).collect()
    } else {
        Vec::new()
    };
    // 用于相关度打分与 web 词法闸门的全部检索词：原词 + 译文 + 离线扩展 + LLM 扩展。
    let mut search_terms: Vec<String> = vec![keyword.clone()];
    search_terms.extend(translations.iter().cloned());
    search_terms.extend(offline_expand(&keyword));
    search_terms.extend(llm_terms.iter().cloned());
    search_terms.sort();
    search_terms.dedup();
    search_terms.retain(|s| !s.trim().is_empty());

    let mut items: Vec<ResourceItem> = Vec::new();

    // 1) 领域 API 精确源（结构化直链，精度最高）。
    //    对原词 + 若干译文并发查（控制扇出），跨语种命中外文版资源；中英译文优先纳入。
    if query.seed_sites {
        let mut api_terms = vec![keyword.clone()];
        for t in translations.iter() {
            if api_terms.len() >= 4 {
                break;
            }
            api_terms.push(t.clone());
        }
        let api_futs = api_terms
            .iter()
            .map(|t| api_sources(app, t, &kinds, query.limit));
        for group in join_all(api_futs).await {
            items.extend(group);
        }
    }

    // 2) 通用全网发现：多引擎 ×（dork + 扩展词）→ 候选页 → 嗅探。
    if query.web_discovery {
        let engines: Vec<String> = if query.engines.is_empty() {
            engines::known_engines().iter().map(|s| s.to_string()).collect()
        } else {
            query.engines.clone()
        };
        let mut queries = Vec::new();
        for kw in &search_terms {
            queries.extend(build_queries(kw, &kinds));
        }
        queries.sort();
        queries.dedup();
        queries.truncate(36);

        let mut hit_futs = Vec::new();
        for eng in &engines {
            for q in &queries {
                hit_futs.push(timed(10, engines::search(&cfg, eng, q, 12)));
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
        hits.truncate(90);
        let sniff_futs = hits.iter().map(|h| timed(9, sniffer::sniff(&h.url, &h.title, &kinds)));
        for group in join_all(sniff_futs).await {
            items.extend(group);
        }
    }

    // 3) 去重 → HEAD 死链校验 + Content-Type 定类 → 多特征打分 → 词法闸门 → 按类分组截断排序。
    let mut seen = std::collections::HashSet::new();
    items.retain(|it| seen.insert(it.id.clone()));
    // 解析层：对“只有详情页、没有直链”的条目（Anna's/OpenLibrary/DOI 落地页等）尽力解析出
    // 可下载直链（md5→LibGen/IPFS、OpenLibrary→IA、DOI→OA PDF），让更多源真正可下。
    resolve::resolve_items(&mut items).await;
    let mut items = sniffer::verify(items, &kinds).await;
    let terms: Vec<Vec<String>> = search_terms.iter().map(|t| tokenize(t)).collect();
    let terms_lc: Vec<String> = search_terms.iter().map(|s| s.to_lowercase()).collect();
    for it in items.iter_mut() {
        it.relevance = score(it, &terms, &terms_lc);
    }
    // web 词法闸门 + 全局相关度下限：全网抓取的结果标题必须命中至少一个检索词，
    // 否则判为“一个无关候选页灌入的噪声文件”直接丢弃（结构化 API 源已按词检索，豁免）。
    const MIN_SCORE: f32 = 0.30;
    items.retain(|it| {
        if it.relevance < MIN_SCORE {
            return false;
        }
        if it.source == "web" {
            best_lexical(&it.title, &terms) > 0.0
                || terms_lc.iter().any(|t| !t.is_empty() && it.title.to_lowercase().contains(t))
        } else {
            true
        }
    });
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
