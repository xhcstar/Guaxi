use std::path::PathBuf;

#[path = "../../../utils_zip_support.rs"]
mod utils_zip_support;

// The CUDA runtime cannot live inside the NSIS installer: together with the app
// binary it pushes `makensis` past its 2 GB data-block limit. It is published as
// tar.zst bundles next to the setup executable instead, and the installer hook
// expands them into the install root where the loader finds them.
const REUSE_EXISTING_FLAG: &str = "--reuse-existing";
const SHIPPED_EXTENSION: &str = "dll";

fn parse_arg(flag: &str) -> Result<PathBuf, String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == flag {
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?;
            return Ok(PathBuf::from(value));
        }
    }
    Err(format!("missing required argument: {flag}"))
}

fn main() {
    let source_dir = parse_arg("--source-dir").unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let bundle_dir = parse_arg("--bundle-dir").unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let reuse_existing = std::env::args().skip(1).any(|arg| arg == REUSE_EXISTING_FLAG);

    match utils_zip_support::prepare_payload_bundles(
        &source_dir,
        &bundle_dir,
        reuse_existing,
        Some(SHIPPED_EXTENSION),
    ) {
        Ok(result) if result.reused_existing => {
            println!(
                "[cuda.bundle] reusing existing bundleDir={} bundleCount={} elapsedMs={}",
                result.bundle_dir.display(),
                result.bundle_count,
                result.elapsed_ms
            );
        }
        Ok(result) => {
            println!(
                "[cuda.bundle] prepared bundleDir={} strategy={} bundleCount={} rebuilt={} reused={} elapsedMs={} totalBytes={}",
                result.bundle_dir.display(),
                result.strategy.label(),
                result.bundle_count,
                result.rebuilt_bundle_count,
                result.reused_bundle_count,
                result.elapsed_ms,
                result.total_bytes
            );
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
