//! 认知层运行时：全局 AppHandle、后台循环、总开关。
//!
//! 四条循环，节奏刻意错开，避免同时抢 llama-server 与磁盘：
//! - **打分循环**（20s）：给新 percept 求向量、算新颖度；顺带补 episode / belief / interest 的向量。
//!   全程本地模型，便宜，所以可以跑得勤。
//! - **审议循环**（默认 5 分钟）：产出提醒 + 治理器投递。会花 token，受每日预算约束。
//! - **简报循环**（5 分钟检查一次）：到了画像里设定的时刻且今天还没出，就出一份。
//! - **日常维护**（每 6 小时检查，每天做一次）：兴趣衰减、轨迹裁剪、策略重训。
//!
//! 所有循环都受总开关 `cognition_enabled` 控制（默认开）。关掉之后只保留
//! 采集入口的写入（percept 仍然进库，只是不打分不审议），这样重新打开时不丢数据。

use crate::cognition::store;
use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};

static APP: OnceCell<AppHandle> = OnceCell::new();
static LOOPS_STARTED: AtomicBool = AtomicBool::new(false);

/// 由 main.rs 的 setup 注入。没有它，后台循环产出的提醒无法广播到前端。
pub fn init_app_handle(handle: AppHandle) {
    if APP.set(handle).is_err() {
        debug!("[cognition] AppHandle 已初始化，忽略重复注入");
    }
}

pub fn app_handle() -> Option<AppHandle> {
    APP.get().cloned()
}

/// 总开关。
pub fn enabled() -> bool {
    store::get_bool("cognition_enabled", true)
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    store::set_setting("cognition_enabled", if on { "1" } else { "0" })?;
    info!("[cognition] 认知层已{}", if on { "开启" } else { "关闭" });
    Ok(())
}

/// 审议间隔（秒）。默认 300：足够及时，又不至于把 llama-server 占满。
fn deliberate_interval_secs() -> u64 {
    store::get_i64("deliberate_interval_secs", 300).clamp(60, 86_400) as u64
}

/// 审议用的模型 id。默认 6（本地），用户可在设置里改成远端以提高判决质量。
fn deliberate_modelid() -> i32 {
    store::get_i64("deliberate_modelid", 6) as i32
}

/// 启动全部后台循环（幂等）。
pub fn start_background_loops() {
    if LOOPS_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    // 首次启动先把库和画像建起来，后面各循环就不必各自兜底。
    match store::ensure_db() {
        Ok(_) => {
            if let Err(e) = crate::cognition::profile::ensure_profile_file() {
                warn!("[cognition] 画像文件初始化失败: {}", e);
            }
            info!("[cognition] cognition.db 就绪");
        }
        Err(e) => {
            warn!("[cognition] cognition.db 初始化失败，认知层将不可用: {}", e);
            return;
        }
    }

    spawn_scoring_loop();
    spawn_deliberate_loop();
    spawn_brief_loop();
    spawn_maintenance_loop();
    // 启动即做两件收拾：
    // 1) 给清洗与质量闸门上线之前入库的旧感知补一次同样的处理（只跑一次）；
    // 2) 清一次未读垃圾——上次运行留下的、用户没看的自动采集内容。
    // 两者都遍历全表、跑正则，用 spawn_blocking 而不是 spawn，避免占住异步工作线程。
    tokio::spawn(async {
        sleep(Duration::from_secs(8)).await;
        let _ = tokio::task::spawn_blocking(|| {
            match crate::cognition::percept::recleanup_legacy() {
                Ok((0, 0)) => {}
                Ok((washed, removed)) => info!(
                    "[cognition] 旧感知重洗：{} 条清掉标签残渣，{} 条判为不合格已删除",
                    washed, removed
                ),
                Err(e) => warn!("[cognition] 旧感知重洗失败: {}", e),
            }
            match crate::cognition::percept::purge_stale_unread(None) {
                Ok(0) => {}
                Ok(n) => info!("[cognition] 启动清理：移除 {} 条未读的自动采集内容", n),
                Err(e) => warn!("[cognition] 启动清理失败: {}", e),
            }
        })
        .await;
    });
    info!("[cognition] 后台循环已启动");
}

/// 退出前的收尾。
///
/// WAL + 每次写入即提交，所以不需要刷盘；这里做的是清理：把过了保留期、
/// 用户没看的自动采集内容删掉，别让它们跨越一次重启继续占着收件箱和磁盘。
/// 同步执行是刻意的——此刻 tokio 运行时可能已经在收尾，spawn 出去的任务不保证跑完。
pub fn shutdown() {
    match crate::cognition::percept::purge_on_exit() {
        Ok(0) => debug!("[cognition] 认知层收尾：无需清理"),
        Ok(n) => info!("[cognition] 认知层收尾：清理 {} 条未读的自动采集内容", n),
        Err(e) => debug!("[cognition] 认知层收尾清理跳过: {}", e),
    }
}

/// 打分 + 向量补齐。全部本地模型。
fn spawn_scoring_loop() {
    tokio::spawn(async move {
        // 启动后先等一会儿，让主界面先起来、llama-server 不与首屏抢资源。
        sleep(Duration::from_secs(25)).await;
        loop {
            if !enabled() {
                sleep(Duration::from_secs(30)).await;
                continue;
            }
            match crate::cognition::percept::score_pending(24).await {
                Ok(0) => {}
                Ok(n) => debug!("[cognition] 已给 {} 条感知打分", n),
                Err(e) => debug!("[cognition] 感知打分暂不可用: {}", e),
            }
            // 三个补齐任务各让一小步，避免某一类把 embedding 服务长期占住。
            let _ = crate::cognition::episode::embed_pending(16).await;
            let _ = crate::cognition::belief::embed_pending(16).await;
            let _ = crate::cognition::profile::ensure_interest_vectors(16).await;
            sleep(Duration::from_secs(20)).await;
        }
    });
}

/// 审议循环：产出提醒 + 投递。
fn spawn_deliberate_loop() {
    tokio::spawn(async move {
        // 比打分循环晚一点起，保证第一轮审议时已经有打过分的感知可用。
        sleep(Duration::from_secs(90)).await;
        loop {
            if !enabled() {
                sleep(Duration::from_secs(60)).await;
                continue;
            }
            let handle = app_handle();
            let report =
                crate::cognition::nudge::deliberate(handle.as_ref(), deliberate_modelid()).await;
            if report.produced > 0 || report.delivered > 0 {
                info!(
                    "[cognition] 审议：新增 {} 条、投递 {} 条（阈值拦 {}、预算拦 {}、免打扰拦 {}）",
                    report.produced,
                    report.delivered,
                    report.skipped_threshold,
                    report.skipped_budget,
                    report.skipped_quiet
                );
            }
            sleep(Duration::from_secs(deliberate_interval_secs())).await;
        }
    });
}

/// 简报循环：只在到点且今天还没出过时生成一次。
fn spawn_brief_loop() {
    tokio::spawn(async move {
        sleep(Duration::from_secs(150)).await;
        loop {
            if enabled() && crate::cognition::brief::due_now() {
                let handle = app_handle();
                match crate::cognition::brief::generate(
                    handle.as_ref(),
                    deliberate_modelid(),
                    "morning",
                )
                .await
                {
                    Ok(b) => info!("[cognition] 简报已出：{}", b.day),
                    Err(e) => warn!("[cognition] 简报生成失败: {}", e),
                }
            }
            sleep(Duration::from_secs(300)).await;
        }
    });
}

/// 日常维护：兴趣衰减、轨迹裁剪、策略重训。按"上次执行日期"判定，每天一次。
fn spawn_maintenance_loop() {
    tokio::spawn(async move {
        sleep(Duration::from_secs(240)).await;
        loop {
            if enabled() {
                let last = store::get_setting("last_maintenance_day").unwrap_or_default();
                let today = store::today();
                if last != today {
                    match crate::cognition::profile::decay_interests() {
                        Ok(n) => debug!("[cognition] 兴趣衰减 {} 项", n),
                        Err(e) => warn!("[cognition] 兴趣衰减失败: {}", e),
                    }
                    let keep = store::get_i64("episode_keep_days", 180);
                    match crate::cognition::episode::prune(keep) {
                        Ok(n) if n > 0 => info!("[cognition] 裁剪过期轨迹 {} 段", n),
                        Ok(_) => {}
                        Err(e) => warn!("[cognition] 轨迹裁剪失败: {}", e),
                    }
                    match crate::cognition::nudge::train_policy() {
                        Ok(r) => debug!("[cognition] 策略训练：{}", r.reason),
                        Err(e) => warn!("[cognition] 策略训练失败: {}", e),
                    }
                    match crate::cognition::percept::purge_stale_unread(None) {
                        Ok(n) if n > 0 => info!("[cognition] 清理未读采集 {} 条", n),
                        Ok(_) => {}
                        Err(e) => warn!("[cognition] 清理未读采集失败: {}", e),
                    }
                    let _ = store::set_setting("last_maintenance_day", &today);
                }
            }
            sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}
