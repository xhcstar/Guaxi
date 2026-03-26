use std::path::PathBuf;

#[path = "utils_zip_support.rs"]
mod utils_zip_support;

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
    let utils_dir = manifest_dir.join("utils");
    let bundle = utils_dir.join("utils.zip");
    let profile = std::env::var("PROFILE").unwrap_or_default();
    let reuse_existing = matches!(
        std::env::var("GUAXI_REUSE_UTILS_ZIP").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    );

    println!("cargo:rerun-if-changed={}", utils_dir.display());
    println!("cargo:rerun-if-changed={}", bundle.display());
    println!("cargo:rerun-if-env-changed=GUAXI_REUSE_UTILS_ZIP");
    println!("cargo:rerun-if-env-changed=PROFILE");

    if profile != "debug" {
        match utils_zip_support::prepare_utils_bundle(&manifest_dir, reuse_existing) {
            Ok(result) if result.reused_existing => {
                println!(
                    "cargo:warning=[utils.zip] reusing existing bundle={} elapsedMs={}",
                    result.bundle_path.display(),
                    result.elapsed_ms
                );
            }
            Ok(result) => {
                println!(
                    "cargo:warning=[utils.zip] prepared bundle={} strategy={} elapsedMs={} fileCount={} emptyDirCount={} totalBytes={}",
                    result.bundle_path.display(),
                    result.strategy.label(),
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
        println!("cargo:warning=[utils.zip] skipped in debug profile");
    }
    tauri_build::build()
}
