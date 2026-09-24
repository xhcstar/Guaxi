//! 资源嗅探下载引擎的核心数据模型：检索条目、下载任务、代理与检索配置。
use serde::{Deserialize, Serialize};

/// 资源类型。按文件用途归类，前端徽标与分流策略都依赖它。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Book,
    Paper,
    Video,
    Audio,
    Image,
    Code,
    Other,
}

impl ResourceKind {
    /// 由文件扩展名粗判类型，嗅探阶段无法拿到 Content-Type 时的兜底。
    pub fn from_ext(ext: &str) -> Self {
        match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
            "pdf" | "epub" | "mobi" | "azw3" | "djvu" => ResourceKind::Book,
            "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "ts" => ResourceKind::Video,
            "mp3" | "flac" | "m4a" | "wav" | "aac" | "ogg" | "opus" => ResourceKind::Audio,
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "svg" | "tiff" => ResourceKind::Image,
            _ => ResourceKind::Other,
        }
    }
}

/// 一条可下载资源。搜索层归一后的统一形态，也是入库的基础。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceItem {
    pub id: String,
    pub title: String,
    pub kind: ResourceKind,
    /// 来源标识（引擎名或站点名），用于溯源与去重。
    pub source: String,
    /// 承载资源的页面 URL（详情页），可能与直链不同。
    pub source_url: String,
    /// 可直接下载的链接；磁力/需二次解析时可能为空。
    pub download_url: Option<String>,
    /// magnet: 链接（书籍/大文件常见）。一期不下，仅登记。
    pub magnet: Option<String>,
    /// 是否需要 yt-dlp 解析（视频/音频站点页面）。
    pub needs_ytdlp: bool,
    pub format: Option<String>,
    pub thumbnail: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<String>,
    pub summary: Option<String>,
    /// 字节大小提示（可能来自 Content-Length，未知为 None）。
    pub size_hint: Option<u64>,
    /// 相关度 0.0~1.0，供前端排序与过滤。
    pub relevance: f32,
    pub tags: Vec<String>,
}

/// 检索请求。资源类型多选 + 引擎/富集站开关都在这里。
#[derive(Debug, Clone, Deserialize)]
pub struct SearchQuery {
    pub keyword: String,
    /// 想找的资源类型；空表示全部。
    #[serde(default)]
    pub kinds: Vec<ResourceKind>,
    /// 启用的搜索引擎 id（engines.rs 中登记）；空则用默认集合。
    #[serde(default)]
    pub engines: Vec<String>,
    /// 是否启用通用全网发现（结果页抓取 + 嗅探）。
    #[serde(default = "default_true")]
    pub web_discovery: bool,
    /// 是否启用富集站精确源（arXiv/OpenAlex 等）。
    #[serde(default = "default_true")]
    pub seed_sites: bool,
    /// 单类型最多返回条数，控制扇出规模。
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// 是否启用 LLM 查询扩展（同义词/中英/相关术语），提升召回与精度。
    #[serde(default)]
    pub use_llm: bool,
}

fn default_true() -> bool {
    true
}
fn default_limit() -> usize {
    40
}

/// 下载任务状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Active,
    Paused,
    Completed,
    Error,
}

/// 一个分块的下载进度，续传时据此只补未完成区间。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkState {
    pub start: u64,
    pub end: u64,
    pub downloaded: u64,
}

impl ChunkState {
    pub fn is_done(&self) -> bool {
        self.downloaded >= self.end - self.start + 1
    }
}

/// 下载任务。持久化到 SQLite，重启后可恢复队列。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: String,
    pub title: String,
    pub kind: ResourceKind,
    /// 多镜像 URL，首个失败自动切换下一个。
    pub urls: Vec<String>,
    /// magnet（磁力任务，一期占位不下）。
    pub magnet: Option<String>,
    /// 是否走 yt-dlp 进程下载（视频/音频站点）。
    pub via_ytdlp: bool,
    pub dest: String,
    pub total_size: u64,
    pub downloaded: u64,
    pub chunks: Vec<ChunkState>,
    pub status: DownloadStatus,
    /// 瞬时速度 B/s，节流上报。
    pub speed: u64,
    pub eta_secs: Option<u64>,
    pub priority: i32,
    pub retries: u32,
    pub error: Option<String>,
    /// 命中的来源，用于入库溯源。
    pub source: String,
    pub source_url: Option<String>,
    pub created_at: i64,
}

/// 代理模式。大陆场景：境外站点走代理、境内直连。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    None,
    System,
    Custom,
}

/// 代理配置。search 与 downloader 共用，按目标域分流。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub mode: ProxyMode,
    /// Custom 模式下的代理 URL，支持 http(s):// 与 socks5://。
    pub url: Option<String>,
    /// 直连白名单（域名后缀）。命中的域不走代理，即使全局开了代理。
    #[serde(default)]
    pub bypass: Vec<String>,
    /// 是否对境外域名自动套用代理（智能分流轻量版）。
    #[serde(default)]
    pub proxy_foreign_only: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        ProxyConfig {
            mode: ProxyMode::System,
            url: None,
            bypass: vec![
                "localhost".into(),
                "127.0.0.1".into(),
                ".cn".into(),
                "baidu.com".into(),
                "bilibili.com".into(),
            ],
            proxy_foreign_only: false,
        }
    }
}
