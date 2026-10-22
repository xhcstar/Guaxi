use std::path::PathBuf;

#[path = "utils_zip_support.rs"]
mod utils_zip_support;

fn main() {
    // Windows 主线程栈默认只有 1MB，而 Tauri 的 generate_handler! 会把所有命令展开
    // 到同一个闭包里的巨大 match 中；debug 构建（-O0）不合并各分支的栈槽，帧大小≈
    // 命令数 × 0.9KB。命令涨到 900 多个后该帧已约 870KB，IPC 回调又跑在主线程上，
    // 再叠加命令自身的帧就会爆栈（0xC00000FD，表现为启动几秒后进程直接退出）。
    // 这里把主线程栈保留区提到 16MB；只影响虚拟地址保留，提交仍按页增长。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg-bins=/STACK:16777216");
    }

    // Cargo runs this script before compiling the main Tauri crate.
    // We use it as a release-mode safety net: even if the outer npm wrapper
    // was skipped, a release build still gets the external utils bundles.
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
    let utils_dir = manifest_dir.join("utils");
    let profile = std::env::var("PROFILE").unwrap_or_default();
    let reuse_existing = matches!(
        std::env::var("GUAXI_REUSE_UTILS_ZIP").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    );
    let skip_bundle_prepare = matches!(
        std::env::var("GUAXI_SKIP_BUNDLE_PREPARE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    );

    println!("cargo:rerun-if-changed={}", utils_dir.display());
    println!("cargo:rerun-if-env-changed=GUAXI_REUSE_UTILS_ZIP");
    println!("cargo:rerun-if-env-changed=GUAXI_SKIP_BUNDLE_PREPARE");
    println!("cargo:rerun-if-env-changed=PROFILE");

    if skip_bundle_prepare {
        println!("cargo:warning=[utils.bundle] skipped by GUAXI_SKIP_BUNDLE_PREPARE");
        tauri_build::build();
        return;
    }

    if profile != "debug" {
        // Debug/dev reads utils directly from the source tree, so preparing
        // tar.zst bundles here would only waste time.
        match utils_zip_support::prepare_utils_bundles(&manifest_dir, reuse_existing) {
            Ok(result) if result.reused_existing => {
                println!(
                    "cargo:warning=[utils.bundle] reusing existing bundleDir={} bundleCount={} elapsedMs={}",
                    result.bundle_dir.display(),
                    result.bundle_count,
                    result.elapsed_ms
                );
            }
            Ok(result) => {
                println!(
                    "cargo:warning=[utils.bundle] prepared bundleDir={} strategy={} bundleCount={} elapsedMs={} fileCount={} emptyDirCount={} totalBytes={}",
                    result.bundle_dir.display(),
                    result.strategy.label(),
                    result.bundle_count,
                    result.elapsed_ms,
                    result.file_count,
                    result.empty_dir_count,
                    result.total_bytes
                );
            }
            Err(_error) if !utils_dir.is_dir() => {}
            Err(error) => panic!("{error}"),
        }
    } else {
        // Keep the debug log explicit so developers know the skip is intentional.
        println!("cargo:warning=[utils.bundle] skipped in debug profile");
    }
    tauri_build::build()
}
