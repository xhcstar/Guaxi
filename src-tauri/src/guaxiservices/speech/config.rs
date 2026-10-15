//! 把应用配置解析成引擎配置。
//!
//! 这里保留了旧 CLI 路径的模型判定规则（目录名提示 + 文件存在性），
//! 目的只是换掉执行方式，不改变「同样输入得到同样引擎」这一行为。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tracing::warn;

use crate::guaxiservices::bundled_resources;

use super::asr::{AsrEngineConfig, AsrModel};
use super::tts::{TtsEngineConfig, TtsModel};

/// 默认线程数：语音推理是纯计算，给足核数即可。
/// 注意 sherpa 的 CLI 默认是 1 线程，进程内调用要显式给值才不会退化成单核。
fn default_num_threads() -> i32 {
    // 上限留余量：推理是整核跑的，吃满所有逻辑核会拖慢界面。
    // 但 8 对现在的多核机器过于保守——长音频转写（听力同步一集就是十几分钟）
    // 是纯 CPU 瓶颈，核没用满就是白等，所以放宽到 16 这一档。
    std::thread::available_parallelism()
        .map(|value| value.get() as i32)
        .unwrap_or(4)
        .clamp(1, 16)
}

/// 解析 `utils/...` 这类随包资源路径（含 bundle 解压与 debug 回退），
/// 其它路径原样返回。
pub fn resolve_resource_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return PathBuf::new();
    }
    let normalized = trimmed.replace('\\', "/");
    if normalized == "utils" || normalized.starts_with("utils/") {
        if let Ok(path) = bundled_resources::ensure_relative_dir(&normalized) {
            // 资源层万一解析出空路径，宁可退回原值也不要把它当结果传下去：
            // 空路径最终只会变成一句「文件不存在：」这种没法排查的报错。
            if !path.as_os_str().is_empty() {
                return path;
            }
        }
    }
    PathBuf::from(trimmed)
}

/// 在候选文件里取第一个存在的，用于 lexicon 这类「有哪个用哪个」的资源。
fn first_existing(dir: &Path, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        let path = dir.join(name);
        path.is_file().then(|| path.to_string_lossy().to_string())
    })
}

/// 空值视为未设置，避免把 `VAR=` 当成有效覆盖。
fn env_override(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 解析 TTS 引擎配置。
///
/// `mode_hint` 是调用方对模型族的提示（"kokoro" / "vits"），为空时按模型路径推断。
/// 环境变量 `GUAXI_SHERPA_ONNX_TTS_{MODEL,TOKENS,RULE_FSTS}` 可覆盖对应参数，
/// 便于排查问题时临时换模型而不改应用配置。
pub fn resolve_tts_engine_config(
    model: &str,
    tokens: &str,
    mode_hint: &str,
    provider: &str,
    rule_fsts: Option<&str>,
) -> Result<TtsEngineConfig, String> {
    let model = env_override("GUAXI_SHERPA_ONNX_TTS_MODEL").unwrap_or_else(|| model.to_string());
    let tokens = env_override("GUAXI_SHERPA_ONNX_TTS_TOKENS").unwrap_or_else(|| tokens.to_string());

    let mut model_path = resolve_resource_path(&model);
    // 允许直接给模型目录（旧 CLI 路径支持这种写法），取目录里的 model.onnx。
    if model_path.is_dir() {
        model_path = model_path.join("model.onnx");
    }
    if !model_path.is_file() {
        // 把收到的原值一起带出来：路径为空这类问题光看「不存在：」根本没法排查。
        return Err(format!(
            "TTS 模型文件不存在：{}（收到 model=\"{}\"）",
            model_path.display(),
            model
        ));
    }
    let base_dir = model_path
        .parent()
        .ok_or_else(|| "无法确定 TTS 模型所在目录".to_string())?
        .to_path_buf();

    let tokens_path = if tokens.trim().is_empty() {
        base_dir.join("tokens.txt")
    } else {
        resolve_resource_path(&tokens)
    };
    if !tokens_path.is_file() {
        return Err(format!("TTS tokens 文件不存在：{}", tokens_path.display()));
    }

    let data_dir = base_dir.join("espeak-ng-data");
    let data_dir = data_dir.is_dir().then(|| data_dir.to_string_lossy().to_string());

    let hint = mode_hint.trim().to_lowercase();
    let model_lower = model_path.to_string_lossy().to_lowercase();
    let is_kokoro = hint == "kokoro" || (hint.is_empty() && model_lower.contains("kokoro"));
    let is_vits = hint == "vits"
        || (hint.is_empty() && model_lower.contains("vits"))
        || (!is_kokoro && model_lower.contains("vits"));

    let model = if is_kokoro {
        let voices = base_dir.join("voices.bin");
        let dict_dir = base_dir.join("dict");
        TtsModel::Kokoro {
            model: model_path.to_string_lossy().to_string(),
            tokens: tokens_path.to_string_lossy().to_string(),
            voices: voices.is_file().then(|| voices.to_string_lossy().to_string()),
            data_dir,
            dict_dir: dict_dir.is_dir().then(|| dict_dir.to_string_lossy().to_string()),
            lexicon: first_existing(&base_dir, &["lexicon-zh.txt", "lexicon-us-en.txt", "lexicon-gb-en.txt", "lexicon.txt"]),
        }
    } else if is_vits {
        TtsModel::Vits {
            model: model_path.to_string_lossy().to_string(),
            tokens: tokens_path.to_string_lossy().to_string(),
            data_dir,
            lexicon: first_existing(&base_dir, &["lexicon.txt", "lexicon-zh.txt", "lexicon-us-en.txt"]),
        }
    } else {
        return Err(format!(
            "无法判断 TTS 模型类型（既不像 kokoro 也不像 vits）：{}",
            model_path.display()
        ));
    };

    Ok(TtsEngineConfig {
        model,
        rule_fsts: env_override("GUAXI_SHERPA_ONNX_TTS_RULE_FSTS").or_else(|| {
            rule_fsts
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        }),
        provider: normalize_provider(provider),
        num_threads: default_num_threads(),
        // 默认共用同一个实例；需要独立实例的调用方用 `with_instance` 换标签。
        instance_tag: String::new(),
    })
}

/// 解析 ASR 引擎配置。`model` 既可以是模型目录，也可以是单个 onnx 文件。
pub fn resolve_asr_engine_config(model: &str, provider: &str) -> Result<AsrEngineConfig, String> {
    let model_path = resolve_resource_path(model);
    if !model_path.exists() {
        return Err(format!(
            "ASR 模型不存在：{}（收到 model=\"{}\"）",
            model_path.display(),
            model
        ));
    }

    let asr_model = if model_path.is_dir() {
        detect_dir_model(&model_path)?
    } else {
        detect_file_model(&model_path)
    };

    Ok(AsrEngineConfig {
        model: asr_model,
        provider: normalize_provider(provider),
        num_threads: default_num_threads(),
    })
}

/// 按「目录名提示 + 文件存在性」判定模型族。
/// 顺序不能随意调整：`model.onnx` 会被多种模型共用，先匹配到谁就是谁。
fn detect_dir_model(dir: &Path) -> Result<AsrModel, String> {
    let tokens = dir.join("tokens.txt");
    let tokens = tokens
        .is_file()
        .then(|| tokens.to_string_lossy().to_string())
        .ok_or_else(|| format!("ASR 模型目录缺少 tokens.txt：{}", dir.display()))?;

    let dir_name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_lowercase();
    let has = |name: &str| dir.join(name).is_file();
    let path_of = |name: &str| dir.join(name).to_string_lossy().to_string();

    if (dir_name.contains("sense") || dir_name.contains("sensevoice"))
        && (has("model.int8.onnx") || has("model.onnx"))
    {
        let file = if has("model.int8.onnx") {
            "model.int8.onnx"
        } else {
            "model.onnx"
        };
        return Ok(AsrModel::SenseVoice {
            model: path_of(file),
            tokens,
            language: None,
            use_itn: true,
        });
    }
    if has("encoder.onnx") && has("decoder.onnx") && has("joiner.onnx") {
        return Ok(AsrModel::Transducer {
            encoder: path_of("encoder.onnx"),
            decoder: path_of("decoder.onnx"),
            joiner: path_of("joiner.onnx"),
            tokens,
        });
    }
    if dir_name.contains("whisper") && has("encoder.int8.onnx") && has("decoder.int8.onnx") {
        return Ok(AsrModel::Whisper {
            encoder: path_of("encoder.int8.onnx"),
            decoder: path_of("decoder.int8.onnx"),
            tokens,
            language: None,
        });
    }
    if dir_name.contains("nemo") && has("model.onnx") {
        return Ok(AsrModel::NemoCtc {
            model: path_of("model.onnx"),
            tokens,
        });
    }
    if has("model.int8.onnx") {
        return Ok(AsrModel::Paraformer {
            model: path_of("model.int8.onnx"),
            tokens,
        });
    }
    if has("model.onnx") {
        return Ok(AsrModel::Paraformer {
            model: path_of("model.onnx"),
            tokens,
        });
    }

    Err(format!(
        "ASR 模型目录未识别到可用模型文件：{}",
        dir.display()
    ))
}

/// 单个 onnx 文件：无法从目录名判断，只能靠文件名推断，兜底按 paraformer 处理。
fn detect_file_model(path: &Path) -> AsrModel {
    let tokens = path
        .parent()
        .map(|dir| dir.join("tokens.txt"))
        .filter(|candidate| candidate.is_file())
        .map(|candidate| candidate.to_string_lossy().to_string())
        .unwrap_or_default();
    let file = path.to_string_lossy().to_lowercase();
    let model = path.to_string_lossy().to_string();

    if file.contains("sense") {
        AsrModel::SenseVoice {
            model,
            tokens,
            language: None,
            use_itn: true,
        }
    } else {
        AsrModel::Paraformer { model, tokens }
    }
}

/// 解析 sherpa 的 whisper ONNX 模型目录。
///
/// 这类文件的命名带模型名前缀（`base.en-encoder.int8.onnx`），通用探测规则里
/// 找的是 `encoder.onnx`，匹配不到，所以单独走一条。
/// 同名同时存在 int8 与 fp32 时优先 int8——更小更快，识别质量足够。
pub fn resolve_whisper_onnx_engine_config(
    dir: &Path,
    provider: &str,
    language: Option<&str>,
) -> Result<AsrEngineConfig, String> {
    let pick = |stem: &str, ext: &str| -> Option<String> {
        let entries = std::fs::read_dir(dir).ok()?;
        let names = entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        for candidate in [format!("-{}.int8.{}", stem, ext), format!("-{}.{}", stem, ext)] {
            if let Some(found) = names.iter().find(|name| name.ends_with(&candidate)) {
                return Some(dir.join(found).to_string_lossy().to_string());
            }
        }
        None
    };

    let encoder = pick("encoder", "onnx")
        .ok_or_else(|| format!("whisper 模型目录里找不到 encoder：{}", dir.display()))?;
    let decoder = pick("decoder", "onnx")
        .ok_or_else(|| format!("whisper 模型目录里找不到 decoder：{}", dir.display()))?;
    let tokens = pick("tokens", "txt")
        .ok_or_else(|| format!("whisper 模型目录里找不到 tokens.txt：{}", dir.display()))?;

    Ok(AsrEngineConfig {
        model: AsrModel::Whisper {
            encoder,
            decoder,
            tokens,
            language: language.map(str::to_string),
        },
        provider: normalize_provider(provider),
        num_threads: default_num_threads(),
    })
}

/// 标点模型路径。找不到就返回 `None`——没有它识别照常工作，只是输出不带标点。
///
/// 模型包解出来会多一层同名目录，所以 `punctuation/model.onnx` 与
/// `punctuation/*/model.onnx` 两种布局都要认。
///
/// 顺带做一次体积校验：截断的 ONNX 会让 sherpa 的 create 抛出 C++ 异常，
/// 而那种异常穿透 FFI 会**直接 abort 进程**（路径不存在反而是优雅返回 None）。
/// 与其赌下载完整性，不如在这里把明显不完整的文件挡掉。
pub fn resolve_punctuation_model() -> Option<PathBuf> {
    const MIN_MODEL_BYTES: u64 = 10 * 1024 * 1024;

    let root = bundled_resources::ensure_relative_dir("utils/Models/punctuation").ok()?;
    if !root.is_dir() {
        return None;
    }

    let usable = |path: PathBuf| {
        std::fs::metadata(&path)
            .map(|meta| meta.len() >= MIN_MODEL_BYTES)
            .unwrap_or(false)
            .then_some(path)
    };

    let direct = root.join("model.onnx");
    if direct.is_file() {
        return usable(direct);
    }

    std::fs::read_dir(&root)
        .ok()?
        .flatten()
        .map(|entry| entry.path().join("model.onnx"))
        .find_map(|candidate| candidate.is_file().then(|| usable(candidate)).flatten())
}

/// 探测 CUDA 是否真的能用。
///
/// 为什么必须"先探测再请求"，而不能"先试、失败再回退"：
/// provider=cuda 在缺少 CUDA 组件时，onnxruntime 抛出的 C++ 异常会穿透 FFI 边界，
/// Rust 捕不住，**直接 abort 整个进程**（本机实测确认，报 "Rust cannot catch foreign exceptions"）。
/// 也就是说回退分支根本没机会执行——只能提前判断，不能事后补救。
///
/// 判断有三级，缺一不可：
///   · NVIDIA 驱动（nvcuda.dll）——没有它任何 CUDA EP 都不可能工作；
///   · onnxruntime 的 CUDA EP 动态库找得到；
///   · 它**真的能被加载起来**。EP 库的导入表还挂着 cudart/cublas 等 CUDA 运行时，
///     只看文件在不在会把「装了驱动和旧版 CUDA 运行时」判成可用：EP 库要的导出
///     在旧版 cudart 里没有，LoadLibrary 就返回 error 127（实测：CUDA 12.6 的
///     cudart64_12.dll 缺 cudaLibraryLoadData 等 4 个符号，而 EP 库是按这些符号链接的），
///     接着就是上面那条 abort 路径。
///
/// 探测要把 EP 库整份映射一遍（约 500MB 的镜像，按需分页，实际驻留很小），
/// 而 provider 每解析一次就会问一次，所以结果缓存：进程内只探一次，
/// 装完 CUDA 运行时需要重启应用才能生效。
pub fn cuda_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(probe_cuda)
}

fn probe_cuda() -> bool {
    if !cfg!(target_os = "windows") {
        // 其它平台暂不探测：判不准就退回 CPU，宁可慢一点也不要崩。
        return false;
    }

    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    if !PathBuf::from(root).join("System32").join("nvcuda.dll").is_file() {
        warn!("CUDA 不可用（缺 NVIDIA 驱动 nvcuda.dll），本次改用 CPU");
        return false;
    }

    let mut attempted = Vec::new();
    let mut loaded = false;
    for candidate in cuda_provider_dll_candidates() {
        match load_dynamic_library(&candidate) {
            Ok(()) => {
                loaded = true;
                break;
            }
            Err(err) => attempted.push(format!("{}（{}）", candidate.display(), err)),
        }
    }
    if !loaded {
        warn!(
            "CUDA 不可用（CUDA EP 动态库加载失败，通常是 CUDA 运行时版本旧于 EP 库要求的版本），\
             本次改用 CPU：{}",
            attempted.join("；")
        );
        return false;
    }

    // EP 库自己的导入表只挂了 cudart/cublas/cublasLt，cuDNN 是它建会话时再去加载的。
    // 少一个 cuDNN，ORT 就在那里抛 C++ 异常把进程带走（同上，Rust 拦不住），
    // 所以这里替它先试一次。
    if let Err(err) = load_dynamic_library_by_name(CUDNN_DLL) {
        warn!(
            "CUDA 不可用（cuDNN 加载失败：{}），本次改用 CPU；缺 cuDNN {}",
            err, CUDNN_DLL
        );
        return false;
    }

    true
}

/// cuDNN 9 的主库名。CUDA 12 / 13 两种变体都叫这个名字。
const CUDNN_DLL: &str = "cudnn64_9.dll";

/// CUDA EP 动态库的候选位置。
///
/// onnxruntime 是拿裸文件名去 LoadLibrary 的，命中的是 Windows 的标准搜索顺序；
/// exe 目录排第一，与实测崩溃时它选中的那个路径一致。打包分发会把 sherpa 的运行库
/// 收在 `utils/sherpa-onnx` 下，那里也列一个候选。
fn cuda_provider_dll_candidates() -> Vec<PathBuf> {
    const EP_DLL: &str = "onnxruntime_providers_cuda.dll";

    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    else {
        return Vec::new();
    };

    vec![
        exe_dir.join(EP_DLL),
        exe_dir.join("utils").join("sherpa-onnx").join(EP_DLL),
    ]
}

/// 真正加载一次动态库，失败原因里带上 Windows 错误码。
///
/// 只 LoadLibrary 不取符号：导入表解析本身就覆盖了「依赖 DLL 缺失」和
/// 「依赖 DLL 版本旧、没有需要的导出」这两种情形，正是要拦下的两类问题。
#[cfg(target_os = "windows")]
fn load_dynamic_library(path: &Path) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(lp_lib_file_name: *const u16) -> *mut std::ffi::c_void;
        fn FreeLibrary(h_lib_module: *mut std::ffi::c_void) -> i32;
        fn GetLastError() -> u32;
    }

    let wide = OsStr::new(path.as_os_str())
        .encode_wide()
        .chain(once(0))
        .collect::<Vec<u16>>();
    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
    if handle.is_null() {
        return Err(format!("错误码={}", unsafe { GetLastError() }));
    }
    unsafe {
        let _ = FreeLibrary(handle);
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn load_dynamic_library(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// 按裸文件名加载一次动态库：走 Windows 的标准搜索顺序（exe 目录 → 系统目录 → PATH），
/// 也就是 onnxruntime 自己找这些运行库时用的那条路径。
#[cfg(target_os = "windows")]
fn load_dynamic_library_by_name(name: &str) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(lp_lib_file_name: *const u16) -> *mut std::ffi::c_void;
        fn FreeLibrary(h_lib_module: *mut std::ffi::c_void) -> i32;
        fn GetLastError() -> u32;
    }

    let wide = OsStr::new(name).encode_wide().chain(once(0)).collect::<Vec<u16>>();
    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
    if handle.is_null() {
        return Err(format!("错误码={}", unsafe { GetLastError() }));
    }
    unsafe {
        let _ = FreeLibrary(handle);
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn load_dynamic_library_by_name(_name: &str) -> Result<(), String> {
    Ok(())
}

/// 把调用方给的 provider 归一化成 sherpa 认的名字。
///
/// 空串（调用方没指定）和 "auto" 都表示"跟着硬件走"：有可用的 CUDA 就用 CUDA。
/// 各朗读入口（划词朗读、听写…）都不传 provider，所以这条默认值决定了它们在
/// 有 GPU 的机器上能不能吃到加速。
/// 默认放宽是安全的——cuda 一律要先过 `cuda_available` 的探测，过不了就退回 CPU。
pub fn normalize_provider(provider: &str) -> String {
    let requested = match provider.trim().to_lowercase().as_str() {
        "cuda" | "gpu" => "cuda",
        "coreml" => "coreml",
        "directml" => "directml",
        "trt" | "tensorrt" => "trt",
        "cpu" => "cpu",
        "" | "auto" => {
            if cuda_available() {
                "cuda"
            } else {
                "cpu"
            }
        }
        other => {
            warn!(provider = other, "未知的执行 provider，按 CPU 处理");
            "cpu"
        }
    };

    // 需要 CUDA 的 provider 一律先确认可用性，见 cuda_available 的说明。
    // 不通过时 probe_cuda 已经打过一条带原因的 warn，这里只补上调用方请求的是什么。
    if matches!(requested, "cuda" | "trt") && !cuda_available() {
        warn!(requested = requested, "改用 CPU 执行");
        return "cpu".to_string();
    }

    requested.to_string()
}
