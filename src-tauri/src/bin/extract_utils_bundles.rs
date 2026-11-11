use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[path = "../../utils_zip_support.rs"]
mod utils_zip_support;

// The installer passes arguments explicitly so the helper can stay shell-agnostic
// and avoid fragile inline PowerShell extraction logic.
fn parse_arg(flag: &str) -> Result<PathBuf, String> {
    parse_optional_arg(flag).ok_or_else(|| format!("missing required argument: {flag}"))
}

fn parse_optional_arg(flag: &str) -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == flag {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

/// 安装器传 `--console-encoding ansi`：它的详情面板按系统本地代码页解码子进程
/// stdout，UTF-8 中文进去会变成「瀹夎瑁呮棩蹇」这类乱码。默认仍是 UTF-8，
/// 所以 `cargo run --bin extract_utils_bundles` 在终端里输出照旧。
fn apply_console_encoding() {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let requested = if arg == "--console-encoding" {
            args.next()
        } else {
            arg.strip_prefix("--console-encoding=").map(str::to_string)
        };
        if let Some(value) = requested {
            let value = value.trim().to_ascii_lowercase();
            let encoding = if matches!(value.as_str(), "ansi" | "gbk" | "acp" | "local") {
                utils_zip_support::ConsoleEncoding::Ansi
            } else {
                utils_zip_support::ConsoleEncoding::Utf8
            };
            utils_zip_support::set_console_encoding(encoding);
            return;
        }
    }
}

/// 安装失败时给出可执行的方向，而不是把 Rust 的原始错误直接甩给用户。
fn report_failure(error: &str, bundle_root: &Path, destination: &Path, log_file: Option<&Path>) {
    utils_zip_support::report_error(&format!("[utils.bundle] 安装阶段解压失败：{error}"));
    utils_zip_support::report_error(&format!(
        "[utils.bundle] 分包目录 {} · 目标 {}",
        bundle_root.display(),
        destination.display()
    ));
    if let Some(log_file) = log_file {
        utils_zip_support::report_error(&format!(
            "[utils.bundle] 完整日志：{}",
            log_file.display()
        ));
    }
    utils_zip_support::report_error(
        "[utils.bundle] 常见原因：1) utils_bundles 子目录不完整，或与安装器不是同一版本；\
         2) 目标磁盘空间不足；3) 杀毒软件拦截写入；4) 目标目录没有写权限。",
    );
}

fn main() {
    let started = Instant::now();
    // 安装器的详情面板（nsExec::ExecToLog）按系统 ANSI 代码页解码子进程输出，
    // 必须先切编码再开口说话，否则前面几行仍然是乱码。
    apply_console_encoding();
    let bundle_root = parse_arg("--bundle-root").unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let destination = parse_arg("--destination").unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });

    // The installer passes --state-file so repeated installs can skip bundles
    // whose files are already present with the same size/mtime signature.
    // Without it (older packages) we keep the previous full-extraction behavior.
    let state_file = parse_optional_arg("--state-file");
    // --log-file 让安装器的详情面板滚过去的日志能留档，方便用户回传排查。
    let log_file = parse_optional_arg("--log-file");
    if let Some(path) = log_file.as_deref() {
        if let Err(error) = utils_zip_support::set_log_file(path) {
            // 日志写不了不影响解包，只在安装日志里留一句。
            eprintln!(
                "[utils.bundle] failed to open install log {}: {error}",
                path.display()
            );
        } else {
            utils_zip_support::emit_line(&format!(
                "[utils.bundle] 安装日志 {} · bundleRoot={} · destination={} · incremental={}",
                path.display(),
                bundle_root.display(),
                destination.display(),
                state_file.is_some()
            ));
        }
    }

    let result = if state_file.is_some() {
        utils_zip_support::sync_bundle_archives_to_destination(
            &bundle_root,
            &destination,
            state_file.as_deref(),
        )
    } else {
        utils_zip_support::extract_bundle_archives_to_destination(&bundle_root, &destination).map(
            |bundle_count| utils_zip_support::ExtractResult {
                bundle_count,
                ..utils_zip_support::ExtractResult::default()
            },
        )
    }
    .unwrap_or_else(|error| {
        report_failure(&error, &bundle_root, &destination, log_file.as_deref());
        std::process::exit(1);
    });

    // Payloads extracted into the install root (the CUDA runtime DLLs) must not
    // leave bookkeeping files next to the app, and their "destination parent" is
    // outside $INSTDIR entirely. The installer hook passes --no-marker for them.
    let write_marker = !std::env::args().skip(1).any(|arg| arg == "--no-marker");
    if write_marker {
        // Keep the completion marker next to `utils`, not inside it, so the user's
        // runtime resource directory stays clean and only contains actual resources.
        let marker = destination
            .parent()
            .unwrap_or(&destination)
            .join(".guaxi_utils_extract_complete");
        fs::write(&marker, b"ok").unwrap_or_else(|error| {
            report_failure(
                &format!("写入完成标记失败 {}: {error}", marker.display()),
                &bundle_root,
                &destination,
                log_file.as_deref(),
            );
            std::process::exit(1);
        });

        let legacy_marker = destination.join(".guaxi_utils_extract_complete");
        if legacy_marker.is_file() {
            let _ = fs::remove_file(&legacy_marker);
        }
    }

    utils_zip_support::emit_line(&format!(
        "[utils.bundle] extracted bundleRoot={} destination={} bundleCount={} extracted={} skippedBundles={} writtenFiles={} skippedFiles={} elapsed={:.1}s",
        bundle_root.display(),
        destination.display(),
        result.bundle_count,
        result.extracted_bundle_count,
        result.skipped_bundle_count,
        result.written_file_count,
        result.skipped_file_count,
        started.elapsed().as_secs_f64()
    ));
}
