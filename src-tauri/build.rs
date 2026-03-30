use std::path::PathBuf;

#[path = "utils_zip_support.rs"]
mod utils_zip_support;

fn main() {
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
