//! 资源源注册表：声明式列出所有领域 API 源，供前端渲染「资源源」开关面板，
//! 并供检索层按用户的禁用集（设置项 `sources_off`）过滤要并发请求的源。
use serde::Serialize;
use std::collections::HashSet;

/// 一个资源源的元信息。`id` 用于开关与过滤；`kind` 为分组域（含多域源的 "mixed"）。
#[derive(Debug, Clone, Serialize)]
pub struct SourceMeta {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: &'static str,
    /// 需第三方 Key：未在设置里填 Key 则自动跳过（默认即不产出结果）。
    pub needs_key: bool,
    /// 境外源：大陆需代理可达。
    pub foreign: bool,
}

const fn s(id: &'static str, label: &'static str, kind: &'static str, needs_key: bool, foreign: bool) -> SourceMeta {
    SourceMeta { id, label, kind, needs_key, foreign }
}

/// 全部可用源（与 search::api_sources 的 src! id 一一对应）。
pub fn source_metas() -> Vec<SourceMeta> {
    vec![
        // 论文
        s("arxiv", "arXiv", "paper", false, true),
        s("openalex", "OpenAlex", "paper", false, true),
        s("semantic_scholar", "Semantic Scholar", "paper", false, true),
        s("crossref", "Crossref", "paper", false, true),
        s("doaj", "DOAJ", "paper", false, true),
        s("unpaywall", "Unpaywall", "paper", false, true),
        s("europepmc", "Europe PMC", "paper", false, true),
        s("core", "CORE", "paper", true, true),
        s("pubmed", "PubMed / PMC", "paper", false, true),
        s("dblp", "DBLP", "paper", false, true),
        s("hal", "HAL", "paper", false, true),
        // 书籍
        s("gutendex", "Project Gutenberg", "book", false, true),
        s("libgen", "Library Genesis", "book", false, true),
        s("annas", "Anna's Archive", "book", false, true),
        s("open_library", "Open Library", "book", false, true),
        s("doab", "DOAB", "book", false, true),
        // 代码
        s("github", "GitHub", "code", false, true),
        s("gitlab", "GitLab", "code", false, true),
        s("crates_io", "crates.io", "code", false, true),
        s("npm", "npm", "code", false, true),
        s("codeberg", "Codeberg", "code", false, true),
        s("hf_models", "Hugging Face 模型", "code", false, true),
        // 数据集
        s("zenodo", "Zenodo", "dataset", false, true),
        s("hf_datasets", "Hugging Face 数据集", "dataset", false, true),
        s("dataverse", "Harvard Dataverse", "dataset", false, true),
        // 图片
        s("baidu_image", "百度图片", "image", false, false),
        s("wikimedia", "Wikimedia Commons", "image", false, true),
        s("met", "大都会艺术博物馆", "image", false, true),
        s("nasa", "NASA", "image", false, true),
        s("artic", "芝加哥艺术博物馆", "image", false, true),
        s("cleveland", "克利夫兰艺术博物馆", "image", false, true),
        s("unsplash", "Unsplash", "image", true, true),
        s("pixabay", "Pixabay", "image", true, true),
        s("smithsonian", "史密森尼", "image", true, true),
        s("europeana", "Europeana", "image", true, true),
        // 音频
        s("librivox", "LibriVox 有声书", "audio", false, true),
        s("jamendo", "Jamendo", "audio", true, true),
        s("ccmixter", "ccMixter", "audio", false, true),
        // 视频
        s("invidious", "YouTube (Invidious)", "video", false, true),
        s("bilibili", "哔哩哔哩", "video", false, false),
        s("peertube", "PeerTube", "video", false, true),
        s("dailymotion", "Dailymotion", "video", false, true),
        // 多域综合源
        s("internet_archive", "Internet Archive", "mixed", false, true),
        s("openverse", "Openverse", "mixed", false, true),
        s("pexels", "Pexels", "mixed", true, true),
        s("ytsearch", "yt-dlp 搜索 (YouTube)", "mixed", false, true),
    ]
}

/// 用户禁用的源 id 集合（设置项 `sources_off`，JSON 字符串数组）。缺省为空=全部启用。
pub fn disabled_set() -> HashSet<String> {
    super::super::store::get_setting("sources_off")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .map(|v| v.into_iter().collect())
        .unwrap_or_default()
}
