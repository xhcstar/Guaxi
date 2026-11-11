//! GPU 构建期的 CUDA 运行库自检。
//!
//! 为什么要有这一步：onnxruntime 的 CUDA EP 加载不起来时，它抛的 C++ 异常会穿透
//! Rust FFI 边界，运行期拦不住——结果只有两种：进程直接 abort，或者静默退回 CPU。
//! 所以「这套 DLL 到底能不能用」必须在打包前问清楚，不能等用户装完才发现
//! 下载的 GPU 版其实一直在跑 CPU。
//!
//! 判定口径与运行期 `guaxiservices::speech::config::cuda_available()` 保持一致：
//! 检查 EP 文件、cudart 的关键导出与 cuDNN 是否齐备；**不**在独立进程里裸加载
//! CUDA EP——那会在 EP 自己的 DllMain 里误报 1114（原因见 `check` 内注释）。
//! 两处口径要一起改，否则会出现「构建说不能用、运行却能用」的偏差。
//!
//! 退出码刻意区分两类问题，让调用方决定是中断构建还是只提醒：
//!   0 = 通过
//!   1 = 用法错误
//!   2 = 打包缺陷：目录里该有的文件没有 —— 构建流程的锅，必须修
//!   3 = 本机跑不起来：文件都在，但加载/初始化失败（驱动、CUDA 版本、没插显卡等）
//!
//! 用法：verify_cuda_runtime [--dir <存放 DLL 的目录>]（默认取当前 exe 所在目录）

use std::path::{Path, PathBuf};

/// EP 库文件名。onnxruntime 是按这个名字去 LoadLibrary 的。
const EP_DLL: &str = "onnxruntime_providers_cuda.dll";
/// cuDNN 9 的主库名，CUDA 12 / 13 两种变体都叫这个名字。
const CUDNN_DLL: &str = "cudnn64_9.dll";

const EXIT_OK: i32 = 0;
const EXIT_USAGE: i32 = 1;
const EXIT_PACKAGING: i32 = 2;
const EXIT_ENVIRONMENT: i32 = 3;

fn usage() {
    println!("用法：verify_cuda_runtime [--dir <存放 CUDA DLL 的目录>]");
    println!("  不带 --dir 时检查当前 exe 所在目录（等价于运行期 onnxruntime 的查找位置）。");
}

fn main() {
    let mut dir: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => match args.next() {
                Some(value) => dir = Some(PathBuf::from(value)),
                None => {
                    eprintln!("[cuda-verify] --dir 后面要跟一个目录");
                    std::process::exit(EXIT_USAGE);
                }
            },
            "--help" | "-h" => {
                usage();
                std::process::exit(EXIT_OK);
            }
            other => {
                eprintln!("[cuda-verify] 未知参数：{}", other);
                usage();
                std::process::exit(EXIT_USAGE);
            }
        }
    }

    let dir = dir.or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
    });
    let Some(dir) = dir else {
        eprintln!("[cuda-verify] 无法确定待检查的目录，请显式传 --dir");
        std::process::exit(EXIT_USAGE);
    };

    println!("[cuda-verify] 检查目录：{}", dir.display());

    if !cfg!(target_os = "windows") {
        // 其它平台不走这套 Windows 加载语义，这里不假装能判断。
        println!("[cuda-verify] 非 Windows 平台，跳过（仅 Windows 需要这份自检）。");
        std::process::exit(EXIT_OK);
    }

    std::process::exit(check(&dir));
}

#[cfg(target_os = "windows")]
fn check(dir: &Path) -> i32 {
    // 驱动优先：没有 nvcuda.dll 时任何 CUDA EP 都不可能工作。但这属于「本机没有可用的
    // N 卡/驱动」，不是打包缺陷——换台有 N 卡的机器就能用，所以单独一个退出码。
    if !nvcuda_present() {
        eprintln!(
            "[cuda-verify] 本机没有 NVIDIA 驱动（System32\\nvcuda.dll 不存在）：\
             这份包在别的机器上仍可能可用，本机不会启用 GPU。"
        );
        return EXIT_ENVIRONMENT;
    }

    // 文件齐不齐属于打包缺陷，先查完再谈加载，免得把「少文件」说成「版本不对」。
    let mut missing = Vec::new();
    for name in [EP_DLL, CUDNN_DLL] {
        if !dir.join(name).is_file() {
            missing.push(name);
        }
    }
    if !missing.is_empty() {
        eprintln!(
            "[cuda-verify] 打包缺陷：{} 里缺少 {}",
            dir.display(),
            missing.join("、")
        );
        return EXIT_PACKAGING;
    }

    // 把待检查目录挂进 DLL 搜索路径，EP 库的 cudart / cublas / cublasLt 才会从那里解析
    // ——与 onnxruntime 在 exe 目录下找依赖是同一回事。
    if !prepend_search_dir(dir) {
        eprintln!("[cuda-verify] 无法把 {} 加入 DLL 搜索路径", dir.display());
        return EXIT_ENVIRONMENT;
    }

    // 不在这里 LoadLibrary CUDA EP。
    //
    // 原因（2026-10-25 实测）：EP 的 DllMain 依赖 onnxruntime 在创建 Env/Session 时
    // 才装配的 shared-provider 回调。进程里裸加载它会在自己的初始化里空指针崩溃
    // （错误码 1114），但 ORT 真正建会话时加载同一个 DLL 完全正常——同一份 EP 已经
    // 在 sherpa CLI 与进程内合成里跑通。裸加载是误报，所以自检改为「运行库齐备性」：
    //   · cudart 具备 EP 需要的 4 个 12.8+ 导出；
    //   · cuDNN 9 能按名加载（EP 建会话时才会用到它）。
    match check_cudart_symbols() {
        Ok(()) => {}
        Err(message) => {
            eprintln!("[cuda-verify] CUDA 运行库不满足要求：{message}");
            return EXIT_ENVIRONMENT;
        }
    }

    // EP 的静态导入还有 cublas / cublasLt。
    for name in ["cublas64_12.dll", "cublasLt64_12.dll"] {
        match load_library_by_name(name) {
            Ok(()) => {}
            Err(code) => {
                eprintln!(
                    "[cuda-verify] {} 加载失败（Windows 错误码={}）：{}",
                    name,
                    code,
                    explain(code)
                );
                return EXIT_ENVIRONMENT;
            }
        }
    }

    match load_library_by_name(CUDNN_DLL) {
        Ok(()) => {}
        Err(code) => {
            eprintln!(
                "[cuda-verify] cuDNN 加载失败（Windows 错误码={}）：{}",
                code,
                explain(code)
            );
            return EXIT_ENVIRONMENT;
        }
    }

    println!("[cuda-verify] 通过：CUDA 运行库与 cuDNN 齐备；EP 将由 onnxruntime 建会话时加载。");
    EXIT_OK
}

#[cfg(not(target_os = "windows"))]
fn check(_dir: &Path) -> i32 {
    EXIT_OK
}

/// 把错误码翻译成人能直接照做的提示。这几条都是实测踩过的。
#[cfg(target_os = "windows")]
fn explain(code: u32) -> &'static str {
    match code {
        126 => "找不到依赖的 DLL（常见是缺 cuDNN、cublas 或 CUDA 运行时）",
        127 => "依赖 DLL 里没有需要的导出符号——通常是 CUDA 运行时版本旧于 EP 库要求的版本",
        1114 => "DLL 初始化例程失败：依赖都找到了，但 EP 在自己的初始化里失败（CUDA 运行时与 EP 版本不配套、驱动不匹配等）",
        _ => "见上面的错误码",
    }
}

#[cfg(target_os = "windows")]
fn nvcuda_present() -> bool {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    PathBuf::from(root).join("System32").join("nvcuda.dll").is_file()
}

#[cfg(target_os = "windows")]
mod win {
    use std::ffi::{c_void, CString, OsStr};
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(lp_lib_file_name: *const u16) -> *mut c_void;
        fn FreeLibrary(h_lib_module: *mut c_void) -> i32;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
        fn SetDllDirectoryW(lp_path_name: *const u16) -> i32;
        fn GetLastError() -> u32;
    }

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(once(0)).collect()
    }

    /// 把目录加到 DLL 搜索路径（排在最前面）。
    pub fn prepend_search_dir(dir: &Path) -> bool {
        let wide = wide(dir.as_os_str());
        unsafe { SetDllDirectoryW(wide.as_ptr()) != 0 }
    }

    /// 校验 cudart 是否具备 CUDA EP 需要的 4 个 12.8+ 导出。
    pub fn check_cudart_symbols() -> Result<(), String> {
        let handle = unsafe { LoadLibraryW(wide(OsStr::new("cudart64_12.dll")).as_ptr()) };
        if handle.is_null() {
            return Err(format!(
                "cudart64_12.dll 无法加载（Windows 错误码={}）",
                unsafe { GetLastError() }
            ));
        }
        let mut missing = Vec::new();
        for symbol in [
            "cudaLibraryLoadData",
            "cudaLibraryGetKernel",
            "cudaLibraryUnload",
            "cudaKernelSetAttributeForDevice",
        ] {
            let name = CString::new(symbol).map_err(|e| e.to_string())?;
            let proc = unsafe { GetProcAddress(handle, name.as_ptr() as *const u8) };
            if proc.is_null() {
                missing.push(symbol);
            }
        }
        unsafe {
            let _ = FreeLibrary(handle);
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "cudart64_12.dll 缺少导出 {}（需要 CUDA 12.8+ 运行时）",
                missing.join("、")
            ))
        }
    }

    /// 按裸文件名加载，走标准搜索顺序（含 prepend_search_dir 加进去的那个目录）。
    pub fn load_library_by_name(name: &str) -> Result<(), u32> {
        let wide = wide(OsStr::new(name));
        let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
        if handle.is_null() {
            return Err(unsafe { GetLastError() });
        }
        unsafe {
            let _ = FreeLibrary(handle);
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
use win::{check_cudart_symbols, load_library_by_name, prepend_search_dir};
