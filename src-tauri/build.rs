use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

fn latest_modified_in_dir(dir: &Path) -> Option<SystemTime> {
    let mut latest = None;
    if let Ok(entries) = fs::read_dir(dir) {
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

fn should_skip(relative: &str) -> bool {
    let normalized = relative.replace('\\', "/");
    matches!(
        normalized.as_str(),
        "utils.zip"
            | "SlidevEnv.zip"
            | "sherpa-onnx.zip"
            | "whisper.zip"
            | "llama.zip"
            | "python_libs.zip"
            | "Models/tts.zip"
            | "Models/whisper_models.zip"
    ) || normalized.contains(".__extracting_")
        || normalized.ends_with("/.guaxi_extract_complete")
        || normalized.ends_with("/.guaxi_utils_extract_complete")
        || normalized == ".guaxi_extract_complete"
        || normalized == ".guaxi_utils_extract_complete"
}

fn add_dir_to_zip(
    writer: &mut ZipWriter<File>,
    root_dir: &Path,
    current_dir: &Path,
    options: SimpleFileOptions,
) {
    let mut entries = fs::read_dir(current_dir)
        .unwrap_or_else(|e| panic!("failed to read dir {}: {e}", current_dir.display()))
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(root_dir)
            .unwrap_or_else(|e| panic!("failed to strip prefix {} from {}: {e}", root_dir.display(), path.display()));
        let relative_text = relative.to_string_lossy().replace('\\', "/");
        if should_skip(&relative_text) {
            continue;
        }

        if path.is_dir() {
            writer
                .add_directory(format!("{relative_text}/"), options)
                .unwrap_or_else(|e| panic!("failed to add dir {relative_text} to zip: {e}"));
            add_dir_to_zip(writer, root_dir, &path, options);
            continue;
        }

        writer
            .start_file(relative_text.clone(), options)
            .unwrap_or_else(|e| panic!("failed to start file {relative_text} in zip: {e}"));

        let mut file = File::open(&path)
            .unwrap_or_else(|e| panic!("failed to open {}: {e}", path.display()));
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        writer
            .write_all(&buffer)
            .unwrap_or_else(|e| panic!("failed to write {relative_text} to zip: {e}"));
    }
}

fn prepare_utils_bundle(manifest_dir: &Path) {
    let utils_dir = manifest_dir.join("utils");
    let bundle = utils_dir.join("utils.zip");

    println!("cargo:rerun-if-changed={}", utils_dir.display());
    println!("cargo:rerun-if-changed={}", bundle.display());

    if !utils_dir.is_dir() {
        return;
    }

    let src_latest = latest_modified_in_dir(&utils_dir);
    let zip_latest = fs::metadata(&bundle).ok().and_then(|m| m.modified().ok());
    if let (Some(src_ts), Some(zip_ts)) = (src_latest, zip_latest) {
        if zip_ts >= src_ts {
            return;
        }
    }

    if bundle.exists() {
        let _ = fs::remove_file(&bundle);
    }

    let file = File::create(&bundle)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", bundle.display()));
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    add_dir_to_zip(&mut writer, &utils_dir, &utils_dir, options);
    writer
        .finish()
        .unwrap_or_else(|e| panic!("failed to finish {}: {e}", bundle.display()));
}

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
    prepare_utils_bundle(&manifest_dir);
    tauri_build::build()
}
