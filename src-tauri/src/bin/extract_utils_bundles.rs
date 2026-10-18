use std::fs;
use std::path::PathBuf;

#[path = "../../utils_zip_support.rs"]
mod utils_zip_support;

// The installer passes arguments explicitly so the helper can stay shell-agnostic
// and avoid fragile inline PowerShell extraction logic.
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
    let bundle_root = parse_arg("--bundle-root").unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let destination = parse_arg("--destination").unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });

    let bundle_count = utils_zip_support::extract_bundle_archives_to_destination(
        &bundle_root,
        &destination,
    )
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
        "[utils.bundle] extracted bundleRoot={} destination={} bundleCount={}",
        bundle_root.display(),
        destination.display(),
        bundle_count
    );
}
