//! 免费翻译 API 池。
//!
//! 目标：每次划词都能「快、稳、不被封」地拿到译文。
//! - 池化：多家免费接口注册在同一个池里，按实时健康度（EMA 时延 + 连续失败）动态排序。
//! - 竞速：只对前 N 家做阶梯式并发（第一家立刻发，之后每隔 hedge_ms 补一家），
//!   谁先返回用谁，其余立即取消。既比串行快，也不会每次把所有接口全打一遍。
//! - 防封：进程级固定 UA、每家接口最小调用间隔、429/403/503 指数级冷却、
//!   连续失败熔断、译文缓存（相同文本不重复请求）。
//! - 大陆优先：目标语言为中文（或 GUAXI_CN_ACCEL=1）时优先排大陆可直连的接口。

use futures_util::StreamExt;
use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::Client;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::warn;

/// 单次调用失败的归类，决定冷却策略：Throttled 会触发长冷却，其它只累计熔断计数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailKind {
    /// 网络层错误（超时、连接失败）。
    Network,
    /// 被限流 / 拒绝访问（429、403、503），需要长时间退避以免被封。
    Throttled,
    /// 拿到响应但内容不可用（空结果、非 JSON、字段缺失）。
    Bad,
}

#[derive(Debug, Clone)]
pub struct PoolError {
    pub message: String,
    pub kind: FailKind,
}

impl PoolError {
    fn new(kind: FailKind, message: impl Into<String>) -> Self {
        PoolError { message: message.into(), kind }
    }
    fn net(message: impl Into<String>) -> Self {
        PoolError::new(FailKind::Network, message)
    }
    fn bad(message: impl Into<String>) -> Self {
        PoolError::new(FailKind::Bad, message)
    }
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PoolError {}

type ProviderFuture = Pin<Box<dyn Future<Output = Result<Vec<String>, PoolError>> + Send + 'static>>;
type ProviderFn = fn(Vec<String>, String, String) -> ProviderFuture;

/// 接入区域：CnDirect 表示大陆可直连，Global 需要国际网络。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    CnDirect,
    Global,
}

pub struct ProviderSpec {
    pub name: &'static str,
    pub region: Region,
    /// 是否支持一次请求翻译多条文本。false 的接口由池负责并发拆分。
    pub batch: bool,
    /// 单次请求最多携带多少条文本。
    pub max_batch: usize,
    /// 同一接口两次请求的最小间隔，避免高频触发风控。
    pub min_interval_ms: u64,
    /// 静态基准时延，用于冷启动排序。
    pub base_rtt_ms: f64,
    pub call: ProviderFn,
}

#[derive(Debug, Clone, Default)]
struct Health {
    ema_rtt_ms: f64,
    samples: u64,
    ok_count: u64,
    fail_count: u64,
    /// 连续失败次数，达到阈值后熔断。
    streak_fails: u32,
    /// 被限流的累计次数，决定冷却时长的指数级别。
    throttle_strikes: u32,
    /// 熔断/冷却截止时间。
    blocked_until: Option<Instant>,
    /// 最近一次派发时间，用于最小间隔限速。
    last_dispatch: Option<Instant>,
    last_error: Option<String>,
}

/// 池内全部接口。顺序只影响同分时的稳定性，真实优先级由健康度打分决定。
static PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec {
        name: "Transmart",
        region: Region::CnDirect,
        batch: true,
        max_batch: 24,
        min_interval_ms: 120,
        base_rtt_ms: 420.0,
        call: transmart_provider,
    },
    ProviderSpec {
        name: "Youdao",
        region: Region::CnDirect,
        batch: false,
        max_batch: 1,
        min_interval_ms: 160,
        base_rtt_ms: 500.0,
        call: youdao_provider,
    },
    ProviderSpec {
        name: "Caiyun",
        region: Region::CnDirect,
        batch: true,
        max_batch: 16,
        min_interval_ms: 200,
        base_rtt_ms: 700.0,
        call: caiyun_provider,
    },
    ProviderSpec {
        name: "Bing",
        region: Region::CnDirect,
        batch: false,
        max_batch: 1,
        min_interval_ms: 200,
        base_rtt_ms: 900.0,
        call: bing_provider,
    },
    ProviderSpec {
        name: "GoogleDict",
        region: Region::Global,
        batch: true,
        max_batch: 16,
        min_interval_ms: 200,
        base_rtt_ms: 1000.0,
        call: google_dict_provider,
    },
    ProviderSpec {
        name: "Google",
        region: Region::Global,
        batch: false,
        max_batch: 1,
        min_interval_ms: 200,
        base_rtt_ms: 1100.0,
        call: google_gtx_provider,
    },
    ProviderSpec {
        name: "MyMemory",
        region: Region::Global,
        batch: false,
        max_batch: 1,
        min_interval_ms: 400,
        base_rtt_ms: 1800.0,
        call: mymemory_provider,
    },
];

// ---------------------------------------------------------------- 环境变量与常量

fn env_u64(key: &str, default_value: u64, lo: u64, hi: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default_value)
        .clamp(lo, hi)
}

fn env_csv(key: &str) -> Vec<String> {
    env::var(key)
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn env_flag(key: &str) -> bool {
    let v = env::var(key).unwrap_or_default().to_ascii_lowercase();
    v == "1" || v == "true" || v == "yes"
}

/// 单次尝试的超时。低于这个值就认为这家接口没戏，交给下一家。
fn attempt_timeout_ms() -> u64 {
    env_u64("GUAXI_TRANSLATE_ATTEMPT_MS", 2600, 800, 15000)
}

/// 阶梯并发的补发间隔：第一家发出后隔多久补第二家。
fn hedge_delay_ms() -> u64 {
    env_u64("GUAXI_TRANSLATE_HEDGE_MS", 320, 0, 3000)
}

/// 一次翻译最多同时压上几家接口。压得越多越快，但更容易触发风控。
fn fanout() -> usize {
    env_u64("GUAXI_TRANSLATE_FANOUT", 3, 1, 6) as usize
}

/// 非批量接口翻译多条文本时的并发度。
fn per_text_concurrency() -> usize {
    env_u64("GUAXI_TRANSLATE_TEXT_CONCURRENCY", 3, 1, 8) as usize
}

fn cache_ttl() -> Duration {
    Duration::from_secs(env_u64("GUAXI_TRANSLATE_CACHE_TTL_S", 30 * 60, 30, 24 * 3600))
}

/// 目标语言是中文（或显式开启）时走大陆优先排序。
pub fn cn_first(to: &str) -> bool {
    if env_flag("GUAXI_CN_ACCEL") {
        return true;
    }
    let t = to.trim().to_ascii_lowercase();
    t.starts_with("zh")
}

// ---------------------------------------------------------------- HTTP 客户端与 UA

/// 一台机器只应该表现为一个浏览器：进程启动时随机挑一个 UA 并固定下来。
/// 每次请求都换 UA 反而是明显的机器人特征。
static SESSION_UA: Lazy<&'static str> = Lazy::new(|| {
    const POOL: &[&str] = &[
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 Edg/129.0.0.0",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    ];
    POOL[rand::rng().random_range(0..POOL.len())]
});

/// 共享连接池：复用 TLS 连接是这类小请求最大的提速点，
/// 每次 Client::builder() 都要重新握手，单次就要几百毫秒。
fn http() -> &'static Client {
    static CLIENT: Lazy<Client> = Lazy::new(|| {
        Client::builder()
            .user_agent(*SESSION_UA)
            .connect_timeout(Duration::from_millis(2500))
            .timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(4)
            .build()
            .expect("failed to build translate pool http client")
    });
    &CLIENT
}

/// Transmart 需要一个形似浏览器指纹的 client_key，同一会话内保持不变。
static TRANSMART_CLIENT_KEY: Lazy<String> = Lazy::new(|| {
    format!(
        "browser-chrome-131.0.0-Windows 10-{}-{}",
        uuid::Uuid::new_v4(),
        chrono::Utc::now().timestamp_millis()
    )
});

// ---------------------------------------------------------------- 健康度与熔断

static HEALTH: Lazy<Mutex<HashMap<&'static str, Health>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn with_health<T>(f: impl FnOnce(&mut HashMap<&'static str, Health>) -> T) -> Option<T> {
    HEALTH.lock().ok().map(|mut guard| f(&mut guard))
}

fn health_snapshot(name: &'static str, base_rtt_ms: f64) -> Health {
    with_health(|map| {
        map.entry(name)
            .or_insert_with(|| Health {
                ema_rtt_ms: base_rtt_ms,
                ..Health::default()
            })
            .clone()
    })
    .unwrap_or_else(|| Health {
        ema_rtt_ms: base_rtt_ms,
        ..Health::default()
    })
}

fn jitter_ms(max: u64) -> u64 {
    if max == 0 {
        return 0;
    }
    rand::rng().random_range(0..=max)
}

fn note_dispatch(name: &'static str, base_rtt_ms: f64) {
    with_health(|map| {
        let entry = map.entry(name).or_insert_with(|| Health {
            ema_rtt_ms: base_rtt_ms,
            ..Health::default()
        });
        entry.last_dispatch = Some(Instant::now());
    });
}

fn note_success(name: &'static str, base_rtt_ms: f64, elapsed_ms: f64) {
    with_health(|map| {
        let entry = map.entry(name).or_insert_with(|| Health {
            ema_rtt_ms: base_rtt_ms,
            ..Health::default()
        });
        entry.ema_rtt_ms = entry.ema_rtt_ms * 0.7 + elapsed_ms.max(1.0) * 0.3;
        entry.samples = entry.samples.saturating_add(1);
        entry.ok_count = entry.ok_count.saturating_add(1);
        entry.streak_fails = 0;
        entry.throttle_strikes = entry.throttle_strikes.saturating_sub(1);
        entry.blocked_until = None;
        entry.last_error = None;
    });
}

fn note_failure(name: &'static str, base_rtt_ms: f64, elapsed_ms: f64, err: &PoolError) {
    with_health(|map| {
        let entry = map.entry(name).or_insert_with(|| Health {
            ema_rtt_ms: base_rtt_ms,
            ..Health::default()
        });
        // 失败也计入时延，但加惩罚项，让排序自然把坏接口沉到后面。
        entry.ema_rtt_ms = entry.ema_rtt_ms * 0.7 + (elapsed_ms + 1200.0).min(9000.0) * 0.3;
        entry.samples = entry.samples.saturating_add(1);
        entry.fail_count = entry.fail_count.saturating_add(1);
        entry.streak_fails = entry.streak_fails.saturating_add(1);
        entry.last_error = Some(err.message.clone());

        let cooldown_ms = if err.kind == FailKind::Throttled {
            // 被限流是最危险的信号：指数退避到最多 30 分钟，彻底停手比重试更安全。
            entry.throttle_strikes = entry.throttle_strikes.saturating_add(1);
            let shift = entry.throttle_strikes.saturating_sub(1).min(5);
            60_000u64.saturating_mul(1u64 << shift).min(30 * 60_000) + jitter_ms(15_000)
        } else if entry.streak_fails >= 3 {
            // 普通失败：连续 3 次才熔断，避免偶发抖动就把接口踢出池子。
            let shift = (entry.streak_fails - 3).min(4);
            30_000u64.saturating_mul(1u64 << shift).min(5 * 60_000) + jitter_ms(5_000)
        } else {
            0
        };

        if cooldown_ms > 0 {
            entry.blocked_until = Some(Instant::now() + Duration::from_millis(cooldown_ms));
            warn!(
                "[TranslatePool] {} 冷却 {}ms（{:?}）: {}",
                name, cooldown_ms, err.kind, err.message
            );
        }
    });
}

/// 排序分：EMA 时延 + 连续失败惩罚 + 跨境惩罚。分数越低越优先。
fn score_of(spec: &ProviderSpec, health: &Health, cn_first: bool) -> f64 {
    let mut score = health.ema_rtt_ms;
    score += health.streak_fails as f64 * 400.0;
    if cn_first && spec.region == Region::Global {
        // 大陆网络下国际接口即便偶尔快，成功率也不稳定，统一后置。
        score += 2500.0;
    }
    score
}

// ---------------------------------------------------------------- 选池

fn provider_enabled(name: &'static str) -> bool {
    let lower = name.to_ascii_lowercase();
    let allow = env_csv("GUAXI_TRANSLATE_PROVIDERS");
    if !allow.is_empty() {
        return allow.iter().any(|x| *x == lower);
    }
    !env_csv("GUAXI_TRANSLATE_PROVIDERS_DISABLE").iter().any(|x| *x == lower)
}

/// 按健康度挑出本次要用的接口。永不返回空：全部在冷却时也会放行最早解封的一家，
/// 否则用户会直接看到「全部失败」，体验比多打一次请求更糟。
fn pick_providers(text_count: usize, to: &str, want: usize) -> Vec<&'static ProviderSpec> {
    let cn = cn_first(to);
    let now = Instant::now();
    let mut ready: Vec<(f64, &'static ProviderSpec)> = Vec::new();
    let mut throttled_only: Vec<(f64, &'static ProviderSpec)> = Vec::new();
    let mut cooling: Vec<(Instant, &'static ProviderSpec)> = Vec::new();

    for spec in PROVIDERS.iter() {
        if !provider_enabled(spec.name) {
            continue;
        }
        let health = health_snapshot(spec.name, spec.base_rtt_ms);
        let mut score = score_of(spec, &health, cn);
        if text_count > 1 && !spec.batch {
            // 不支持批量的接口翻译 N 条要发 N 个请求，按并发度折算额外耗时。
            let rounds = (text_count as f64 / per_text_concurrency() as f64).ceil();
            score += health.ema_rtt_ms * (rounds - 1.0).max(0.0);
        }

        if let Some(until) = health.blocked_until {
            if until > now {
                cooling.push((until, spec));
                continue;
            }
        }
        let interval_ok = health
            .last_dispatch
            .map(|t| now.duration_since(t) >= Duration::from_millis(spec.min_interval_ms))
            .unwrap_or(true);
        if interval_ok {
            ready.push((score, spec));
        } else {
            throttled_only.push((score + 600.0, spec));
        }
    }

    let mut merged = ready;
    if merged.len() < want {
        merged.extend(throttled_only);
    }
    merged.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<&'static ProviderSpec> = merged.into_iter().take(want).map(|(_, s)| s).collect();
    if out.is_empty() {
        cooling.sort_by_key(|(until, _)| *until);
        if let Some((_, spec)) = cooling.first() {
            out.push(spec);
        }
    }
    out
}

// ---------------------------------------------------------------- 译文缓存

#[derive(Debug, Clone)]
struct CacheEntry {
    translated: String,
    expires_at: Instant,
}

static CACHE: Lazy<Mutex<HashMap<String, CacheEntry>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn cache_key(from: &str, to: &str, text: &str) -> String {
    format!(
        "{}|{}|{}",
        from.trim().to_ascii_lowercase(),
        to.trim().to_ascii_lowercase(),
        text.trim()
    )
}

pub fn cache_get(from: &str, to: &str, text: &str) -> Option<String> {
    let mut guard = CACHE.lock().ok()?;
    let key = cache_key(from, to, text);
    match guard.get(&key) {
        Some(entry) if entry.expires_at > Instant::now() => Some(entry.translated.clone()),
        Some(_) => {
            guard.remove(&key);
            None
        }
        None => None,
    }
}

/// 写缓存：进程内 TTL 缓存 + 落盘的持久缓存（后台批量写，不阻塞翻译链路）。
pub fn cache_put_engine(from: &str, to: &str, text: &str, translated: &str, engine: &str) {
    let source = text.trim();
    let target = translated.trim();
    if source.is_empty() || target.is_empty() || target.contains("[Translation Failed]") {
        return;
    }
    if let Ok(mut guard) = CACHE.lock() {
        if guard.len() > 4096 {
            let now = Instant::now();
            guard.retain(|_, v| v.expires_at > now);
            if guard.len() > 4096 {
                guard.clear();
            }
        }
        guard.insert(
            cache_key(from, to, source),
            CacheEntry {
                translated: target.to_string(),
                expires_at: Instant::now() + cache_ttl(),
            },
        );
    }
    crate::commands::translate_cache::queue_put(from, to, source, target, engine);
}

/// 只写进程内缓存（用于把持久缓存的命中结果回填内存，避免再往磁盘写一遍）。
fn cache_put_memory_only(from: &str, to: &str, text: &str, translated: &str) {
    if let Ok(mut guard) = CACHE.lock() {
        guard.insert(
            cache_key(from, to, text.trim()),
            CacheEntry {
                translated: translated.trim().to_string(),
                expires_at: Instant::now() + cache_ttl(),
            },
        );
    }
}

/// 查缓存：先内存，再落盘库。命中磁盘时回填内存。
pub async fn cache_get_persistent(from: &str, to: &str, text: &str) -> Option<String> {
    if let Some(hit) = cache_get(from, to, text) {
        return Some(hit);
    }
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return None;
    }
    let probe = vec![trimmed.clone()];
    let found = crate::commands::translate_cache::get_many(from, to, &probe).await;
    let hit = found.get(&trimmed)?.clone();
    cache_put_memory_only(from, to, &trimmed, &hit);
    Some(hit)
}

// ---------------------------------------------------------------- 单次调用

/// 单次接口请求的超时。预算必须按「一次请求」给，不能套在整个批次上：
/// 批量接口按 max_batch 分块串行发送，整篇听力（几十组句子）会被拆成多次请求，
/// 若共用一份 2.6s 预算，每次请求都成功也会在预算处被判超时，白等一轮再换下一家。
async fn call_provider(
    spec: &'static ProviderSpec,
    texts: Vec<String>,
    from: String,
    to: String,
) -> Result<Vec<String>, PoolError> {
    match tokio::time::timeout(Duration::from_millis(attempt_timeout_ms()), (spec.call)(texts, from, to)).await {
        Ok(inner) => inner,
        Err(_) => Err(PoolError::net(format!("{} 超时", spec.name))),
    }
}

/// 调用一家接口翻译一组文本。批量接口按 max_batch 分块；非批量接口由这里做有界并发拆分，
/// 而不是逐条串行——串行是原本划词要等七八秒的主因。每次实际请求各自计时（见 call_provider）。
async fn invoke_provider(
    spec: &'static ProviderSpec,
    texts: Vec<String>,
    from: String,
    to: String,
) -> Result<Vec<String>, PoolError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    if spec.batch {
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(spec.max_batch.max(1)) {
            let part = call_provider(spec, chunk.to_vec(), from.clone(), to.clone()).await?;
            if part.len() < chunk.len() {
                return Err(PoolError::bad(format!(
                    "{} 返回条数不足（{}/{}）",
                    spec.name,
                    part.len(),
                    chunk.len()
                )));
            }
            out.extend(part.into_iter().take(chunk.len()));
        }
        return Ok(out);
    }

    if texts.len() == 1 {
        return call_provider(spec, texts, from, to).await;
    }

    let conc = per_text_concurrency();
    let results = futures_util::stream::iter(texts.into_iter().enumerate())
        .map(|(idx, text)| {
            let from = from.clone();
            let to = to.clone();
            async move { (idx, call_provider(spec, vec![text], from, to).await) }
        })
        .buffer_unordered(conc)
        .collect::<Vec<_>>()
        .await;

    let mut out = vec![String::new(); results.len()];
    for (idx, result) in results {
        let part = result?;
        if let Some(first) = part.into_iter().next() {
            out[idx] = first;
        } else {
            return Err(PoolError::bad(format!("{} 返回空结果", spec.name)));
        }
    }
    Ok(out)
}

/// 一次尝试的整体上限：单次请求预算 × 预计要发几轮请求，再封顶。
/// 单请求预算（见 call_provider）保证某一次请求卡住不会吃掉整批的额度；
/// 这层上限保证某家慢接口不会把调用方一直吊着。
fn attempt_budget_ms(spec: &ProviderSpec, text_count: usize) -> u64 {
    const TOTAL_CAP_MS: u64 = 20_000;
    let per_round = if spec.batch {
        spec.max_batch.max(1)
    } else {
        per_text_concurrency()
    };
    let rounds = (text_count.max(1) + per_round - 1) / per_round;
    attempt_timeout_ms()
        .saturating_mul(rounds.max(1) as u64)
        .min(TOTAL_CAP_MS)
}

/// 带健康度记账的单次尝试。
async fn run_attempt(
    spec: &'static ProviderSpec,
    texts: Vec<String>,
    from: String,
    to: String,
) -> Result<Vec<String>, PoolError> {
    note_dispatch(spec.name, spec.base_rtt_ms);
    let started = Instant::now();
    let budget = Duration::from_millis(attempt_budget_ms(spec, texts.len()));
    let result = match tokio::time::timeout(budget, invoke_provider(spec, texts, from, to)).await {
        Ok(inner) => inner,
        Err(_) => Err(PoolError::net(format!("{} 超时", spec.name))),
    };
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    match &result {
        Ok(_) => note_success(spec.name, spec.base_rtt_ms, elapsed_ms),
        Err(e) => note_failure(spec.name, spec.base_rtt_ms, elapsed_ms, e),
    }
    result
}

/// 阶梯式竞速：先发最优的一家，之后每隔 hedge_ms 补一家，任何一家成功即取消其余。
/// 相比「全部并发」少打无用请求，相比「串行重试」又不会把失败时延叠加起来。
pub async fn translate_batch(
    texts: &[String],
    from: &str,
    to: &str,
) -> Result<(Vec<String>, String), PoolError> {
    translate_batch_with_fanout(texts, from, to, fanout()).await
}

/// 同上，但并发家数由调用方指定。逐条补救时传 1：只问当前最优的一家，
/// 否则 N 条 × fanout 家会在瞬间打出成百个请求，直接把免费接口打成风控冷却。
async fn translate_batch_with_fanout(
    texts: &[String],
    from: &str,
    to: &str,
    want: usize,
) -> Result<(Vec<String>, String), PoolError> {
    if texts.is_empty() {
        return Ok((Vec::new(), String::new()));
    }
    warmup();
    let candidates = pick_providers(texts.len(), to, want);
    if candidates.is_empty() {
        return Err(PoolError::bad("翻译接口池为空（检查 GUAXI_TRANSLATE_PROVIDERS）"));
    }
    let hedge = hedge_delay_ms();
    let mut join_set = tokio::task::JoinSet::new();
    for (idx, spec) in candidates.iter().enumerate() {
        let spec: &'static ProviderSpec = spec;
        let texts_c = texts.to_vec();
        let from_c = from.to_string();
        let to_c = to.to_string();
        let delay = hedge.saturating_mul(idx as u64);
        join_set.spawn(async move {
            if delay > 0 {
                sleep(Duration::from_millis(delay + jitter_ms(60))).await;
            }
            (spec.name, run_attempt(spec, texts_c, from_c, to_c).await)
        });
    }

    let mut errors: Vec<String> = Vec::new();
    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok((name, Ok(results))) => {
                join_set.abort_all();
                return Ok((results, name.to_string()));
            }
            Ok((name, Err(e))) => errors.push(format!("{}: {}", name, e.message)),
            Err(e) => errors.push(format!("task: {}", e)),
        }
    }
    Err(PoolError::bad(format!("全部翻译接口失败（{}）", errors.join("；"))))
}

/// 是否所有启用的接口都在冷却中（被风控、额度用完、集体失效）。
/// 这种情况下再发网络请求纯属浪费，也会加深封禁，直接转本地小模型。
pub fn all_enabled_blocked() -> bool {
    let now = Instant::now();
    let mut any_enabled = false;
    for spec in PROVIDERS.iter() {
        if !provider_enabled(spec.name) {
            continue;
        }
        any_enabled = true;
        let health = health_snapshot(spec.name, spec.base_rtt_ms);
        let blocked = health.blocked_until.map(|t| t > now).unwrap_or(false);
        if !blocked {
            return false;
        }
    }
    any_enabled
}

fn needs_retry(text: &str) -> bool {
    let t = text.trim();
    t.is_empty() || t.contains("[Translation Failed]")
}

/// 用本地小模型补齐仍未拿到译文的条目。返回实际用上的引擎名。
async fn fill_with_local_model(
    out: &mut [String],
    texts: &[String],
    from: &str,
    to: &str,
) -> Option<String> {
    if !crate::commands::translate_local::local_translate_enabled() {
        return None;
    }
    let pending: Vec<usize> = out
        .iter()
        .enumerate()
        .filter(|(_, t)| needs_retry(t))
        .map(|(i, _)| i)
        .collect();
    if pending.is_empty() {
        return None;
    }
    let inputs: Vec<String> = pending.iter().map(|i| texts[*i].clone()).collect();
    warn!(
        "[TranslatePool] 免费接口不可用，改用本地小模型翻译 {} 条",
        inputs.len()
    );
    match crate::commands::translate_local::translate_texts_local(&inputs, from, to).await {
        Ok(results) => {
            let mut used = false;
            for (offset, idx) in pending.into_iter().enumerate() {
                let translated = results.get(offset).cloned().unwrap_or_default();
                if translated.trim().is_empty() {
                    continue;
                }
                cache_put_engine(from, to, &texts[idx], &translated, "LocalModel");
                out[idx] = translated;
                used = true;
            }
            if used {
                Some("LocalModel".to_string())
            } else {
                None
            }
        }
        Err(e) => {
            warn!("[TranslatePool] 本地小模型兜底失败: {}", e);
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolBatchOutcome {
    pub texts: Vec<String>,
    pub engine: String,
    pub error: Option<String>,
}

/// 缓存感知的批量翻译：命中缓存的直接返回，未命中的整批走竞速，
/// 整批失败再按条并发补救（而不是串行逐条重试）。
///
/// 缓存分两层：进程内 TTL 缓存挡住连续重复请求，SQLite 持久缓存跨重启复用，
/// 全文翻译第二次打开同一篇文档基本零网络请求。
pub async fn translate_texts(texts: &[String], from: &str, to: &str) -> PoolBatchOutcome {
    let mut out = vec![String::new(); texts.len()];
    let mut pending_idx: Vec<usize> = Vec::new();
    for (idx, text) in texts.iter().enumerate() {
        match cache_get(from, to, text) {
            Some(hit) => out[idx] = hit,
            None => pending_idx.push(idx),
        }
    }

    // 第二层：落盘缓存。一次批量查询把整批未命中的都问掉，不是逐条查。
    if !pending_idx.is_empty() {
        let probe: Vec<String> = pending_idx
            .iter()
            .map(|i| texts[*i].trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        if !probe.is_empty() {
            let found = crate::commands::translate_cache::get_many(from, to, &probe).await;
            if !found.is_empty() {
                pending_idx.retain(|idx| {
                    let key = texts[*idx].trim();
                    match found.get(key) {
                        Some(hit) => {
                            cache_put_memory_only(from, to, key, hit);
                            out[*idx] = hit.clone();
                            false
                        }
                        None => true,
                    }
                });
            }
        }
    }

    if pending_idx.is_empty() {
        return PoolBatchOutcome { texts: out, engine: "Cache".into(), error: None };
    }

    let pending: Vec<String> = pending_idx.iter().map(|i| texts[*i].clone()).collect();
    let mut engine = String::new();
    let mut error: Option<String> = None;

    // 所有免费接口都在冷却 → 不再打网络，直接本地兜底。
    if all_enabled_blocked() {
        if let Some(local_engine) = fill_with_local_model(&mut out, texts, from, to).await {
            return PoolBatchOutcome { texts: out, engine: local_engine, error: None };
        }
        error = Some("所有免费翻译接口均在冷却中，且本地小模型不可用".into());
    }

    match translate_batch(&pending, from, to).await {
        Ok((results, used)) => {
            engine = used;
            for (offset, translated) in results.into_iter().enumerate() {
                if let Some(idx) = pending_idx.get(offset) {
                    cache_put_engine(from, to, &texts[*idx], &translated, &engine);
                    out[*idx] = translated;
                }
            }
        }
        Err(e) => {
            error = Some(e.message.clone());
            let conc = per_text_concurrency();
            let retried = futures_util::stream::iter(pending_idx.clone())
                .map(|idx| {
                    let text = texts[idx].clone();
                    let from = from.to_string();
                    let to = to.to_string();
                    async move { (idx, translate_batch_with_fanout(&[text], &from, &to, 1).await) }
                })
                .buffer_unordered(conc)
                .collect::<Vec<_>>()
                .await;
            for (idx, result) in retried {
                match result {
                    Ok((results, used)) => {
                        let translated = results.into_iter().next().unwrap_or_default();
                        if translated.trim().is_empty() {
                            out[idx] = "[Translation Failed]".into();
                        } else {
                            if engine.is_empty() {
                                engine = used.clone();
                            }
                            cache_put_engine(from, to, &texts[idx], &translated, &used);
                            out[idx] = translated;
                        }
                    }
                    Err(_) => out[idx] = "[Translation Failed]".into(),
                }
            }
        }
    }

    // 网络路径仍有漏网的条目 → 本地小模型补齐。
    if out.iter().any(|t| needs_retry(t)) {
        if let Some(local_engine) = fill_with_local_model(&mut out, texts, from, to).await {
            if engine.is_empty() {
                engine = local_engine;
            } else {
                engine = format!("{}+{}", engine, local_engine);
            }
        }
    }

    if out.iter().all(|t| !t.trim().is_empty() && !t.contains("[Translation Failed]")) {
        error = None;
    }
    PoolBatchOutcome { texts: out, engine, error }
}

/// 供划词面板使用：按健康度取前 want 家，用于并排展示多引擎结果。
pub fn ranked_for_selection(to: &str, want: usize) -> Vec<&'static ProviderSpec> {
    warmup();
    pick_providers(1, to, want)
}

/// 单条文本、指定接口的一次尝试（同样计入健康度）。
pub async fn attempt_one(
    spec: &'static ProviderSpec,
    text: &str,
    from: &str,
    to: &str,
) -> Result<String, PoolError> {
    let results = run_attempt(spec, vec![text.to_string()], from.to_string(), to.to_string()).await?;
    let first = results.into_iter().next().unwrap_or_default();
    if first.trim().is_empty() {
        return Err(PoolError::bad(format!("{} 返回空结果", spec.name)));
    }
    Ok(first)
}

/// Bing 的 token 需要先抓一次翻译页，冷启动要 1~3 秒。
/// 进程内提前在后台预热，避免第一次划词被它拖慢。
pub fn warmup() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if !provider_enabled("Bing") {
            return;
        }
        tokio::spawn(async {
            if let Err(e) = bing_token().await {
                warn!("[TranslatePool] Bing token 预热失败: {}", e.message);
            }
        });
    });
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStatus {
    pub name: String,
    pub region: String,
    pub batch: bool,
    pub enabled: bool,
    pub ema_rtt_ms: f64,
    pub samples: u64,
    pub ok_count: u64,
    pub fail_count: u64,
    pub streak_fails: u32,
    pub throttle_strikes: u32,
    pub cooling_ms: u64,
    pub last_error: Option<String>,
}

/// 池子实时状态，用于排查「哪家挂了 / 哪家在冷却」。
pub fn pool_status() -> Vec<ProviderStatus> {
    let now = Instant::now();
    PROVIDERS
        .iter()
        .map(|spec| {
            let h = health_snapshot(spec.name, spec.base_rtt_ms);
            ProviderStatus {
                name: spec.name.to_string(),
                region: if spec.region == Region::CnDirect { "cn".into() } else { "global".into() },
                batch: spec.batch,
                enabled: provider_enabled(spec.name),
                ema_rtt_ms: (h.ema_rtt_ms * 10.0).round() / 10.0,
                samples: h.samples,
                ok_count: h.ok_count,
                fail_count: h.fail_count,
                streak_fails: h.streak_fails,
                throttle_strikes: h.throttle_strikes,
                cooling_ms: h
                    .blocked_until
                    .filter(|t| *t > now)
                    .map(|t| t.duration_since(now).as_millis() as u64)
                    .unwrap_or(0),
                last_error: h.last_error,
            }
        })
        .collect()
}

// ---------------------------------------------------------------- HTTP 辅助

fn brief(body: &str) -> String {
    let trimmed = body.trim();
    let mut s: String = trimmed.chars().take(180).collect();
    s = s.replace('\n', " ").replace('\r', " ");
    if trimmed.chars().count() > 180 {
        s.push('…');
    }
    s
}

fn looks_like_html(body: &str) -> bool {
    let b = body.trim_start().to_ascii_lowercase();
    b.starts_with("<!doctype html") || b.starts_with("<html")
}

/// 把 HTTP 状态映射成冷却策略：429/403/503 一律视为风控信号。
fn classify_status(code: u16) -> FailKind {
    match code {
        401 | 403 | 407 | 418 | 429 | 503 | 509 => FailKind::Throttled,
        _ => FailKind::Bad,
    }
}

async fn send_text(name: &str, req: reqwest::RequestBuilder) -> Result<String, PoolError> {
    let response = req
        .send()
        .await
        .map_err(|e| PoolError::net(format!("{} 网络错误: {}", name, e)))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| PoolError::net(format!("{} 读取响应失败: {}", name, e)))?;
    if !status.is_success() {
        return Err(PoolError::new(
            classify_status(status.as_u16()),
            format!("{} HTTP {}: {}", name, status.as_u16(), brief(&body)),
        ));
    }
    if body.trim().is_empty() {
        return Err(PoolError::bad(format!("{} 空响应", name)));
    }
    Ok(body)
}

async fn send_json(name: &str, req: reqwest::RequestBuilder) -> Result<serde_json::Value, PoolError> {
    let body = send_text(name, req).await?;
    if looks_like_html(&body) {
        // 返回 HTML 通常是验证码页或风控页，按限流处理。
        return Err(PoolError::new(
            FailKind::Throttled,
            format!("{} 返回 HTML（疑似风控）: {}", name, brief(&body)),
        ));
    }
    serde_json::from_str(&body)
        .map_err(|e| PoolError::bad(format!("{} JSON 解析失败: {} ({})", name, e, brief(&body))))
}

// ---------------------------------------------------------------- 语言码映射

fn lc(lang: &str) -> String {
    lang.trim().to_ascii_lowercase()
}

fn is_auto(lang: &str) -> bool {
    let l = lc(lang);
    l.is_empty() || l == "auto"
}

/// 粗略语种识别，供不支持 auto 的接口使用（有汉字当中文，其余当英文）。
fn detect_rough(text: &str) -> &'static str {
    for c in text.chars() {
        if ('\u{4e00}'..='\u{9fff}').contains(&c) {
            return "zh";
        }
        if ('\u{3040}'..='\u{30ff}').contains(&c) {
            return "ja";
        }
        if ('\u{ac00}'..='\u{d7af}').contains(&c) {
            return "ko";
        }
        if ('\u{0400}'..='\u{04ff}').contains(&c) {
            return "ru";
        }
    }
    "en"
}

/// zh / zh-cn / zh-hans → 简体；zh-tw / zh-hant → 繁体。
fn zh_variant(lang: &str) -> Option<bool> {
    match lc(lang).as_str() {
        "zh" | "zh-cn" | "zh-hans" | "zh_hans" | "zh-chs" => Some(true),
        "zh-tw" | "zh-hant" | "zh_hant" | "zh-cht" | "zh-hk" => Some(false),
        _ => None,
    }
}

fn map_bing(lang: &str) -> String {
    match zh_variant(lang) {
        Some(true) => "zh-Hans".into(),
        Some(false) => "zh-Hant".into(),
        None => {
            if is_auto(lang) {
                "auto-detect".into()
            } else {
                lc(lang)
            }
        }
    }
}

fn map_google(lang: &str) -> String {
    match zh_variant(lang) {
        Some(true) => "zh-CN".into(),
        Some(false) => "zh-TW".into(),
        None => if is_auto(lang) { "auto".into() } else { lc(lang) },
    }
}

fn map_transmart(lang: &str) -> String {
    match zh_variant(lang) {
        Some(true) => "zh".into(),
        Some(false) => "zh-TW".into(),
        None => if is_auto(lang) { "auto".into() } else { lc(lang) },
    }
}

fn map_youdao(lang: &str) -> String {
    match zh_variant(lang) {
        Some(true) => "zh-CHS".into(),
        Some(false) => "zh-CHT".into(),
        None => if is_auto(lang) { "auto".into() } else { lc(lang) },
    }
}

/// 彩云只认 `{源}2{目标}` 且语种代码是两字母，源语言可以是 auto。
fn map_caiyun_pair(from: &str, to: &str, sample: &str) -> String {
    let short = |lang: &str| -> String {
        match zh_variant(lang) {
            Some(_) => "zh".into(),
            None => {
                let l = lc(lang);
                l.split(['-', '_']).next().unwrap_or("en").to_string()
            }
        }
    };
    let src = if is_auto(from) { "auto".to_string() } else { short(from) };
    let tgt = short(to);
    let tgt = if tgt.is_empty() { "zh".to_string() } else { tgt };
    // 彩云不支持 auto2auto 之类的组合，源和目标相同时按粗略识别兜底。
    if src == tgt {
        let detected = detect_rough(sample);
        if detected == tgt {
            return format!("{}2{}", if tgt == "zh" { "en" } else { "zh" }, tgt);
        }
        return format!("{}2{}", detected, tgt);
    }
    format!("{}2{}", src, tgt)
}

fn map_mymemory_pair(from: &str, to: &str, sample: &str) -> String {
    let one = |lang: &str| -> String {
        match zh_variant(lang) {
            Some(true) => "zh-CN".into(),
            Some(false) => "zh-TW".into(),
            None => lc(lang),
        }
    };
    let src = if is_auto(from) { detect_rough(sample).to_string() } else { one(from) };
    format!("{}|{}", src, one(to))
}

// ---------------------------------------------------------------- 各家接口实现

/// 腾讯交互翻译。大陆直连、延迟最低，且原生支持一次多条，是池子里的首选。
fn transmart_provider(texts: Vec<String>, from: String, to: String) -> ProviderFuture {
    Box::pin(async move {
        let payload = serde_json::json!({
            "header": {
                "fn": "auto_translation",
                "client_key": TRANSMART_CLIENT_KEY.as_str(),
            },
            "type": "plain",
            "model_category": "normal",
            "source": { "lang": map_transmart(&from), "text_list": texts },
            "target": { "lang": map_transmart(&to) }
        });
        let value = send_json(
            "Transmart",
            http()
                .post("https://transmart.qq.com/api/imt")
                .header("Content-Type", "application/json")
                .json(&payload),
        )
        .await?;

        let ret_code = value["header"]["ret_code"].as_str().unwrap_or_default();
        if ret_code != "succ" {
            let kind = if ret_code.contains("Auth") || ret_code.contains("Limit") || ret_code.contains("Frequen") {
                FailKind::Throttled
            } else {
                FailKind::Bad
            };
            return Err(PoolError::new(
                kind,
                format!("Transmart ret_code={} {}", ret_code, brief(&value.to_string())),
            ));
        }
        let lines: Vec<String> = value["auto_translation"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|x| x.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default();
        if lines.iter().all(|s| s.trim().is_empty()) {
            return Err(PoolError::bad("Transmart 空结果"));
        }
        // 单条多行输入时接口会按行拆开返回，需要拼回一条。
        if texts.len() == 1 && lines.len() > 1 {
            return Ok(vec![lines.join("\n")]);
        }
        Ok(lines)
    })
}

/// 有道公开演示接口。大陆直连、单条延迟很低，作为 Transmart 的同级备份。
fn youdao_provider(texts: Vec<String>, from: String, to: String) -> ProviderFuture {
    Box::pin(async move {
        let text = texts.into_iter().next().unwrap_or_default();
        let value = send_json(
            "Youdao",
            http().get("https://aidemo.youdao.com/trans").query(&[
                ("q", text.as_str()),
                ("from", map_youdao(&from).as_str()),
                ("to", map_youdao(&to).as_str()),
            ]),
        )
        .await?;

        let code = value["errorCode"]
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| value["errorCode"].as_i64().map(|n| n.to_string()))
            .unwrap_or_default();
        if code != "0" {
            // 411/412/413 是有道的频率与授权类错误码，按风控退避。
            let kind = if matches!(code.as_str(), "411" | "412" | "413" | "202" | "401") {
                FailKind::Throttled
            } else {
                FailKind::Bad
            };
            return Err(PoolError::new(kind, format!("Youdao errorCode={}", code)));
        }
        let out = value["translation"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if out.trim().is_empty() {
            return Err(PoolError::bad("Youdao 空结果"));
        }
        Ok(vec![out])
    })
}

/// 彩云小译公开体验 token。大陆直连、支持一次多条；token 可用 GUAXI_CAIYUN_TOKEN 覆盖。
fn caiyun_provider(texts: Vec<String>, from: String, to: String) -> ProviderFuture {
    Box::pin(async move {
        let sample = texts.first().cloned().unwrap_or_default();
        let trans_type = map_caiyun_pair(&from, &to, &sample);
        let token = env::var("GUAXI_CAIYUN_TOKEN")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "3975l6lr5pcbvidl6jl2".to_string());
        let payload = serde_json::json!({
            "source": texts,
            "trans_type": trans_type,
            "request_id": "guaxi",
            "detect": true,
        });
        let value = send_json(
            "Caiyun",
            http()
                .post("https://api.interpreter.caiyunai.com/v1/translator")
                .header("Content-Type", "application/json")
                .header("x-authorization", format!("token {}", token))
                .json(&payload),
        )
        .await?;

        if value["rc"].as_i64().unwrap_or(-1) != 0 {
            return Err(PoolError::bad(format!(
                "Caiyun rc={} {}",
                value["rc"],
                brief(&value.to_string())
            )));
        }
        let out: Vec<String> = value["target"]
            .as_array()
            .map(|arr| arr.iter().map(|x| x.as_str().unwrap_or_default().to_string()).collect())
            .unwrap_or_default();
        if out.iter().all(|s| s.trim().is_empty()) {
            return Err(PoolError::bad("Caiyun 空结果"));
        }
        Ok(out)
    })
}

/// 必应网页版翻译。
///
/// 注意：老的 `https://edge.microsoft.com/translate/auth` 取 token 的方式已经失效（固定 404），
/// 这也是原来划词老报错的主因。改成抓一次 www.bing.com/translator 页面拿 IG/IID/token，
/// 缓存 8 分钟复用，避免每次翻译都去抓页面（既慢又容易被风控）。
#[derive(Debug, Clone)]
struct BingToken {
    ig: String,
    iid: String,
    key: String,
    token: String,
    expires_at: Instant,
}

static BING_TOKEN: Lazy<tokio::sync::Mutex<Option<BingToken>>> =
    Lazy::new(|| tokio::sync::Mutex::new(None));

async fn bing_token() -> Result<BingToken, PoolError> {
    // 用 async 锁串行化抓取：并发划词时只会有一次抓页面请求。
    let mut guard = BING_TOKEN.lock().await;
    if let Some(cached) = guard.as_ref() {
        if cached.expires_at > Instant::now() {
            return Ok(cached.clone());
        }
    }
    let html = send_text(
        "Bing",
        http()
            .get("https://www.bing.com/translator")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8"),
    )
    .await?;

    let pick = |re: &str| -> String {
        regex::Regex::new(re)
            .ok()
            .and_then(|r| r.captures(&html))
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
            .unwrap_or_default()
    };
    let ig = pick(r#"IG:"([^"]+)""#);
    let iid = pick(r#"data-iid="([^"]+)""#);
    let helper = regex::Regex::new(r#"params_AbusePreventionHelper\s*=\s*\[([0-9]+),"([^"]+)""#)
        .ok()
        .and_then(|r| r.captures(&html))
        .map(|c| {
            (
                c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default(),
                c.get(2).map(|m| m.as_str().to_string()).unwrap_or_default(),
            )
        })
        .unwrap_or_default();

    if ig.is_empty() || helper.0.is_empty() || helper.1.is_empty() {
        return Err(PoolError::new(
            FailKind::Throttled,
            "Bing 页面未取到 token（可能被风控或页面结构变更）",
        ));
    }
    let fresh = BingToken {
        ig,
        iid: if iid.is_empty() { "translator.5023".into() } else { iid },
        key: helper.0,
        token: helper.1,
        expires_at: Instant::now() + Duration::from_secs(8 * 60),
    };
    *guard = Some(fresh.clone());
    Ok(fresh)
}

fn bing_provider(texts: Vec<String>, from: String, to: String) -> ProviderFuture {
    Box::pin(async move {
        let text = texts.into_iter().next().unwrap_or_default();
        let tk = bing_token().await?;
        let url = format!(
            "https://www.bing.com/ttranslatev3?isVertical=1&&IG={}&IID={}",
            tk.ig, tk.iid
        );
        let value = send_json(
            "Bing",
            http()
                .post(&url)
                .header("Referer", "https://www.bing.com/translator")
                .header("Origin", "https://www.bing.com")
                .form(&[
                    ("fromLang", map_bing(&from).as_str()),
                    ("text", text.as_str()),
                    ("to", map_bing(&to).as_str()),
                    ("token", tk.token.as_str()),
                    ("key", tk.key.as_str()),
                ]),
        )
        .await;

        let value = match value {
            Ok(v) => v,
            Err(e) => {
                // token 过期时必应会直接拒绝，清缓存让下一次重新抓页面。
                if e.kind == FailKind::Throttled {
                    *BING_TOKEN.lock().await = None;
                }
                return Err(e);
            }
        };

        let out = value
            .get(0)
            .and_then(|v| v.get("translations"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if out.trim().is_empty() {
            return Err(PoolError::bad(format!("Bing 空结果 {}", brief(&value.to_string()))));
        }
        Ok(vec![out])
    })
}

/// Chrome 划词字典用的谷歌接口。支持一次多条，返回体最干净；需要国际网络。
fn google_dict_provider(texts: Vec<String>, from: String, to: String) -> ProviderFuture {
    Box::pin(async move {
        let sl = map_google(&from);
        let tl = map_google(&to);
        let mut query: Vec<(&str, String)> = vec![
            ("client", "dict-chrome-ex".into()),
            ("sl", sl),
            ("tl", tl),
        ];
        for text in &texts {
            query.push(("q", text.clone()));
        }
        let value = send_json(
            "GoogleDict",
            http().get("https://clients5.google.com/translate_a/t").query(&query),
        )
        .await?;

        let arr = value
            .as_array()
            .ok_or_else(|| PoolError::bad("GoogleDict 响应格式异常"))?;
        // 指定源语言时返回 ["译文", ...]，sl=auto 时返回 [["译文","en"], ...]，两种都要吃下。
        let out: Vec<String> = if arr.iter().all(|x| x.is_string()) {
            arr.iter().map(|x| x.as_str().unwrap_or_default().to_string()).collect()
        } else {
            arr.iter()
                .map(|x| {
                    x.as_array()
                        .and_then(|inner| inner.first())
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string()
                })
                .collect()
        };
        if out.iter().all(|s| s.trim().is_empty()) {
            return Err(PoolError::bad("GoogleDict 空结果"));
        }
        Ok(out)
    })
}

/// 谷歌 gtx 接口。长文本分句返回，需要拼接；一次只能一条。
fn google_gtx_provider(texts: Vec<String>, from: String, to: String) -> ProviderFuture {
    Box::pin(async move {
        let text = texts.into_iter().next().unwrap_or_default();
        let value = send_json(
            "Google",
            http()
                .get("https://translate.googleapis.com/translate_a/single")
                .query(&[
                    ("client", "gtx"),
                    ("sl", map_google(&from).as_str()),
                    ("tl", map_google(&to).as_str()),
                    ("dt", "t"),
                    ("q", text.as_str()),
                ]),
        )
        .await?;

        let parts = value
            .get(0)
            .and_then(|v| v.as_array())
            .ok_or_else(|| PoolError::bad("Google 响应格式异常"))?;
        let mut out = String::new();
        for part in parts {
            if let Some(s) = part.get(0).and_then(|v| v.as_str()) {
                out.push_str(s);
            }
        }
        if out.trim().is_empty() {
            return Err(PoolError::bad("Google 空结果"));
        }
        Ok(vec![out])
    })
}

/// MyMemory：无需 key 的国际接口，速度一般，作为最后的兜底。
fn mymemory_provider(texts: Vec<String>, from: String, to: String) -> ProviderFuture {
    Box::pin(async move {
        let text = texts.into_iter().next().unwrap_or_default();
        let pair = map_mymemory_pair(&from, &to, &text);
        let value = send_json(
            "MyMemory",
            http()
                .get("https://api.mymemory.translated.net/get")
                .query(&[("q", text.as_str()), ("langpair", pair.as_str()), ("de", "guaxi@local")]),
        )
        .await?;

        let status = value["responseStatus"]
            .as_i64()
            .or_else(|| value["responseStatus"].as_str().and_then(|s| s.parse().ok()))
            .unwrap_or(0);
        let out = value["responseData"]["translatedText"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let quota_hit = value["quotaFinished"].as_bool().unwrap_or(false)
            || out.contains("MYMEMORY WARNING")
            || out.contains("QUERY LENGTH LIMIT");
        if quota_hit || status == 429 || status == 403 {
            return Err(PoolError::new(FailKind::Throttled, "MyMemory 配额或频率受限"));
        }
        if status != 200 || out.trim().is_empty() {
            return Err(PoolError::bad(format!("MyMemory responseStatus={}", status)));
        }
        Ok(vec![out])
    })
}

