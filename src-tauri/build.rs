use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

fn latest_modified_in_dir(dir: &Path) -> Option<SystemTime> {
    let mut latest = None;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(ts) = latest_modified_in_dir(&path) {
                    latest = Some(latest.map_or(ts, |cur: SystemTime| cur.max(ts)));
                }
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                if let Ok(ts) = meta.modified() {
                    latest = Some(latest.map_or(ts, |cur: SystemTime| cur.max(ts)));
                }
            }
        }
    }
    latest
}

#[cfg(target_os = "windows")]
fn prepare_dir_bundle(dir: &Path) {
    let bundle = dir.with_extension("zip");

    println!("cargo:rerun-if-changed={}", dir.display());
    println!("cargo:rerun-if-changed={}", bundle.display());

    if !dir.is_dir() {
        return;
    }

    let src_latest = latest_modified_in_dir(dir);
    let zip_latest = std::fs::metadata(&bundle).ok().and_then(|m| m.modified().ok());

    if let (Some(src_ts), Some(zip_ts)) = (src_latest, zip_latest) {
        if zip_ts >= src_ts {
            return;
        }
    }

    if bundle.exists() {
        let _ = std::fs::remove_file(&bundle);
    }

    let src = dir.to_string_lossy().replace('\'', "''");
    let dst = bundle.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='Stop'; $src='{src}'; $dst='{dst}'; if (Test-Path -LiteralPath $dst) {{ Remove-Item -LiteralPath $dst -Force }}; Compress-Archive -Path (Join-Path $src '*') -DestinationPath $dst -CompressionLevel Optimal"
    );

    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .status()
        .expect("failed to launch powershell to prepare bundled resource zip");

    if !status.success() {
        panic!("failed to prepare bundled resource zip with status {status}");
    }
}

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
    #[cfg(target_os = "windows")]
    for rel in [
        "utils/SlidevEnv",
        "utils/sherpa-onnx",
        "utils/whisper",
        "utils/llama",
        "utils/python_libs",
        "utils/Models/tts",
        "utils/Models/whisper_models",
    ] {
        prepare_dir_bundle(&manifest_dir.join(rel));
    }
    tauri_build::build()
}
