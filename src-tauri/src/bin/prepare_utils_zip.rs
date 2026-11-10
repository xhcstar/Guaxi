use std::path::PathBuf;

#[path = "../../utils_zip_support.rs"]
mod utils_zip_support;

// Historical flag name is kept for compatibility with existing local scripts.
// Internally the implementation now produces tar.zst bundle parts, not zip files.
const REUSE_EXISTING_FLAG: &str = "--reuse-existing";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn should_reuse_existing_bundle() -> bool {
    std::env::args().skip(1).any(|arg| arg == REUSE_EXISTING_FLAG)
}

fn main() {
    let reuse_existing = should_reuse_existing_bundle();
    let manifest_dir = manifest_dir();

    // This binary is the explicit, developer-facing way to prepare release
    // runtime bundles ahead of `tauri build`.
    match utils_zip_support::prepare_utils_bundles(&manifest_dir, reuse_existing) {
        Ok(result) if result.reused_existing => {
            println!(
                "[utils.bundle] reusing existing bundleDir={} bundleCount={} elapsedMs={}",
                result.bundle_dir.display(),
                result.bundle_count,
                result.elapsed_ms
            );
        }
        Ok(result) => {
            println!(
                "[utils.bundle] prepared bundleDir={} strategy={} bundleCount={} rebuilt={} reused={} elapsedMs={} fileCount={} emptyDirCount={} totalBytes={}",
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
        Err(error) => panic!("{error}"),
    }
}
