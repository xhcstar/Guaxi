use std::path::{Path, PathBuf};

#[path = "utils_zip_support.rs"]
mod utils_zip_support;

/// 预览页 vendor 资源的缓存指纹：内容一变，URL 就变，浏览器缓存自然失效。
fn preview_asset_version(manifest_dir: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let asset_dir = manifest_dir.join("assets").join("vendor");
    println!("cargo:rerun-if-changed={}", asset_dir.display());

    let Ok(read_dir) = std::fs::read_dir(&asset_dir) else {
        return String::from("unversioned");
    };
    let mut files: Vec<PathBuf> = read_dir
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    files.sort();

    let mut hasher = DefaultHasher::new();
    for path in files {
        if !path.is_file() {
            continue;
        }
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
            .hash(&mut hasher);
        if let Ok(bytes) = std::fs::read(&path) {
            bytes.hash(&mut hasher);
        }
    }
    format!("{:016x}", hasher.finish())
}

/// 图表预览的单文件产物（`assets/vendor/guaxi-chart-preview.js`）由 Vite 打成，
/// 被本 crate 用 `include_str!` 内联进二进制，缺了就是编译错误。
///
/// 正常流程由 npm 编排（`tools/run_tauri_with_utils.mjs`）在启动 Tauri 前生成；
/// 该文件已列入 .gitignore —— 它是构建产物，且不同 Node/工具链环境压缩出的短变量名
/// 不同（语义等价、字节不同），两台机器交替构建会互相覆盖工作区。
///
/// 这里兜底「绕过 npm、直接 cargo build / check」的场景：缺失时尝试用 node 补生成。
fn ensure_chart_preview_bundle(manifest_dir: &Path) {
    let bundle = manifest_dir
        .join("assets")
        .join("vendor")
        .join("guaxi-chart-preview.js");
    println!("cargo:rerun-if-changed={}", bundle.display());
    if bundle.is_file() {
        return;
    }

    let Some(root) = manifest_dir.parent() else {
        return;
    };
    let script = root.join("tools").join("build_chart_preview_bundle.mjs");
    if !script.is_file() {
        return;
    }

    println!("cargo:warning=[chart.preview] 产物缺失，尝试用 node 生成（需要先执行 npm install）");
    let generated = std::process::Command::new("node")
        .arg(&script)
        .current_dir(root)
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    if generated && bundle.is_file() {
        println!("cargo:warning=[chart.preview] 已生成 {}", bundle.display());
        return;
    }

    panic!(
        "缺少图表预览产物 {}，自动生成也失败了。请先 `npm install`，再执行 `npm run build:chart-preview`（或任一次 `npm run tauri -- dev`）后重试。",
        bundle.display()
    );
}

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
    // 必须排在 preview_asset_version 之前：那个指纹会遍历 assets/vendor 下的全部文件，
    // 产物若此时还不存在，算出来的版本号就是错的（且 include_str! 会直接编译失败）。
    ensure_chart_preview_bundle(&manifest_dir);
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

    // 浏览器预览页把 vendor 资源以 `?v=<内容指纹>` 引入并允许浏览器永久缓存，
    // 指纹必须随资源内容变化，否则升级后浏览器会一直用旧的 mermaid / ECharts。
    println!(
        "cargo:rustc-env=GUAXI_PREVIEW_ASSET_VERSION={}",
        preview_asset_version(&manifest_dir)
    );

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
                    "cargo:warning=[utils.bundle] prepared bundleDir={} strategy={} bundleCount={} rebuilt={} reused={} elapsedMs={} fileCount={} emptyDirCount={} totalBytes={}",
                    result.bundle_dir.display(),
                    result.strategy.label(),
                    result.bundle_count,
                    result.rebuilt_bundle_count,
                    result.reused_bundle_count,
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
