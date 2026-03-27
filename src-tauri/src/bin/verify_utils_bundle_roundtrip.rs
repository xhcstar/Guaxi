#[path = "../../utils_zip_support.rs"]
mod utils_zip_support;

use std::fs;
use std::path::Path;

use tempfile::TempDir;

fn write_text(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create parent {} failed: {e}", parent.display()))?;
    }
    fs::write(path, content).map_err(|e| format!("write {} failed: {e}", path.display()))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let workspace = TempDir::new().map_err(|e| format!("create temp workspace failed: {e}"))?;
    let manifest_dir = workspace.path();
    let utils_dir = manifest_dir.join("utils");
    fs::create_dir_all(&utils_dir)
        .map_err(|e| format!("create utils dir {} failed: {e}", utils_dir.display()))?;

    write_text(&utils_dir.join("tool.exe"), "binary-placeholder")?;
    write_text(
        &utils_dir.join("Models/tts/config.json"),
        r#"{"name":"demo"}"#,
    )?;
    write_text(
        &utils_dir.join("SlidevEnv/node_modules/pkg/index.js"),
        "console.log('ok');",
    )?;
    fs::create_dir_all(utils_dir.join("llama/empty-dir"))
        .map_err(|e| format!("create empty dir failed: {e}"))?;

    write_text(
        &utils_dir.join("MarkdownConvertor/ignored.txt"),
        "should be skipped",
    )?;
    write_text(
        &utils_dir.join("Models/surya_models/ignored.bin"),
        "skip me",
    )?;
    write_text(&utils_dir.join("utils.zip"), "legacy zip")?;

    let prepare = utils_zip_support::prepare_utils_bundles(manifest_dir, false)?;
    let bundle_dir = utils_dir.join(utils_zip_support::bundle_dir_name());
    let extract_dir = manifest_dir.join("restored_utils");
    let extracted_count =
        utils_zip_support::extract_bundle_archives_to_destination(&bundle_dir, &extract_dir)?;

    if extracted_count != prepare.bundle_count {
        return Err(format!(
            "bundle count mismatch: extracted={} prepared={}",
            extracted_count, prepare.bundle_count
        ));
    }

    let tool = fs::read_to_string(extract_dir.join("tool.exe"))
        .map_err(|e| format!("read extracted tool failed: {e}"))?;
    let tts = fs::read_to_string(extract_dir.join("Models/tts/config.json"))
        .map_err(|e| format!("read extracted tts config failed: {e}"))?;
    let slidev = fs::read_to_string(extract_dir.join("SlidevEnv/node_modules/pkg/index.js"))
        .map_err(|e| format!("read extracted slidev asset failed: {e}"))?;

    if tool != "binary-placeholder" {
        return Err(format!("unexpected extracted tool content: {tool}"));
    }
    if tts != r#"{"name":"demo"}"# {
        return Err(format!("unexpected extracted tts content: {tts}"));
    }
    if slidev != "console.log('ok');" {
        return Err(format!("unexpected extracted slidev content: {slidev}"));
    }
    if !extract_dir.join("llama/empty-dir").is_dir() {
        return Err("empty directory was not restored".to_string());
    }
    if extract_dir.join("MarkdownConvertor").exists() {
        return Err("skipped MarkdownConvertor unexpectedly restored".to_string());
    }
    if extract_dir.join("Models/surya_models").exists() {
        return Err("skipped surya_models unexpectedly restored".to_string());
    }

    println!(
        "[utils.bundle] smoke test passed bundleDir={} bundleCount={} extractedCount={}",
        prepare.bundle_dir.display(),
        prepare.bundle_count,
        extracted_count
    );

    Ok(())
}
