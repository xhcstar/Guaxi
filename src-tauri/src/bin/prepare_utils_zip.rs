use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

const REUSE_EXISTING_FLAG: &str = "--reuse-existing";
const KEEP_EXISTING_FLAG: &str = "--keep-existing";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn utils_dir() -> PathBuf {
    manifest_dir().join("utils")
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

fn should_reuse_existing_bundle() -> bool {
    std::env::args().skip(1).any(|arg| {
        arg == REUSE_EXISTING_FLAG || arg == KEEP_EXISTING_FLAG
    })
}

fn main() {
    let utils_dir = utils_dir();
    let bundle = utils_dir.join("utils.zip");
    let reuse_existing = should_reuse_existing_bundle();

    if !utils_dir.is_dir() {
        panic!("utils dir not found: {}", utils_dir.display());
    }

    if reuse_existing && bundle.exists() {
        println!("reusing {}", bundle.display());
        return;
    }

    if bundle.exists() {
        fs::remove_file(&bundle)
            .unwrap_or_else(|e| panic!("failed to remove old {}: {e}", bundle.display()));
    }

    let file =
        File::create(&bundle).unwrap_or_else(|e| panic!("failed to create {}: {e}", bundle.display()));
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    add_dir_to_zip(&mut writer, &utils_dir, &utils_dir, options);
    writer
        .finish()
        .unwrap_or_else(|e| panic!("failed to finish {}: {e}", bundle.display()));

    println!("prepared {}", bundle.display());
}
