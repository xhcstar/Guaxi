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

/// 跨机源码对照（远端来源）：读取 gdb 所在主机上的源码文件（截断到 max_bytes），
/// 供 GDB 调试器的源码面板按当前帧 file:line 对照高亮。
#[command]
pub async fn ssh_read_text_file(
    app: AppHandle,
    profile_id: String,
    path: String,
    max_bytes: Option<u64>,
) -> Result<String, String> {
    let cap = max_bytes.unwrap_or(1_048_576).max(1);
    let cmd = format!("head -c {cap} -- {}", shell_quote(&path));
    exec_collect(&app, &profile_id, &cmd).await
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

// ============================ 计算器：证书 / 密钥 / CSR / ASN.1 解析 ============================

fn oid_to_string(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::new();
    let first = bytes[0] as u32;
    parts.push((first / 40).to_string());
    parts.push((first % 40).to_string());
    let mut val: u64 = 0;
    for &b in &bytes[1..] {
        val = (val << 7) | (b & 0x7f) as u64;
        if b & 0x80 == 0 {
            parts.push(val.to_string());
            val = 0;
        }
    }
    parts.join(".")
}

fn asn1_tag_name(tag: u8) -> &'static str {
    if tag & 0xc0 == 0x80 {
        return "[context]";
    }
    match tag & 0x1f {
        0x01 => "BOOLEAN",
        0x02 => "INTEGER",
        0x03 => "BIT STRING",
        0x04 => "OCTET STRING",
        0x05 => "NULL",
        0x06 => "OID",
        0x0c => "UTF8String",
        0x10 => "SEQUENCE",
        0x11 => "SET",
        0x13 => "PrintableString",
        0x16 => "IA5String",
        0x17 => "UTCTime",
        0x18 => "GeneralizedTime",
        _ => "?",
    }
}

fn asn1_preview(tag: u8, content: &[u8]) -> String {
    let t = tag & 0x1f;
    match t {
        0x06 => oid_to_string(content),
        0x0c | 0x13 | 0x16 | 0x17 | 0x18 => String::from_utf8_lossy(content).trim().to_string(),
        0x02 => {
            if content.len() <= 8 {
                let mut v: i128 = 0;
                for &b in content {
                    v = (v << 8) | b as i128;
                }
                format!("{v}")
            } else {
                format!("0x{}", hex::encode(&content[..content.len().min(24)]))
            }
        }
        _ => {
            let n = content.len().min(24);
            let mut s = format!("0x{}", hex::encode(&content[..n]));
            if content.len() > n {
                s.push('…');
            }
            s
        }
    }
}

fn asn1_dump(data: &[u8], out: &mut String, depth: usize) {
    let mut i = 0usize;
    while i + 1 < data.len() {
        let tag = data[i];
        i += 1;
        let first = data[i];
        i += 1;
        let len = if first & 0x80 == 0 {
            first as usize
        } else {
            let n = (first & 0x7f) as usize;
            if n == 0 || n > 4 || i + n > data.len() {
                break;
            }
            let mut l = 0usize;
            for _ in 0..n {
                l = (l << 8) | data[i] as usize;
                i += 1;
            }
            l
        };
        let end = (i + len).min(data.len());
        let content = &data[i..end];
        let constructed = tag & 0x20 != 0;
        let indent = "  ".repeat(depth);
        let name = asn1_tag_name(tag);
        if constructed && depth < 12 {
            out.push_str(&format!("{indent}{name} (len {len})\n"));
            asn1_dump(content, out, depth + 1);
        } else {
            out.push_str(&format!("{indent}{name} (len {len}) {}\n", asn1_preview(tag, content)));
        }
        i = end;
    }
}

#[derive(Serialize)]
pub struct CryptoReport {
    pub kind: String,
    pub summary: Vec<(String, String)>,
    pub asn1: String,
}

/// 计算器：解析 PEM/DER 的证书 / CSR / 公钥 / 私钥 / 任意 ASN.1，返回摘要 + ASN.1 树。
#[command]
pub fn crypto_inspect(input: String) -> Result<CryptoReport, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("输入为空".to_string());
    }

    // 1) 取 DER 字节 + 标签
    let (label, der): (String, Vec<u8>) = if trimmed.contains("-----BEGIN") {
        let block = pem::parse(trimmed.as_bytes()).map_err(|e| format!("PEM 解析失败: {e}"))?;
        (block.tag().to_string(), block.contents().to_vec())
    } else {
        // 尝试 base64（去空白），再尝试 hex
        let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
        use base64::Engine;
        if let Ok(b) = base64::engine::general_purpose::STANDARD.decode(&compact) {
            ("DER(base64)".to_string(), b)
        } else if let Ok(b) = hex::decode(&compact) {
            ("DER(hex)".to_string(), b)
        } else {
            return Err("无法识别输入：需要 PEM、base64 或 hex 编码的 DER".to_string());
        }
    };

    let mut summary: Vec<(String, String)> = vec![("PEM 标签".into(), label.clone())];

    // 2) 证书走 x509-parser 出富字段
    let kind = if label.contains("CERTIFICATE") && !label.contains("REQUEST") {
        match x509_parser::parse_x509_certificate(&der) {
            Ok((_, cert)) => {
                summary.push(("版本".into(), format!("v{}", cert.version().0 + 1)));
                summary.push(("序列号".into(), cert.raw_serial_as_string()));
                summary.push(("主题".into(), cert.subject().to_string()));
                summary.push(("颁发者".into(), cert.issuer().to_string()));
                summary.push(("有效期起".into(), cert.validity().not_before.to_string()));
                summary.push(("有效期止".into(), cert.validity().not_after.to_string()));
                summary.push(("签名算法".into(), cert.signature_algorithm.algorithm.to_id_string()));
                "X.509 证书".to_string()
            }
            Err(e) => {
                summary.push(("证书解析".into(), format!("失败: {e}")));
                "证书(解析失败)".to_string()
            }
        }
    } else if label.contains("REQUEST") {
        "证书请求 CSR".to_string()
    } else if label.contains("PRIVATE KEY") {
        "私钥".to_string()
    } else if label.contains("PUBLIC KEY") {
        "公钥".to_string()
    } else {
        "ASN.1 结构".to_string()
    };

    summary.push(("DER 长度".into(), format!("{} 字节", der.len())));

    let mut asn1 = String::new();
    asn1_dump(&der, &mut asn1, 0);

    Ok(CryptoReport { kind, summary, asn1 })
}
