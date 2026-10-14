//! GPU 构建期的 CUDA 运行库自检。
//!
//! 为什么要有这一步：onnxruntime 的 CUDA EP 加载不起来时，它抛的 C++ 异常会穿透
//! Rust FFI 边界，运行期拦不住——结果只有两种：进程直接 abort，或者静默退回 CPU。
//! 所以「这套 DLL 到底能不能用」必须在打包前问清楚，不能等用户装完才发现
//! 下载的 GPU 版其实一直在跑 CPU。
//!
//! 判定口径与运行期 `guaxiservices::speech::config::cuda_available()` 保持一致：
//! 加载 EP 库（顺带覆盖它对 cudart / cublas / cublasLt 的静态导入），再按名加载 cuDNN。
//! 两处口径要一起改，否则会出现「构建说能用、运行却不用」的偏差。
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

    match load_library(&dir.join(EP_DLL)) {
        Ok(()) => {}
        Err(code) => {
            eprintln!(
                "[cuda-verify] CUDA EP 加载失败（Windows 错误码={}）：{}",
                code,
                explain(code)
            );
            return EXIT_ENVIRONMENT;
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

    println!("[cuda-verify] 通过：CUDA EP 与 cuDNN 均可加载，GPU 可启用。");
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
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(lp_lib_file_name: *const u16) -> *mut std::ffi::c_void;
        fn FreeLibrary(h_lib_module: *mut std::ffi::c_void) -> i32;
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

    /// 只加载、不取符号：导入表解析本身就能同时覆盖「依赖缺失」和「依赖版本旧、没有需要的导出」。
    pub fn load_library(path: &Path) -> Result<(), u32> {
        let wide = wide(path.as_os_str());
        let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
        if handle.is_null() {
            return Err(unsafe { GetLastError() });
        }
        unsafe {
            let _ = FreeLibrary(handle);
        }
        Ok(())
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
use win::{load_library, load_library_by_name, prepend_search_dir};
