use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fs_extra::dir::{copy as copy_dir, CopyOptions};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};
use uuid::Uuid;
use walkdir::WalkDir;
use zip::ZipArchive;

use crate::guaxiservices::{bundled_resources, lsp};

#[derive(Debug, Clone, Deserialize)]
struct CatalogRoot {
    version: u32,
    extensions: Vec<CatalogExtension>,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogExtension {
    id: String,
    display_name: String,
    description: Option<String>,
    open_vsx: Option<String>,
    vscode_marketplace: Option<String>,
    languages: Vec<String>,
    compatibility_level: String,
    recommended: bool,
    lsp: CatalogLspRuntime,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogLspRuntime {
    server_id: String,
    runtime: String,
    bin_name: String,
    args: Vec<String>,
    entry_paths: Vec<String>,
    resolve_order: Vec<String>,
    fallback_server: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct InstalledExtensionMeta {
    source: String,
    installed_at: String,
    download_url: Option<String>,
    open_vsx: Option<String>,
    vscode_marketplace: Option<String>,
    original_file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LspExtensionRuntimeStatus {
    pub ready: bool,
    pub runtime: String,
    pub program_path: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LspExtensionState {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub source: String,
    pub version: String,
    pub languages: Vec<String>,
    pub installed: bool,
    pub installed_path: Option<String>,
    pub compatibility_level: String,
    pub recommended: bool,
    pub open_vsx: Option<String>,
    pub vscode_marketplace: Option<String>,
    pub manifest_name: Option<String>,
    pub manifest_publisher: Option<String>,
    pub manifest_browser: Option<String>,
    pub manifest_main: Option<String>,
    pub activation_events: Vec<String>,
    pub runtime_status: LspExtensionRuntimeStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenVsxSearchResult {
    pub id: String,
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub display_name: String,
    pub description: Option<String>,
    pub download_url: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VscodeMarketplaceSearchResult {
    pub id: String,
    pub publisher: String,
    pub name: String,
    pub version: String,
    pub display_name: String,
    pub description: Option<String>,
    pub download_url: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedLspExtensionCommand {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) envs: Vec<(String, String)>,
    pub(crate) cwd: Option<String>,
    pub(crate) extension_id: String,
    pub(crate) extension_display_name: String,
    pub(crate) server_id: String,
}

#[derive(Debug, Clone)]
struct InstalledExtensionRecord {
    id: String,
    version: String,
    installed_at: String,
    display_name: String,
    description: Option<String>,
    source: String,
    install_dir: PathBuf,
    open_vsx: Option<String>,
    vscode_marketplace: Option<String>,
    manifest_name: Option<String>,
    manifest_publisher: Option<String>,
    manifest_browser: Option<String>,
    manifest_main: Option<String>,
    activation_events: Vec<String>,
    languages: Vec<String>,
}

static LSP_EXTENSION_CATALOG: Lazy<CatalogRoot> = Lazy::new(|| {
    serde_json::from_str(include_str!("../../../shared/lsp_extensions.json"))
        .expect("invalid shared LSP extension catalog")
});

fn normalize_string(raw: &str) -> String {
    raw.trim().to_string()
}

fn normalize_language(raw: &str) -> String {
    raw.trim().to_lowercase()
}

fn normalize_extension_id(raw: &str) -> String {
    raw.trim().to_lowercase()
}

fn exe_dir() -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("failed to resolve current exe: {e}"))?;
    let dir = current_exe
        .parent()
        .ok_or_else(|| "failed to resolve current exe directory".to_string())?
        .to_path_buf();
    Ok(dir)
}

fn extensions_root_dir() -> Result<PathBuf, String> {
    let dir = exe_dir()?.join("extensions");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create extensions root: {e}"))?;
    Ok(dir)
}

fn extension_meta_path(dir: &Path) -> PathBuf {
    dir.join(".guaxi-extension.json")
}

fn temp_root_dir() -> Result<PathBuf, String> {
    let dir = exe_dir()?.join("tmp").join("lsp_extensions");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create extension temp root: {e}"))?;
    Ok(dir)
}

fn slidev_env_root() -> Result<PathBuf, String> {
    bundled_resources::ensure_relative_dir("utils/SlidevEnv")
}

fn slidev_node_path() -> Option<PathBuf> {
    let root = slidev_env_root().ok()?;
    #[cfg(target_os = "windows")]
    let candidate = root.join("node.exe");
    #[cfg(not(target_os = "windows"))]
    let candidate = root.join("node");
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

fn slidev_bin_dir() -> Option<PathBuf> {
    let root = slidev_env_root().ok()?;
    let dir = root.join("node_modules").join(".bin");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

fn sanitize_dir_fragment(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
}

fn install_dir_name(id: &str, version: &str) -> String {
    format!(
        "{}-{}",
        sanitize_dir_fragment(&normalize_extension_id(id)),
        sanitize_dir_fragment(version)
    )
}

fn unique_install_dir(id: &str, version: &str) -> Result<PathBuf, String> {
    let root = extensions_root_dir()?;
    let base_name = install_dir_name(id, version);
    let base = root.join(&base_name);
    if !base.exists() {
        return Ok(base);
    }

    Ok(root.join(format!(
        "{}-{}",
        base_name,
        sanitize_dir_fragment(&Uuid::new_v4().to_string())
    )))
}

fn parse_extension_identifier(id: &str) -> Result<(String, String), String> {
    let normalized = normalize_extension_id(id);
    let (namespace, name) = normalized
        .split_once('.')
        .ok_or_else(|| format!("invalid extension identifier: {id}"))?;
    if namespace.is_empty() || name.is_empty() {
        return Err(format!("invalid extension identifier: {id}"));
    }
    Ok((namespace.to_string(), name.to_string()))
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn is_downloadable_fallback(server_id: &str) -> bool {
    matches!(
        server_id,
        "clangd"
            | "rust-analyzer"
            | "lua-language-server"
            | "texlab"
            | "marksman"
            | "mermaid-lsp"
            | "ruff"
            | "biome"
    )
}

fn catalog_by_id() -> HashMap<String, CatalogExtension> {
    LSP_EXTENSION_CATALOG
        .extensions
        .iter()
        .map(|item| (normalize_extension_id(&item.id), item.clone()))
        .collect()
}

fn read_extension_meta(dir: &Path) -> InstalledExtensionMeta {
    let meta_path = extension_meta_path(dir);
    if let Ok(content) = fs::read_to_string(&meta_path) {
        if let Ok(meta) = serde_json::from_str::<InstalledExtensionMeta>(&content) {
            return meta;
        }
    }

    InstalledExtensionMeta {
        source: "unknown".to_string(),
        installed_at: now_rfc3339(),
        download_url: None,
        open_vsx: None,
        vscode_marketplace: None,
        original_file_name: None,
    }
}

fn write_extension_meta(dir: &Path, meta: &InstalledExtensionMeta) -> Result<(), String> {
    let path = extension_meta_path(dir);
    let content = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("failed to serialize extension metadata: {e}"))?;
    fs::write(&path, content).map_err(|e| format!("failed to write extension metadata: {e}"))
}

fn parse_manifest_from_path(path: &Path) -> Option<Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&content).ok()
}

fn is_extension_manifest(manifest: &Value) -> bool {
    manifest.get("name").and_then(Value::as_str).is_some()
        && manifest.get("publisher").and_then(Value::as_str).is_some()
}

fn find_manifest_path(root: &Path) -> Option<PathBuf> {
    for entry in WalkDir::new(root)
        .min_depth(1)
        .max_depth(4)
        .into_iter()
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name() != "package.json" {
            continue;
        }
        let path = entry.path().to_path_buf();
        let Some(manifest) = parse_manifest_from_path(&path) else {
            continue;
        };
        if is_extension_manifest(&manifest) {
            return Some(path);
        }
    }
    None
}

fn manifest_root_from_dir(root: &Path) -> Option<(PathBuf, Value)> {
    let manifest_path = find_manifest_path(root)?;
    let manifest = parse_manifest_from_path(&manifest_path)?;
    let manifest_root = manifest_path.parent()?.to_path_buf();
    Some((manifest_root, manifest))
}

fn read_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items.iter()
                .filter_map(Value::as_str)
                .map(normalize_string)
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn extract_manifest_languages(manifest: &Value) -> Vec<String> {
    let mut result = vec![];
    if let Some(items) = manifest
        .get("contributes")
        .and_then(|item| item.get("languages"))
        .and_then(Value::as_array)
    {
        for item in items {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                let normalized = normalize_language(id);
                if !normalized.is_empty() {
                    result.push(normalized);
                }
            }
        }
    }
    result.sort();
    result.dedup();
    result
}

fn record_from_dir(dir: &Path) -> Option<InstalledExtensionRecord> {
    let meta = read_extension_meta(dir);
    let (manifest_root, manifest) = manifest_root_from_dir(dir)?;

    let publisher = normalize_string(manifest.get("publisher")?.as_str()?);
    let name = normalize_string(manifest.get("name")?.as_str()?);
    if publisher.is_empty() || name.is_empty() {
        return None;
    }

    let id = normalize_extension_id(&format!("{publisher}.{name}"));
    let version = normalize_string(
        manifest
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("0.0.0"),
    );
    let display_name = normalize_string(
        manifest
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or(&name),
    );

    let languages = {
        let manifest_languages = extract_manifest_languages(&manifest);
        if !manifest_languages.is_empty() {
            manifest_languages
        } else {
            catalog_by_id()
                .get(&id)
                .map(|entry| entry.languages.iter().map(|lang| normalize_language(lang)).collect())
                .unwrap_or_default()
        }
    };

    Some(InstalledExtensionRecord {
        id,
        version,
        installed_at: meta.installed_at.clone(),
        display_name,
        description: manifest
            .get("description")
            .and_then(Value::as_str)
            .map(normalize_string),
        source: meta.source,
        install_dir: manifest_root,
        open_vsx: meta.open_vsx,
        vscode_marketplace: meta.vscode_marketplace,
        manifest_name: Some(name),
        manifest_publisher: Some(publisher),
        manifest_browser: manifest
            .get("browser")
            .and_then(Value::as_str)
            .map(normalize_string),
        manifest_main: manifest
            .get("main")
            .and_then(Value::as_str)
            .map(normalize_string),
        activation_events: read_string_array(manifest.get("activationEvents")),
        languages,
    })
}

fn load_installed_extensions() -> Result<Vec<InstalledExtensionRecord>, String> {
    let root = extensions_root_dir()?;
    let mut result = vec![];
    for entry in fs::read_dir(&root).map_err(|e| format!("failed to read extensions root: {e}"))? {
        let entry = entry.map_err(|e| format!("failed to read extension entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(record) = record_from_dir(&path) {
            result.push(record);
        }
    }

    // 设计思路：
    // 1. Windows 上 LSP 子进程可能仍把旧扩展目录作为 cwd，导致删除旧版本返回 os error 32。
    // 2. 安装新版本时我们会使用唯一目录名，因此这里按 extension id 只保留 installed_at 最新的一条，
    //    避免一个被锁住的旧目录继续参与 runtime 解析或重复出现在设置页。
    result.sort_by(|a, b| {
        a.id.cmp(&b.id)
            .then(b.installed_at.cmp(&a.installed_at))
            .then(a.display_name.cmp(&b.display_name))
    });
    result.dedup_by(|a, b| normalize_extension_id(&a.id) == normalize_extension_id(&b.id));
    result.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(result)
}

fn extract_zip_to_dir(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("failed to open zip archive: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("failed to parse zip archive: {e}"))?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|e| format!("failed to read zip entry: {e}"))?;

        let Some(out_path) = file.enclosed_name().map(|path| dest_dir.join(path)) else {
            continue;
        };

        if file.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("failed to create extracted directory: {e}"))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create extracted parent directory: {e}"))?;
        }

        let mut out_file =
            fs::File::create(&out_path).map_err(|e| format!("failed to create extracted file: {e}"))?;
        io::copy(&mut file, &mut out_file).map_err(|e| format!("failed to write extracted file: {e}"))?;
    }

    Ok(())
}

fn try_remove_extension_dir(target: &Path, extension_id: &str, reason: &str) -> bool {
    for attempt in 1..=3 {
        match fs::remove_dir_all(target) {
            Ok(_) => {
                info!(
                    extension_id = extension_id,
                    path = target.to_string_lossy().as_ref(),
                    attempt = attempt,
                    reason = reason,
                    "removed LSP extension directory"
                );
                return true;
            }
            Err(error) => {
                warn!(
                    extension_id = extension_id,
                    path = target.to_string_lossy().as_ref(),
                    attempt = attempt,
                    reason = reason,
                    error = ?error,
                    "failed to remove LSP extension directory, will retry or defer cleanup"
                );
                std::thread::sleep(Duration::from_millis(120 * attempt));
            }
        }
    }

    if let Some(parent) = target.parent() {
        let pending_name = format!(
            ".delete-pending-{}-{}",
            sanitize_dir_fragment(&normalize_extension_id(extension_id)),
            sanitize_dir_fragment(&Uuid::new_v4().to_string())
        );
        let pending_path = parent.join(pending_name);
        match fs::rename(target, &pending_path) {
            Ok(_) => {
                warn!(
                    extension_id = extension_id,
                    from = target.to_string_lossy().as_ref(),
                    to = pending_path.to_string_lossy().as_ref(),
                    reason = reason,
                    "renamed locked LSP extension directory for deferred cleanup"
                );
                let _ = fs::remove_dir_all(&pending_path);
                return true;
            }
            Err(error) => {
                warn!(
                    extension_id = extension_id,
                    path = target.to_string_lossy().as_ref(),
                    pending_path = pending_path.to_string_lossy().as_ref(),
                    reason = reason,
                    error = ?error,
                    "locked LSP extension directory could not be renamed; installation will continue with a unique target directory"
                );
            }
        }
    }

    false
}

fn remove_existing_extension_versions(extension_id: &str) -> Result<(), String> {
    let root = extensions_root_dir()?;
    let installed = load_installed_extensions()?;
    for item in installed {
        if normalize_extension_id(&item.id) != normalize_extension_id(extension_id) {
            continue;
        }
        let target = item.install_dir;
        if target.exists() {
            info!(
                extension_id = extension_id,
                path = target.to_string_lossy().as_ref(),
                "removing previous installed extension version"
            );
            let _ = try_remove_extension_dir(&target, extension_id, "replace-installed-version");
        }
    }

    for entry in fs::read_dir(&root).map_err(|e| format!("failed to read extensions root: {e}"))? {
        let entry = entry.map_err(|e| format!("failed to read extension directory entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|item| item.to_str()) {
            if name.starts_with(&format!("{}-", sanitize_dir_fragment(extension_id))) {
                let _ = try_remove_extension_dir(&path, extension_id, "cleanup-stale-version");
            }
        }
    }

    Ok(())
}

fn copy_dir_contents(src_dir: &Path, dest_dir: &Path) -> Result<(), String> {
    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir).map_err(|e| format!("failed to reset destination directory: {e}"))?;
    }
    fs::create_dir_all(dest_dir).map_err(|e| format!("failed to create destination directory: {e}"))?;

    let mut options = CopyOptions::new();
    options.copy_inside = true;
    options.overwrite = true;
    options.content_only = true;

    copy_dir(src_dir, dest_dir, &options).map_err(|e| format!("failed to copy extension directory: {e}"))?;
    Ok(())
}

fn build_path_env_with_roots(roots: &[PathBuf]) -> Vec<(String, String)> {
    let mut all_paths = vec![];
    for root in roots {
        all_paths.push(root.clone());
    }
    if let Some(old) = std::env::var_os("PATH") {
        all_paths.extend(std::env::split_paths(&old));
    }

    let joined = std::env::join_paths(all_paths)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    vec![("PATH".to_string(), joined)]
}

fn resolve_extension_bin(dir: &Path, bin_name: &str) -> Option<PathBuf> {
    if bin_name.trim().is_empty() {
        return None;
    }

    let bin_dir = dir.join("node_modules").join(".bin");
    #[cfg(target_os = "windows")]
    {
        let cmd = bin_dir.join(format!("{bin_name}.cmd"));
        if cmd.is_file() {
            return Some(cmd);
        }
        let exe = bin_dir.join(format!("{bin_name}.exe"));
        if exe.is_file() {
            return Some(exe);
        }
    }

    let direct = bin_dir.join(bin_name);
    if direct.is_file() {
        return Some(direct);
    }

    None
}

fn node_package_candidates_for_bin(bin_name: &str) -> Vec<String> {
    let normalized = bin_name.trim();
    let mut candidates = match normalized {
        "basedpyright-langserver" => vec!["basedpyright".to_string()],
        "bash-language-server" => vec!["bash-language-server".to_string()],
        "typescript-language-server" => vec!["typescript-language-server".to_string()],
        _ => vec![],
    };
    if !normalized.is_empty() && !candidates.iter().any(|item| item == normalized) {
        candidates.push(normalized.to_string());
    }
    candidates
}

fn resolve_node_module_bin_entry(dir: &Path, bin_name: &str) -> Option<PathBuf> {
    if bin_name.trim().is_empty() {
        return None;
    }

    for package_name in node_package_candidates_for_bin(bin_name) {
        let package_dir = dir.join("node_modules").join(&package_name);
        let manifest_path = package_dir.join("package.json");
        let Some(manifest) = parse_manifest_from_path(&manifest_path) else {
            continue;
        };
        let Some(bin) = manifest.get("bin") else {
            continue;
        };
        let Some(relative) = (match bin {
            Value::String(value) => Some(value.as_str()),
            Value::Object(map) => map
                .get(bin_name)
                .or_else(|| map.values().next())
                .and_then(Value::as_str),
            _ => None,
        }) else {
            continue;
        };
        let entry = package_dir.join(relative);
        if entry.is_file() {
            return Some(entry);
        }
    }

    None
}

fn extension_bin_candidate_paths(dir: &Path, bin_name: &str) -> Vec<String> {
    if bin_name.trim().is_empty() {
        return vec![];
    }

    let bin_dir = dir.join("node_modules").join(".bin");
    let mut candidates = vec![];
    #[cfg(target_os = "windows")]
    {
        candidates.push(bin_dir.join(format!("{bin_name}.cmd")));
        candidates.push(bin_dir.join(format!("{bin_name}.exe")));
    }
    candidates.push(bin_dir.join(bin_name));
    for package_name in node_package_candidates_for_bin(bin_name) {
        candidates.push(
            dir.join("node_modules")
                .join(package_name)
                .join("package.json"),
        );
    }
    candidates
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

fn resolve_relative_entry(dir: &Path, entry_paths: &[String]) -> Option<PathBuf> {
    entry_paths
        .iter()
        .map(|item| dir.join(item))
        .find(|path| path.is_file())
}

fn relative_entry_candidate_paths(dir: &Path, entry_paths: &[String]) -> Vec<String> {
    entry_paths
        .iter()
        .map(|item| dir.join(item).to_string_lossy().to_string())
        .collect()
}

fn resolve_runtime_status(record: Option<&InstalledExtensionRecord>, catalog: Option<&CatalogExtension>) -> LspExtensionRuntimeStatus {
    let Some(catalog) = catalog else {
        return LspExtensionRuntimeStatus {
            ready: false,
            runtime: "unknown".to_string(),
            program_path: None,
            message: Some("extension is installed but not covered by the Guaxi LSP compatibility catalog".to_string()),
        };
    };

    let runtime = normalize_string(&catalog.lsp.runtime);

    let Some(record) = record else {
        return LspExtensionRuntimeStatus {
            ready: false,
            runtime,
            program_path: None,
            message: Some("extension not installed yet".to_string()),
        };
    };

    let install_dir = &record.install_dir;
    for item in &catalog.lsp.resolve_order {
        match item.as_str() {
            "extension_bin" => {
                if let Some(path) = resolve_extension_bin(install_dir, &catalog.lsp.bin_name) {
                    return LspExtensionRuntimeStatus {
                        ready: true,
                        runtime: runtime.clone(),
                        program_path: Some(path.to_string_lossy().to_string()),
                        message: Some("resolved from extension packaged node binary".to_string()),
                    };
                }
                if runtime == "node_binary" {
                    if let (Some(node_path), Some(entry_path)) = (
                        slidev_node_path(),
                        resolve_node_module_bin_entry(install_dir, &catalog.lsp.bin_name),
                    ) {
                        return LspExtensionRuntimeStatus {
                            ready: true,
                            runtime: runtime.clone(),
                            program_path: Some(node_path.to_string_lossy().to_string()),
                            message: Some(format!(
                                "resolved from extension node package entry: {}",
                                entry_path.to_string_lossy()
                            )),
                        };
                    }
                }
            }
            "extension_entry" => {
                if let Some(path) = resolve_relative_entry(install_dir, &catalog.lsp.entry_paths) {
                    return LspExtensionRuntimeStatus {
                        ready: true,
                        runtime: runtime.clone(),
                        program_path: Some(path.to_string_lossy().to_string()),
                        message: Some("resolved from extension bundled entry".to_string()),
                    };
                }
            }
            "slidev_env_bin" => {
                if let Some(root) = slidev_env_root().ok() {
                    if let Some(path) = resolve_extension_bin(&root, &catalog.lsp.bin_name) {
                        return LspExtensionRuntimeStatus {
                            ready: true,
                            runtime: runtime.clone(),
                            program_path: Some(path.to_string_lossy().to_string()),
                            message: Some("resolved from bundled SlidevEnv node runtime".to_string()),
                        };
                    }
                    if runtime == "node_binary" {
                        if let (Some(node_path), Some(entry_path)) = (
                            slidev_node_path(),
                            resolve_node_module_bin_entry(&root, &catalog.lsp.bin_name),
                        ) {
                            return LspExtensionRuntimeStatus {
                                ready: true,
                                runtime: runtime.clone(),
                                program_path: Some(node_path.to_string_lossy().to_string()),
                                message: Some(format!(
                                    "resolved from bundled SlidevEnv node package entry: {}",
                                    entry_path.to_string_lossy()
                                )),
                            };
                        }
                    }
                }
            }
            "downloadable_server" => {
                if let Some(server_id) = catalog.lsp.fallback_server.as_deref() {
                    if is_downloadable_fallback(server_id) {
                        return LspExtensionRuntimeStatus {
                            ready: true,
                            runtime: runtime.clone(),
                            program_path: None,
                            message: Some("server runtime can be auto-downloaded on first use".to_string()),
                        };
                    }
                }
            }
            _ => {}
        }
    }

    LspExtensionRuntimeStatus {
        ready: false,
        runtime,
        program_path: None,
        message: Some("runtime entry could not be resolved from extension, SlidevEnv, or fallback server".to_string()),
    }
}

fn build_extension_state(
    record: Option<InstalledExtensionRecord>,
    catalog: Option<CatalogExtension>,
) -> LspExtensionState {
    let runtime_status = resolve_runtime_status(record.as_ref(), catalog.as_ref());
    let id = record
        .as_ref()
        .map(|item| item.id.clone())
        .or_else(|| catalog.as_ref().map(|item| normalize_extension_id(&item.id)))
        .unwrap_or_default();
    let display_name = record
        .as_ref()
        .map(|item| item.display_name.clone())
        .or_else(|| catalog.as_ref().map(|item| item.display_name.clone()))
        .unwrap_or_else(|| id.clone());
    let description = record
        .as_ref()
        .and_then(|item| item.description.clone())
        .or_else(|| catalog.as_ref().and_then(|item| item.description.clone()));
    let languages = if let Some(record) = &record {
        record.languages.clone()
    } else {
        catalog
            .as_ref()
            .map(|item| item.languages.iter().map(|lang| normalize_language(lang)).collect())
            .unwrap_or_default()
    };
    let compatibility_level = catalog
        .as_ref()
        .map(|item| item.compatibility_level.clone())
        .unwrap_or_else(|| "C".to_string());
    let recommended = catalog.as_ref().map(|item| item.recommended).unwrap_or(false);
    let open_vsx = record
        .as_ref()
        .and_then(|item| item.open_vsx.clone())
        .or_else(|| catalog.as_ref().and_then(|item| item.open_vsx.clone()));
    let vscode_marketplace = record
        .as_ref()
        .and_then(|item| item.vscode_marketplace.clone())
        .or_else(|| catalog.as_ref().and_then(|item| item.vscode_marketplace.clone()));

    LspExtensionState {
        id,
        display_name,
        description,
        source: record
            .as_ref()
            .map(|item| item.source.clone())
            .unwrap_or_else(|| "catalog".to_string()),
        version: record
            .as_ref()
            .map(|item| item.version.clone())
            .unwrap_or_else(|| "not-installed".to_string()),
        languages,
        installed: record.is_some(),
        installed_path: record
            .as_ref()
            .map(|item| item.install_dir.to_string_lossy().to_string()),
        compatibility_level,
        recommended,
        open_vsx,
        vscode_marketplace,
        manifest_name: record.as_ref().and_then(|item| item.manifest_name.clone()),
        manifest_publisher: record.as_ref().and_then(|item| item.manifest_publisher.clone()),
        manifest_browser: record.as_ref().and_then(|item| item.manifest_browser.clone()),
        manifest_main: record.as_ref().and_then(|item| item.manifest_main.clone()),
        activation_events: record
            .as_ref()
            .map(|item| item.activation_events.clone())
            .unwrap_or_default(),
        runtime_status,
    }
}

pub fn list_lsp_extensions() -> Result<Vec<LspExtensionState>, String> {
    info!(
        catalog_version = LSP_EXTENSION_CATALOG.version,
        "listing LSP extension registry state"
    );

    let installed = load_installed_extensions()?;
    let installed_by_id = installed
        .into_iter()
        .map(|item| (normalize_extension_id(&item.id), item))
        .collect::<HashMap<_, _>>();
    let catalog_by_id = catalog_by_id();

    let mut ids = HashSet::new();
    ids.extend(catalog_by_id.keys().cloned());
    ids.extend(installed_by_id.keys().cloned());

    let mut result = ids
        .into_iter()
        .map(|id| {
            build_extension_state(
                installed_by_id.get(&id).cloned(),
                catalog_by_id.get(&id).cloned(),
            )
        })
        .collect::<Vec<_>>();

    result.sort_by(|a, b| {
        a.installed
            .cmp(&b.installed)
            .reverse()
            .then(a.display_name.cmp(&b.display_name))
    });
    Ok(result)
}

async fn download_to_file(url: &str, destination: &Path) -> Result<(), String> {
    let client = Client::new();
    let response = client
        .get(url)
        .header("User-Agent", "guaxi-lsp-extension")
        .send()
        .await
        .map_err(|e| format!("failed to download extension archive: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "failed to download extension archive: HTTP {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("failed to read extension archive response: {e}"))?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("failed to create download parent directory: {e}"))?;
    }

    fs::write(destination, &bytes).map_err(|e| format!("failed to save extension archive: {e}"))
}

fn finalize_install_from_unpacked_root(
    unpacked_root: &Path,
    meta: InstalledExtensionMeta,
    expected_extension_id: Option<&str>,
) -> Result<LspExtensionState, String> {
    let (manifest_root, manifest) = manifest_root_from_dir(unpacked_root)
        .ok_or_else(|| "failed to locate extension package.json in unpacked extension".to_string())?;

    let publisher = normalize_string(
        manifest
            .get("publisher")
            .and_then(Value::as_str)
            .ok_or_else(|| "extension manifest is missing publisher".to_string())?,
    );
    let name = normalize_string(
        manifest
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "extension manifest is missing name".to_string())?,
    );
    let version = normalize_string(
        manifest
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("0.0.0"),
    );
    let id = normalize_extension_id(&format!("{publisher}.{name}"));
    let expected_id = expected_extension_id
        .map(normalize_extension_id)
        .filter(|item| !item.is_empty());
    if let Some(expected_id) = expected_id.as_deref() {
        if id != expected_id {
            warn!(
                expected_extension_id = expected_id,
                manifest_extension_id = id.as_str(),
                "extension archive manifest id mismatch"
            );
            return Err(format!(
                "扩展包内容不匹配：请求安装 {}，但 VSIX manifest 是 {}。已阻止安装错误扩展。",
                expected_id, id
            ));
        }
    }

    remove_existing_extension_versions(&id)?;

    let install_dir = unique_install_dir(&id, &version)?;
    copy_dir_contents(&manifest_root, &install_dir)?;
    write_extension_meta(&install_dir, &meta)?;

    info!(
        extension_id = id.as_str(),
        version = version.as_str(),
        install_dir = install_dir.to_string_lossy().as_ref(),
        source = meta.source.as_str(),
        "extension installation finalized"
    );

    let record = record_from_dir(&install_dir)
        .ok_or_else(|| "failed to rebuild installed extension record after install".to_string())?;
    Ok(build_extension_state(Some(record), catalog_by_id().get(&id).cloned()))
}

fn write_catalog_extension_package(
    dest_dir: &Path,
    catalog: &CatalogExtension,
) -> Result<(), String> {
    let (publisher, name) = parse_extension_identifier(&catalog.id)?;
    fs::create_dir_all(dest_dir).map_err(|e| format!("failed to create catalog extension dir: {e}"))?;

    let manifest = json!({
        "publisher": publisher,
        "name": name,
        "displayName": catalog.display_name,
        "description": catalog.description,
        "version": format!("catalog.{}", LSP_EXTENSION_CATALOG.version),
        "activationEvents": catalog.languages.iter().map(|lang| format!("onLanguage:{lang}")).collect::<Vec<_>>(),
        "contributes": {
            "languages": catalog.languages.iter().map(|lang| json!({ "id": lang })).collect::<Vec<_>>()
        },
        "guaxi": {
            "catalogExtension": true,
            "serverId": catalog.lsp.server_id,
            "runtime": catalog.lsp.runtime,
            "fallbackServer": catalog.lsp.fallback_server,
        }
    });
    let manifest_content = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("failed to serialize catalog extension manifest: {e}"))?;
    fs::write(dest_dir.join("package.json"), manifest_content)
        .map_err(|e| format!("failed to write catalog extension manifest: {e}"))?;

    let mut readme = fs::File::create(dest_dir.join("README.md"))
        .map_err(|e| format!("failed to create catalog extension readme: {e}"))?;
    writeln!(
        readme,
        "# {}\n\nThis is a Guaxi managed LSP catalog extension. It provides a stable extension registry entry and lets Guaxi resolve the configured language server runtime.",
        catalog.display_name
    )
    .map_err(|e| format!("failed to write catalog extension readme: {e}"))?;
    Ok(())
}

async fn verify_catalog_extension_runtime(
    state: &LspExtensionState,
    source_label: &str,
) -> Result<Vec<lsp::LspRuntimeProbeResult>, String> {
    if !state.installed {
        return Err(format!("{} did not install extension {}", source_label, state.id));
    }

    let languages = state
        .languages
        .iter()
        .map(|item| normalize_language(item))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    if languages.is_empty() {
        return Err(format!("{} installed extension {} but no languages were declared", source_label, state.id));
    }

    if !state.runtime_status.ready {
        warn!(
            extension_id = state.id.as_str(),
            source = source_label,
            languages = ?languages,
            runtime = state.runtime_status.runtime.as_str(),
            message = state.runtime_status.message.as_deref().unwrap_or(""),
            "catalog LSP extension installed but runtime is not ready; keep extension record and report pending runtime"
        );
        return Ok(vec![]);
    }

    let mut probes = vec![];
    let mut errors = vec![];
    for language in languages {
        match lsp::probe_lsp_runtime_for_language(&language, None).await {
            Ok(probe) => {
                if !probe.completion_provider {
                    errors.push(format!(
                        "{}: initialize succeeded but completionProvider is missing",
                        language
                    ));
                    continue;
                }
                info!(
                    extension_id = state.id.as_str(),
                    source = source_label,
                    language = language.as_str(),
                    completion_provider = probe.completion_provider,
                    hover_provider = probe.hover_provider,
                    definition_provider = probe.definition_provider,
                    semantic_tokens_provider = probe.semantic_tokens_provider,
                    server_name = probe.server_name.as_deref().unwrap_or(""),
                    server_version = probe.server_version.as_deref().unwrap_or(""),
                    "catalog LSP extension runtime capability probe completed"
                );
                probes.push(probe);
            }
            Err(error) => {
                errors.push(format!("{}: {}", language, error));
            }
        }
    }

    if !errors.is_empty() {
        return Err(format!(
            "{} runtime probe failed for {}: {}",
            source_label,
            state.id,
            errors.join(" | ")
        ));
    }

    Ok(probes)
}

async fn verify_if_catalog_extension_runtime(
    state: LspExtensionState,
    source_label: &str,
) -> Result<LspExtensionState, String> {
    let catalog = catalog_by_id();
    let id = normalize_extension_id(&state.id);
    if !catalog.contains_key(&id) {
        return Ok(state);
    }

    match verify_catalog_extension_runtime(&state, source_label).await {
        Ok(probes) => {
            info!(
                extension_id = id.as_str(),
                source = source_label,
                probe_count = probes.len(),
                probes = ?probes,
                "verified installed catalog LSP extension runtime"
            );
            Ok(state)
        }
        Err(error) => {
            rollback_failed_catalog_install(&id, &error);
            Err(error)
        }
    }
}

fn rollback_failed_catalog_install(extension_id: &str, reason: &str) {
    warn!(
        extension_id = extension_id,
        reason = reason,
        "rolling back LSP extension installation because runtime verification failed"
    );
    if let Err(error) = remove_existing_extension_versions(extension_id) {
        warn!(
            extension_id = extension_id,
            error = error.as_str(),
            "failed to rollback LSP extension after runtime verification failure"
        );
    }
}

fn install_catalog_lsp_extension_record(
    catalog: &CatalogExtension,
) -> Result<LspExtensionState, String> {
    let id = normalize_extension_id(&catalog.id);
    let version = format!("catalog.{}", LSP_EXTENSION_CATALOG.version);
    remove_existing_extension_versions(&id)?;
    let install_dir = unique_install_dir(&id, &version)?;
    write_catalog_extension_package(&install_dir, catalog)?;
    write_extension_meta(
        &install_dir,
        &InstalledExtensionMeta {
            source: "guaxi-catalog".to_string(),
            installed_at: now_rfc3339(),
            download_url: None,
            open_vsx: catalog.open_vsx.clone(),
            vscode_marketplace: catalog.vscode_marketplace.clone(),
            original_file_name: None,
        },
    )?;

    info!(
        extension_id = id.as_str(),
        install_dir = install_dir.to_string_lossy().as_ref(),
        "catalog LSP extension installed"
    );
    let record = record_from_dir(&install_dir)
        .ok_or_else(|| "failed to rebuild catalog extension record after install".to_string())?;
    Ok(build_extension_state(Some(record), Some(catalog.clone())))
}

pub async fn install_catalog_lsp_extension(extension_id: &str) -> Result<LspExtensionState, String> {
    let id = normalize_extension_id(extension_id);
    if id.is_empty() {
        return Err("extension_id cannot be empty".to_string());
    }
    let catalog = catalog_by_id()
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("unknown LSP catalog extension: {}", extension_id))?;

    // 设计说明：
    // 1. “安装成功”必须代表后端可以真实启动 language server，而不只是写入一个扩展登记项。
    // 2. 因此按真实 VSIX -> Open VSX -> Guaxi catalog fallback 的顺序尝试，每次安装后都发送 initialize 探测。
    // 3. 探测要求 completionProvider 存在；语法高亮由前端 TextMate grammar 保底，semanticTokens 会在日志中记录。
    let mut attempts = vec![];

    if let Some(identifier) = catalog.vscode_marketplace.as_deref() {
        match install_vscode_marketplace_extension(identifier, None, None).await {
            Ok(state) => match verify_catalog_extension_runtime(&state, "VSCode Marketplace").await {
                Ok(probes) => {
                    info!(
                        extension_id = id.as_str(),
                        probe_count = probes.len(),
                        probes = ?probes,
                        "catalog LSP extension verified from VSCode Marketplace"
                    );
                    return Ok(state);
                }
                Err(error) => {
                    rollback_failed_catalog_install(&id, &error);
                    attempts.push(error);
                }
            },
            Err(error) => {
                warn!(
                    extension_id = id.as_str(),
                    error = error.as_str(),
                    "VSCode Marketplace catalog install attempt failed"
                );
                attempts.push(format!("VSCode Marketplace install failed: {}", error));
            }
        }
    }

    if let Some(identifier) = catalog.open_vsx.as_deref() {
        match install_open_vsx_extension(identifier, None, None).await {
            Ok(state) => match verify_catalog_extension_runtime(&state, "Open VSX").await {
                Ok(probes) => {
                    info!(
                        extension_id = id.as_str(),
                        probe_count = probes.len(),
                        probes = ?probes,
                        "catalog LSP extension verified from Open VSX"
                    );
                    return Ok(state);
                }
                Err(error) => {
                    rollback_failed_catalog_install(&id, &error);
                    attempts.push(error);
                }
            },
            Err(error) => {
                warn!(
                    extension_id = id.as_str(),
                    error = error.as_str(),
                    "Open VSX catalog install attempt failed"
                );
                attempts.push(format!("Open VSX install failed: {}", error));
            }
        }
    }

    let state = install_catalog_lsp_extension_record(&catalog)?;
    match verify_catalog_extension_runtime(&state, "Guaxi catalog fallback").await {
        Ok(probes) => {
            info!(
                extension_id = id.as_str(),
                probe_count = probes.len(),
                probes = ?probes,
                "catalog LSP extension verified from Guaxi fallback runtime"
            );
            Ok(state)
        }
        Err(error) => {
            rollback_failed_catalog_install(&id, &error);
            attempts.push(error);
            Err(format!(
                "LSP 扩展 {} 安装后未能通过真实启动检查，已回滚。尝试结果: {}",
                id,
                attempts.join(" || ")
            ))
        }
    }
}

pub async fn import_vsix_extension(vsix_path: &str) -> Result<LspExtensionState, String> {
    let source_path = PathBuf::from(vsix_path.trim());
    if !source_path.is_file() {
        return Err(format!("vsix file not found: {}", source_path.to_string_lossy()));
    }

    info!(
        path = source_path.to_string_lossy().as_ref(),
        "importing VSIX extension"
    );

    let unpack_root = temp_root_dir()?.join(format!("vsix_{}", Uuid::new_v4()));
    fs::create_dir_all(&unpack_root).map_err(|e| format!("failed to create VSIX temp directory: {e}"))?;
    extract_zip_to_dir(&source_path, &unpack_root)?;

    let result = finalize_install_from_unpacked_root(
        &unpack_root,
        InstalledExtensionMeta {
            source: "local-vsix".to_string(),
            installed_at: now_rfc3339(),
            download_url: None,
            open_vsx: None,
            vscode_marketplace: None,
            original_file_name: source_path.file_name().and_then(|item| item.to_str()).map(normalize_string),
        },
        None,
    );

    let _ = fs::remove_dir_all(&unpack_root);
    let state = result?;
    verify_if_catalog_extension_runtime(state, "local VSIX").await
}

pub async fn import_extension_directory(dir_path: &str) -> Result<LspExtensionState, String> {
    let source_dir = PathBuf::from(dir_path.trim());
    if !source_dir.is_dir() {
        return Err(format!("extension directory not found: {}", source_dir.to_string_lossy()));
    }

    info!(
        path = source_dir.to_string_lossy().as_ref(),
        "importing unpacked extension directory"
    );

    let state = finalize_install_from_unpacked_root(
        &source_dir,
        InstalledExtensionMeta {
            source: "local-directory".to_string(),
            installed_at: now_rfc3339(),
            download_url: None,
            open_vsx: None,
            vscode_marketplace: None,
            original_file_name: source_dir.file_name().and_then(|item| item.to_str()).map(normalize_string),
        },
        None,
    )?;
    verify_if_catalog_extension_runtime(state, "local directory").await
}

fn parse_open_vsx_result(item: &Value) -> Option<OpenVsxSearchResult> {
    let namespace = normalize_string(item.get("namespace")?.as_str()?);
    let name = normalize_string(item.get("name")?.as_str()?);
    let id = normalize_extension_id(&format!("{namespace}.{name}"));
    let version = normalize_string(item.get("version").and_then(Value::as_str).unwrap_or("latest"));
    let display_name = normalize_string(
        item.get("displayName")
            .and_then(Value::as_str)
            .or_else(|| item.get("display_name").and_then(Value::as_str))
            .unwrap_or(&name),
    );

    let download_url = item
        .get("files")
        .and_then(|files| files.get("download"))
        .and_then(Value::as_str)
        .map(normalize_string)
        .or_else(|| item.get("downloadUrl").and_then(Value::as_str).map(normalize_string));

    let icon_url = item
        .get("files")
        .and_then(|files| files.get("icon"))
        .and_then(Value::as_str)
        .map(normalize_string)
        .or_else(|| item.get("iconUrl").and_then(Value::as_str).map(normalize_string));

    Some(OpenVsxSearchResult {
        id,
        namespace,
        name,
        version,
        display_name,
        description: item
            .get("description")
            .and_then(Value::as_str)
            .map(normalize_string),
        download_url,
        icon_url,
    })
}

pub async fn search_open_vsx_extensions(query: &str) -> Result<Vec<OpenVsxSearchResult>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }

    let url = format!(
        "https://open-vsx.org/api/-/search?size=20&query={}",
        urlencoding::encode(trimmed)
    );
    info!(query = trimmed, url = url.as_str(), "searching Open VSX extensions");

    let client = Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "guaxi-lsp-extension")
        .send()
        .await
        .map_err(|e| format!("failed to query Open VSX search API: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Open VSX search failed: HTTP {}", response.status()));
    }

    let payload = response
        .json::<Value>()
        .await
        .map_err(|e| format!("failed to parse Open VSX search response: {e}"))?;

    let items = payload
        .get("extensions")
        .and_then(Value::as_array)
        .or_else(|| payload.get("results").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default();

    let mut result = items
        .iter()
        .filter_map(parse_open_vsx_result)
        .collect::<Vec<_>>();

    result.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(result)
}

fn parse_marketplace_extension(item: &Value) -> Option<VscodeMarketplaceSearchResult> {
    let name = normalize_string(item.get("extensionName")?.as_str()?);
    let publisher = normalize_string(
        item.get("publisher")
            .and_then(|publisher| publisher.get("publisherName"))
            .and_then(Value::as_str)?,
    );
    let id = normalize_extension_id(&format!("{publisher}.{name}"));
    let display_name = normalize_string(
        item.get("displayName")
            .and_then(Value::as_str)
            .unwrap_or(&name),
    );
    let version_item = item
        .get("versions")
        .and_then(Value::as_array)
        .and_then(|items| items.first())?;
    let version = normalize_string(version_item.get("version").and_then(Value::as_str).unwrap_or("latest"));
    let files = version_item
        .get("files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let asset_url = |asset_type: &str| {
        files
            .iter()
            .find(|file| file.get("assetType").and_then(Value::as_str) == Some(asset_type))
            .and_then(|file| file.get("source").and_then(Value::as_str))
            .map(normalize_string)
    };

    let download_url = asset_url("Microsoft.VisualStudio.Services.VSIXPackage")
        .or_else(|| {
            Some(format!(
                "https://marketplace.visualstudio.com/_apis/public/gallery/publishers/{}/vsextensions/{}/{}/vspackage",
                publisher, name, version
            ))
        });
    let icon_url = asset_url("Microsoft.VisualStudio.Services.Icons.Default");

    Some(VscodeMarketplaceSearchResult {
        id,
        publisher,
        name,
        version,
        display_name,
        description: item
            .get("shortDescription")
            .or_else(|| item.get("description"))
            .and_then(Value::as_str)
            .map(normalize_string),
        download_url,
        icon_url,
    })
}

pub async fn search_vscode_marketplace_extensions(
    query: &str,
) -> Result<Vec<VscodeMarketplaceSearchResult>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }

    // 设计说明：
    // 1. VSCode Marketplace 没有像 Open VSX 那样简单的公开搜索 GET 接口，VSCode 客户端使用 gallery
    //    extensionquery API 查询，这里按 VS Code Target + 搜索文本组合过滤。
    // 2. flags 取回版本、asset files、manifest 等下载所需信息；真正安装仍统一走 VSIX 解包与 manifest 解析。
    // 3. 这样配置页可以同时支持 Marketplace 与 Open VSX，但运行时 registry 仍只有一份。
    let body = json!({
        "filters": [{
            "criteria": [
                { "filterType": 8, "value": "Microsoft.VisualStudio.Code" },
                { "filterType": 10, "value": trimmed }
            ],
            "pageNumber": 1,
            "pageSize": 20,
            "sortBy": 0,
            "sortOrder": 0
        }],
        "flags": 914
    });

    info!(query = trimmed, "searching VSCode Marketplace extensions");
    let client = Client::new();
    let response = client
        .post("https://marketplace.visualstudio.com/_apis/public/gallery/extensionquery")
        .header("Accept", "application/json;api-version=3.0-preview.1")
        .header("Content-Type", "application/json")
        .header("User-Agent", "guaxi-lsp-extension")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("failed to query VSCode Marketplace: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("VSCode Marketplace search failed: HTTP {}", response.status()));
    }

    let payload = response
        .json::<Value>()
        .await
        .map_err(|e| format!("failed to parse VSCode Marketplace response: {e}"))?;
    let items = payload
        .get("results")
        .and_then(Value::as_array)
        .and_then(|results| results.first())
        .and_then(|result| result.get("extensions"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut result = items
        .iter()
        .filter_map(parse_marketplace_extension)
        .collect::<Vec<_>>();
    result.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(result)
}

async fn resolve_open_vsx_download_from_search(
    identifier: &str,
    requested_version: Option<&str>,
) -> Result<Option<(String, String, String)>, String> {
    let normalized_identifier = normalize_extension_id(identifier);
    if normalized_identifier.is_empty() {
        return Ok(None);
    }

    // 设计说明：
    // 1. Open VSX 的 detail API 对个别扩展会返回 404，但 search API 仍能给出可下载的 files.download。
    // 2. 这里做一次“精确 ID 优先”的兜底搜索，避免目录安装按钮因为单一 detail 接口失败而完全不可用。
    let search_result = search_open_vsx_extensions(&normalized_identifier).await?;
    let matched = search_result
        .into_iter()
        .find(|item| normalize_extension_id(&item.id) == normalized_identifier);

    let Some(item) = matched else {
        warn!(
            extension_id = normalized_identifier.as_str(),
            "Open VSX search fallback did not find an exact extension match"
        );
        return Ok(None);
    };

    let resolved_version = requested_version
        .map(normalize_string)
        .filter(|item| !item.is_empty())
        .unwrap_or_else(|| normalize_string(&item.version));
    let resolved_version = if resolved_version.is_empty() {
        "latest".to_string()
    } else {
        resolved_version
    };

    let resolved_url = item.download_url.clone().unwrap_or_else(|| {
        format!(
            "https://open-vsx.org/api/{}/{}/{}/file/{}-{}.vsix",
            item.namespace, item.name, resolved_version, item.id, resolved_version
        )
    });

    info!(
        extension_id = normalized_identifier.as_str(),
        version = resolved_version.as_str(),
        download_url = resolved_url.as_str(),
        "resolved Open VSX extension download URL from search fallback"
    );

    Ok(Some((resolved_url, resolved_version, item.id)))
}

async fn resolve_open_vsx_download(
    identifier: &str,
    version: Option<&str>,
    download_url: Option<&str>,
) -> Result<(String, String, String), String> {
    let (namespace, name) = parse_extension_identifier(identifier)?;
    let requested_version = version.map(normalize_string);

    if let Some(url) = download_url {
        let effective_version = requested_version.unwrap_or_else(|| "latest".to_string());
        return Ok((normalize_string(url), effective_version, format!("{namespace}.{name}")));
    }

    let detail_url = if let Some(version) = requested_version.as_deref() {
        format!("https://open-vsx.org/api/{namespace}/{name}/{version}")
    } else {
        format!("https://open-vsx.org/api/{namespace}/{name}")
    };

    info!(
        extension_id = identifier,
        detail_url = detail_url.as_str(),
        "resolving Open VSX extension download URL"
    );

    let client = Client::new();
    let response = client
        .get(&detail_url)
        .header("User-Agent", "guaxi-lsp-extension")
        .send()
        .await
        .map_err(|e| format!("failed to query Open VSX extension detail: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        warn!(
            extension_id = identifier,
            detail_url = detail_url.as_str(),
            status = status.as_u16(),
            "Open VSX detail lookup failed, attempting search fallback"
        );

        if let Some(fallback) =
            resolve_open_vsx_download_from_search(identifier, requested_version.as_deref()).await?
        {
            return Ok(fallback);
        }

        return Err(format!(
            "Open VSX detail lookup failed for {}: HTTP {}",
            identifier,
            status
        ));
    }

    let payload = response
        .json::<Value>()
        .await
        .map_err(|e| format!("failed to parse Open VSX extension detail: {e}"))?;

    let resolved_version = payload
        .get("version")
        .and_then(Value::as_str)
        .map(normalize_string)
        .or(requested_version)
        .unwrap_or_else(|| "latest".to_string());

    let resolved_url = payload
        .get("files")
        .and_then(|files| files.get("download"))
        .and_then(Value::as_str)
        .map(normalize_string)
        .or_else(|| payload.get("download").and_then(Value::as_str).map(normalize_string))
        .unwrap_or_else(|| {
            format!(
                "https://open-vsx.org/api/{namespace}/{name}/{resolved_version}/file/{namespace}.{name}-{resolved_version}.vsix"
            )
        });

    Ok((resolved_url, resolved_version, format!("{namespace}.{name}")))
}

pub async fn install_open_vsx_extension(
    identifier: &str,
    version: Option<&str>,
    download_url: Option<&str>,
) -> Result<LspExtensionState, String> {
    let (resolved_url, resolved_version, resolved_id) =
        resolve_open_vsx_download(identifier, version, download_url).await?;
    let download_file = temp_root_dir()?.join(format!(
        "{}-{}.vsix",
        sanitize_dir_fragment(&resolved_id),
        sanitize_dir_fragment(&resolved_version)
    ));

    info!(
        extension_id = resolved_id.as_str(),
        version = resolved_version.as_str(),
        download_url = resolved_url.as_str(),
        destination = download_file.to_string_lossy().as_ref(),
        "downloading Open VSX extension archive"
    );

    download_to_file(&resolved_url, &download_file).await?;

    let unpack_root = temp_root_dir()?.join(format!("openvsx_{}", Uuid::new_v4()));
    fs::create_dir_all(&unpack_root).map_err(|e| format!("failed to create Open VSX temp directory: {e}"))?;
    extract_zip_to_dir(&download_file, &unpack_root)?;

    let result = finalize_install_from_unpacked_root(
        &unpack_root,
        InstalledExtensionMeta {
            source: "openvsx".to_string(),
            installed_at: now_rfc3339(),
            download_url: Some(resolved_url),
            open_vsx: Some(resolved_id.clone()),
            vscode_marketplace: None,
            original_file_name: download_file.file_name().and_then(|item| item.to_str()).map(normalize_string),
        },
        Some(&resolved_id),
    );

    let _ = fs::remove_file(&download_file);
    let _ = fs::remove_dir_all(&unpack_root);
    let state = result?;
    verify_if_catalog_extension_runtime(state, "Open VSX").await
}

pub async fn install_vscode_marketplace_extension(
    identifier: &str,
    version: Option<&str>,
    download_url: Option<&str>,
) -> Result<LspExtensionState, String> {
    let (publisher, name) = parse_extension_identifier(identifier)?;
    let requested_version = version.map(normalize_string).filter(|item| !item.is_empty());

    let resolved = if let Some(url) = download_url {
        (
            normalize_string(url),
            requested_version.unwrap_or_else(|| "latest".to_string()),
            format!("{publisher}.{name}"),
        )
    } else {
        let search_result = search_vscode_marketplace_extensions(&format!("{publisher}.{name}")).await?;
        let matched = search_result
            .into_iter()
            .find(|item| normalize_extension_id(&item.id) == normalize_extension_id(identifier))
            .ok_or_else(|| format!("VSCode Marketplace did not return extension: {}", identifier))?;
        let resolved_version = requested_version.unwrap_or_else(|| matched.version.clone());
        let resolved_url = matched.download_url.unwrap_or_else(|| {
            format!(
                "https://marketplace.visualstudio.com/_apis/public/gallery/publishers/{}/vsextensions/{}/{}/vspackage",
                publisher, name, resolved_version
            )
        });
        (resolved_url, resolved_version, matched.id)
    };

    let (resolved_url, resolved_version, resolved_id) = resolved;
    let download_file = temp_root_dir()?.join(format!(
        "{}-{}.vsix",
        sanitize_dir_fragment(&resolved_id),
        sanitize_dir_fragment(&resolved_version)
    ));

    info!(
        extension_id = resolved_id.as_str(),
        version = resolved_version.as_str(),
        download_url = resolved_url.as_str(),
        destination = download_file.to_string_lossy().as_ref(),
        "downloading VSCode Marketplace extension archive"
    );

    download_to_file(&resolved_url, &download_file).await?;

    let unpack_root = temp_root_dir()?.join(format!("marketplace_{}", Uuid::new_v4()));
    fs::create_dir_all(&unpack_root)
        .map_err(|e| format!("failed to create Marketplace temp directory: {e}"))?;
    extract_zip_to_dir(&download_file, &unpack_root)?;

    let expected_id = resolved_id.clone();
    let result = finalize_install_from_unpacked_root(
        &unpack_root,
        InstalledExtensionMeta {
            source: "vscode-marketplace".to_string(),
            installed_at: now_rfc3339(),
            download_url: Some(resolved_url),
            open_vsx: None,
            vscode_marketplace: Some(resolved_id),
            original_file_name: download_file.file_name().and_then(|item| item.to_str()).map(normalize_string),
        },
        Some(&expected_id),
    );

    let _ = fs::remove_file(&download_file);
    let _ = fs::remove_dir_all(&unpack_root);
    let state = result?;
    verify_if_catalog_extension_runtime(state, "VSCode Marketplace").await
}

pub fn remove_lsp_extension(extension_id: &str) -> Result<(), String> {
    let normalized = normalize_extension_id(extension_id);
    if normalized.is_empty() {
        return Err("extension_id cannot be empty".to_string());
    }

    info!(extension_id = normalized.as_str(), "removing installed LSP extension");
    remove_existing_extension_versions(&normalized)
}

pub(crate) fn has_installed_lsp_extension_for_language(language: &str) -> Result<bool, String> {
    let normalized_language = normalize_language(language);
    if normalized_language.is_empty() {
        return Ok(false);
    }

    let catalog = catalog_by_id();
    let installed = load_installed_extensions()?;
    let matched = installed.into_iter().any(|item| {
        if !item.languages.iter().any(|lang| normalize_language(lang) == normalized_language) {
            return false;
        }
        catalog.contains_key(&normalize_extension_id(&item.id))
    });
    Ok(matched)
}

pub(crate) async fn resolve_lsp_command_for_language(
    language: &str,
) -> Result<Option<ResolvedLspExtensionCommand>, String> {
    let normalized_language = normalize_language(language);
    if normalized_language.is_empty() {
        return Ok(None);
    }

    let installed = load_installed_extensions()?;
    let catalog = catalog_by_id();

    let mut candidates = installed
        .into_iter()
        .filter(|item| item.languages.iter().any(|lang| normalize_language(lang) == normalized_language))
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| a.display_name.cmp(&b.display_name));

    for candidate in candidates {
        let Some(catalog_entry) = catalog.get(&normalize_extension_id(&candidate.id)).cloned() else {
            continue;
        };

        info!(
            language = normalized_language.as_str(),
            extension_id = candidate.id.as_str(),
            runtime = catalog_entry.lsp.runtime.as_str(),
            "attempting to resolve LSP command from installed extension"
        );

        for strategy in &catalog_entry.lsp.resolve_order {
            match strategy.as_str() {
                "extension_bin" => {
                    if let Some(path) = resolve_extension_bin(&candidate.install_dir, &catalog_entry.lsp.bin_name) {
                        let env_roots = vec![
                            candidate.install_dir.clone(),
                            candidate.install_dir.join("node_modules").join(".bin"),
                            slidev_env_root().unwrap_or_else(|_| PathBuf::from(".")),
                        ];
                        return Ok(Some(ResolvedLspExtensionCommand {
                            program: path.to_string_lossy().to_string(),
                            args: catalog_entry.lsp.args.clone(),
                            envs: build_path_env_with_roots(&env_roots),
                            cwd: Some(candidate.install_dir.to_string_lossy().to_string()),
                            extension_id: candidate.id.clone(),
                            extension_display_name: candidate.display_name.clone(),
                            server_id: catalog_entry.lsp.server_id.clone(),
                        }));
                    }
                    if normalize_string(&catalog_entry.lsp.runtime) == "node_binary" {
                        if let (Some(node_path), Some(entry_path)) = (
                            slidev_node_path(),
                            resolve_node_module_bin_entry(&candidate.install_dir, &catalog_entry.lsp.bin_name),
                        ) {
                            // 设计思路：
                            // 1. 有些 VSCode/Open VSX 扩展或离线包只带 node_modules/<pkg>/package.json，
                            //    没有生成 node_modules/.bin/<bin>.cmd。
                            // 2. 这类情况仍然可以用内置 Node 直接执行 package.json 的 bin 入口，
                            //    避免因为缺一个 shim 文件就把已下载的 LSP 插件判定为不可用。
                            let mut args = vec![entry_path.to_string_lossy().to_string()];
                            args.extend(catalog_entry.lsp.args.clone());
                            let env_roots = vec![
                                candidate.install_dir.clone(),
                                candidate.install_dir.join("node_modules").join(".bin"),
                                slidev_env_root().unwrap_or_else(|_| PathBuf::from(".")),
                            ];
                            info!(
                                language = normalized_language.as_str(),
                                extension_id = candidate.id.as_str(),
                                strategy = "extension_bin_node_entry",
                                node = node_path.to_string_lossy().as_ref(),
                                entry = entry_path.to_string_lossy().as_ref(),
                                cwd = candidate.install_dir.to_string_lossy().as_ref(),
                                "resolved LSP server from extension node package bin entry"
                            );
                            return Ok(Some(ResolvedLspExtensionCommand {
                                program: node_path.to_string_lossy().to_string(),
                                args,
                                envs: build_path_env_with_roots(&env_roots),
                                cwd: Some(candidate.install_dir.to_string_lossy().to_string()),
                                extension_id: candidate.id.clone(),
                                extension_display_name: candidate.display_name.clone(),
                                server_id: catalog_entry.lsp.server_id.clone(),
                            }));
                        }
                    }
                    info!(
                        language = normalized_language.as_str(),
                        extension_id = candidate.id.as_str(),
                        strategy = "extension_bin",
                        bin_name = catalog_entry.lsp.bin_name.as_str(),
                        candidates = ?extension_bin_candidate_paths(&candidate.install_dir, &catalog_entry.lsp.bin_name),
                        "LSP extension runtime strategy did not resolve"
                    );
                }
                "extension_entry" => {
                    if let Some(path) = resolve_relative_entry(&candidate.install_dir, &catalog_entry.lsp.entry_paths) {
                        if normalize_string(&catalog_entry.lsp.runtime) == "node_entry" {
                            if let Some(node_path) = slidev_node_path() {
                                // Design note: VSCode extensions such as basedpyright ship the
                                // language server as a bundled JS entry under the downloaded
                                // extension directory, not as node_modules/.bin. We still use the
                                // bundled Node executable from SlidevEnv, but the server code is
                                // loaded from the installed extension in `extensions`.
                                let mut args = vec![path.to_string_lossy().to_string()];
                                args.extend(catalog_entry.lsp.args.clone());
                                let env_roots = vec![
                                    candidate.install_dir.clone(),
                                    candidate.install_dir.join("node_modules").join(".bin"),
                                    slidev_env_root().unwrap_or_else(|_| PathBuf::from(".")),
                                ];
                                info!(
                                    language = normalized_language.as_str(),
                                    extension_id = candidate.id.as_str(),
                                    strategy = "extension_entry",
                                    runtime = catalog_entry.lsp.runtime.as_str(),
                                    node = node_path.to_string_lossy().as_ref(),
                                    entry = path.to_string_lossy().as_ref(),
                                    cwd = candidate.install_dir.to_string_lossy().as_ref(),
                                    "resolved LSP server from downloaded extension entry"
                                );
                                return Ok(Some(ResolvedLspExtensionCommand {
                                    program: node_path.to_string_lossy().to_string(),
                                    args,
                                    envs: build_path_env_with_roots(&env_roots),
                                    cwd: Some(candidate.install_dir.to_string_lossy().to_string()),
                                    extension_id: candidate.id.clone(),
                                    extension_display_name: candidate.display_name.clone(),
                                    server_id: catalog_entry.lsp.server_id.clone(),
                                }));
                            }
                            info!(
                                language = normalized_language.as_str(),
                                extension_id = candidate.id.as_str(),
                                strategy = "extension_entry",
                                entry = path.to_string_lossy().as_ref(),
                                "LSP extension entry exists but bundled node runtime is missing"
                            );
                        } else {
                            return Ok(Some(ResolvedLspExtensionCommand {
                                program: path.to_string_lossy().to_string(),
                                args: catalog_entry.lsp.args.clone(),
                                envs: vec![],
                                cwd: Some(candidate.install_dir.to_string_lossy().to_string()),
                                extension_id: candidate.id.clone(),
                                extension_display_name: candidate.display_name.clone(),
                                server_id: catalog_entry.lsp.server_id.clone(),
                            }));
                        }
                    }
                    info!(
                        language = normalized_language.as_str(),
                        extension_id = candidate.id.as_str(),
                        strategy = "extension_entry",
                        candidates = ?relative_entry_candidate_paths(&candidate.install_dir, &catalog_entry.lsp.entry_paths),
                        "LSP extension runtime strategy did not resolve"
                    );
                }
                "slidev_env_bin" => {
                    if let Some(root) = slidev_env_root().ok() {
                        if let Some(path) = resolve_extension_bin(&root, &catalog_entry.lsp.bin_name) {
                            let mut env_roots = vec![root.clone()];
                            if let Some(bin_dir) = slidev_bin_dir() {
                                env_roots.push(bin_dir);
                            }
                            return Ok(Some(ResolvedLspExtensionCommand {
                                program: path.to_string_lossy().to_string(),
                                args: catalog_entry.lsp.args.clone(),
                                envs: build_path_env_with_roots(&env_roots),
                                cwd: Some(root.to_string_lossy().to_string()),
                                extension_id: candidate.id.clone(),
                                extension_display_name: candidate.display_name.clone(),
                                server_id: catalog_entry.lsp.server_id.clone(),
                            }));
                        }
                        if normalize_string(&catalog_entry.lsp.runtime) == "node_binary" {
                            if let (Some(node_path), Some(entry_path)) = (
                                slidev_node_path(),
                                resolve_node_module_bin_entry(&root, &catalog_entry.lsp.bin_name),
                            ) {
                                let mut args = vec![entry_path.to_string_lossy().to_string()];
                                args.extend(catalog_entry.lsp.args.clone());
                                let mut env_roots = vec![root.clone()];
                                if let Some(bin_dir) = slidev_bin_dir() {
                                    env_roots.push(bin_dir);
                                }
                                info!(
                                    language = normalized_language.as_str(),
                                    extension_id = candidate.id.as_str(),
                                    strategy = "slidev_env_node_entry",
                                    node = node_path.to_string_lossy().as_ref(),
                                    entry = entry_path.to_string_lossy().as_ref(),
                                    cwd = root.to_string_lossy().as_ref(),
                                    "resolved LSP server from bundled SlidevEnv node package bin entry"
                                );
                                return Ok(Some(ResolvedLspExtensionCommand {
                                    program: node_path.to_string_lossy().to_string(),
                                    args,
                                    envs: build_path_env_with_roots(&env_roots),
                                    cwd: Some(root.to_string_lossy().to_string()),
                                    extension_id: candidate.id.clone(),
                                    extension_display_name: candidate.display_name.clone(),
                                    server_id: catalog_entry.lsp.server_id.clone(),
                                }));
                            }
                        }
                        info!(
                            language = normalized_language.as_str(),
                            extension_id = candidate.id.as_str(),
                            strategy = "slidev_env_bin",
                            bin_name = catalog_entry.lsp.bin_name.as_str(),
                            candidates = ?extension_bin_candidate_paths(&root, &catalog_entry.lsp.bin_name),
                            "LSP extension runtime strategy did not resolve"
                        );
                    } else {
                        info!(
                            language = normalized_language.as_str(),
                            extension_id = candidate.id.as_str(),
                            strategy = "slidev_env_bin",
                            "SlidevEnv root is unavailable while resolving LSP extension runtime"
                        );
                    }
                }
                "downloadable_server" => {
                    if let Some(server_id) = catalog_entry.lsp.fallback_server.as_deref() {
                        info!(
                            language = normalized_language.as_str(),
                            extension_id = candidate.id.as_str(),
                            managed_server = server_id,
                            "extension runtime not directly bundled, requesting managed LSP server runtime"
                        );
                        return Ok(None);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(None)
}
