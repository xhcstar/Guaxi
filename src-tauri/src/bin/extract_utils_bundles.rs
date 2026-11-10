use std::fs;
use std::path::PathBuf;

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

fn main() {
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
    let result = if state_file.is_some() {
        utils_zip_support::sync_bundle_archives_to_destination(
            &bundle_root,
            &destination,
            state_file.as_deref(),
        )
    } else {
        utils_zip_support::extract_bundle_archives_to_destination(&bundle_root, &destination)
            .map(|bundle_count| utils_zip_support::ExtractResult {
                bundle_count,
                ..utils_zip_support::ExtractResult::default()
            })
    }
    .unwrap_or_else(|error| {
        eprintln!("{error}");
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
            eprintln!("failed to write marker {}: {error}", marker.display());
            std::process::exit(1);
        });

        let legacy_marker = destination.join(".guaxi_utils_extract_complete");
        if legacy_marker.is_file() {
            let _ = fs::remove_file(&legacy_marker);
        }
    }

    println!(
        "[utils.bundle] extracted bundleRoot={} destination={} bundleCount={} extracted={} skippedBundles={} writtenFiles={} skippedFiles={}",
        bundle_root.display(),
        destination.display(),
        result.bundle_count,
        result.extracted_bundle_count,
        result.skipped_bundle_count,
        result.written_file_count,
        result.skipped_file_count
    );
}
