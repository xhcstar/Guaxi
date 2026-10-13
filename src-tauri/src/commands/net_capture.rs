//! 网络工具箱 —— 原始抓包 / 注入（运行时动态加载 Npcap 的 wpcap.dll）。
//!
//! 不链接 Npcap SDK、不需要用户装 SDK：运行时用 libloading 打开系统里的
//! wpcap.dll（Windows，随 Npcap/Wireshark 安装）/ libpcap（*nix）。机器没装时
//! `net_pcap_status` 返回未就绪 + 引导安装,不影响其它网络功能与默认构建。
//! 抓到的每个包以 hex 经事件 `net-capture-packet` 推前端。

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use libloading::Library;
use serde::Serialize;
use tauri::{command, AppHandle, Emitter};
use uuid::Uuid;

// -------- wpcap / libpcap FFI 结构 --------

#[repr(C)]
struct PcapIf {
    next: *mut PcapIf,
    name: *const c_char,
    description: *const c_char,
    addresses: *mut c_void,
    flags: c_uint,
}

#[repr(C)]
struct Timeval {
    tv_sec: std::os::raw::c_long,
    tv_usec: std::os::raw::c_long,
}

#[repr(C)]
struct PcapPkthdr {
    ts: Timeval,
    caplen: c_uint,
    len: c_uint,
}

#[repr(C)]
struct BpfProgram {
    bf_len: c_uint,
    bf_insns: *mut c_void,
}

type PcapT = c_void;

const ERRBUF: usize = 256;

/// 进程内只加载一次的 pcap 动态库句柄。
static PCAP_LIB: OnceLock<Result<Library, String>> = OnceLock::new();

fn lib() -> Result<&'static Library, String> {
    let candidates: &[&str] = if cfg!(windows) {
        &["wpcap.dll"]
    } else if cfg!(target_os = "macos") {
        &["libpcap.dylib", "libpcap.A.dylib"]
    } else {
        &["libpcap.so.1", "libpcap.so.0.8", "libpcap.so"]
    };
    let loaded = PCAP_LIB.get_or_init(|| {
        for name in candidates {
            // SAFETY: 仅加载系统 pcap 库并调用其标准 C API。
            if let Ok(l) = unsafe { Library::new(name) } {
                return Ok(l);
            }
        }
        Err("未检测到 Npcap/libpcap。Windows 请安装 Npcap(或 Wireshark 自带)。".to_string())
    });
    match loaded {
        Ok(l) => Ok(l),
        Err(e) => Err(e.clone()),
    }
}

#[derive(Serialize)]
pub struct PcapStatus {
    pub installed: bool,
    pub message: String,
}

/// 打开网络工具箱时探测：是否可用原始抓包（wpcap/libpcap 是否就绪）。
#[command]
pub fn net_pcap_status() -> PcapStatus {
    match lib() {
        Ok(_) => PcapStatus {
            installed: true,
            message: "抓包引擎就绪(Npcap/libpcap 已加载)。".to_string(),
        },
        Err(e) => PcapStatus { installed: false, message: e },
    }
}

#[derive(Serialize)]
pub struct IfaceInfo {
    pub name: String,
    pub desc: String,
}

#[derive(Serialize, Clone)]
struct CapturePacket {
    session_id: String,
    index: u64,
    len: usize,
    hex: String,
}

struct CaptureHandle {
    stop: Arc<AtomicBool>,
}
type CaptureMap = HashMap<String, CaptureHandle>;
static CAPTURES: OnceLock<Mutex<CaptureMap>> = OnceLock::new();
fn captures() -> &'static Mutex<CaptureMap> {
    CAPTURES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 当前在跑的抓包会话数（设置页「服务器」用）。
pub fn active_capture_count() -> usize {
    captures().lock().map(|guard| guard.len()).unwrap_or(0)
}

pub fn shutdown_all_captures() {
    if let Ok(g) = captures().lock() {
        for (_id, h) in g.iter() {
            h.stop.store(true, Ordering::SeqCst);
        }
    }
}

unsafe fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

/// 列出可抓包网卡。
#[command]
pub fn net_list_interfaces() -> Result<Vec<IfaceInfo>, String> {
    let lib = lib()?;
    unsafe {
        let findalldevs: libloading::Symbol<
            unsafe extern "C" fn(*mut *mut PcapIf, *mut c_char) -> c_int,
        > = lib.get(b"pcap_findalldevs\0").map_err(|e| e.to_string())?;
        let freealldevs: libloading::Symbol<unsafe extern "C" fn(*mut PcapIf)> =
            lib.get(b"pcap_freealldevs\0").map_err(|e| e.to_string())?;

        let mut alldevs: *mut PcapIf = std::ptr::null_mut();
        let mut errbuf = [0i8; ERRBUF];
        if findalldevs(&mut alldevs, errbuf.as_mut_ptr()) != 0 {
            return Err(format!("枚举网卡失败: {}", cstr(errbuf.as_ptr())));
        }
        let mut out = Vec::new();
        let mut cur = alldevs;
        while !cur.is_null() {
            out.push(IfaceInfo {
                name: cstr((*cur).name),
                desc: cstr((*cur).description),
            });
            cur = (*cur).next;
        }
        freealldevs(alldevs);
        Ok(out)
    }
}

/// 开始抓包。filter 为 BPF 表达式(如 "tcp port 80")，可空。
#[command]
pub fn net_capture_start(
    app: AppHandle,
    device: String,
    filter: Option<String>,
    max_packets: Option<u64>,
) -> Result<String, String> {
    // 先验证库可用（错误信息更友好）
    lib()?;
    let sid = Uuid::new_v4().to_string();
    let stop = Arc::new(AtomicBool::new(false));
    captures()
        .lock()
        .map_err(|_| "锁失败".to_string())?
        .insert(sid.clone(), CaptureHandle { stop: stop.clone() });

    let sid_thread = sid.clone();
    std::thread::spawn(move || {
        if let Err(e) = capture_loop(&app, &sid_thread, &device, filter, max_packets, &stop) {
            let _ = app.emit(
                "net-capture-error",
                serde_json::json!({ "session_id": sid_thread, "message": e }),
            );
        }
        if let Ok(mut g) = captures().lock() {
            g.remove(&sid_thread);
        }
    });
    Ok(sid)
}

fn capture_loop(
    app: &AppHandle,
    sid: &str,
    device: &str,
    filter: Option<String>,
    max_packets: Option<u64>,
    stop: &Arc<AtomicBool>,
) -> Result<(), String> {
    let lib = lib()?;
    unsafe {
        let open_live: libloading::Symbol<
            unsafe extern "C" fn(*const c_char, c_int, c_int, c_int, *mut c_char) -> *mut PcapT,
        > = lib.get(b"pcap_open_live\0").map_err(|e| e.to_string())?;
        let next_ex: libloading::Symbol<
            unsafe extern "C" fn(*mut PcapT, *mut *mut PcapPkthdr, *mut *const u8) -> c_int,
        > = lib.get(b"pcap_next_ex\0").map_err(|e| e.to_string())?;
        let close: libloading::Symbol<unsafe extern "C" fn(*mut PcapT)> =
            lib.get(b"pcap_close\0").map_err(|e| e.to_string())?;

        let dev_c = CString::new(device).map_err(|_| "网卡名非法".to_string())?;
        let mut errbuf = [0i8; ERRBUF];
        let handle = open_live(dev_c.as_ptr(), 65535, 1, 500, errbuf.as_mut_ptr());
        if handle.is_null() {
            return Err(format!("打开网卡失败: {}", cstr(errbuf.as_ptr())));
        }

        if let Some(f) = filter.filter(|s| !s.trim().is_empty()) {
            let compile: libloading::Symbol<
                unsafe extern "C" fn(*mut PcapT, *mut BpfProgram, *const c_char, c_int, c_uint) -> c_int,
            > = lib.get(b"pcap_compile\0").map_err(|e| e.to_string())?;
            let setfilter: libloading::Symbol<
                unsafe extern "C" fn(*mut PcapT, *mut BpfProgram) -> c_int,
            > = lib.get(b"pcap_setfilter\0").map_err(|e| e.to_string())?;
            let freecode: libloading::Symbol<unsafe extern "C" fn(*mut BpfProgram)> =
                lib.get(b"pcap_freecode\0").map_err(|e| e.to_string())?;
            let fc = CString::new(f).map_err(|_| "过滤器非法".to_string())?;
            let mut prog = BpfProgram { bf_len: 0, bf_insns: std::ptr::null_mut() };
            if compile(handle, &mut prog, fc.as_ptr(), 1, 0xffffffff) != 0 {
                close(handle);
                return Err("BPF 过滤器编译失败".to_string());
            }
            let ok = setfilter(handle, &mut prog) == 0;
            freecode(&mut prog);
            if !ok {
                close(handle);
                return Err("设置过滤器失败".to_string());
            }
        }

        let limit = max_packets.unwrap_or(u64::MAX);
        let mut idx = 0u64;
        while !stop.load(Ordering::SeqCst) && idx < limit {
            let mut hdr: *mut PcapPkthdr = std::ptr::null_mut();
            let mut data: *const u8 = std::ptr::null();
            let rc = next_ex(handle, &mut hdr, &mut data);
            match rc {
                1 => {
                    if hdr.is_null() || data.is_null() {
                        continue;
                    }
                    let len = (*hdr).caplen as usize;
                    let bytes = std::slice::from_raw_parts(data, len);
                    let _ = app.emit(
                        "net-capture-packet",
                        CapturePacket {
                            session_id: sid.to_string(),
                            index: idx,
                            len,
                            hex: hex::encode(bytes),
                        },
                    );
                    idx += 1;
                }
                0 => continue, // 超时，回头检查 stop
                _ => break,     // -1 错误 / -2 EOF
            }
        }
        close(handle);
    }
    Ok(())
}

#[command]
pub fn net_capture_stop(session_id: String) -> Result<(), String> {
    if let Some(h) = captures().lock().map_err(|_| "锁失败".to_string())?.remove(&session_id) {
        h.stop.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// 在指定网卡上原始注入一个完整帧(hex)。
#[command]
pub fn net_send_raw(device: String, data_hex: String) -> Result<usize, String> {
    let lib = lib()?;
    let compact: String = data_hex.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = hex::decode(&compact).map_err(|e| format!("hex 解码失败: {e}"))?;
    unsafe {
        let open_live: libloading::Symbol<
            unsafe extern "C" fn(*const c_char, c_int, c_int, c_int, *mut c_char) -> *mut PcapT,
        > = lib.get(b"pcap_open_live\0").map_err(|e| e.to_string())?;
        let sendpacket: libloading::Symbol<
            unsafe extern "C" fn(*mut PcapT, *const u8, c_int) -> c_int,
        > = lib.get(b"pcap_sendpacket\0").map_err(|e| e.to_string())?;
        let close: libloading::Symbol<unsafe extern "C" fn(*mut PcapT)> =
            lib.get(b"pcap_close\0").map_err(|e| e.to_string())?;

        let dev_c = CString::new(device).map_err(|_| "网卡名非法".to_string())?;
        let mut errbuf = [0i8; ERRBUF];
        let handle = open_live(dev_c.as_ptr(), 65535, 1, 500, errbuf.as_mut_ptr());
        if handle.is_null() {
            return Err(format!("打开网卡失败: {}", cstr(errbuf.as_ptr())));
        }
        let rc = sendpacket(handle, bytes.as_ptr(), bytes.len() as c_int);
        close(handle);
        if rc != 0 {
            return Err("注入失败".to_string());
        }
    }
    Ok(bytes.len())
}
