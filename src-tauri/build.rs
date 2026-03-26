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
        let zip_name = relative.to_string_lossy().replace('\\', "/");

        if path.is_dir() {
            writer
                .add_directory(format!("{zip_name}/"), options)
                .unwrap_or_else(|e| panic!("failed to add dir {zip_name} to zip: {e}"));
            add_dir_to_zip(writer, root_dir, &path, options);
            continue;
        }

        writer
            .start_file(zip_name.clone(), options)
            .unwrap_or_else(|e| panic!("failed to start file {zip_name} in zip: {e}"));

        let mut file = File::open(&path)
            .unwrap_or_else(|e| panic!("failed to open {}: {e}", path.display()));
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        writer
            .write_all(&buffer)
            .unwrap_or_else(|e| panic!("failed to write {zip_name} to zip: {e}"));
    }
}

fn prepare_dir_bundle(dir: &Path) {
    let bundle = dir.with_extension("zip");

    println!("cargo:rerun-if-changed={}", dir.display());
    println!("cargo:rerun-if-changed={}", bundle.display());

    if !dir.is_dir() {
        return;
    }

    let src_latest = latest_modified_in_dir(dir);
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

    add_dir_to_zip(&mut writer, dir, dir, options);
    writer
        .finish()
        .unwrap_or_else(|e| panic!("failed to finish {}: {e}", bundle.display()));
}

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));

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
