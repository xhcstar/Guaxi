use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
use std::time::Duration;

use actix::prelude::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use parking_lot::Mutex as ParkingMutex;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::fs as tokio_fs;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};
use tracing::{error, info, warn};
use walkdir::WalkDir;
use zip::ZipArchive;
use flate2::read::GzDecoder;
use crate::guaxiservices::bundled_resources;
use crate::guaxiservices::lsp_extensions;
use tar::Archive;

#[derive(Debug, Clone)]
struct LspCommandConfig {
    program: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LspRuntimeProbeResult {
    pub language: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub completion_provider: bool,
    pub hover_provider: bool,
    pub definition_provider: bool,
    pub semantic_tokens_provider: bool,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
}

#[derive(Debug, Clone)]
enum LspAssetKind {
    Zip,
    Raw,
    TarGz,
}

#[derive(Debug, Clone)]
struct LspBinarySpec {
    repo: &'static str,
    asset_regex: &'static str,
    bin_name: &'static str,
    asset_kind: LspAssetKind,
    direct_url: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct LspLanguageEntry {
    language: String,
    server: String,
    bin_name: String,
    args: Vec<String>,
    auto_download: bool,
}

static LSP_LANGUAGES: Lazy<Vec<LspLanguageEntry>> = Lazy::new(|| {
    serde_json::from_str(include_str!("../../../shared/lsp_languages.json"))
        .expect("invalid shared LSP language table")
});

static LSP_INSTALLING: Lazy<ParkingMutex<HashSet<String>>> =
    Lazy::new(|| ParkingMutex::new(HashSet::new()));
static LSP_INSTALL_FAILED: Lazy<ParkingMutex<HashSet<String>>> =
    Lazy::new(|| ParkingMutex::new(HashSet::new()));

fn exe_dir() -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("无法获取 exe 路径: {e}"))?;
    let dir = current_exe
        .parent()
        .ok_or_else(|| "无法获取 exe 所在目录".to_string())?
        .to_path_buf();
    Ok(dir)
}

fn lsp_root_dir() -> Result<PathBuf, String> {
    Ok(exe_dir()?.join("lsp"))
}

fn lsp_server_dir(server: &str) -> Result<PathBuf, String> {
    Ok(lsp_root_dir()?.join(server))
}

fn slidev_env_root() -> Result<PathBuf, String> {
    bundled_resources::ensure_relative_dir("utils/SlidevEnv")
}

fn lsp_download_dir() -> Result<PathBuf, String> {
    Ok(exe_dir()?.join("tmp").join("lsp_downloads"))
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("创建目录失败: {}", e))
}

fn lsp_file_name(name: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        if Path::new(name).extension().is_some() {
            name.to_string()
        } else {
            format!("{}.exe", name)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        name.to_string()
    }
}

fn asset_name_from_url(url: &str, fallback: &str) -> String {
    url.split('/').last().filter(|v| !v.is_empty()).unwrap_or(fallback).to_string()
}

fn find_file_by_name(root: &Path, name: &str) -> Option<PathBuf> {
    for entry in WalkDir::new(root).into_iter().flatten() {
        if entry.file_type().is_file() && entry.file_name() == name {
            return Some(entry.path().to_path_buf());
        }
    }
    None
}

fn find_lsp_binary_in_server(server: &str, bin_name: &str) -> Option<PathBuf> {
    let dir = lsp_server_dir(server).ok()?;
    if !dir.is_dir() {
        return None;
    }
    if let Some(path) = find_file_by_name(&dir, bin_name) {
        return Some(path);
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(base) = bin_name.strip_suffix(".exe") {
            if let Some(path) = find_file_by_name(&dir, base) {
                return Some(path);
            }
        }
    }
    None
}

fn is_bundled_node_lsp(server: &str) -> bool {
    matches!(
        server,
        "basedpyright" | "bash-language-server" | "typescript-language-server"
    )
}

fn find_bundled_node_lsp_bin(bin_name: &str) -> Option<PathBuf> {
    if bin_name.trim().is_empty() {
        return None;
    }

    let root = slidev_env_root().ok()?;
    let bin_dir = root.join("node_modules").join(".bin");

    #[cfg(target_os = "windows")]
    {
        let cmd_path = bin_dir.join(format!("{bin_name}.cmd"));
        if cmd_path.is_file() {
            return Some(cmd_path);
        }

        let exe_path = bin_dir.join(format!("{bin_name}.exe"));
        if exe_path.is_file() {
            return Some(exe_path);
        }
    }

    let direct_path = bin_dir.join(bin_name);
    if direct_path.is_file() {
        return Some(direct_path);
    }

    None
}

fn find_program_on_path(bin_name: &str) -> Option<PathBuf> {
    let raw_name = bin_name.trim();
    if raw_name.is_empty() {
        return None;
    }

    let raw_path = PathBuf::from(raw_name);
    if raw_path.is_file() {
        return Some(raw_path);
    }

    let path_env = std::env::var_os("PATH")?;
    let search_names = {
        #[cfg(target_os = "windows")]
        {
            let path = Path::new(raw_name);
            if path.extension().is_some() {
                vec![raw_name.to_string()]
            } else {
                let pathext = std::env::var_os("PATHEXT")
                    .map(|value| {
                        value
                            .to_string_lossy()
                            .split(';')
                            .map(|item| item.trim().trim_start_matches('.').to_string())
                            .filter(|item| !item.is_empty())
                            .collect::<Vec<_>>()
                    })
                    .filter(|items| !items.is_empty())
                    .unwrap_or_else(|| vec!["EXE".to_string(), "CMD".to_string(), "BAT".to_string()]);
                let mut names = vec![raw_name.to_string()];
                names.extend(pathext.into_iter().map(|ext| format!("{raw_name}.{ext}")));
                names
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            vec![raw_name.to_string()]
        }
    };

    std::env::split_paths(&path_env).find_map(|dir| {
        search_names
            .iter()
            .map(|name| dir.join(name))
            .find(|path| path.is_file())
    })
}

fn find_existing_lsp_program(entry: &LspLanguageEntry) -> Option<PathBuf> {
    if is_bundled_node_lsp(&entry.server) {
        if let Some(path) = find_bundled_node_lsp_bin(&entry.bin_name) {
            return Some(path);
        }
    }

    let bin_file = lsp_file_name(&entry.bin_name);
    if let Some(path) = find_lsp_binary_in_server(&entry.server, &bin_file) {
        return Some(path);
    }

    let base_dir = exe_dir().ok()?;
    let mut candidates = vec![base_dir.join("lsp").join(&bin_file)];
    if let Ok(utils_lsp_bin) =
        bundled_resources::ensure_relative_dir(&format!("utils/lsp/{bin_file}"))
    {
        candidates.push(utils_lsp_bin);
    }

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        candidates.push(
            PathBuf::from(manifest_dir)
                .join("utils")
                .join("lsp")
                .join(&bin_file),
        );
    }

    candidates
        .into_iter()
        .find(|path| path.exists())
        .or_else(|| find_program_on_path(&entry.bin_name))
}

fn lsp_language_entry(language: &str) -> Option<&'static LspLanguageEntry> {
    LSP_LANGUAGES.iter().find(|entry| entry.language == language)
}

fn build_lsp_command(entry: &LspLanguageEntry, program: String) -> LspCommandConfig {
    let mut envs = vec![];
    let mut cwd = None;

    if is_bundled_node_lsp(&entry.server) {
        if let Ok(root) = slidev_env_root() {
            let mut paths = vec![root.clone(), root.join("node_modules").join(".bin")];
            if let Some(old) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&old));
            }
            if let Ok(joined) = std::env::join_paths(paths) {
                envs.push(("PATH".to_string(), joined.to_string_lossy().to_string()));
            }
            cwd = Some(root.to_string_lossy().to_string());
        }
    }

    LspCommandConfig {
        program,
        args: entry.args.clone(),
        envs,
        cwd,
    }
}

#[cfg(target_os = "windows")]
fn lsp_binary_spec(name: &str) -> Option<LspBinarySpec> {
    match name {
        "rust-analyzer" => Some(LspBinarySpec {
            repo: "rust-lang/rust-analyzer",
            asset_regex: r"rust-analyzer-x86_64-pc-windows-msvc\.zip$",
            bin_name: "rust-analyzer.exe",
            asset_kind: LspAssetKind::Zip,
            direct_url: None,
        }),
        "clangd" => Some(LspBinarySpec {
            repo: "clangd/clangd",
            asset_regex: r"clangd-windows-.*\.zip$",
            bin_name: "clangd.exe",
            asset_kind: LspAssetKind::Zip,
            direct_url: Some("https://github.com/clangd/clangd/releases/download/21.1.8/clangd-windows-21.1.8.zip"),
        }),
        "texlab" => Some(LspBinarySpec {
            repo: "latex-lsp/texlab",
            asset_regex: r"texlab-x86_64-windows\.zip$",
            bin_name: "texlab.exe",
            asset_kind: LspAssetKind::Zip,
            direct_url: Some("https://github.com/latex-lsp/texlab/releases/download/v5.25.1/texlab-x86_64-windows.zip"),
        }),
        "lua-language-server" => Some(LspBinarySpec {
            repo: "LuaLS/lua-language-server",
            asset_regex: r"win32-x64\.zip$",
            bin_name: "lua-language-server.exe",
            asset_kind: LspAssetKind::Zip,
            direct_url: None,
        }),
        "marksman" => Some(LspBinarySpec {
            repo: "artempyanykh/marksman",
            asset_regex: r"marksman\.exe$",
            bin_name: "marksman.exe",
            asset_kind: LspAssetKind::Raw,
            direct_url: Some("https://github.com/artempyanykh/marksman/releases/download/2026-02-08/marksman.exe"),
        }),
        "mermaid-lsp" => Some(LspBinarySpec {
            repo: "dawsh2/zed-mermaid-preview",
            asset_regex: r"mermaid-lsp-x86_64-pc-windows-msvc\.zip$",
            bin_name: "mermaid-lsp.exe",
            asset_kind: LspAssetKind::Zip,
            direct_url: Some("https://github.com/dawsh2/zed-mermaid-preview/releases/download/v0.1.24/mermaid-lsp-x86_64-pc-windows-msvc.zip"),
        }),
        "ruff" => Some(LspBinarySpec {
            repo: "astral-sh/ruff",
            asset_regex: r"ruff-x86_64-pc-windows-msvc\.zip$",
            bin_name: "ruff.exe",
            asset_kind: LspAssetKind::Zip,
            direct_url: Some("https://github.com/astral-sh/ruff/releases/download/0.15.1/ruff-x86_64-pc-windows-msvc.zip"),
        }),
        "biome" => Some(LspBinarySpec {
            repo: "biomejs/biome",
            asset_regex: r"biome-win32-x64\.exe$",
            bin_name: "biome.exe",
            asset_kind: LspAssetKind::Raw,
            direct_url: Some("https://github.com/biomejs/biome/releases/download/%40biomejs%2Fbiome%402.3.15/biome-win32-x64.exe"),
        }),
        "shellcheck" => Some(LspBinarySpec {
            repo: "koalaman/shellcheck",
            asset_regex: r"shellcheck-v0\.11\.0\.darwin\.x86_64\.tar\.gz$",
            bin_name: "shellcheck",
            asset_kind: LspAssetKind::TarGz,
            direct_url: Some("https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.darwin.x86_64.tar.gz"),
        }),
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
fn lsp_binary_spec(_name: &str) -> Option<LspBinarySpec> {
    None
}

async fn download_to_path(
    client: &Client,
    url: &str,
    dest: &Path,
    token: Option<&str>,
) -> Result<(), String> {
    let mut req = client.get(url).header("User-Agent", "guaxi-lsp");
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("下载失败: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }

    if let Some(parent) = dest.parent() {
        ensure_dir(parent)?;
    }

    let mut file = tokio_fs::File::create(dest)
        .await
        .map_err(|e| format!("写入失败: {e}"))?;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let data = chunk.map_err(|e| format!("下载失败: {e}"))?;
        file.write_all(&data)
            .await
            .map_err(|e| format!("写入失败: {e}"))?;
    }

    Ok(())
}

fn extract_zip_to_dir(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("打开压缩包失败: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("读取压缩包失败: {e}"))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("读取压缩包失败: {e}"))?;

        let Some(outpath) = file.enclosed_name().map(|p| dest_dir.join(p)) else {
            continue;
        };

        if file.is_dir() {
            fs::create_dir_all(&outpath).map_err(|e| format!("创建目录失败: {e}"))?;
            continue;
        }

        if let Some(parent) = outpath.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }

        let mut outfile = fs::File::create(&outpath).map_err(|e| format!("写入文件失败: {e}"))?;
        io::copy(&mut file, &mut outfile).map_err(|e| format!("写入文件失败: {e}"))?;
    }

    Ok(())
}

fn extract_targz_to_dir(tar_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(tar_path).map_err(|e| format!("打开压缩包失败: {e}"))?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    archive.unpack(dest_dir).map_err(|e| format!("解压失败: {e}"))?;
    Ok(())
}

async fn fetch_github_asset(
    client: &Client,
    spec: &LspBinarySpec,
    token: Option<&str>,
) -> Result<GithubAsset, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", spec.repo);
    let mut req = client
        .get(url)
        .header("User-Agent", "guaxi-lsp")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("获取发布信息失败: {e}"))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("获取发布信息失败: {e}"))?;

    if !status.is_success() {
        let lower = body.to_lowercase();
        if status.as_u16() == 403 && lower.contains("rate limit") && token.is_none() {
            return Err(
                "GitHub API 触发限流，请在设置中填写 GitHub Token 后重试".to_string(),
            );
        }
        return Err(format!("获取发布信息失败: HTTP {}: {}", status, body));
    }

    let json: Value = serde_json::from_str(&body).map_err(|e| format!("解析发布信息失败: {e}"))?;
    let assets = json
        .get("assets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            let message = json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("缺少 assets 字段");
            format!("解析发布信息失败: {}", message)
        })?;

    let regex = Regex::new(spec.asset_regex).map_err(|e| format!("解析正则失败: {e}"))?;
    let mut names: Vec<String> = Vec::new();
    for asset in assets {
        let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let download_url = asset
            .get("browser_download_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !name.is_empty() {
            names.push(name.to_string());
        }
        if !name.is_empty() && !download_url.is_empty() && regex.is_match(name) {
            return Ok(GithubAsset {
                name: name.to_string(),
                browser_download_url: download_url.to_string(),
            });
        }
    }

    if names.len() > 8 {
        names.truncate(8);
        names.push("...".to_string());
    }

    Err(format!(
        "未找到匹配的 LSP 资源: {}; 可用资源: {}",
        spec.asset_regex,
        names.join(", ")
    ))
}

async fn download_and_install_lsp_binary(
    server: &str,
    spec: &LspBinarySpec,
    token: Option<&str>,
) -> Result<PathBuf, String> {
    let client = Client::new();
    let asset = if let Some(url) = spec.direct_url {
        info!(server = server, url = url, "使用直链下载 LSP");
        GithubAsset {
            name: asset_name_from_url(url, spec.bin_name),
            browser_download_url: url.to_string(),
        }
    } else {
        fetch_github_asset(&client, spec, token).await?
    };

    let install_dir = lsp_server_dir(server)?;
    if install_dir.exists() {
        fs::remove_dir_all(&install_dir).map_err(|e| format!("清理旧目录失败: {e}"))?;
    }
    ensure_dir(&install_dir)?;

    match spec.asset_kind {
        LspAssetKind::Zip => {
            let download_dir = lsp_download_dir()?;
            ensure_dir(&download_dir)?;

            let zip_path = download_dir.join(&asset.name);
            info!(server = server, asset = asset.name.as_str(), "下载 LSP");
            download_to_path(&client, &asset.browser_download_url, &zip_path, token).await?;

            let zip_path_clone = zip_path.clone();
            let install_dir_clone = install_dir.clone();
            tokio::task::spawn_blocking(move || extract_zip_to_dir(&zip_path_clone, &install_dir_clone))
                .await
                .map_err(|e| format!("解压失败: {e}"))??;

            let Some(bin_path) = find_file_by_name(&install_dir, spec.bin_name) else {
                return Err(format!("未找到 LSP 可执行文件: {}", spec.bin_name));
            };

            info!(path = bin_path.to_string_lossy().as_ref(), "LSP 安装完成");
            Ok(bin_path)
        }
        LspAssetKind::Raw => {
            let bin_file = lsp_file_name(spec.bin_name);
            let bin_path = install_dir.join(&bin_file);
            info!(server = server, asset = asset.name.as_str(), "下载 LSP 可执行文件");
            download_to_path(&client, &asset.browser_download_url, &bin_path, token).await?;

            if !bin_path.exists() {
                return Err(format!("下载后未找到 LSP 可执行文件: {}", bin_file));
            }

            info!(path = bin_path.to_string_lossy().as_ref(), "LSP 安装完成");
            Ok(bin_path)
        }
        LspAssetKind::TarGz => {
            let download_dir = lsp_download_dir()?;
            ensure_dir(&download_dir)?;

            let tar_path = download_dir.join(&asset.name);
            info!(server = server, asset = asset.name.as_str(), "下载 LSP 压缩包");
            download_to_path(&client, &asset.browser_download_url, &tar_path, token).await?;

            let tar_path_clone = tar_path.clone();
            let install_dir_clone = install_dir.clone();
            tokio::task::spawn_blocking(move || extract_targz_to_dir(&tar_path_clone, &install_dir_clone))
                .await
                .map_err(|e| format!("解压失败: {e}"))??;

            let Some(bin_path) = find_file_by_name(&install_dir, spec.bin_name) else {
                return Err(format!("未找到 LSP 可执行文件: {}", spec.bin_name));
            };

            info!(path = bin_path.to_string_lossy().as_ref(), "LSP 安装完成");
            Ok(bin_path)
        }
    }
}

async fn ensure_lsp_binary(
    server: &str,
    bin_name: &str,
    token: Option<&str>,
) -> Result<PathBuf, String> {
    let bin_file = lsp_file_name(bin_name);
    if let Some(path) = find_lsp_binary_in_server(server, &bin_file) {
        return Ok(path);
    }

    let spec = lsp_binary_spec(server).ok_or_else(|| format!("不支持的 LSP 服务器: {}", server))?;
    let key = server.to_string();

    loop {
        if let Some(path) = find_lsp_binary_in_server(server, &bin_file) {
            return Ok(path);
        }

        let should_wait = {
            let mut installing = LSP_INSTALLING.lock();
            if installing.contains(&key) {
                true
            } else {
                installing.insert(key.clone());
                false
            }
        };

        if !should_wait {
            break;
        }

        sleep(Duration::from_millis(500)).await;
    }

    {
        let mut failed = LSP_INSTALL_FAILED.lock();
        failed.remove(&key);
    }

    let result = download_and_install_lsp_binary(server, &spec, token).await;

    let mut installing = LSP_INSTALLING.lock();
    installing.remove(&key);
    drop(installing);

    match result {
        Ok(path) => Ok(path),
        Err(err) => {
            LSP_INSTALL_FAILED.lock().insert(key);
            Err(err)
        }
    }
}

async fn prepare_lsp_command_with_token(
    language: &str,
    token: Option<&str>,
) -> Result<LspCommandConfig, String> {
    if let Some(resolved) = lsp_extensions::resolve_lsp_command_for_language(language).await? {
        info!(
            language = language,
            extension_id = resolved.extension_id.as_str(),
            extension_name = resolved.extension_display_name.as_str(),
            server_id = resolved.server_id.as_str(),
            program = resolved.program.as_str(),
            "resolved LSP runtime from installed extension"
        );
        return Ok(LspCommandConfig {
            program: resolved.program,
            args: resolved.args,
            envs: resolved.envs,
            cwd: resolved.cwd,
        });
    }

    if !lsp_extensions::has_installed_lsp_extension_for_language(language)? {
        return Err(format!(
            "当前语言未安装可用的 LSP 扩展，请先在设置的 LSP 扩展架构中安装对应语言插件: {}",
            language
        ));
    }

    // 设计说明：
    // 1. 用户可见的 LSP 体系已经统一为“扩展优先”，这里不再允许未安装扩展的语言直接走旧语言表。
    // 2. 旧的 binary 下载/SlidevEnv 解析只在“已安装扩展授权该 server”后作为托管 runtime 使用，
    //    解决 marksman/texlab/lua-language-server 这类 VSIX 本身不总是携带完整 server 的生产可用性问题。
    // 3. 这样设置页只保留扩展管理，但已有下载器能力仍能在扩展明确授权后兜底启动真实 LSP server。
    let entry = lsp_language_entry(language)
        .ok_or_else(|| format!("Unsupported language for LSP: {}", language))?;

    let program = if let Some(path) = find_existing_lsp_program(entry) {
        info!(
            language = language,
            server = entry.server.as_str(),
            bin_name = entry.bin_name.as_str(),
            program = path.to_string_lossy().as_ref(),
            "resolved managed LSP program from local runtime"
        );
        path.to_string_lossy().to_string()
    } else if entry.auto_download && lsp_binary_spec(&entry.server).is_some() {
        ensure_lsp_binary(&entry.server, &entry.bin_name, token)
            .await
            .map(|p| {
                info!(
                    language = language,
                    server = entry.server.as_str(),
                    bin_name = entry.bin_name.as_str(),
                    program = p.to_string_lossy().as_ref(),
                    "resolved managed LSP program after auto-download"
                );
                p.to_string_lossy().to_string()
            })?
    } else {
        return Err(format!(
            "LSP runtime is not ready: language={} server={} bin={}。请确认对应语言插件已安装，并且运行时可执行文件存在于扩展、SlidevEnv 或 PATH 中。",
            language, entry.server, entry.bin_name
        ));
    };

    let cfg = build_lsp_command(entry, program);
    info!(
        language = language,
        server = entry.server.as_str(),
        program = cfg.program.as_str(),
        args = ?cfg.args,
        cwd = cfg.cwd.as_deref().unwrap_or(""),
        env_count = cfg.envs.len(),
        "prepared managed LSP command"
    );
    Ok(cfg)
}

async fn prepare_lsp_command(language: &str) -> Result<LspCommandConfig, String> {
    prepare_lsp_command_with_token(language, None).await
}

pub struct LspWsSession {
    language: String,
    child: Option<Child>,
    child_stdin: Option<Arc<Mutex<ChildStdin>>>,
    pending_client_payloads: Vec<Vec<u8>>,
    pending_completion_request_ids: HashSet<String>,
}

impl LspWsSession {
    pub fn new(language: String) -> Self {
        Self {
            language,
            child: None,
            child_stdin: None,
            pending_client_payloads: Vec::new(),
            pending_completion_request_ids: HashSet::new(),
        }
    }
}

fn write_lsp_payload_to_stdin(stdin_arc: Arc<Mutex<ChildStdin>>, payload: Vec<u8>) {
    actix::spawn(async move {
        let mut guard = stdin_arc.lock().await;
        let header = format!("Content-Length: {}\r\n\r\n", payload.len());
        if let Err(error) = guard.write_all(header.as_bytes()).await {
            warn!(error = ?error, "failed to write LSP frame header to server stdin");
            return;
        }
        if let Err(error) = guard.write_all(&payload).await {
            warn!(error = ?error, "failed to write LSP payload to server stdin");
            return;
        }
        if let Err(error) = guard.flush().await {
            warn!(error = ?error, "failed to flush LSP server stdin");
        }
    });
}

fn websocket_close_description(message: &str) -> String {
    let max_bytes = 100usize;
    if message.len() <= max_bytes {
        return message.to_string();
    }

    let mut end = 0usize;
    for (index, ch) in message.char_indices() {
        let next = index + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }

    format!("{}...", &message[..end])
}

async fn read_lsp_payload_from_stdout<R>(
    stdout: &mut R,
    pending: &mut Vec<u8>,
) -> Result<Option<String>, String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = vec![0u8; 8192];
    loop {
        if let Some((header_end, content_len)) = parse_lsp_frame_header(pending) {
            let frame_end = header_end + content_len;
            if pending.len() >= frame_end {
                let payload = pending[header_end..frame_end].to_vec();
                pending.drain(0..frame_end);
                let text = String::from_utf8(payload)
                    .map_err(|e| format!("LSP server emitted non-UTF8 JSON-RPC payload: {e}"))?;
                return Ok(Some(text));
            }
        }

        let n = stdout
            .read(&mut buf)
            .await
            .map_err(|e| format!("failed to read LSP probe stdout: {e}"))?;
        if n == 0 {
            if pending.is_empty() {
                return Ok(None);
            }
            return Err("LSP server stdout closed with a partial JSON-RPC frame".to_string());
        }
        pending.extend_from_slice(&buf[..n]);
        if pending.len() > 1024 * 1024 {
            return Err("LSP probe stdout buffer exceeded 1MiB before a complete frame".to_string());
        }
    }
}

async fn write_lsp_probe_payload_to_child_stdin(
    stdin: &mut ChildStdin,
    payload: &Value,
) -> Result<(), String> {
    let data = serde_json::to_vec(payload)
        .map_err(|e| format!("failed to serialize LSP probe payload: {e}"))?;
    let header = format!("Content-Length: {}\r\n\r\n", data.len());
    stdin
        .write_all(header.as_bytes())
        .await
        .map_err(|e| format!("failed to write LSP probe header: {e}"))?;
    stdin
        .write_all(&data)
        .await
        .map_err(|e| format!("failed to write LSP probe payload: {e}"))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("failed to flush LSP probe stdin: {e}"))
}

fn read_server_info(value: &Value, field: &str) -> Option<String> {
    value
        .get("result")
        .and_then(|item| item.get("serverInfo"))
        .and_then(|item| item.get(field))
        .and_then(Value::as_str)
        .map(|item| item.to_string())
}

fn read_lsp_capability_bool(capabilities: &Value, field: &str) -> bool {
    match capabilities.get(field) {
        Some(Value::Bool(value)) => *value,
        Some(Value::Object(_)) => true,
        _ => false,
    }
}

fn read_lsp_capability_present(capabilities: &Value, field: &str) -> bool {
    !matches!(capabilities.get(field), None | Some(Value::Null) | Some(Value::Bool(false)))
}

pub async fn probe_lsp_runtime_for_language(
    language: &str,
    token: Option<&str>,
) -> Result<LspRuntimeProbeResult, String> {
    let cfg = prepare_lsp_command_with_token(language, token).await?;
    let program_for_log = cfg.program.clone();
    let args_for_log = cfg.args.clone();
    let cwd_for_log = cfg.cwd.clone();
    let env_count_for_log = cfg.envs.len();

    info!(
        language = language,
        program = program_for_log.as_str(),
        args = ?args_for_log,
        cwd = cwd_for_log.as_deref().unwrap_or(""),
        env_count = env_count_for_log,
        "probing LSP runtime with initialize request"
    );

    let mut cmd = Command::new(&cfg.program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.args(&cfg.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &cfg.envs {
        cmd.env(key, value);
    }
    if let Some(dir) = &cfg.cwd {
        cmd.current_dir(dir);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("LSP runtime probe failed to start process: program={} error={}", cfg.program, e))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "LSP runtime probe process did not expose stdin".to_string())?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "LSP runtime probe process did not expose stdout".to_string())?;
    let stderr = child.stderr.take();
    if let Some(stderr) = stderr {
        let language_for_stderr = language.to_string();
        let program_for_stderr = program_for_log.clone();
        actix::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            loop {
                match reader.next_line().await {
                    Ok(Some(line)) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        warn!(
                            language = language_for_stderr.as_str(),
                            program = program_for_stderr.as_str(),
                            stderr = trimmed,
                            "LSP probe stderr"
                        );
                    }
                    Ok(None) => break,
                    Err(error) => {
                        warn!(
                            language = language_for_stderr.as_str(),
                            program = program_for_stderr.as_str(),
                            error = ?error,
                            "failed to read LSP probe stderr"
                        );
                        break;
                    }
                }
            }
        });
    }

    let root_uri = exe_dir()
        .ok()
        .map(|path| format!("file:///{}", path.to_string_lossy().replace('\\', "/")));
    let initialize_payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": std::process::id(),
            "clientInfo": {
                "name": "Guaxi LSP runtime probe",
                "version": env!("CARGO_PKG_VERSION")
            },
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "completion": {
                        "dynamicRegistration": false,
                        "completionItem": {
                            "snippetSupport": true,
                            "documentationFormat": ["markdown", "plaintext"]
                        }
                    },
                    "hover": {
                        "dynamicRegistration": false,
                        "contentFormat": ["markdown", "plaintext"]
                    },
                    "definition": {
                        "dynamicRegistration": false
                    },
                    "semanticTokens": {
                        "dynamicRegistration": false,
                        "tokenTypes": [
                            "namespace", "type", "class", "enum", "interface", "struct",
                            "typeParameter", "parameter", "variable", "property", "enumMember",
                            "event", "function", "method", "macro", "keyword", "modifier",
                            "comment", "string", "number", "regexp", "operator", "decorator"
                        ],
                        "tokenModifiers": [
                            "declaration", "definition", "readonly", "static", "deprecated",
                            "abstract", "async", "modification", "documentation", "defaultLibrary"
                        ],
                        "formats": ["relative"],
                        "requests": {
                            "range": true,
                            "full": { "delta": false }
                        },
                        "multilineTokenSupport": false,
                        "overlappingTokenSupport": false
                    }
                },
                "workspace": {
                    "workspaceFolders": true,
                    "configuration": true
                }
            },
            "workspaceFolders": []
        }
    });

    let probe_result = async {
        write_lsp_probe_payload_to_child_stdin(&mut stdin, &initialize_payload).await?;
        let mut pending = Vec::new();
        loop {
            let Some(payload) = read_lsp_payload_from_stdout(&mut stdout, &mut pending).await? else {
                return Err("LSP runtime probe stdout closed before initialize response".to_string());
            };
            let value = serde_json::from_str::<Value>(&payload)
                .map_err(|e| format!("failed to parse LSP initialize response: {e}; payload={}", truncate_for_log(&payload, 2000)))?;
            if value.get("id").and_then(Value::as_i64) != Some(1) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(format!("LSP initialize returned error: {}", error));
            }

            let capabilities = value
                .get("result")
                .and_then(|item| item.get("capabilities"))
                .cloned()
                .unwrap_or(Value::Null);
            let completion_provider = read_lsp_capability_present(&capabilities, "completionProvider");
            let hover_provider = read_lsp_capability_bool(&capabilities, "hoverProvider");
            let definition_provider = read_lsp_capability_bool(&capabilities, "definitionProvider");
            let semantic_tokens_provider =
                read_lsp_capability_present(&capabilities, "semanticTokensProvider");

            let initialized_payload = json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            });
            let shutdown_payload = json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "shutdown",
                "params": null
            });
            let exit_payload = json!({
                "jsonrpc": "2.0",
                "method": "exit"
            });
            let _ = write_lsp_probe_payload_to_child_stdin(&mut stdin, &initialized_payload).await;
            let _ = write_lsp_probe_payload_to_child_stdin(&mut stdin, &shutdown_payload).await;
            let _ = write_lsp_probe_payload_to_child_stdin(&mut stdin, &exit_payload).await;

            return Ok(LspRuntimeProbeResult {
                language: language.to_string(),
                program: cfg.program.clone(),
                args: cfg.args.clone(),
                cwd: cfg.cwd.clone(),
                completion_provider,
                hover_provider,
                definition_provider,
                semantic_tokens_provider,
                server_name: read_server_info(&value, "name"),
                server_version: read_server_info(&value, "version"),
            });
        }
    };

    let result = timeout(Duration::from_secs(12), probe_result)
        .await
        .map_err(|_| "LSP runtime probe timed out while waiting for initialize response".to_string())?;

    match &result {
        Ok(probe) => {
            info!(
                language = language,
                program = probe.program.as_str(),
                completion_provider = probe.completion_provider,
                hover_provider = probe.hover_provider,
                definition_provider = probe.definition_provider,
                semantic_tokens_provider = probe.semantic_tokens_provider,
                server_name = probe.server_name.as_deref().unwrap_or(""),
                server_version = probe.server_version.as_deref().unwrap_or(""),
                "LSP runtime probe succeeded"
            );
        }
        Err(error) => {
            warn!(
                language = language,
                program = program_for_log.as_str(),
                error = error.as_str(),
                "LSP runtime probe failed"
            );
        }
    }

    let _ = child.kill().await;
    result
}

impl Actor for LspWsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        let language = self.language.clone();
        actix::spawn(async move {
            let cfg = match prepare_lsp_command(&language).await {
                Ok(cfg) => cfg,
                Err(err) => {
                    warn!(language = language.as_str(), error = err.as_str(), "LSP 配置准备失败");
                    let _ = addr.try_send(LspProcessFailed { error: err });
                    return;
                }
            };

            let program_for_log = cfg.program.clone();
            let args_for_log = cfg.args.clone();
            let cwd_for_log = cfg.cwd.clone();
            let env_count_for_log = cfg.envs.len();
            info!(
                language = language.as_str(),
                program = program_for_log.as_str(),
                args = ?args_for_log,
                cwd = cwd_for_log.as_deref().unwrap_or(""),
                env_count = env_count_for_log,
                "starting LSP server process"
            );
            let mut cmd = Command::new(cfg.program);

            #[cfg(windows)]
            cmd.creation_flags(CREATE_NO_WINDOW);

            cmd.args(cfg.args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            for (key, value) in cfg.envs {
                cmd.env(key, value);
            }
            if let Some(dir) = cfg.cwd {
                cmd.current_dir(dir);
            }

            match cmd.spawn() {
                Ok(mut child) => {
                    let stdout = child.stdout.take();
                    let stdin = child.stdin.take();
                    let stderr = child.stderr.take();

                    let stdin_arc = stdin.map(|s| Arc::new(Mutex::new(s)));
                    let process_id = child.id().unwrap_or(0);
                    info!(
                        language = language.as_str(),
                        program = program_for_log.as_str(),
                        process_id = process_id,
                        has_stdin = stdin_arc.is_some(),
                        has_stdout = stdout.is_some(),
                        has_stderr = stderr.is_some(),
                        "LSP server process started"
                    );

                    if let Some(mut stdout) = stdout {
                        let addr_clone = addr.clone();
                        let language_for_stdout = language.clone();
                        let program_for_stdout = program_for_log.clone();
                        actix::spawn(async move {
                            let mut buf = vec![0u8; 8192];
                            let mut pending = Vec::<u8>::new();
                            loop {
                                match stdout.read(&mut buf).await {
                                    Ok(0) => {
                                        warn!(
                                            language = language_for_stdout.as_str(),
                                            program = program_for_stdout.as_str(),
                                            "LSP server stdout closed"
                                        );
                                        break;
                                    }
                                    Ok(n) => {
                                        pending.extend_from_slice(&buf[..n]);

                                        loop {
                                            let Some((header_end, content_len)) =
                                                parse_lsp_frame_header(&pending)
                                            else {
                                                break;
                                            };

                                            let frame_end = header_end + content_len;
                                            if pending.len() < frame_end {
                                                break;
                                            }

                                            let payload = pending[header_end..frame_end].to_vec();
                                            pending.drain(0..frame_end);

                                            match String::from_utf8(payload) {
                                                Ok(data) => {
                                                    addr_clone.do_send(LspStdoutMessage { data });
                                                }
                                                Err(error) => {
                                                    warn!(
                                                        language = language_for_stdout.as_str(),
                                                        program = program_for_stdout.as_str(),
                                                        error = ?error,
                                                        "LSP server stdout emitted non-UTF8 payload"
                                                    );
                                                }
                                            }
                                        }

                                        if pending.len() > 1024 * 1024 {
                                            warn!(
                                                language = language_for_stdout.as_str(),
                                                program = program_for_stdout.as_str(),
                                                pending_bytes = pending.len(),
                                                "clearing oversized partial LSP stdout buffer"
                                            );
                                            pending.clear();
                                        }
                                    }
                                    Err(error) => {
                                        warn!(
                                            language = language_for_stdout.as_str(),
                                            program = program_for_stdout.as_str(),
                                            error = ?error,
                                            "failed to read LSP server stdout"
                                        );
                                        break;
                                    }
                                }
                            }
                        });
                    }

                    if let Some(stderr) = stderr {
                        let language_for_stderr = language.clone();
                        let program_for_stderr = program_for_log.clone();
                        actix::spawn(async move {
                            let mut reader = BufReader::new(stderr).lines();
                            loop {
                                match reader.next_line().await {
                                    Ok(Some(line)) => {
                                        let trimmed = line.trim();
                                        if trimmed.is_empty() {
                                            continue;
                                        }
                                        warn!(
                                            language = language_for_stderr.as_str(),
                                            program = program_for_stderr.as_str(),
                                            stderr = trimmed,
                                            "LSP server stderr"
                                        );
                                    }
                                    Ok(None) => break,
                                    Err(error) => {
                                        warn!(
                                            language = language_for_stderr.as_str(),
                                            program = program_for_stderr.as_str(),
                                            error = ?error,
                                            "failed to read LSP server stderr"
                                        );
                                        break;
                                    }
                                }
                            }
                        });
                    }

                    addr.do_send(LspChildStarted { child, stdin: stdin_arc });
                }
                Err(e) => {
                    let error_message = format!("Failed to start LSP process: program={} error={}", program_for_log, e);
                    error!(
                        program = program_for_log.as_str(),
                        error = ?e,
                        "Failed to start LSP process"
                    );
                    let _ = addr.try_send(LspProcessFailed { error: error_message });
                }
            }
        });

        ctx.run_interval(Duration::from_secs(30), |_act, ctx| {
            ctx.ping(b"ping");
        });
        ctx.run_interval(Duration::from_secs(2), |act, ctx| {
            if let Some(child) = act.child.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        warn!(
                            language = act.language.as_str(),
                            status = status.to_string().as_str(),
                            "LSP server process exited"
                        );
                        act.child = None;
                        act.child_stdin = None;
                        ctx.close(Some(ws::CloseReason {
                            code: ws::CloseCode::Error,
                            description: Some(websocket_close_description(&format!(
                                "LSP server process exited: {status}"
                            ))),
                        }));
                        ctx.stop();
                    }
                    Ok(None) => {}
                    Err(error) => {
                        warn!(
                            language = act.language.as_str(),
                            error = ?error,
                            "failed to poll LSP server process"
                        );
                    }
                }
            }
        });
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        if let Some(mut child) = self.child.take() {
            actix::spawn(async move {
                let _ = child.kill().await;
            });
        }
    }
}

struct LspChildStarted {
    child: Child,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
}

impl Message for LspChildStarted {
    type Result = ();
}

impl Handler<LspChildStarted> for LspWsSession {
    type Result = ();

    fn handle(&mut self, msg: LspChildStarted, _ctx: &mut Self::Context) {
        self.child = Some(msg.child);
        self.child_stdin = msg.stdin.clone();

        if let Some(stdin_arc) = msg.stdin {
            let pending_count = self.pending_client_payloads.len();
            if pending_count > 0 {
                info!(
                    language = self.language.as_str(),
                    pending_count = pending_count,
                    "flushing buffered LSP client messages after server stdin became ready"
                );
            }
            for payload in self.pending_client_payloads.drain(..) {
                write_lsp_payload_to_stdin(stdin_arc.clone(), payload);
            }
        }
    }
}

struct LspStdoutMessage {
    data: String,
}

impl Message for LspStdoutMessage {
    type Result = ();
}

impl Handler<LspStdoutMessage> for LspWsSession {
    type Result = ();

    fn handle(&mut self, msg: LspStdoutMessage, ctx: &mut Self::Context) {
        log_lsp_server_message(
            self.language.as_str(),
            msg.data.as_str(),
            &mut self.pending_completion_request_ids,
        );
        ctx.text(msg.data);
    }
}

struct LspProcessFailed {
    error: String,
}

impl Message for LspProcessFailed {
    type Result = ();
}

impl Handler<LspProcessFailed> for LspWsSession {
    type Result = ();

    fn handle(&mut self, msg: LspProcessFailed, ctx: &mut Self::Context) {
        ctx.close(Some(ws::CloseReason {
            code: ws::CloseCode::Error,
            description: Some(websocket_close_description(&msg.error)),
        }));
        ctx.stop();
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for LspWsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                log_lsp_client_message(
                    self.language.as_str(),
                    text.as_ref(),
                    &mut self.pending_completion_request_ids,
                );
                if let Some(stdin_arc) = &self.child_stdin {
                    let payload = text.into_bytes().to_vec();
                    write_lsp_payload_to_stdin(stdin_arc.clone(), payload);
                } else {
                    // 设计说明：
                    // 1. WebSocket 建立后，前端 language client 会立刻发送 initialize/didOpen/completion。
                    // 2. 后端 LSP 子进程是在 Actor started() 中异步启动的，stdin 尚未就绪时不能丢消息。
                    // 3. 这里先缓冲客户端帧，等 LspChildStarted 后按原顺序写入 stdin，确保初始化和首次补全不丢。
                    let payload = text.into_bytes().to_vec();
                    let buffered_count = self.pending_client_payloads.len() + 1;
                    self.pending_client_payloads.push(payload);
                    info!(
                        language = self.language.as_str(),
                        buffered_count = buffered_count,
                        "buffered LSP client message until server stdin is ready"
                    );
                }
            }
            Ok(ws::Message::Binary(_bin)) => {}
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Pong(_)) => {}
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

pub async fn lsp_ws_route(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, Error> {
    let language = path.into_inner().0;
    let session = LspWsSession::new(language);
    ws::start(session, &req, stream)
}

fn lsp_json_id_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(v) => Some(v.clone()),
        Value::Number(v) => Some(v.to_string()),
        _ => None,
    }
}

fn truncate_for_log(value: &str, max_chars: usize) -> String {
    let mut iter = value.chars();
    let mut result = String::new();
    for _ in 0..max_chars {
        let Some(ch) = iter.next() else {
            return result;
        };
        result.push(ch);
    }
    if iter.next().is_some() {
        result.push_str("...");
    }
    result
}

fn completion_items_preview_for_log(result: &Value, max_items: usize) -> String {
    let Some(items) = extract_completion_items(result) else {
        return "[]".to_string();
    };

    let preview = items
        .iter()
        .take(max_items)
        .map(|item| {
            serde_json::json!({
                "label": extract_completion_label(item),
                "kind": item.get("kind").and_then(|value| value.as_i64()).unwrap_or(-1),
                "detail": item.get("detail").and_then(|value| value.as_str()).unwrap_or(""),
                "insertText": item.get("insertText").and_then(|value| value.as_str()).unwrap_or(""),
                "filterText": item.get("filterText").and_then(|value| value.as_str()).unwrap_or(""),
            })
        })
        .collect::<Vec<_>>();

    serde_json::to_string(&preview).unwrap_or_else(|_| "[]".to_string())
}

fn extract_completion_item_count(result: &Value) -> usize {
    if let Some(items) = result
        .get("items")
        .and_then(|value| value.as_array())
    {
        return items.len();
    }
    if let Some(items) = result.as_array() {
        return items.len();
    }
    0
}

fn extract_completion_items(result: &Value) -> Option<&Vec<Value>> {
    result
        .get("items")
        .and_then(|value| value.as_array())
        .or_else(|| result.as_array())
}

fn extract_completion_label(item: &Value) -> String {
    let Some(label) = item.get("label") else {
        return String::new();
    };
    if let Some(text) = label.as_str() {
        return text.to_string();
    }
    label
        .get("label")
        .or_else(|| label.get("value"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string()
}

fn extract_completion_result_log_summary(result: &Value) -> (usize, String, i64, String) {
    let item_count = extract_completion_item_count(result);
    let Some(items) = extract_completion_items(result) else {
        return (0, String::new(), -1, String::new());
    };
    let first = items.first();
    let first_label = first
        .map(extract_completion_label)
        .unwrap_or_default();
    let first_kind = first
        .and_then(|item| item.get("kind"))
        .and_then(|value| value.as_i64())
        .unwrap_or(-1);
    let first_detail = first
        .and_then(|item| item.get("detail"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();

    (item_count, first_label, first_kind, first_detail)
}

fn log_lsp_client_message(
    language: &str,
    payload: &str,
    pending_completion_request_ids: &mut HashSet<String>,
) {
    let Ok(value) = serde_json::from_str::<Value>(payload) else {
        return;
    };

    let method = value
        .get("method")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    if method.is_empty() {
        return;
    }

    match method {
        "textDocument/didOpen" => {
            let text_document = value.get("params").and_then(|item| item.get("textDocument"));
            let uri = text_document
                .and_then(|item| item.get("uri"))
                .and_then(|item| item.as_str())
                .unwrap_or("");
            let version = text_document
                .and_then(|item| item.get("version"))
                .and_then(|item| item.as_i64())
                .unwrap_or(0);
            let text_len = text_document
                .and_then(|item| item.get("text"))
                .and_then(|item| item.as_str())
                .map(|text| text.len())
                .unwrap_or(0);
            info!(
                language = language,
                uri = uri,
                version = version,
                text_len = text_len,
                "LSP client sent didOpen"
            );
        }
        "textDocument/didChange" => {
            let params = value.get("params");
            let uri = params
                .and_then(|item| item.get("textDocument"))
                .and_then(|item| item.get("uri"))
                .and_then(|item| item.as_str())
                .unwrap_or("");
            let version = params
                .and_then(|item| item.get("textDocument"))
                .and_then(|item| item.get("version"))
                .and_then(|item| item.as_i64())
                .unwrap_or(0);
            let changes = params
                .and_then(|item| item.get("contentChanges"))
                .and_then(|item| item.as_array())
                .cloned()
                .unwrap_or_default();
            let latest_text_len = changes
                .last()
                .and_then(|item| item.get("text"))
                .and_then(|item| item.as_str())
                .map(|text| text.len())
                .unwrap_or(0);
            info!(
                language = language,
                uri = uri,
                version = version,
                change_count = changes.len(),
                latest_text_len = latest_text_len,
                "LSP client sent didChange"
            );
        }
        "textDocument/completion" => {
            let id = value
                .get("id")
                .and_then(lsp_json_id_to_string)
                .unwrap_or_default();
            let params = value.get("params");
            let uri = params
                .and_then(|item| item.get("textDocument"))
                .and_then(|item| item.get("uri"))
                .and_then(|item| item.as_str())
                .unwrap_or("");
            let line = params
                .and_then(|item| item.get("position"))
                .and_then(|item| item.get("line"))
                .and_then(|item| item.as_i64())
                .unwrap_or(-1);
            let character = params
                .and_then(|item| item.get("position"))
                .and_then(|item| item.get("character"))
                .and_then(|item| item.as_i64())
                .unwrap_or(-1);
            let trigger_kind = params
                .and_then(|item| item.get("context"))
                .and_then(|item| item.get("triggerKind"))
                .and_then(|item| item.as_i64())
                .unwrap_or(0);
            let trigger_character = params
                .and_then(|item| item.get("context"))
                .and_then(|item| item.get("triggerCharacter"))
                .and_then(|item| item.as_str())
                .unwrap_or("");

            if !id.is_empty() {
                pending_completion_request_ids.insert(id.clone());
            }

            info!(
                language = language,
                id = id,
                uri = uri,
                line = line,
                character = character,
                trigger_kind = trigger_kind,
                trigger_character = trigger_character,
                payload_preview = truncate_for_log(payload, 1200).as_str(),
                "LSP client sent completion request"
            );
        }
        "initialized" => {
            info!(language = language, "LSP client sent initialized");
        }
        _ => {}
    }
}

fn log_lsp_server_message(
    language: &str,
    payload: &str,
    pending_completion_request_ids: &mut HashSet<String>,
) {
    let Ok(value) = serde_json::from_str::<Value>(payload) else {
        return;
    };

    if let Some(method) = value.get("method").and_then(|item| item.as_str()) {
        match method {
            "textDocument/publishDiagnostics" => {
                let params = value.get("params");
                let uri = params
                    .and_then(|item| item.get("uri"))
                    .and_then(|item| item.as_str())
                    .unwrap_or("");
                let diagnostic_count = params
                    .and_then(|item| item.get("diagnostics"))
                    .and_then(|item| item.as_array())
                    .map(|items| items.len())
                    .unwrap_or(0);
                info!(
                    language = language,
                    uri = uri,
                    diagnostic_count = diagnostic_count,
                    "LSP server published diagnostics"
                );
            }
            _ => {}
        }
        return;
    }

    let Some(id) = value.get("id").and_then(lsp_json_id_to_string) else {
        return;
    };

    if !pending_completion_request_ids.remove(&id) {
        return;
    }

    if let Some(error) = value.get("error") {
        warn!(
            language = language,
            id = id,
            error = error.to_string(),
            "LSP server completion response returned error"
        );
        return;
    }

    let (item_count, first_label, first_kind, first_detail) = value
        .get("result")
        .map(extract_completion_result_log_summary)
        .unwrap_or((0, String::new(), -1, String::new()));
    let completion_preview = value
        .get("result")
        .map(|result| completion_items_preview_for_log(result, 8))
        .unwrap_or_else(|| "[]".to_string());
    info!(
        language = language,
        id = id,
        item_count = item_count,
        first_label = first_label,
        first_kind = first_kind,
        first_detail = first_detail,
        completion_preview = completion_preview.as_str(),
        payload_preview = truncate_for_log(payload, 4000).as_str(),
        "LSP server completion response resolved"
    );
}

fn parse_lsp_frame_header(bytes: &[u8]) -> Option<(usize, usize)> {
    let sep = bytes.windows(4).position(|w| w == b"\r\n\r\n")?;
    let header_end = sep + 4;
    let header = std::str::from_utf8(&bytes[..sep]).ok()?;

    let mut content_len = None;
    for line in header.split("\r\n") {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            if let Ok(v) = rest.trim().parse::<usize>() {
                content_len = Some(v);
                break;
            }
        }
    }

    content_len.map(|len| (header_end, len))
}
