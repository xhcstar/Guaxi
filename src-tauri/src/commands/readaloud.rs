//! 逐句朗读的后端会话。
//!
//! 前端只做两件事：告诉后端"播放头在第几句、想保留多少秒缓冲"，以及播放已经就绪的 WAV。
//! 其余全在这里：原文切分与偏移计算、串行合成、落盘、缓存、容量裁剪、失败兜底。
//!
//! 设计要点（都对应一个具体的踩坑）：
//!
//! 1. **切的是原文，不是清洗后的文本**。前端的当前句高亮/点击定位依赖"段 → 字符偏移"，
//!    清洗会改字符，偏移一错高亮就整体错位。清洗只作用于送引擎的那一份副本。
//! 2. **偏移用 UTF-16 码元**。前端在 JS 里按字符串长度累加文本节点，那就是 UTF-16 码元数；
//!    给字节偏移会让中文全错（一个汉字 3 字节、1 个码元）。
//! 3. **合成串行**。sherpa 的引擎实例不是线程安全的，而且单实例内部已经用 `num_threads`
//!    吃满多核，多路并发只会互相抢 CPU。
//! 4. **引擎用独立实例**（`instance_tag`）。实例锁在整段生成期间一直持有，共用实例的话
//!    逐句朗读会把划词朗读、有声书堵住。
//! 5. **按"时长水位"预取**，不是按句数：用户要的是连续播放，缓冲够就停手，不无脑把整本书合成完。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;
use tracing::{info, warn};

use crate::commands::settingview;
use crate::guaxiservices::bundled_resources;
use crate::guaxiservices::speech::{self, TtsEngineConfig};

/// 朗读专用引擎实例的标签。换标签即换实例，与划词朗读/有声书互不阻塞。
const READALOUD_INSTANCE_TAG: &str = "readaloud";

/// 朗读专用引擎的线程预算。
///
/// 只给 4 线程：合成远快于实时（kokoro 约 180 字耗时 2s，而这段语音要播 40–60s，即约 20 倍速），
/// 留出余量给界面与其它推理，避免一开朗读就把机器占满。
const READALOUD_THREADS: i32 = 4;

/// 段就绪事件：`{ sessionId, index, ok: true, path, durationMs, pauseAfterMs }`，
/// 失败时 `{ sessionId, index, ok: false, error }`。
const EVENT_SEGMENT: &str = "readaloud_segment";

/// 过短的句段合并阈值：相邻句合并后仍不足这个字数就继续并入。
/// 不合并的话，"三个字 + 句号"这类碎片会各付一次引擎调用，预取窗口会被垃圾段占满。
const MIN_SEGMENT_CHARS: usize = 40;

/// 单段字数上限：超过就按子句再切。
/// 一个几百字不分句的段落若整段合成，既让首播等待变长，高亮也会盖住一大片。
const MAX_SEGMENT_CHARS: usize = 120;

/// 没有任何标点的长串（代码块、无标点长文）再切的阈值与目标长度。
///
/// 上游的 `split_oversized_sentence_pro` **只在子句标点处切**，遇到通篇没标点的长串会原样返回，
/// 于是单段可能是几千字：首播要等一次很长的合成，高亮也会盖住整屏。这里按字数硬切兜底。
const HARD_SPLIT_CHARS: usize = 400;
const HARD_SPLIT_TARGET: usize = 300;

/// 连续失败多少段就判定引擎不可用、终止会话——避免无意义地一段段报同样的错。
const MAX_CONSECUTIVE_FAILURES: usize = 5;

/// 缓存目录总量上限，超过就按最旧的会话目录整目录删。
const CACHE_LIMIT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// 未合成段的时长估算（毫秒/字符）：只用于"还要合成到第几段"的水位判断，不需要准。
const ESTIMATED_MS_PER_CHAR: u64 = 220;

// ───────────────────────── 对外类型 ─────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub index: usize,
    /// 段文本（原文切片，已按需合并/再切）。整本留存模式下为空——前端按偏移自己切。
    pub text: String,
    /// 在原文中的 UTF-16 码元偏移，前端据此把段映射成 DOM Range。
    pub start_offset: usize,
    pub end_offset: usize,
    /// 该段播完后的停顿（跟随句尾标点，读起来有语气）。
    pub pause_after_ms: u32,
    /// 磁盘上已有这段音频时给出，前端据此**立刻**起播（不必等合成事件）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached: Option<CachedInfo>,
    /// 整本留存模式下的时间轴（含段后停顿），前端直接拿它驱动高亮与跳转。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedInfo {
    pub path: String,
    pub duration_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareRequest {
    pub session_id: String,
    /// 前端从正文 DOM 抽出的纯文本；偏移都相对它。
    pub text: String,
    /// 书所在的库与文件名：用来定位"这本书自己"的语音留存目录。
    pub library: String,
    pub file_name: String,
    /// 语速，1.0 为原速。进缓存键：不同语速各自缓存。
    #[serde(default = "default_speed")]
    pub speed: f32,
    /// 音色编号；缺省用设置页配的。
    #[serde(default)]
    pub speaker_id: Option<i32>,
}

fn default_speed() -> f32 {
    1.0
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareResponse {
    pub session_id: String,
    pub total_chars: usize,
    pub segments: Vec<Segment>,
    /// 已在缓存里的段数。等于段总数时说明整篇都合成过，起播是瞬时的。
    pub cached_segments: usize,
    /// 本地已缓存的总时长（毫秒），供前端显示"可读时长"。
    pub cached_ms: u64,
    /// `streaming` = 逐段合成流式播放；`complete` = 这本书已留存整本语音，直接播单文件。
    pub mode: String,
    /// `complete` 模式下的整本语音文件路径。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_ms: Option<u64>,
}

/// 整本语音的清单：只记时间轴与身份信息——切分本身是确定性的，重听时按同一份原文重算即可
/// （所以清单不必存每段文本，几 MB 的冗余就省下来了）。
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BookManifest {
    version: u32,
    /// 源文本哈希：正文变了（重新转换、改了原文）就不再复用这份音频。
    text_hash: String,
    speaker: i32,
    speed: f32,
    model: String,
    /// 留存音频的容器格式（`mp3` / `wav`）：决定文件后缀，也决定前端拿哪个文件播。
    format: String,
    sample_rate: u32,
    total_ms: u64,
    segments: Vec<BookSegment>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BookSegment {
    index: usize,
    /// 含段后停顿的区间：停顿期间高亮停在当前句上，跳转也用这条时间轴。
    start_ms: u64,
    end_ms: u64,
}

/// 段状态：None = 待合成，Some(Ok) = 已就绪，Some(Err) = 失败。
type SegmentOutcome = Result<ReadyInfo, String>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadyInfo {
    index: usize,
    path: String,
    duration_ms: u64,
    pause_after_ms: u32,
}

struct Session {
    id: String,
    dir: PathBuf,
    /// 书所属库与文件名：整本语音要存回"这本书自己"的目录里。
    library: String,
    file_name: String,
    /// 源文本哈希：清单里记它，正文变了就不再复用留存的整本语音。
    text_hash: String,
    segments: Vec<Segment>,
    engine: TtsEngineConfig,
    speaker: i32,
    speed: f32,
    outcomes: Mutex<Vec<Option<SegmentOutcome>>>,
    /// 播放头（段序号）与目标缓冲时长，由 `readaloud_prefetch` 更新。
    playhead: AtomicU64,
    target_ms: AtomicU64,
    /// 取消标志：`readaloud_stop` 置位，worker 在段间看到即退出。
    cancel: Arc<AtomicBool>,
    /// worker 唤醒信号（新的预取请求/停止都要叫醒它，否则它可能在长睡）。
    notify: Arc<Notify>,
    worker_running: AtomicBool,
    consecutive_failures: AtomicU64,
    /// 整本语音是否已拼接完成（保证只拼一次）。
    assembled: AtomicBool,
}

impl Session {
    /// 还需要合成到哪一段（不含）：从播放头往后累计时长，达到目标水位即停。
    fn goal_index(&self) -> usize {
        let total = self.segments.len();
        let target = self.target_ms.load(Ordering::Relaxed);
        let mut acc = 0u64;
        let mut index = self.playhead.load(Ordering::Relaxed) as usize;
        let outcomes = self.outcomes.lock().ok();
        while index < total && acc < target {
            let already = outcomes
                .as_ref()
                .and_then(|list| list.get(index))
                .and_then(|slot| slot.as_ref())
                .and_then(|outcome| outcome.as_ref().ok())
                .map(|ready| ready.duration_ms);
            acc += already.unwrap_or_else(|| {
                // 还没合成：按字数估一个时长，够水位判断用就行。
                (self.segments[index].text.chars().count() as u64) * ESTIMATED_MS_PER_CHAR
            });
            index += 1;
        }
        index
    }

    /// 下一个待合成的段：必须是连续的（按顺序合成，保证播放侧拿到的永远是递增序号）。
    fn next_pending(&self) -> Option<usize> {
        let goal = self.goal_index();
        let outcomes = self.outcomes.lock().ok()?;
        for index in 0..goal {
            if outcomes.get(index).map(|slot| slot.is_none()).unwrap_or(false) {
                return Some(index);
            }
        }
        None
    }

    fn record(&self, index: usize, outcome: SegmentOutcome) {
        if let Ok(mut outcomes) = self.outcomes.lock() {
            if let Some(slot) = outcomes.get_mut(index) {
                *slot = Some(outcome);
            }
        }
    }
}

// ───────────────────────── 会话注册表 ─────────────────────────

static SESSIONS: OnceLock<Mutex<HashMap<String, Arc<Session>>>> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<String, Arc<Session>>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

// ───────────────────────── 命令 ─────────────────────────

/// 切分原文、建会话，返回带偏移的段表。同一篇文档重复调用会复用同一目录（缓存命中）。
#[tauri::command]
pub async fn readaloud_prepare(request: PrepareRequest) -> Result<PrepareResponse, String> {
    if request.text.trim().is_empty() {
        return Err("正文为空，无法朗读".to_string());
    }

    // 引擎配置在这里就解析：模型文件缺失等问题要在**起播前**报出来，
    // 而不是等第一句合成失败才让用户面对一个沉默的播放按钮。
    let engine = resolve_readaloud_engine()?;
    let speaker = request.speaker_id.unwrap_or_else(configured_speaker);

    let mut segments = split_segments(&request.text);
    if segments.is_empty() {
        return Err("没有切出可朗读的内容".to_string());
    }

    // 先看这本书有没有留存好的整本语音：有且参数一致就直接用它——
    // 重听零合成，跳句只是一次 seek，这是"再听一遍要快"的全部实现。
    if let Some(complete) = load_complete_book(&request, &engine, speaker, segments.clone()) {
        return Ok(complete);
    }

    let dir = cache_dir(&request.library, &request.file_name, &request.text, &engine, request.speed);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建朗读缓存目录失败：{e}"))?;
    prune_cache(&dir);

    let total = segments.len();
    let mut outcomes = Vec::with_capacity(total);
    let mut cached_segments = 0usize;
    let mut cached_ms = 0u64;
    for segment in segments.iter_mut() {
        let path = segment_path(&dir, segment.index);
        match wav_duration_ms(&path) {
            Some(duration_ms) => {
                cached_segments += 1;
                cached_ms += duration_ms;
                let path_str = path.to_string_lossy().to_string();
                segment.cached = Some(CachedInfo {
                    path: path_str.clone(),
                    duration_ms,
                });
                outcomes.push(Some(Ok(ReadyInfo {
                    index: segment.index,
                    path: path_str,
                    duration_ms,
                    pause_after_ms: segment.pause_after_ms,
                })));
            }
            None => outcomes.push(None),
        }
    }

    let session = Arc::new(Session {
        id: request.session_id.clone(),
        dir,
        library: request.library.clone(),
        file_name: request.file_name.clone(),
        text_hash: text_hash(&request.text),
        segments: segments.clone(),
        engine,
        speaker,
        speed: request.speed,
        outcomes: Mutex::new(outcomes),
        playhead: AtomicU64::new(0),
        target_ms: AtomicU64::new(0),
        cancel: Arc::new(AtomicBool::new(true)),
        notify: Arc::new(Notify::new()),
        worker_running: AtomicBool::new(false),
        consecutive_failures: AtomicU64::new(0),
        assembled: AtomicBool::new(false),
    });

    info!(
        session = request.session_id.as_str(),
        segments = total,
        cached_segments,
        "朗读会话已建立（流式）"
    );

    sessions()
        .lock()
        .map_err(|_| "朗读会话表被占用".to_string())?
        .insert(request.session_id.clone(), session);

    Ok(PrepareResponse {
        session_id: request.session_id,
        total_chars: request.text.chars().count(),
        segments,
        cached_segments,
        cached_ms,
        mode: "streaming".to_string(),
        audio_path: None,
        total_ms: None,
    })
}

/// 声明播放头位置与目标缓冲时长，并确保合成 worker 在跑。**立刻返回**。
#[tauri::command]
pub fn readaloud_prefetch(
    app: AppHandle,
    session_id: String,
    playhead: usize,
    target_seconds: u64,
) -> Result<(), String> {
    let session = find_session(&session_id)?;
    session.playhead.store(playhead as u64, Ordering::Relaxed);
    session
        .target_ms
        .store(target_seconds.max(1) * 1000, Ordering::Relaxed);
    // 允许重新开工：上一次 stop 会把标志置位，这里复位。
    session.cancel.store(false, Ordering::Relaxed);
    ensure_worker(app, session.clone());
    session.notify.notify_waiters();
    Ok(())
}

/// 停止合成（暂停朗读）：worker 会在当前段结束后退出，已生成的音频保留为缓存。
#[tauri::command]
pub fn readaloud_stop(session_id: String) -> Result<(), String> {
    let session = find_session(&session_id)?;
    session.cancel.store(true, Ordering::Relaxed);
    session.notify.notify_waiters();
    Ok(())
}

/// 结束会话：停合成、释放内存会话，并在没有别的会话用朗读引擎时卸载它（回收那份模型）。
#[tauri::command]
pub fn readaloud_drop(session_id: String) -> Result<(), String> {
    let removed = sessions()
        .lock()
        .map_err(|_| "朗读会话表被占用".to_string())?
        .remove(&session_id);
    if let Some(session) = removed {
        session.cancel.store(true, Ordering::Relaxed);
        session.notify.notify_waiters();
        // 整本已留存时，分段音频就是纯冗余副本：删掉，让占用回到"一本书一份 PCM"。
        // 未留存（用户只听了一半就关掉）则保留，下次接着合成。
        if session.assembled.load(Ordering::SeqCst) {
            if let Err(err) = bundled_resources::remove_dir_all_with_retry(&session.dir) {
                warn!(dir = %session.dir.display(), error = err.as_str(), "清理分段朗读音频失败");
            }
        }
        let still_used = sessions()
            .lock()
            .map(|map| map.values().any(|other| other.engine == session.engine))
            .unwrap_or(false);
        if !still_used && speech::release_engine(&session.engine) {
            info!("朗读引擎已卸载，内存已回收");
        }
    }
    Ok(())
}

fn find_session(session_id: &str) -> Result<Arc<Session>, String> {
    sessions()
        .lock()
        .map_err(|_| "朗读会话表被占用".to_string())?
        .get(session_id)
        .cloned()
        .ok_or_else(|| format!("朗读会话不存在：{session_id}"))
}

// ───────────────────────── 合成 worker ─────────────────────────

/// 确保该会话有一个在跑的合成 worker。已跑则只唤醒，不重复起。
fn ensure_worker(app: AppHandle, session: Arc<Session>) {
    if session
        .worker_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    tauri::async_runtime::spawn(async move {
        worker_loop(app, session.clone()).await;
        session.worker_running.store(false, Ordering::SeqCst);
    });
}

/// worker 主循环：顺序合成到水位满足为止；没有待办就睡，等 `notify` 或兜底轮询。
///
/// 每次只做一段，且在段与段之间检查取消标志——这既让暂停能及时生效，
/// 也保证它不会长时间霸住引擎锁（那是与其它 TTS 共存的唯一争用点）。
async fn worker_loop(app: AppHandle, session: Arc<Session>) {
    loop {
        if session.cancel.load(Ordering::Relaxed) {
            return;
        }
        let Some(index) = session.next_pending() else {
            // 水位已满足。等唤醒（预取/停止/结束都会叫醒），兜底 2s 轮询。
            tokio::select! {
                _ = session.notify.notified() => {}
                _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            }
            continue;
        };

        let segment = match session.segments.get(index) {
            Some(segment) => segment.clone(),
            None => return,
        };

        match synthesize_segment(&session, &segment).await {
            Ok(ready) => {
                session.record(index, Ok(ready.clone()));
                session.consecutive_failures.store(0, Ordering::Relaxed);
                if let Err(err) = app.emit(
                    EVENT_SEGMENT,
                    SegmentEvent {
                        session_id: session.id.clone(),
                        index,
                        ok: true,
                        ready: Some(ready),
                        error: None,
                    },
                ) {
                    warn!(error = %err, "朗读段事件广播失败");
                }
            }
            Err(err) => {
                session.record(index, Err(err.clone()));
                let failures = session.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
                warn!(index, error = err.as_str(), "朗读片段合成失败");
                let _ = app.emit(
                    EVENT_SEGMENT,
                    SegmentEvent {
                        session_id: session.id.clone(),
                        index,
                        ok: false,
                        ready: None,
                        error: Some(err),
                    },
                );
                if failures as usize >= MAX_CONSECUTIVE_FAILURES {
                    // 引擎级故障：继续逐段重试只会刷屏，直接收工并把错误交给前端提示。
                    warn!(failures, "朗读连续失败达到上限，停止本次合成");
                    session.cancel.store(true, Ordering::Relaxed);
                    return;
                }
            }
        }

        // 全部段落都处理完了就顺手把整本语音留存下来（条件不满足时它立刻返回）。
        maybe_assemble(&app, &session);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SegmentEvent {
    session_id: String,
    index: usize,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    ready: Option<ReadyInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// 合成一段并落盘。失败重试一次——单次抖动不该让用户听到一个缺口。
async fn synthesize_segment(session: &Session, segment: &Segment) -> Result<ReadyInfo, String> {
    let text = text_for_engine(&segment.text);
    if text.is_empty() {
        // 纯空白段（合并后可能出现）：不调引擎，直接给一个零长度音频占位。
        let path = write_silence(&session.dir, segment.index)?;
        return Ok(ReadyInfo {
            index: segment.index,
            path,
            duration_ms: 0,
            pause_after_ms: segment.pause_after_ms,
        });
    }

    let mut last_error = String::new();
    for attempt in 0..2 {
        let config = session.engine.clone();
        let input = text.clone();
        let speaker = session.speaker;
        let speed = session.speed;
        let synthesized = tokio::task::spawn_blocking(move || {
            speech::tts::synthesize(&config, &input, speaker, speed)
        })
        .await
        .map_err(|e| format!("合成任务失败：{e}"))?;

        match synthesized {
            Ok(audio) => {
                let sample_rate = audio.sample_rate.max(1) as f64;
                let duration_ms = (audio.samples.len() as f64 / sample_rate * 1000.0).round() as u64;
                let bytes = audio.to_pcm16_wav();
                let path = segment_path(&session.dir, segment.index);
                std::fs::write(&path, bytes)
                    .map_err(|e| format!("写入朗读音频失败：{e}（{}）", path.display()))?;
                return Ok(ReadyInfo {
                    index: segment.index,
                    path: path.to_string_lossy().to_string(),
                    duration_ms,
                    pause_after_ms: segment.pause_after_ms,
                });
            }
            Err(err) => {
                last_error = err;
                if attempt == 0 {
                    warn!(index = segment.index, error = last_error.as_str(), "合成失败，重试一次");
                }
            }
        }
    }
    Err(last_error)
}

/// 送引擎前的轻量清洗：只压掉换行与多余空白。
///
/// 刻意不做标点归一/去噪那一套：段文本同时是前端高亮的依据，改动字符会让偏移与显示脱节。
/// 引擎对多余空白不敏感，但对"文本和界面上看到的不是同一句"很敏感。
fn text_for_engine(segment: &str) -> String {
    segment.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ───────────────────────── 切分与偏移 ─────────────────────────

/// 把原文切成可朗读的段，并给出每段在原文中的 **UTF-16 码元**偏移。
///
/// 之所以不是字节偏移：前端在 JS 里按字符串长度累加文本节点，长度就是 UTF-16 码元数，
/// 给字节偏移会让中文整体错位（一个汉字 3 字节、1 个码元）。
///
/// 之所以切原文而不是清洗后的文本：每段都必须是原文的**连续子串**，才能用顺序 `find`
/// 精确定位；清洗会改字符，定位就废了。
pub(crate) fn split_segments(text: &str) -> Vec<Segment> {
    let sentences = crate::commands::tts_book::split_into_sentences_pro(text);
    if sentences.is_empty() {
        return Vec::new();
    }

    // 先按顺序把每个分句定位到原文（字节区间），后面合并/再切都在这套坐标里做。
    let located = locate_sequentially(text, &sentences);

    // 过短的相邻句合并：用原始切片当合并后的文本，而不是把两句用空格拼起来——
    // 拼出来的字符串可能不是原文子串（原文里可能是换行），偏移就对不上了。
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut group: Option<(usize, usize, usize)> = None; // (start, end, chars)
    for (start, end) in located {
        let chars = text[start..end].chars().count();
        match group {
            Some((group_start, _, group_chars)) if group_chars < MIN_SEGMENT_CHARS => {
                group = Some((group_start, end, group_chars + chars));
            }
            Some((group_start, group_end, _)) => {
                groups.push((group_start, group_end));
                group = Some((start, end, chars));
            }
            None => group = Some((start, end, chars)),
        }
    }
    if let Some((start, end, _)) = group {
        groups.push((start, end));
    }

    let mut segments = Vec::new();
    for (start, end) in groups {
        let slice = &text[start..end];
        // 超长段按子句再切：整段几百字一起合成，首播等得久，高亮也会盖住一大片。
        if slice.chars().count() > MAX_SEGMENT_CHARS {
            for piece in crate::commands::tts_book::split_oversized_sentence_pro(slice, MAX_SEGMENT_CHARS) {
                push_with_hard_split(&mut segments, piece);
            }
        } else {
            segments.push(slice.to_string());
        }
    }

    // 再把"合并/再切之后的段"按顺序定位一次，得到最终偏移。
    let final_ranges = locate_sequentially(text, &segments);
    let mut out = Vec::with_capacity(segments.len());
    for (index, ((start, end), segment_text)) in final_ranges.into_iter().zip(segments).enumerate() {
        let start_offset = utf16_len(&text[..start]);
        let end_offset = start_offset + utf16_len(&text[start..end]);
        out.push(Segment {
            index,
            pause_after_ms: readaloud_pause_ms(&segment_text),
            text: segment_text,
            start_offset,
            end_offset,
            cached: None,
            start_ms: None,
            end_ms: None,
        });
    }
    out
}

/// 标点切分之后仍然过长的段，按字数硬切。
///
/// 硬切可能落在词中间——但只有"通篇无标点的长串"才会走到这里，那种内容本来就没有可用的
/// 语意边界；宁可切得糙一点，也不要让一段几千字拖长首播等待、把高亮撑满整屏。
fn push_with_hard_split(out: &mut Vec<String>, piece: String) {
    if piece.chars().count() <= HARD_SPLIT_CHARS {
        out.push(piece);
        return;
    }
    let mut buffer = String::new();
    let mut count = 0usize;
    for ch in piece.chars() {
        buffer.push(ch);
        count += 1;
        if count >= HARD_SPLIT_TARGET {
            out.push(std::mem::take(&mut buffer));
            count = 0;
        }
    }
    if !buffer.trim().is_empty() {
        out.push(buffer);
    }
}

/// 顺序把每段定位到原文的字节区间。
///
/// 段都是原文子串，所以从上一段的末尾往后找即可；找不到（理论上不该发生）时给一个零宽区间，
/// 保留该段的音频、只是不高亮它——比整条链崩掉或丢内容都好。
fn locate_sequentially(text: &str, pieces: &[String]) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(pieces.len());
    let mut cursor = 0usize;
    for piece in pieces {
        if piece.is_empty() {
            out.push((cursor, cursor));
            continue;
        }
        match text[cursor..].find(piece.as_str()) {
            Some(relative) => {
                let start = cursor + relative;
                let end = start + piece.len();
                cursor = end;
                out.push((start, end));
            }
            None => {
                warn!(piece = piece.as_str(), "朗读分段在原文中定位失败，该段不高亮");
                out.push((cursor, cursor));
            }
        }
    }
    out
}

/// 段后停顿。复用有声书那套按标点给停顿的规则，但补上英文句末标点：
/// 原规则只认中文句号，英文句子会一律按 110ms 处理，读起来赶。
fn readaloud_pause_ms(text: &str) -> u32 {
    let trimmed = text.trim_end();
    if trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?') {
        return 380;
    }
    crate::commands::tts_book::derive_pause_after_ms(trimmed)
}

fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

// ───────────────────────── 引擎与缓存 ─────────────────────────

/// 解析朗读专用引擎配置（独立实例 + 固定线程预算）。
///
/// 这里顺带把"模型文件不存在"这类问题在起播前报出来：`resolve_tts_engine_config`
/// 会校验模型与 tokens 路径，并给出可操作的报错文案。
fn resolve_readaloud_engine() -> Result<TtsEngineConfig, String> {
    let configured = settingview::get_tts_config();
    let base = speech::resolve_tts_engine_config(
        &configured.model,
        &configured.tokens,
        "",
        &configured.provider,
        None,
    )?;
    Ok(base.with_instance(READALOUD_INSTANCE_TAG, READALOUD_THREADS))
}

fn configured_speaker() -> i32 {
    settingview::get_tts_config()
        .speaker_id
        .trim()
        .parse::<i32>()
        .unwrap_or(0)
}

fn exe_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法确定可执行文件目录".to_string())
}

/// 分段缓存目录：`{exe_dir}/readaloud/{16 位哈希}`。
/// 键里带上文本哈希与音色/语速/模型，所以同一本书改了内容或换了音色会自然落到新目录，不会串音。
fn cache_dir(library: &str, file_name: &str, text: &str, engine: &TtsEngineConfig, speed: f32) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    library.hash(&mut hasher);
    file_name.hash(&mut hasher);
    text.hash(&mut hasher);
    model_label(engine).hash(&mut hasher);
    (speed.to_bits()).hash(&mut hasher);
    let key = hasher.finish();
    exe_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("readaloud")
        .join(format!("{key:016x}"))
}

fn segment_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("{index:05}.wav"))
}

/// 读 WAV 头算出时长。读精确值而不是按文件大小估算：采样率有 22050/24000 两档，
/// 估错会让"可读时长"与实际听感差近一成。返回 None 表示文件不存在或不是本模块写的 WAV。
fn wav_duration_ms(path: &Path) -> Option<u64> {
    use std::io::Read;
    let mut header = [0u8; 44];
    let mut file = std::fs::File::open(path).ok()?;
    file.read_exact(&mut header).ok()?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return None;
    }
    let channels = u16::from_le_bytes([header[22], header[23]]).max(1) as u64;
    let sample_rate = u32::from_le_bytes([header[24], header[25], header[26], header[27]]) as u64;
    let bits = u16::from_le_bytes([header[34], header[35]]) as u64;
    let data_size = u32::from_le_bytes([header[40], header[41], header[42], header[43]]) as u64;
    if sample_rate == 0 || bits == 0 || data_size == 0 {
        return None;
    }
    let frames = data_size / (bits / 8) / channels;
    Some(frames * 1000 / sample_rate)
}

fn write_silence(dir: &Path, index: usize) -> Result<String, String> {
    let buffer = speech::AudioBuffer::new(vec![0.0f32; 220], 22050, 1);
    let path = segment_path(dir, index);
    std::fs::write(&path, buffer.to_pcm16_wav())
        .map_err(|e| format!("写入朗读音频失败：{e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// 缓存容量裁剪：超过上限就按目录修改时间从最旧的开始整目录删，直到降到上限的 80%。
///
/// 不裁的话逐句朗读会把每个字都变成 WAV 永久堆着；有声书那条链路从不删音频，
/// 是因为它按需重跑成本高，而朗读的音频随时可以重新合成。
fn prune_cache(keep_dir: &Path) {
    let root = match keep_dir.parent() {
        Some(parent) => parent,
        None => return,
    };
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    let mut dirs: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
    let mut total = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let size = dir_size(&path);
        let modified = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        total += size;
        dirs.push((path, size, modified));
    }
    if total <= CACHE_LIMIT_BYTES {
        return;
    }

    dirs.sort_by_key(|(_, _, modified)| *modified);
    let target = CACHE_LIMIT_BYTES / 5 * 4;
    for (path, size, _) in dirs {
        if total <= target {
            break;
        }
        if path == keep_dir {
            continue; // 正在用的会话目录不动
        }
        if bundled_resources::remove_dir_all_with_retry(&path).is_ok() {
            total = total.saturating_sub(size);
            info!(dir = %path.display(), "朗读缓存超限，已删除最旧的会话目录");
        }
    }
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.metadata().ok())
                .filter(|meta| meta.is_file())
                .map(|meta| meta.len())
                .sum()
        })
        .unwrap_or(0)
}

// ───────────────────────── 整本语音留存 ─────────────────────────
//
// 听过的整本语音要留下来、和书对应、重听不再合成。做法：合成完全部段落后把分段 WAV
// 拼成一份整本文件，连同一条时间轴清单写进**书自己的目录**；下次 `prepare` 命中就直接播
// 那一个文件——跳句只是一次 seek，零合成。

/// 清单版本：格式变了就作废旧清单（宁可重新合成，也不能拿错的时间轴去驱动高亮）。
const MANIFEST_VERSION: u32 = 1;

/// 整本语音拼接完成时广播：`{ sessionId, path, totalMs }`。
const EVENT_BOOK: &str = "readaloud_book";

/// 这本书的语音留存目录：`{librariesRoot}/Libraries/{library}/{stem}/audio`。
///
/// 放在**书自己的目录**里（与 markdown/、word/、pdf/ 同级），而不是临时缓存目录：
/// 随库一起移动/备份/清理，用户在文件管理器里也能直接找到。路径规则与
/// `reconvert_imported_file` 保持一致。
fn audio_dir_for(library: &str, file_name: &str) -> Result<PathBuf, String> {
    let stem = book_stem(file_name)?;
    let root = settingview::get_libraries_root();
    if root.trim().is_empty() {
        return Err("未配置文件库根目录，无法定位书的语音目录".to_string());
    }
    Ok(PathBuf::from(root)
        .join("Libraries")
        .join(library)
        .join(stem)
        .join("audio"))
}

fn book_stem(file_name: &str) -> Result<String, String> {
    Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .ok_or_else(|| format!("无法从文件名解析书名：{file_name}"))
}

fn text_hash(text: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// 模型身份：清单里记它，换模型后旧音频自动作废（与分段缓存键同一口径）。
fn model_label(engine: &TtsEngineConfig) -> String {
    format!("{:?}|{}", engine.model, engine.provider)
}

/// 这本书是否已留存整本语音；是则返回"直接播单文件"的准备结果。
///
/// 判定刻意严格：文本哈希、音色、语速、模型、段数任一不符都当过期——宁可重新合成，
/// 也不能让用户听到与正文对不上的音频（时间轴与高亮会整体错位）。
fn load_complete_book(
    request: &PrepareRequest,
    engine: &TtsEngineConfig,
    speaker: i32,
    segments: Vec<Segment>,
) -> Option<PrepareResponse> {
    let dir = audio_dir_for(&request.library, &request.file_name).ok()?;
    let stem = book_stem(&request.file_name).ok()?;
    let raw = std::fs::read_to_string(dir.join(format!("{stem}.json"))).ok()?;
    let manifest: BookManifest = serde_json::from_str(&raw).ok()?;
    // 后缀由清单里的格式决定（mp3 / wav 都可能是留存产物，WAV 是编码失败时的兜底）。
    if manifest.format != "mp3" && manifest.format != "wav" {
        return None;
    }
    let audio_path = dir.join(format!("{stem}.{}", manifest.format));
    if !audio_path.is_file() {
        return None;
    }
    if manifest.version != MANIFEST_VERSION
        || manifest.text_hash != text_hash(&request.text)
        || manifest.speaker != speaker
        || (manifest.speed - request.speed).abs() > 1e-3
        || manifest.model != model_label(engine)
        || manifest.segments.len() != segments.len()
    {
        return None;
    }

    let mut segments = segments;
    for (segment, timing) in segments.iter_mut().zip(manifest.segments.iter()) {
        segment.start_ms = Some(timing.start_ms);
        segment.end_ms = Some(timing.end_ms);
    }
    info!(
        book = stem.as_str(),
        total_ms = manifest.total_ms,
        "命中留存的整本语音，重听零合成"
    );
    Some(PrepareResponse {
        session_id: request.session_id.clone(),
        total_chars: request.text.chars().count(),
        cached_segments: segments.len(),
        cached_ms: manifest.total_ms,
        segments,
        mode: "complete".to_string(),
        audio_path: Some(audio_path.to_string_lossy().to_string()),
        total_ms: Some(manifest.total_ms),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BookEvent {
    session_id: String,
    path: String,
    total_ms: u64,
}

/// 全部段落都处理完之后拼接整本语音（每个段落结束后都会来问一次，条件不满足就立刻返回）。
fn maybe_assemble(app: &AppHandle, session: &Arc<Session>) {
    {
        let Ok(outcomes) = session.outcomes.lock() else {
            return;
        };
        if outcomes.iter().any(|slot| slot.is_none()) {
            return; // 还有段没处理完，拼出来就是半本书
        }
    }
    if session.assembled.swap(true, Ordering::SeqCst) {
        return; // 只拼一次
    }

    let app = app.clone();
    // 把 Arc 与需要的数据先克隆成自有值：异步任务要求 'static，借用会逃逸出函数体。
    let owned = session.clone();
    let segments = session.segments.clone();
    let session_id = session.id.clone();
    tauri::async_runtime::spawn(async move {
        let outcome = tokio::task::spawn_blocking(move || assemble_book(&owned, &segments)).await;
        match outcome {
            Ok(Ok((path, total_ms))) => {
                info!(path = %path.display(), total_ms, "整本语音已留存");
                let _ = app.emit(
                    EVENT_BOOK,
                    BookEvent {
                        session_id,
                        path: path.to_string_lossy().to_string(),
                        total_ms,
                    },
                );
            }
            // 拼接失败不影响本次朗读（分段音频还在，本次照常播完）。
            Ok(Err(err)) => warn!(error = err.as_str(), "整本语音拼接失败"),
            Err(err) => warn!(error = %err, "整本语音拼接任务失败"),
        }
    });
}

/// 拼接整本语音并写清单，返回 (文件路径, 总时长毫秒)。
///
/// 写完先落在 `.part`，完整了再改名——重听时不会读到半个文件。
/// **段后停顿要写进音频**：否则重听时整本的节奏与首次朗读不一致，清单时间也对不上。
/// 整本留存：优先 MP3，编码失败再退回 WAV。
///
/// MP3 用 64kbps 单声道——语音够听，体积约为 WAV 的 1/6（一本 10 小时的书约 300MB 而不是 1.7GB）。
/// 编码失败不该让"留存"这件事整体失败：退回 WAV，体积大但一定能成。
fn assemble_book(session: &Session, segments: &[Segment]) -> Result<(PathBuf, u64), String> {
    // 目录问题（没配库根目录等）要直接报出来，别伪装成"编码失败"再去试 WAV。
    let dir = audio_dir_for(&session.library, &session.file_name)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建语音目录失败：{e}"))?;
    match assemble_book_mp3(session, segments, &dir) {
        Ok(result) => Ok(result),
        Err(err) => {
            warn!(error = err.as_str(), "MP3 编码失败，退回 WAV 留存整本语音");
            assemble_book_wav(session, segments, &dir)
        }
    }
}

/// 编码结果：采样率、总时长，以及每段在整本音频里的时间轴。
struct Mp3Book {
    sample_rate: u32,
    total_ms: u64,
    timeline: Vec<BookSegment>,
}

/// MP3 编码核心：按顺序编码各段（含段后停顿）并记录时间轴。
///
/// 与 Session 解耦是为了能单独测——编码参数与时间轴累加是最容易悄悄出错的地方。
/// `inputs` 是 (分段 WAV 路径, 该段之后的停顿毫秒)。
fn encode_book_mp3(inputs: &[(PathBuf, u32)], out_path: &Path) -> Result<Mp3Book, String> {
    use mp3lame_encoder::{Bitrate, Builder, FlushGap, Mode};
    use std::io::Write;

    // 编码器参数必须在开始编码前确定，所以先探一段音频拿采样率与声道数。
    let mut sample_rate = WAV_FALLBACK_RATE;
    let mut channels: u16 = 1;
    for (path, _) in inputs {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if let Some((format, _)) = parse_pcm_wav(&bytes) {
            sample_rate = format.0;
            channels = format.1;
            break;
        }
    }
    // 只支持 sherpa 实际产出的形态。别的形态宁可退回 WAV，也不要编出一份听不了的音频。
    if channels != 1 {
        return Err(format!("整本语音只支持单声道 PCM 输入，实际 {channels} 声道"));
    }

    let mut encoder = Builder::new()
        .ok_or_else(|| "MP3 编码器初始化失败".to_string())?
        .with_sample_rate(sample_rate)
        .map_err(|e| format!("MP3 编码器配置失败：{e}"))?
        .with_num_channels(1)
        .map_err(|e| format!("MP3 编码器配置失败：{e}"))?
        .with_brate(Bitrate::Kbps64)
        .map_err(|e| format!("MP3 编码器配置失败：{e}"))?
        .with_mode(Mode::Mono)
        .map_err(|e| format!("MP3 编码器配置失败：{e}"))?
        .build()
        .map_err(|e| format!("MP3 编码器配置失败：{e}"))?;

    let mut out = std::io::BufWriter::new(
        std::fs::File::create(out_path).map_err(|e| format!("写入整本语音失败：{e}"))?,
    );
    let mut mp3_buffer: Vec<u8> = Vec::with_capacity(128 * 1024);
    let mut cursor_ms: u64 = 0;
    let mut timeline = Vec::with_capacity(inputs.len());

    for (index, (path, pause_after_ms)) in inputs.iter().enumerate() {
        let start_ms = cursor_ms;
        if let Ok(bytes) = std::fs::read(path) {
            if let Some((_, data)) = parse_pcm_wav(&bytes) {
                let samples = pcm16_to_i16(&data);
                encode_samples(&mut encoder, &samples, &mut out, &mut mp3_buffer)?;
                cursor_ms += samples.len() as u64 * 1000 / sample_rate.max(1) as u64;
            }
        }

        // 段后停顿也要写进音频：否则重听时的节奏与首次朗读不一致，清单时间也对不上。
        let pause_ms = *pause_after_ms as u64;
        if pause_ms > 0 {
            let frames = sample_rate as u64 * pause_ms / 1000;
            let silence = vec![0i16; frames as usize];
            encode_samples(&mut encoder, &silence, &mut out, &mut mp3_buffer)?;
            cursor_ms += pause_ms;
        }

        timeline.push(BookSegment {
            index,
            start_ms,
            end_ms: cursor_ms,
        });
    }

    mp3_buffer.clear();
    encoder
        .flush_to_vec::<FlushGap>(&mut mp3_buffer)
        .map_err(|e| format!("MP3 编码收尾失败：{e}"))?;
    out.write_all(&mp3_buffer)
        .map_err(|e| format!("写入整本语音失败：{e}"))?;
    out.flush().map_err(|e| format!("写入整本语音失败：{e}"))?;
    drop(out);

    Ok(Mp3Book {
        sample_rate,
        total_ms: cursor_ms,
        timeline,
    })
}

/// 边拼边编码，不落中间 WAV：一整本几小时的 PCM 先写成文件再转码，等于多占一份 1.7GB 的临时盘。
fn assemble_book_mp3(session: &Session, segments: &[Segment], dir: &Path) -> Result<(PathBuf, u64), String> {
    let stem = book_stem(&session.file_name)?;
    let final_path = dir.join(format!("{stem}.mp3"));
    // 完整了再改名：重听时不会读到半个文件。
    let part_path = dir.join(format!("{stem}.mp3.part"));

    let inputs: Vec<(PathBuf, u32)> = segments
        .iter()
        .map(|segment| {
            (
                segment_path(&session.dir, segment.index),
                segment.pause_after_ms,
            )
        })
        .collect();
    let book = encode_book_mp3(&inputs, &part_path)?;

    std::fs::rename(&part_path, &final_path).map_err(|e| format!("保存整本语音失败：{e}"))?;
    write_manifest(
        dir,
        &stem,
        session,
        book.sample_rate,
        book.total_ms,
        book.timeline,
        "mp3",
    )?;
    Ok((final_path, book.total_ms))
}

/// 分帧送进编码器。一次只处理一帧（1152 样本），几小时的音频也不会把内存撑起来。
fn encode_samples(
    encoder: &mut mp3lame_encoder::Encoder,
    samples: &[i16],
    out: &mut std::io::BufWriter<std::fs::File>,
    mp3_buffer: &mut Vec<u8>,
) -> Result<(), String> {
    use mp3lame_encoder::MonoPcm;
    use std::io::Write;
    const FRAME_SAMPLES: usize = 1152;
    for chunk in samples.chunks(FRAME_SAMPLES) {
        mp3_buffer.clear();
        encoder
            .encode_to_vec(MonoPcm(chunk), mp3_buffer)
            .map_err(|e| format!("MP3 编码失败：{e}"))?;
        out.write_all(mp3_buffer)
            .map_err(|e| format!("写入整本语音失败：{e}"))?;
    }
    Ok(())
}

fn pcm16_to_i16(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}

fn write_manifest(
    dir: &Path,
    stem: &str,
    session: &Session,
    sample_rate: u32,
    total_ms: u64,
    segments: Vec<BookSegment>,
    format: &str,
) -> Result<(), String> {
    let manifest = BookManifest {
        version: MANIFEST_VERSION,
        text_hash: session.text_hash.clone(),
        speaker: session.speaker,
        speed: session.speed,
        model: model_label(&session.engine),
        format: format.to_string(),
        sample_rate,
        total_ms,
        segments,
    };
    let json =
        serde_json::to_string_pretty(&manifest).map_err(|e| format!("序列化语音清单失败：{e}"))?;
    std::fs::write(dir.join(format!("{stem}.json")), json)
        .map_err(|e| format!("写入语音清单失败：{e}"))
}

/// WAV 兜底：与 MP3 路径同形，只是不编码（体积大但一定能成）。
fn assemble_book_wav(session: &Session, segments: &[Segment], dir: &Path) -> Result<(PathBuf, u64), String> {
    use std::io::Write;
    let stem = book_stem(&session.file_name)?;
    let final_path = dir.join(format!("{stem}.wav"));
    let part_path = dir.join(format!("{stem}.wav.part"));

    let mut out = std::fs::File::create(&part_path).map_err(|e| format!("写入整本语音失败：{e}"))?;
    write_wav_header(&mut out, WAV_FALLBACK_RATE, 1, 16, 0)?;

    let mut data_bytes: u64 = 0;
    let mut sample_rate = WAV_FALLBACK_RATE;
    let mut channels: u16 = 1;
    let mut bits: u16 = 16;
    let mut cursor_ms: u64 = 0;
    let mut timeline = Vec::with_capacity(segments.len());

    for segment in segments {
        let start_ms = cursor_ms;
        let bytes = std::fs::read(segment_path(&session.dir, segment.index)).unwrap_or_default();
        if let Some((format, data)) = parse_pcm_wav(&bytes) {
            sample_rate = format.0;
            channels = format.1;
            bits = format.2;
            out.write_all(&data).map_err(|e| format!("写入整本语音失败：{e}"))?;
            data_bytes += data.len() as u64;
            let frame_bytes = (bits as u64 / 8) * channels as u64;
            let frames = if frame_bytes > 0 { data.len() as u64 / frame_bytes } else { 0 };
            cursor_ms += frames * 1000 / sample_rate.max(1) as u64;
        }

        let pause_ms = segment.pause_after_ms as u64;
        if pause_ms > 0 {
            let frame_bytes = (bits as u64 / 8) * channels as u64;
            let silence = sample_rate as u64 * pause_ms / 1000 * frame_bytes;
            write_zeros(&mut out, silence)?;
            data_bytes += silence;
            cursor_ms += pause_ms;
        }

        timeline.push(BookSegment {
            index: segment.index,
            start_ms,
            end_ms: cursor_ms,
        });
    }

    if data_bytes + 36 > u32::MAX as u64 {
        let _ = std::fs::remove_file(&part_path);
        return Err(format!(
            "整本语音超过 WAV 单文件上限（约 4GB，当前 {:.1}GB）",
            data_bytes as f64 / 1024.0 / 1024.0 / 1024.0
        ));
    }

    // 回填真实尺寸后改名：原子替换，重听时不会读到半个文件。
    use std::io::{Seek, SeekFrom};
    out.seek(SeekFrom::Start(0)).map_err(|e| format!("写入整本语音失败：{e}"))?;
    write_wav_header(&mut out, sample_rate, channels, bits, data_bytes as u32)?;
    out.flush().map_err(|e| format!("写入整本语音失败：{e}"))?;
    drop(out);
    std::fs::rename(&part_path, &final_path).map_err(|e| format!("保存整本语音失败：{e}"))?;

    write_manifest(&dir, &stem, session, sample_rate, cursor_ms, timeline, "wav")?;
    Ok((final_path, cursor_ms))
}

/// 取到分段 WAV 的格式与 PCM 数据。复用语音模块里已有的 chunk 遍历（含 LIST/fact 等前置块的容错）。
fn parse_pcm_wav(bytes: &[u8]) -> Option<((u32, u16, u16), Vec<u8>)> {
    let (fmt, data) = speech::wav::parse_wav_chunks(bytes).ok()?;
    if fmt.len() < 16 {
        return None;
    }
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]).max(1);
    let sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let bits = u16::from_le_bytes([fmt[14], fmt[15]]);
    if sample_rate == 0 || bits == 0 {
        return None;
    }
    Some(((sample_rate, channels, bits), data))
}

/// 44 字节的规范 PCM16 WAV 头。自己写而不是复用 `to_pcm16_wav`：整本文件是流式写出的，
/// 尺寸要等写完才能回填，不能先攒在内存里。
fn write_wav_header(
    out: &mut std::fs::File,
    sample_rate: u32,
    channels: u16,
    bits: u16,
    data_size: u32,
) -> Result<(), String> {
    use std::io::Write;
    let channels = channels.max(1);
    let block_align = channels * bits / 8;
    let byte_rate = sample_rate.max(1) * block_align as u32;
    let mut header = [0u8; 44];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&(36u32.saturating_add(data_size)).to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes());
    header[20..22].copy_from_slice(&1u16.to_le_bytes());
    header[22..24].copy_from_slice(&channels.to_le_bytes());
    header[24..28].copy_from_slice(&sample_rate.max(1).to_le_bytes());
    header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    header[32..34].copy_from_slice(&block_align.to_le_bytes());
    header[34..36].copy_from_slice(&bits.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&data_size.to_le_bytes());
    out.write_all(&header).map_err(|e| format!("写入整本语音失败：{e}"))
}

/// 写一段静音（句间停顿）。分块写，避免为几个小时的音频再分配一块大内存。
fn write_zeros(out: &mut std::fs::File, count: u64) -> Result<(), String> {
    use std::io::Write;
    const CHUNK: usize = 64 * 1024;
    let buffer = vec![0u8; CHUNK];
    let mut remaining = count;
    while remaining > 0 {
        let take = CHUNK.min(remaining as usize);
        out.write_all(&buffer[..take])
            .map_err(|e| format!("写入整本语音失败：{e}"))?;
        remaining -= take as u64;
    }
    Ok(())
}

/// 分段音频缺失时兜底用的采样率（实际每段都会带上真实值，写头时会用回填的那份）。
const WAV_FALLBACK_RATE: u32 = 24000;

#[cfg(test)]
mod tests {
    use super::*;

    /// 偏移必须能精确还原段文本——这是高亮与点击定位的地基。
    fn assert_offsets_round_trip(text: &str) {
        let units: Vec<u16> = text.encode_utf16().collect();
        let segments = split_segments(text);
        assert!(!segments.is_empty(), "应切出内容：{text:?}");

        let mut last_end = 0usize;
        for segment in &segments {
            assert!(
                segment.start_offset >= last_end,
                "偏移必须单调不回退：{} -> {}",
                last_end,
                segment.start_offset
            );
            assert!(segment.end_offset >= segment.start_offset);
            last_end = segment.end_offset;

            let restored = String::from_utf16(&units[segment.start_offset..segment.end_offset])
                .expect("偏移必须落在 UTF-16 边界上");
            assert_eq!(
                restored, segment.text,
                "按偏移取回的文本与段文本不一致（中文/emoji 下最容易错）"
            );
        }
    }

    #[test]
    fn offsets_are_utf16_units_not_bytes() {
        // 中文 + emoji + 英文混排：字节偏移会在这里立刻暴露。
        assert_offsets_round_trip(
            "第一句话在这里。Here is the second one! 第三句带个表情 😀 结束。乱入的 English 也要正确。",
        );
    }

    #[test]
    fn offsets_survive_newlines_and_short_lines() {
        assert_offsets_round_trip("标题\n\n这是一段正文，用来测试换行处的偏移是否正确。\n短行\n又一段正文，继续测试。");
    }

    #[test]
    fn tiny_sentences_get_merged() {
        let text = "好。行。可以。这是一句真正长一点点的话，用来把合并阈值撑过去。";
        let segments = split_segments(text);
        // 前三段各只有一两个字，应该被并成一段，而不是各自占一次引擎调用。
        assert!(
            segments.len() < 4,
            "过短分句应被合并，实际段数 {}：{:?}",
            segments.len(),
            segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>()
        );
        assert_offsets_round_trip(text);
    }

    #[test]
    fn long_sentence_with_clauses_is_split() {
        let clause = format!("{}，", "甲".repeat(60));
        let text = format!("开头一句。{}结尾一句。", clause.repeat(4));
        let segments = split_segments(&text);
        assert!(
            segments.len() >= 3,
            "带子句标点的长句应被切开，实际 {} 段",
            segments.len()
        );
        assert!(
            segments.iter().all(|s| s.text.chars().count() <= HARD_SPLIT_CHARS),
            "不应留下超长段：{:?}",
            segments.iter().map(|s| s.text.chars().count()).collect::<Vec<_>>()
        );
        assert_offsets_round_trip(&text);
    }

    /// 通篇没有标点的长串：上游切分器不会动它，得靠这里的硬切兜底。
    #[test]
    fn punctuation_free_run_is_hard_split() {
        let run = "甲".repeat(900);
        let text = format!("开头一句。{run}。结尾一句。");
        let segments = split_segments(&text);
        assert!(
            segments.len() >= 4,
            "无标点长串必须硬切，否则首播要等一次超长合成，实际 {} 段",
            segments.len()
        );
        assert!(
            segments.iter().all(|s| s.text.chars().count() <= HARD_SPLIT_CHARS),
            "硬切后仍应满足上限：{:?}",
            segments.iter().map(|s| s.text.chars().count()).collect::<Vec<_>>()
        );
        assert_offsets_round_trip(&text);
    }

    #[test]
    fn indices_are_sequential_from_zero() {
        let text = "第一段需要写得足够长，长到不会被合并阈值并进下一句里去，这样才能形成独立的一段内容。\
                    第二段同样要写得足够长，确保它自己也是一段而不是被并掉的一部分内容。";
        let segments = split_segments(text);
        assert!(segments.len() >= 2, "应切出多段：{:?}", segments.len());
        for (position, segment) in segments.iter().enumerate() {
            assert_eq!(segment.index, position, "段序号必须是 0..n 连续");
        }
    }

    #[test]
    fn english_sentences_get_longer_pause() {
        // 原规则只认中文句号，英文句末会一律 110ms，读起来太赶。
        assert_eq!(readaloud_pause_ms("This is a sentence."), 380);
        assert_eq!(readaloud_pause_ms("这句话结束了。"), 380);
        assert_eq!(readaloud_pause_ms("这只是逗号，"), 150);
    }

    #[test]
    fn engine_text_is_single_line() {
        assert_eq!(text_for_engine("一\n二\t三   四"), "一 二 三 四");
    }

    /// MP3 编码与时间轴累加：留存链路里最容易悄悄出错的一段（编码参数、单声道假设、
    /// 停顿是否算进时间轴），单独钉住。
    #[test]
    fn mp3_encoding_produces_continuous_timeline() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        // 两段各 0.1 秒的 24kHz 单声道 PCM16，第一段后带 380ms 停顿。
        let mut inputs = Vec::new();
        for index in 0..2 {
            let path = tmp.path().join(format!("{index:05}.wav"));
            let samples = vec![0.1f32; 2400];
            std::fs::write(&path, speech::AudioBuffer::new(samples, 24000, 1).to_pcm16_wav())
                .expect("写分段 wav");
            inputs.push((path, if index == 0 { 380 } else { 0 }));
        }

        let out = tmp.path().join("book.mp3");
        let book = encode_book_mp3(&inputs, &out).expect("MP3 编码应成功");

        assert_eq!(book.sample_rate, 24000, "采样率应取自分段音频");
        assert_eq!(book.timeline.len(), 2);
        assert!(
            out.metadata().expect("mp3 元信息").len() > 256,
            "MP3 文件不应为空"
        );

        let first = &book.timeline[0];
        let second = &book.timeline[1];
        assert!(first.end_ms >= 100, "第一段应约 100ms：{first:?}");
        assert_eq!(second.start_ms, first.end_ms, "时间轴必须首尾相接");
        assert!(
            second.end_ms >= second.start_ms + 90,
            "第二段应约 100ms：{second:?}"
        );
        assert!(
            book.total_ms >= 570,
            "总时长应含句间停顿（100+380+100）：{}",
            book.total_ms
        );
    }
}
