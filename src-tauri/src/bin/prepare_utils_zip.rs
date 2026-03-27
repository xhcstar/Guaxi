use std::path::PathBuf;

#[path = "../../utils_zip_support.rs"]
mod utils_zip_support;

const REUSE_EXISTING_FLAG: &str = "--reuse-existing";
const KEEP_EXISTING_FLAG: &str = "--keep-existing";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn should_reuse_existing_bundle() -> bool {
    std::env::args().skip(1).any(|arg| {
        arg == REUSE_EXISTING_FLAG || arg == KEEP_EXISTING_FLAG
    })
}

fn main() {
    let reuse_existing = should_reuse_existing_bundle();
    let manifest_dir = manifest_dir();

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
                "[utils.bundle] prepared bundleDir={} strategy={} bundleCount={} elapsedMs={} fileCount={} emptyDirCount={} totalBytes={}",
                result.bundle_dir.display(),
                result.strategy.label(),
                result.bundle_count,
                result.elapsed_ms,
                result.file_count,
                result.empty_dir_count,
                result.total_bytes
            );
        }
        Err(error) => panic!("{error}"),
    }
}
