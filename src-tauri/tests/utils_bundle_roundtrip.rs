#[path = "../utils_zip_support.rs"]
mod utils_zip_support;

use std::fs;
use std::path::Path;

use tempfile::TempDir;

fn write_text(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

#[test]
fn prepare_and_extract_tar_zst_bundles_roundtrip() {
    let workspace = TempDir::new().expect("temp workspace");
    let manifest_dir = workspace.path();
    let utils_dir = manifest_dir.join("utils");
    fs::create_dir_all(&utils_dir).expect("create utils dir");

    write_text(&utils_dir.join("tool.exe"), "binary-placeholder");
    write_text(
        &utils_dir.join("Models/tts/config.json"),
        r#"{"name":"demo"}"#,
    );
    write_text(
        &utils_dir.join("SlidevEnv/node_modules/pkg/index.js"),
        "console.log('ok');",
    );
    fs::create_dir_all(utils_dir.join("llama/empty-dir")).expect("create empty dir");

    write_text(
        &utils_dir.join("MarkdownConvertor/ignored.txt"),
        "should be skipped",
    );
    write_text(
        &utils_dir.join("Models/surya_models/ignored.bin"),
        "skip me",
    );
    write_text(&utils_dir.join("utils.zip"), "legacy zip");

    let prepare = utils_zip_support::prepare_utils_bundles(manifest_dir, false)
        .expect("prepare bundles");

    assert!(!prepare.reused_existing);
    assert_eq!(prepare.strategy.label(), "TarZstdRustMT");
    assert!(prepare.bundle_count >= 4);

    let bundle_dir = utils_dir.join(utils_zip_support::bundle_dir_name());
    assert!(bundle_dir.is_dir());

    let bundle_names = fs::read_dir(&bundle_dir)
        .expect("read bundle dir")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert!(bundle_names.iter().any(|name| name == "tool.exe.tar.zst"));
    assert!(bundle_names.iter().any(|name| name == "Models.tar.zst"));
    assert!(bundle_names.iter().any(|name| name == "SlidevEnv.tar.zst"));
    assert!(bundle_names.iter().any(|name| name == "llama.tar.zst"));
    assert!(!bundle_names.iter().any(|name| name.contains("MarkdownConvertor")));
    assert!(!bundle_names.iter().any(|name| name.contains("surya_models")));

    let extract_dir = manifest_dir.join("restored_utils");
    let extracted_count =
        utils_zip_support::extract_bundle_archives_to_destination(&bundle_dir, &extract_dir)
            .expect("extract bundles");

    assert_eq!(extracted_count, prepare.bundle_count);
    assert_eq!(
        fs::read_to_string(extract_dir.join("tool.exe")).expect("read tool"),
        "binary-placeholder"
    );
    assert_eq!(
        fs::read_to_string(extract_dir.join("Models/tts/config.json")).expect("read tts config"),
        r#"{"name":"demo"}"#
    );
    assert_eq!(
        fs::read_to_string(extract_dir.join("SlidevEnv/node_modules/pkg/index.js"))
            .expect("read slidev"),
        "console.log('ok');"
    );
    assert!(extract_dir.join("llama/empty-dir").is_dir());
    assert!(!extract_dir.join("MarkdownConvertor").exists());
    assert!(!extract_dir.join("Models/surya_models").exists());
}
