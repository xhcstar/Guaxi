//! 终端调试工坊 —— 远程执行 / 定时采集 / coredump / 二进制分析（批6：A6/A7/G/H）。
//!
//! A6/A7/G 复用 ssh_terminal 的连接，通过 exec 通道跑命令并收集输出；A7 把周期
//! 采集写成带 `**** 时间戳 ****` banner 的文件，直接喂回快照分析管道。H 用 goblin
//! 解析本地 ELF/PE 并算各段熵值（识别加密/压缩段）。

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use russh::ChannelMsg;
use serde::Serialize;
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

use crate::commands::ssh_terminal::connect_authenticated;

fn tools_dir(app: &AppHandle, sub: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {e}"))?
        .join("remote_terminal")
        .join(sub);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {e}"))?;
    Ok(dir)
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// 在一个 exec 通道上跑命令，收集全部输出直到 EOF。
async fn exec_collect(app: &AppHandle, profile_id: &str, command: &str) -> Result<String, String> {
    let handle = connect_authenticated(app, profile_id).await?;
    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    channel
        .exec(true, command.as_bytes())
        .await
        .map_err(|e| format!("执行命令失败: {e}"))?;
    let mut out: Vec<u8> = Vec::new();
    loop {
        match channel.wait().await {
            Some(ChannelMsg::Data { ref data }) | Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                out.extend_from_slice(data);
            }
            Some(ChannelMsg::ExitStatus { .. }) | None => break,
            _ => {}
        }
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

#[derive(Serialize)]
pub struct ExecResult {
    pub output: String,
    pub saved_path: String,
}

/// A6：远程命令执行，输出落盘（可再导入分析管道）。
#[command]
pub async fn ssh_exec_capture(
    app: AppHandle,
    profile_id: String,
    command: String,
) -> Result<ExecResult, String> {
    let output = exec_collect(&app, &profile_id, &command).await?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let path = tools_dir(&app, "exec_captures")?.join(format!("exec-{ts}.log"));
    let banner = format!(
        "********************************************\n{}\n********************************************\n{}\n",
        chrono::Local::now().to_rfc2822(),
        command
    );
    std::fs::write(&path, format!("{banner}{output}"))
        .map_err(|e| format!("写入失败: {e}"))?;
    Ok(ExecResult { output, saved_path: path.to_string_lossy().to_string() })
}

/// G：coredump 事后分析——远端 gdb batch 一次抽全部线程栈 + 寄存器。
#[command]
pub async fn gdb_coredump_analyze(
    app: AppHandle,
    profile_id: String,
    program: String,
    core: String,
) -> Result<ExecResult, String> {
    let cmd = format!(
        "gdb -q -batch -ex 'set pagination off' -ex 'thread apply all bt full' -ex 'info registers' {} {}",
        shell_quote(&program),
        shell_quote(&core)
    );
    let output = exec_collect(&app, &profile_id, &cmd).await?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let path = tools_dir(&app, "coredumps")?.join(format!("core-{ts}.txt"));
    std::fs::write(&path, &output).map_err(|e| format!("写入失败: {e}"))?;
    Ok(ExecResult { output, saved_path: path.to_string_lossy().to_string() })
}

// ============================ A7：定时采集 ============================

struct Collector {
    stop: Arc<AtomicBool>,
}

type CollectorMap = HashMap<String, Collector>;
static COLLECTORS: OnceLock<Mutex<CollectorMap>> = OnceLock::new();

fn collectors() -> &'static Mutex<CollectorMap> {
    COLLECTORS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn shutdown_all_collectors() {
    if let Ok(g) = collectors().lock() {
        for (_id, c) in g.iter() {
            c.stop.store(true, Ordering::SeqCst);
        }
    }
}

#[derive(Serialize)]
pub struct CollectorHandle {
    pub id: String,
    pub path: String,
}

/// A7：周期性在远端跑命令，把每次输出加 banner 追加到文件——直接喂回快照管道。
#[command]
pub async fn ssh_start_periodic(
    app: AppHandle,
    profile_id: String,
    command: String,
    interval_secs: u64,
    label: Option<String>,
) -> Result<CollectorHandle, String> {
    let interval = interval_secs.max(1);
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let safe: String = label
        .unwrap_or_else(|| "periodic".into())
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let path = tools_dir(&app, "periodic")?.join(format!("{safe}-{ts}.log"));
    let path_str = path.to_string_lossy().to_string();

    let id = Uuid::new_v4().to_string();
    let stop = Arc::new(AtomicBool::new(false));
    collectors()
        .lock()
        .map_err(|_| "采集器锁失败".to_string())?
        .insert(id.clone(), Collector { stop: stop.clone() });

    let app_c = app.clone();
    tokio::spawn(async move {
        while !stop.load(Ordering::SeqCst) {
            match exec_collect(&app_c, &profile_id, &command).await {
                Ok(output) => {
                    let banner = format!(
                        "********************************************\n{}\n********************************************\n{}\n",
                        chrono::Local::now().to_rfc2822(),
                        command
                    );
                    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
                        let _ = f.write_all(banner.as_bytes());
                        let _ = f.write_all(output.as_bytes());
                        let _ = f.write_all(b"\n");
                    }
                }
                Err(_) => {}
            }
            // 分段睡眠以便及时响应停止。
            for _ in 0..interval {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    });

    Ok(CollectorHandle { id, path: path_str })
}

#[command]
pub fn ssh_stop_periodic(id: String) -> Result<(), String> {
    if let Some(c) = collectors().lock().map_err(|_| "采集器锁失败".to_string())?.remove(&id) {
        c.stop.store(true, Ordering::SeqCst);
    }
    Ok(())
}

// ============================ H：二进制 / ELF / PE 分析 ============================

#[derive(Serialize)]
pub struct SectionInfo {
    pub name: String,
    pub size: u64,
    pub vaddr: String,
    pub entropy: f64,
}

#[derive(Serialize)]
pub struct BinaryReport {
    pub format: String,
    pub arch: String,
    pub overall_entropy: f64,
    pub sections: Vec<SectionInfo>,
}

/// 香农熵（bit/byte，0..8）：高熵段多为加密/压缩，低熵段多为代码/零填充。
fn entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u64; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let len = data.len() as f64;
    let mut e = 0.0;
    for c in counts {
        if c > 0 {
            let p = c as f64 / len;
            e -= p * p.log2();
        }
    }
    e
}

/// H1/H2：解析本地 ELF/PE，列出各段 + 熵值。
#[command]
pub fn binary_analyze(path: String) -> Result<BinaryReport, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("读取文件失败: {e}"))?;
    let overall = entropy(&bytes);
    let obj = goblin::Object::parse(&bytes).map_err(|e| format!("解析失败: {e}"))?;
    match obj {
        goblin::Object::Elf(elf) => {
            let mut sections = Vec::new();
            for sh in &elf.section_headers {
                let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("").to_string();
                let off = sh.sh_offset as usize;
                let sz = sh.sh_size as usize;
                let slice = bytes.get(off..off.saturating_add(sz)).unwrap_or(&[]);
                sections.push(SectionInfo {
                    name,
                    size: sz as u64,
                    vaddr: format!("0x{:x}", sh.sh_addr),
                    entropy: entropy(slice),
                });
            }
            Ok(BinaryReport {
                format: "ELF".into(),
                arch: format!("machine=0x{:x}", elf.header.e_machine),
                overall_entropy: overall,
                sections,
            })
        }
        goblin::Object::PE(pe) => {
            let mut sections = Vec::new();
            for s in &pe.sections {
                let name = s.name().unwrap_or("").to_string();
                let off = s.pointer_to_raw_data as usize;
                let sz = s.size_of_raw_data as usize;
                let slice = bytes.get(off..off.saturating_add(sz)).unwrap_or(&[]);
                sections.push(SectionInfo {
                    name,
                    size: sz as u64,
                    vaddr: format!("0x{:x}", s.virtual_address),
                    entropy: entropy(slice),
                });
            }
            Ok(BinaryReport {
                format: "PE".into(),
                arch: format!("machine=0x{:x}", pe.header.coff_header.machine),
                overall_entropy: overall,
                sections,
            })
        }
        _ => Ok(BinaryReport {
            format: "unknown".into(),
            arch: String::new(),
            overall_entropy: overall,
            sections: Vec::new(),
        }),
    }
}
