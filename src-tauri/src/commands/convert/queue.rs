//! 持久化转换队列的后台 worker。
//!
//! 模型：
//! - N 个 worker 循环并发从 SQLite `conversion_tasks` 认领 pending 任务
//!   （认领是原子 UPDATE，天然防重复）。
//! - fast 引擎可并发；marker / mineru（GPU 重引擎）通过 `heavy_lock` 串行化，避免显存争抢。
//! - 每个进度事件都落库 + 广播；进程执行期间轮询取消标志，支持中途 kill。
//! - 启动时把中断态任务退回 pending 实现断点续跑。
//!
//! 事件：
//! - `conversion_progress`（保持旧结构 {current,total,message,fileName}，向后兼容）
//! - `conversion_task_update`（完整任务对象，驱动新的任务面板）

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Notify, Semaphore};
use tokio::time::sleep;
use tracing::{error, info, warn};

use super::engine::{self, Engine};
use super::task_store::{self, ConversionTask};

static QUEUE: OnceLock<Arc<ConversionQueue>> = OnceLock::new();

/// 全局最大并发（含 fast/heavy）。heavy 另受 heavy_lock 串行约束。
const MAX_WORKERS: usize = 3;

/// marker 引擎收尾：清理 surya 派生的推理服务残留进程。
///
/// 这些服务（VLM / 快速布局 / OCR 识别各一个）是 surya 以**分离进程**方式拉起来的，
/// 正常退出时靠 atexit 收掉；但转换被强杀（用户取消、等待失败、应用退出）时 atexit 不会执行，
/// 它们会变成孤儿继续占着内存/显存——上千页的任务里每个服务都是几百 MB 到数 GB，
/// 不收拾会越积越多。因此取消与正常结束两条路径都要扫一遍。
fn cleanup_marker_surya_servers(engine: Engine) {
    if engine == Engine::Marker {
        super::provision::cleanup_surya_servers();
    }
}

#[derive(Clone, Serialize)]
struct LegacyProgress {
    current: u32,
    total: u32,
    message: String,
    #[serde(rename = "fileName", skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failed: Option<bool>,
}

pub struct ConversionQueue {
    app: AppHandle,
    notify: Arc<Notify>,
    /// 重引擎（marker/mineru）串行化。
    heavy_lock: Arc<Semaphore>,
}

impl ConversionQueue {
    /// 在 Tauri setup 中调用一次：启动 worker 池并恢复中断任务。
    pub fn init(app: AppHandle) {
        if QUEUE.get().is_some() {
            return;
        }
        let queue = Arc::new(ConversionQueue {
            app,
            notify: Arc::new(Notify::new()),
            heavy_lock: Arc::new(Semaphore::new(1)),
        });
        let _ = QUEUE.set(queue.clone());

        // 断点续跑：把上次中断的任务退回 pending
        match task_store::requeue_interrupted() {
            Ok(n) if n > 0 => info!(recovered = n, "恢复中断的转换任务"),
            Ok(_) => {}
            Err(e) => warn!(error = %e, "恢复中断任务失败"),
        }

        for worker_id in 0..MAX_WORKERS {
            let q = queue.clone();
            tauri::async_runtime::spawn(async move {
                q.worker_loop(worker_id).await;
            });
        }
        // 唤醒一次以处理已恢复的 pending 任务
        queue.notify.notify_waiters();
    }

    pub fn get() -> Option<Arc<ConversionQueue>> {
        QUEUE.get().cloned()
    }

    /// 入队后唤醒 worker。
    pub fn wake(&self) {
        self.notify.notify_waiters();
    }

    async fn worker_loop(self: Arc<Self>, worker_id: usize) {
        loop {
            let claimed = match task_store::claim_next_pending() {
                Ok(c) => c,
                Err(e) => {
                    error!(worker_id, error = %e, "认领任务失败");
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };

            match claimed {
                Some(task) => {
                    self.emit_task_update(task.id);
                    if let Err(e) = self.run_task(task).await {
                        warn!(worker_id, error = %e, "任务执行返回错误");
                    }
                    // 处理完立即尝试下一个
                }
                None => {
                    // 无任务：等待唤醒或定时轮询（兜底）
                    let notified = self.notify.notified();
                    tokio::select! {
                        _ = notified => {}
                        _ = sleep(Duration::from_secs(30)) => {}
                    }
                }
            }
        }
    }

    async fn run_task(self: &Arc<Self>, task: ConversionTask) -> Result<(), String> {
        let task_id = task.id;
        let file_name = task.file_name.clone();

        // 已被取消？（认领与执行之间的竞态）
        if task_store::is_canceled(task_id).unwrap_or(false) {
            return Ok(());
        }

        self.emit_progress(&file_name, 2, 100, "分析文档...", None);

        // 解析引擎（auto 走探针，MinerU 缺失回退 marker）
        let (eng, probe) =
            engine::resolve_engine(&task.requested_engine, &task.input_path).await;
        let probe_json = probe
            .as_ref()
            .and_then(|p| serde_json::to_string(p).ok());
        if let Err(e) = task_store::set_resolved_engine(task_id, eng.as_str(), probe_json.as_deref())
        {
            warn!(task_id, error = %e, "写入解析引擎失败");
        }
        self.emit_task_update(task_id);

        let engine_label = match eng {
            Engine::Fast => "快速引擎",
            Engine::Marker => "Marker 引擎",
            Engine::Mineru => "MinerU 强引擎",
            Engine::Anydoc => "极速引擎",
        };
        self.emit_progress(
            &file_name,
            5,
            100,
            &format!("使用{}转换...", engine_label),
            None,
        );

        // 极速引擎：纯 Rust 进程内直调，无子进程、无装配、无进度协议，走独立分支。
        if eng == Engine::Anydoc {
            return self
                .run_anydoc_task(task_id, &file_name, &task.input_path, &task.output_dir)
                .await;
        }

        // 重引擎串行化
        let _heavy_permit = if matches!(eng, Engine::Marker | Engine::Mineru) {
            Some(self.heavy_lock.clone().acquire_owned().await.map_err(|e| e.to_string())?)
        } else {
            None
        };

        // 取得 permit 后再次确认未被取消
        if task_store::is_canceled(task_id).unwrap_or(false) {
            return Ok(());
        }

        // 首次使用某引擎可能需要装配依赖（下载 Python/pip 安装），耗时较长。
        // build_plan 内部是阻塞式（含 pip install），放到阻塞线程避免卡住异步运行时。
        self.emit_progress(&file_name, 6, 100, "准备转换引擎（首次可能需下载依赖）...", None);
        let input_path = task.input_path.clone();
        let output_dir = task.output_dir.clone();
        let plan_res = tokio::task::spawn_blocking(move || {
            engine::build_plan(eng, &input_path, &output_dir)
        })
        .await;

        let plan = match plan_res {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => {
                self.finish_failed(task_id, &file_name, &format!("构建转换命令失败: {}", e));
                return Ok(());
            }
            Err(e) => {
                self.finish_failed(task_id, &file_name, &format!("装配引擎任务失败: {}", e));
                return Ok(());
            }
        };

        let engine = plan.engine;
        let expected_md = plan.expected_md_path.clone();
        let mut cmd = plan.command;

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                self.finish_failed(task_id, &file_name, &format!("启动转换进程失败: {}", e));
                return Ok(());
            }
        };

        // 转换进程（托管 python）可能跑几分钟到几十分钟，主程序退出时必须一起结束，
        // 否则会残留一个在后台继续吃 CPU/显存的 python 进程。
        let converter_pid = child.id().unwrap_or(0);
        crate::guaxiservices::process_shutdown::register_managed_pid("doc-convert(python)", converter_pid);

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
        let output_file_holder: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        // stdout：进度 + 结果
        let stdout_handle = if let Some(stdout) = stdout {
            let this = self.clone();
            let fname = file_name.clone();
            let ofh = output_file_holder.clone();
            Some(tauri::async_runtime::spawn(async move {
                let mut reader = BufReader::new(stdout);
                let mut buf = Vec::new();
                loop {
                    buf.clear();
                    match reader.read_until(b'\n', &mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                    let line = String::from_utf8_lossy(&buf);
                    let trimmed = line.trim_matches(&['\r', '\n'][..]).trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let v: serde_json::Value = match serde_json::from_str(trimmed) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    match v.get("type").and_then(|t| t.as_str()) {
                        Some("conversion_progress") => {
                            let current = v.get("current").and_then(|x| x.as_u64()).unwrap_or(0);
                            let total = v.get("total").and_then(|x| x.as_u64()).unwrap_or(100);
                            let message = v
                                .get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("转换中...")
                                .to_string();
                            let _ = task_store::update_progress(
                                task_id,
                                current as i64,
                                total as i64,
                                None,
                                Some(&message),
                            );
                            this.emit_progress(&fname, current as u32, total as u32, &message, None);
                            this.emit_task_update(task_id);
                        }
                        Some("conversion_result") => {
                            if let Some(p) = v.get("output_file").and_then(|p| p.as_str()) {
                                if let Ok(mut g) = ofh.lock() {
                                    *g = Some(p.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }))
        } else {
            None
        };

        // stderr：保留尾部用于报错
        let stderr_handle = if let Some(stderr) = stderr {
            let tail = stderr_tail.clone();
            let fname = file_name.clone();
            Some(tauri::async_runtime::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut buf = Vec::new();
                loop {
                    buf.clear();
                    match reader.read_until(b'\n', &mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                    let line = String::from_utf8_lossy(&buf);
                    let trimmed = line.trim_matches(&['\r', '\n'][..]).trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(mut g) = tail.lock() {
                        if g.len() >= 200 {
                            g.pop_front();
                        }
                        g.push_back(trimmed.to_string());
                    }
                    warn!(file_name = fname.as_str(), line = trimmed, "converter stderr");
                }
            }))
        } else {
            None
        };

        // 等待进程结束，同时轮询取消
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {
                    if task_store::is_canceled(task_id).unwrap_or(false) {
                        crate::guaxiservices::process_shutdown::unregister_managed_pid(converter_pid);
                        let _ = child.kill().await;
                        info!(task_id, "任务被取消，已终止子进程");
                        cleanup_marker_surya_servers(engine);
                        // 取消状态已在 DB，直接返回
                        if let Some(h) = stdout_handle {
                            let _ = h.await;
                        }
                        if let Some(h) = stderr_handle {
                            let _ = h.await;
                        }
                        self.emit_task_update(task_id);
                        return Ok(());
                    }
                    sleep(Duration::from_millis(400)).await;
                }
                Err(e) => break Err(e.to_string()),
            }
        };

        if let Some(h) = stdout_handle {
            let _ = h.await;
        }
        if let Some(h) = stderr_handle {
            let _ = h.await;
        }
        crate::guaxiservices::process_shutdown::unregister_managed_pid(converter_pid);
        cleanup_marker_surya_servers(engine);

        let status = match status {
            Ok(s) => s,
            Err(e) => {
                self.finish_failed(task_id, &file_name, &format!("等待转换进程失败: {}", e));
                return Ok(());
            }
        };

        if !status.success() {
            let tail = stderr_tail
                .lock()
                .ok()
                .map(|g| {
                    let mut v: Vec<String> = g.iter().rev().take(60).cloned().collect();
                    v.reverse();
                    v.join("\n")
                })
                .unwrap_or_default();
            let msg = if tail.trim().is_empty() {
                format!("转换进程异常退出: {}", status)
            } else {
                format!("转换失败: {}\n{}", status, tail)
            };
            self.finish_failed(task_id, &file_name, &msg);
            return Ok(());
        }

        // 校验产物
        let reported = output_file_holder.lock().ok().and_then(|g| g.clone());
        let md_path = reported
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| expected_md.clone());

        if md_path.exists() {
            match task_store::mark_succeeded(task_id, md_path.to_str()) {
                Ok(true) => {
                    self.emit_progress(&file_name, 100, 100, "转换完成", None);
                    self.emit_task_update(task_id);
                    info!(task_id, engine = eng.as_str(), md = %md_path.display(), "转换成功");
                }
                // 转换期间被取消：保持 canceled，不广播成功事件。
                Ok(false) => self.emit_task_update(task_id),
                Err(e) => error!(task_id, "写入成功状态出错：{}", e),
            }
        } else {
            self.finish_failed(
                task_id,
                &file_name,
                &format!("转换结束但未找到输出文件: {}", expected_md.display()),
            );
        }

        Ok(())
    }

    /// 极速引擎（anydoc）执行路径：进程内转换，无子进程、无装配、无进度协议。
    ///
    /// 转换本身是阻塞的（整份读入 + 解析），放到阻塞线程池执行；耗时通常在毫秒级，
    /// 因此不做进度轮询，也无法中途 kill——只在转换前后各查一次取消标志。
    /// panic 隔离由 `engine::convert_with_anydoc` 内部负责，这里的 JoinError 是第二道保险。
    async fn run_anydoc_task(
        self: &Arc<Self>,
        task_id: i64,
        file_name: &str,
        input_path: &str,
        output_dir: &str,
    ) -> Result<(), String> {
        if task_store::is_canceled(task_id).unwrap_or(false) {
            return Ok(());
        }

        let (_, expected_md) = engine::expected_md_path_for(input_path, output_dir);
        self.emit_progress(file_name, 20, 100, "极速引擎转换中...", None);

        let in_path = PathBuf::from(input_path);
        let out_path = expected_md.clone();
        let result =
            tokio::task::spawn_blocking(move || engine::convert_with_anydoc(&in_path, &out_path))
                .await;

        // 转换期间被取消：DB 已是 canceled，不再回写成功。
        if task_store::is_canceled(task_id).unwrap_or(false) {
            self.emit_task_update(task_id);
            return Ok(());
        }

        match result {
            Ok(Ok(())) => {
                if !expected_md.exists() {
                    self.finish_failed_terminal(
                        task_id,
                        file_name,
                        &format!("转换结束但未找到输出文件: {}", expected_md.display()),
                    );
                    return Ok(());
                }
                match task_store::mark_succeeded(task_id, expected_md.to_str()) {
                    Ok(true) => {
                        self.emit_progress(file_name, 100, 100, "转换完成", None);
                        info!(task_id, engine = "anydoc", md = %expected_md.display(), "转换成功");
                    }
                    // 已被取消或已是终态：保持原状，不广播成功。
                    Ok(false) => {}
                    Err(e) => error!(task_id, "写入成功状态出错：{}", e),
                }
                self.emit_task_update(task_id);
            }
            // 转换返回的确定性错误（加密/扫描件/格式不支持/结构损坏/超限/内部 panic）重试无用。
            Ok(Err(e)) => self.finish_failed_terminal(task_id, file_name, &e),
            Err(e) => self.finish_failed_terminal(
                task_id,
                file_name,
                &format!("极速引擎任务异常: {}", e),
            ),
        }

        Ok(())
    }

    /// 标记失败：若还有重试额度则回落 pending（自动重试），否则终态失败。
    fn finish_failed(&self, task_id: i64, file_name: &str, error: &str) {
        match task_store::mark_failed(task_id, error) {
            Ok(true) => {
                warn!(task_id, "转换失败，将自动重试：{}", error);
                self.emit_progress(file_name, 0, 100, "转换失败，等待重试", Some(true));
                self.wake();
            }
            Ok(false) => {
                error!(task_id, "转换最终失败：{}", error);
                self.emit_progress(file_name, 0, 100, "转换失败", Some(true));
            }
            Err(e) => error!(task_id, "写入失败状态出错：{}", e),
        }
        self.emit_task_update(task_id);
    }

    /// 终态失败：确定性错误（文档加密、扫描件需 OCR、格式不支持、结构损坏、超出安全限制、
    /// 解析库 panic）重试也不会变好，直接置 failed——不消耗重试额度，也不会在 UI 上
    /// 反复出现「等待重试」。
    fn finish_failed_terminal(&self, task_id: i64, file_name: &str, error: &str) {
        if let Err(e) = task_store::mark_failed_terminal(task_id, error) {
            error!(task_id, "写入失败状态出错：{}", e);
        }
        error!(task_id, "极速引擎最终失败：{}", error);
        self.emit_progress(file_name, 0, 100, "转换失败", Some(true));
        self.emit_task_update(task_id);
    }

    fn emit_progress(
        &self,
        file_name: &str,
        current: u32,
        total: u32,
        message: &str,
        failed: Option<bool>,
    ) {
        let _ = self.app.emit(
            "conversion_progress",
            LegacyProgress {
                current,
                total,
                message: message.to_string(),
                file_name: Some(file_name.to_string()),
                failed,
            },
        );
    }

    fn emit_task_update(&self, task_id: i64) {
        if let Ok(Some(task)) = task_store::get_task(task_id) {
            let _ = self.app.emit("conversion_task_update", &task);
        }
    }
}
