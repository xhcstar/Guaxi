use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
use std::time::Duration;

use crate::guaxiservices::lsp_extensions;
use actix::prelude::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use parking_lot::Mutex as ParkingMutex;
use regex::Regex;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tar::Archive;
use tokio::fs as tokio_fs;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};
use tracing::{error, info, warn};
use walkdir::WalkDir;
use zip::ZipArchive;

#[derive(Debug, Clone)]
struct LspCommandConfig {
    program: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    cwd: Option<String>,
    extension_id: Option<String>,
    server_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LspRuntimeProbeResult {
    pub language: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub completion_provider: bool,
    pub completion_item_count: usize,
    pub completion_first_label: Option<String>,
    pub hover_provider: bool,
    pub definition_provider: bool,
    pub semantic_tokens_provider: bool,
    pub semantic_tokens_request_ok: bool,
    pub semantic_tokens_token_count: usize,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
}

struct LspProbeDocument {
    uri: String,
    language_id: String,
    text: String,
    completion_line: u32,
    completion_character: u32,
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
static ACTIVE_LSP_EXTENSION_PROCESSES: Lazy<ParkingMutex<HashMap<String, Vec<LspProcessRef>>>> =
    Lazy::new(|| ParkingMutex::new(HashMap::new()));
const LSP_EDITOR_RUNTIME_PREPARE_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone)]
struct LspProcessRef {
    process_id: u32,
    addr: Addr<LspWsSession>,
}

fn normalize_extension_id_for_process(raw: &str) -> String {
    raw.trim().to_lowercase()
}

fn register_lsp_extension_process(extension_id: &str, process_id: u32, addr: Addr<LspWsSession>) {
    let normalized = normalize_extension_id_for_process(extension_id);
    if normalized.is_empty() || process_id == 0 {
        return;
    }

    let mut processes = ACTIVE_LSP_EXTENSION_PROCESSES.lock();
    let entries = processes.entry(normalized.clone()).or_default();
    entries.retain(|item| item.process_id != process_id);
    entries.push(LspProcessRef { process_id, addr });
    info!(
        extension_id = normalized.as_str(),
        process_id = process_id,
        active_count = entries.len(),
        "registered active LSP process for extension"
    );
}

fn unregister_lsp_extension_process(
    extension_id: Option<&str>,
    process_id: Option<u32>,
    reason: &str,
) {
    let Some(process_id) = process_id.filter(|id| *id != 0) else {
        return;
    };
    let Some(extension_id) = extension_id
        .map(normalize_extension_id_for_process)
        .filter(|id| !id.is_empty())
    else {
        return;
    };

    let mut processes = ACTIVE_LSP_EXTENSION_PROCESSES.lock();
    if let Some(entries) = processes.get_mut(&extension_id) {
        let before = entries.len();
        entries.retain(|item| item.process_id != process_id);
        let after = entries.len();
        if entries.is_empty() {
            processes.remove(&extension_id);
        }
        info!(
            extension_id = extension_id.as_str(),
            process_id = process_id,
            removed = before.saturating_sub(after),
            reason = reason,
            "unregistered active LSP process for extension"
        );
    }
}

pub(crate) async fn stop_lsp_processes_for_extension(extension_id: &str) -> usize {
    let normalized = normalize_extension_id_for_process(extension_id);
    if normalized.is_empty() {
        return 0;
    }

    let processes = ACTIVE_LSP_EXTENSION_PROCESSES
        .lock()
        .get(&normalized)
        .cloned()
        .unwrap_or_default();
    if processes.is_empty() {
        info!(
            extension_id = normalized.as_str(),
            "no active LSP process needs to be stopped before extension mutation"
        );
        return 0;
    }

    info!(
        extension_id = normalized.as_str(),
        process_count = processes.len(),
        "stopping active LSP processes before extension mutation"
    );

    let mut stopped = 0usize;
    for process in processes {
        match process
            .addr
            .send(StopLspSession {
                reason: format!("extension-mutation:{normalized}"),
            })
            .await
        {
            Ok(_) => {
                stopped += 1;
                info!(
                    extension_id = normalized.as_str(),
                    process_id = process.process_id,
                    "requested active LSP process stop"
                );
            }
            Err(error) => {
                warn!(
                    extension_id = normalized.as_str(),
                    process_id = process.process_id,
                    error = ?error,
                    "failed to request active LSP process stop"
                );
            }
        }
    }

    sleep(Duration::from_millis(350)).await;
    stopped
}

fn exe_dir() -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("无法获取 exe 路径: {e}"))?;
    let dir = current_exe
        .parent()
        .ok_or_else(|| "无法获取 exe 所在目录".to_string())?
        .to_path_buf();
    Ok(dir)
}

fn lsp_root_dir() -> Result<PathBuf, String> {
    // 设计说明：
    // 1. LSP server 只能来自 Guaxi 管理的扩展目录或 LspRuntimes 目录，不能误用 utils/lsp 这类历史残留工具。
    // 2. 这里把托管二进制统一放到 LspRuntimes/lsp/<server>，和 Node 类 LSP 的 LspRuntimes/node 保持同一运行时边界。
    // 3. SlidevEnv 只提供 node.exe 解释器，不能作为 language server 包或 native LSP 二进制的来源。
    Ok(exe_dir()?.join("LspRuntimes").join("lsp"))
}

fn lsp_server_dir(server: &str) -> Result<PathBuf, String> {
    Ok(lsp_root_dir()?.join(server))
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
    url.split('/')
        .last()
        .filter(|v| !v.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn proxy_env_is_set(name: &str) -> bool {
    std::env::var(name)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn lsp_binary_http_client(operation: &str) -> Result<Client, String> {
    info!(
        operation,
        http_proxy = proxy_env_is_set("HTTP_PROXY") || proxy_env_is_set("http_proxy"),
        https_proxy = proxy_env_is_set("HTTPS_PROXY") || proxy_env_is_set("https_proxy"),
        all_proxy = proxy_env_is_set("ALL_PROXY") || proxy_env_is_set("all_proxy"),
        no_proxy = proxy_env_is_set("NO_PROXY") || proxy_env_is_set("no_proxy"),
        "building managed LSP binary HTTP client with system/env proxy support"
    );

    // 设计思路：
    // 1. 托管 LSP 二进制通常来自 GitHub Release，跨地区访问速度差异很大，所以保留 reqwest 默认代理能力。
    // 2. 不记录代理地址，只记录代理环境是否存在，避免把带认证信息的代理 URL 写进日志。
    // 3. 设置连接和请求总超时，下载源不可达时能尽快给出可诊断错误，而不是长时间卡住安装流程。
    ClientBuilder::new()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| format!("failed to build managed LSP binary HTTP client: {e}"))
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

fn find_existing_lsp_program(entry: &LspLanguageEntry) -> Option<PathBuf> {
    let bin_file = lsp_file_name(&entry.bin_name);
    if let Some(path) = find_lsp_binary_in_server(&entry.server, &bin_file) {
        info!(
            server = entry.server.as_str(),
            bin_name = entry.bin_name.as_str(),
            program = path.to_string_lossy().as_ref(),
            "resolved managed LSP program from approved runtime directory"
        );
        return Some(path);
    }

    info!(
        server = entry.server.as_str(),
        bin_name = entry.bin_name.as_str(),
        runtime_dir = lsp_server_dir(&entry.server)
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default()
            .as_str(),
        "managed LSP program is not present in approved runtime directory"
    );
    None
}

fn lsp_language_entry(language: &str) -> Option<&'static LspLanguageEntry> {
    LSP_LANGUAGES
        .iter()
        .find(|entry| entry.language == language)
}

fn build_lsp_command(
    entry: &LspLanguageEntry,
    program: String,
    extension_id: Option<String>,
) -> LspCommandConfig {
    // 设计说明：这里仅用于托管 native LSP fallback。Node 类 LSP 统一由
    // lsp_extensions.rs 解析为“SlidevEnv/node.exe + LspRuntimes/node 包入口”，
    // 避免把 SlidevEnv/node_modules 误当作 language server 来源。
    LspCommandConfig {
        program,
        args: entry.args.clone(),
        envs: vec![],
        cwd: None,
        extension_id,
        server_id: entry.server.clone(),
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

    let resp = req.send().await.map_err(|e| format!("下载失败: {e}"))?;

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
    archive
        .unpack(dest_dir)
        .map_err(|e| format!("解压失败: {e}"))?;
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
            return Err("GitHub API 触发限流，请在设置中填写 GitHub Token 后重试".to_string());
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
    let client = lsp_binary_http_client("download-managed-lsp-binary")?;
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
            tokio::task::spawn_blocking(move || {
                extract_zip_to_dir(&zip_path_clone, &install_dir_clone)
            })
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
            info!(
                server = server,
                asset = asset.name.as_str(),
                "下载 LSP 可执行文件"
            );
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
            info!(
                server = server,
                asset = asset.name.as_str(),
                "下载 LSP 压缩包"
            );
            download_to_path(&client, &asset.browser_download_url, &tar_path, token).await?;

            let tar_path_clone = tar_path.clone();
            let install_dir_clone = install_dir.clone();
            tokio::task::spawn_blocking(move || {
                extract_targz_to_dir(&tar_path_clone, &install_dir_clone)
            })
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
    allow_runtime_download: bool,
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
            extension_id: Some(resolved.extension_id),
            server_id: resolved.server_id,
        });
    }

    if !lsp_extensions::has_installed_lsp_extension_for_language(language)? {
        return Err(format!(
            "当前语言未安装可用的 LSP 扩展，请先在设置的 LSP 扩展架构中安装对应语言插件: {}",
            language
        ));
    }

    // 设计说明：
    // 1. WebSocket 启动阶段只允许连接已经准备好的扩展或 LspRuntimes，不从 utils/SlidevEnv/node_modules、utils/lsp、PATH 捡历史残留。
    // 2. 已安装扩展仍然可以授权托管 runtime；安装/探测阶段负责下载或补齐 runtime，编辑器输入时不触发安装。
    // 3. 这样设置页、运行时目录和实际补全进程会看到同一套来源边界，避免不同模块互相串用。
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
    } else if allow_runtime_download
        && entry.auto_download
        && lsp_binary_spec(&entry.server).is_some()
    {
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
            "LSP runtime is not ready: language={} server={} bin={}。请确认对应语言插件已安装，并且运行时可执行文件存在于扩展目录或 LspRuntimes 目录中。",
            language, entry.server, entry.bin_name
        ));
    };

    let extension_id = lsp_extensions::installed_lsp_extension_ids_for_language(language)?
        .into_iter()
        .find(|item| !item.trim().is_empty());
    let cfg = build_lsp_command(entry, program, extension_id);
    info!(
        language = language,
        server = entry.server.as_str(),
        extension_id = cfg.extension_id.as_deref().unwrap_or(""),
        program = cfg.program.as_str(),
        args = ?cfg.args,
        cwd = cfg.cwd.as_deref().unwrap_or(""),
        env_count = cfg.envs.len(),
        "prepared managed LSP command"
    );
    Ok(cfg)
}

async fn prepare_lsp_command(language: &str) -> Result<LspCommandConfig, String> {
    // 设计说明：
    // 1. 编辑器 WebSocket 路径只负责连接已经准备好的 runtime，不能在用户敲代码时触发下载/解压。
    // 2. 下载和构建 runtime 必须发生在安装任务或健康探测任务里，失败时通过状态栏提示，不阻塞前端编辑体验。
    // 3. 如果 runtime 缺失，这里快速失败并关闭 LSP WebSocket，Monaco 仍可继续作为普通编辑器使用。
    prepare_lsp_command_with_token(language, None, false).await
}

pub struct LspWsSession {
    language: String,
    child: Option<Child>,
    child_stdin: Option<Arc<Mutex<ChildStdin>>>,
    extension_id: Option<String>,
    server_id: Option<String>,
    process_id: Option<u32>,
    pending_client_payloads: Vec<Vec<u8>>,
    pending_completion_request_ids: HashSet<String>,
}

impl LspWsSession {
    pub fn new(language: String) -> Self {
        Self {
            language,
            child: None,
            child_stdin: None,
            extension_id: None,
            server_id: None,
            process_id: None,
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
            return Err(
                "LSP probe stdout buffer exceeded 1MiB before a complete frame".to_string(),
            );
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

fn lsp_probe_server_request_result(method: &str, params: Option<&Value>) -> Value {
    match method {
        "workspace/configuration" => {
            let count = params
                .and_then(|item| item.get("items"))
                .and_then(Value::as_array)
                .map(|items| items.len())
                .unwrap_or(0);
            Value::Array((0..count).map(|_| json!({})).collect())
        }
        "workspace/workspaceFolders" => json!([]),
        "client/registerCapability" | "client/unregisterCapability" => Value::Null,
        "window/showMessageRequest" => Value::Null,
        _ => Value::Null,
    }
}

async fn respond_to_lsp_probe_server_request_if_needed(
    stdin: &mut ChildStdin,
    language: &str,
    value: &Value,
) -> Result<bool, String> {
    let Some(method) = value.get("method").and_then(Value::as_str) else {
        return Ok(false);
    };
    let Some(id) = value.get("id").cloned() else {
        return Ok(false);
    };

    // 设计说明：
    // 1. Pyright 等 Node LSP 在 initialized/didOpen 后会主动发 workspace/configuration 请求。
    // 2. 探针不是完整编辑器客户端，如果不回复这些服务端请求，server 会一直等待配置结果，
    //    后续 completion/semanticTokens 就会被卡住，最终把可用 runtime 误判为超时失败。
    // 3. 这里返回最小安全结果，并记录日志，既保证安装校验能走完，也不会伪造用户配置。
    let result = lsp_probe_server_request_result(method, value.get("params"));
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    });
    info!(
        language = language,
        request_method = method,
        "responding to LSP probe server request with minimal client capability result"
    );
    write_lsp_probe_payload_to_child_stdin(stdin, &response).await?;
    Ok(true)
}

fn read_lsp_capability_bool(capabilities: &Value, field: &str) -> bool {
    match capabilities.get(field) {
        Some(Value::Bool(value)) => *value,
        Some(Value::Object(_)) => true,
        _ => false,
    }
}

fn read_lsp_capability_present(capabilities: &Value, field: &str) -> bool {
    !matches!(
        capabilities.get(field),
        None | Some(Value::Null) | Some(Value::Bool(false))
    )
}

fn lsp_probe_document_for_language(language: &str) -> LspProbeDocument {
    let language_id = language.trim().to_lowercase();
    let (ext, text, completion_line, completion_character) = match language_id.as_str() {
        "javascript" => (
            "js",
            "const guaxiValue = { alpha: 1, beta: 2 };\nguaxiValue.".to_string(),
            1,
            11,
        ),
        "typescript" => (
            "ts",
            "const guaxiValue = { alpha: number; beta: number } = { alpha: 1, beta: 2 };\nguaxiValue.".to_string(),
            1,
            11,
        ),
        "html" => (
            "html",
            "<!doctype html>\n<html>\n<body>\n<div cla".to_string(),
            3,
            8,
        ),
        "css" => (
            "css",
            ".guaxi-probe {\n  col".to_string(),
            1,
            5,
        ),
        "json" => (
            "json",
            "{\n  \"name\": \"guaxi\",\n  \"".to_string(),
            2,
            3,
        ),
        "python" => ("py", "import os\nos.".to_string(), 1, 3),
        "c" => (
            "c",
            "#include <stdio.h>\nint main(void) {\n    pri\n}\n".to_string(),
            2,
            7,
        ),
        "cpp" => (
            "cpp",
            "#include <string>\nint main() {\n    std::string guaxi_value;\n    guaxi_value.\n}\n".to_string(),
            3,
            16,
        ),
        "rust" => (
            "rs",
            "fn main() {\n    let guaxi_value = String::new();\n    guaxi_value.\n}\n".to_string(),
            2,
            16,
        ),
        "java" => (
            "java",
            "class GuaxiProbe {\n  void run() {\n    String value = \"\";\n    value.\n  }\n}\n".to_string(),
            3,
            10,
        ),
        "markdown" => (
            "md",
            "# Guaxi Heading\n\n[Guaxi Heading](#".to_string(),
            2,
            17,
        ),
        "shell" => ("sh", "echo $".to_string(), 0, 6),
        "lua" => ("lua", "local guaxi_value = string\nstring.".to_string(), 1, 7),
        "mermaid" => (
            "mmd",
            "graph TD\n  A-->".to_string(),
            1,
            7,
        ),
        "latex" => (
            "tex",
            "\\documentclass{article}\n\\begin{document}\n\\ref{".to_string(),
            2,
            5,
        ),
        _ => ("txt", "guaxi".to_string(), 0, 5),
    };
    let base_dir = exe_dir().ok().unwrap_or_else(|| PathBuf::from("."));
    let uri = format!(
        "file:///{}/__guaxi_lsp_probe__/probe.{}",
        base_dir.to_string_lossy().replace('\\', "/"),
        ext
    );
    LspProbeDocument {
        uri,
        language_id,
        text,
        completion_line,
        completion_character,
    }
}

fn validate_lsp_completion_probe_result(result: &Value) -> Result<(usize, Option<String>), String> {
    // 设计说明：
    // 1. LSP completion 的合法返回值包含 CompletionList、CompletionItem[] 和 null。
    // 2. null/空数组只说明当前探针位置没有候选，不能用来判定 rust-analyzer、TexLab、Marksman 等 server 不可用。
    // 3. 真正的安装健康检查由 initialize、capabilities 和进程是否持续响应决定；候选数量只作为日志诊断信号。
    if result.is_null() {
        return Ok((0, None));
    }
    let Some(items) = extract_completion_items(result) else {
        return Err(format!(
            "completion response did not contain a completion list: {}",
            truncate_for_log(&result.to_string(), 1000)
        ));
    };
    if items.is_empty() {
        return Ok((0, None));
    }
    Ok((
        items.len(),
        items
            .first()
            .map(extract_completion_label)
            .filter(|item| !item.is_empty()),
    ))
}

fn validate_lsp_semantic_tokens_probe_result(result: &Value) -> Result<usize, String> {
    if result.is_null() {
        return Ok(0);
    }

    let Some(data) = result.get("data").and_then(Value::as_array) else {
        return Err(format!(
            "semanticTokens response did not contain a data array: {}",
            truncate_for_log(&result.to_string(), 1000)
        ));
    };

    if data.len() % 5 != 0 {
        return Err(format!(
            "semanticTokens response data length is invalid: {}",
            data.len()
        ));
    }

    Ok(data.len() / 5)
}

pub async fn probe_lsp_runtime_for_language(
    language: &str,
    token: Option<&str>,
) -> Result<LspRuntimeProbeResult, String> {
    let cfg = prepare_lsp_command_with_token(language, token, true).await?;
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

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "LSP runtime probe failed to start process: program={} error={}",
            cfg.program, e
        )
    })?;
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
        tokio::spawn(async move {
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
            let Some(payload) = read_lsp_payload_from_stdout(&mut stdout, &mut pending).await?
            else {
                return Err(
                    "LSP runtime probe stdout closed before initialize response".to_string()
                );
            };
            let value = serde_json::from_str::<Value>(&payload).map_err(|e| {
                format!(
                    "failed to parse LSP initialize response: {e}; payload={}",
                    truncate_for_log(&payload, 2000)
                )
            })?;
            if respond_to_lsp_probe_server_request_if_needed(&mut stdin, language, &value).await? {
                continue;
            }
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
            let completion_provider =
                read_lsp_capability_present(&capabilities, "completionProvider");
            let hover_provider = read_lsp_capability_bool(&capabilities, "hoverProvider");
            let definition_provider = read_lsp_capability_bool(&capabilities, "definitionProvider");
            let semantic_tokens_provider =
                read_lsp_capability_present(&capabilities, "semanticTokensProvider");

            let initialized_payload = json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            });
            let probe_document = lsp_probe_document_for_language(language);
            let did_open_payload = json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": probe_document.uri.clone(),
                        "languageId": probe_document.language_id.clone(),
                        "version": 1,
                        "text": probe_document.text.clone(),
                    }
                }
            });
            let completion_payload = json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": {
                        "uri": probe_document.uri.clone(),
                    },
                    "position": {
                        "line": probe_document.completion_line,
                        "character": probe_document.completion_character,
                    },
                    "context": {
                        "triggerKind": 1
                    }
                }
            });
            let semantic_tokens_payload = json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/semanticTokens/full",
                "params": {
                    "textDocument": {
                        "uri": probe_document.uri.clone(),
                    }
                }
            });
            let shutdown_payload = json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "shutdown",
                "params": null
            });
            let exit_payload = json!({
                "jsonrpc": "2.0",
                "method": "exit"
            });
            let _ = write_lsp_probe_payload_to_child_stdin(&mut stdin, &initialized_payload).await;
            if !completion_provider {
                let _ = write_lsp_probe_payload_to_child_stdin(&mut stdin, &shutdown_payload).await;
                let _ = write_lsp_probe_payload_to_child_stdin(&mut stdin, &exit_payload).await;
                return Ok(LspRuntimeProbeResult {
                    language: language.to_string(),
                    program: cfg.program.clone(),
                    args: cfg.args.clone(),
                    cwd: cfg.cwd.clone(),
                    completion_provider,
                    completion_item_count: 0,
                    completion_first_label: None,
                    hover_provider,
                    definition_provider,
                    semantic_tokens_provider,
                    semantic_tokens_request_ok: false,
                    semantic_tokens_token_count: 0,
                    server_name: read_server_info(&value, "name"),
                    server_version: read_server_info(&value, "version"),
                });
            }

            // 设计思路：
            // 1. initialize 只能证明进程能启动，不能证明 LSP 真的能处理编辑器请求。
            // 2. 这里打开一份最小测试文档并发送 completion；若 server 声明 semanticTokensProvider，
            //    还要追加 semanticTokens/full，避免把只会 initialize 的半成品 runtime 标记为可用。
            // 3. 失败时错误会回传给扩展安装任务，由任务退避重试配置，避免把“半配置好”的 runtime 标记为可用。
            write_lsp_probe_payload_to_child_stdin(&mut stdin, &did_open_payload).await?;
            write_lsp_probe_payload_to_child_stdin(&mut stdin, &completion_payload).await?;

            let (completion_item_count, completion_first_label) = loop {
                let Some(payload) = read_lsp_payload_from_stdout(&mut stdout, &mut pending).await?
                else {
                    return Err(
                        "LSP runtime probe stdout closed before completion response".to_string()
                    );
                };
                let value = serde_json::from_str::<Value>(&payload).map_err(|e| {
                    format!(
                        "failed to parse LSP completion response: {e}; payload={}",
                        truncate_for_log(&payload, 2000)
                    )
                })?;
                if respond_to_lsp_probe_server_request_if_needed(&mut stdin, language, &value)
                    .await?
                {
                    continue;
                }
                if value.get("id").and_then(Value::as_i64) != Some(2) {
                    continue;
                }
                if let Some(error) = value.get("error") {
                    return Err(format!("LSP completion returned error: {}", error));
                }
                let result = value
                    .get("result")
                    .ok_or_else(|| "LSP completion response did not contain result".to_string())?;
                break validate_lsp_completion_probe_result(result)?;
            };

            let semantic_tokens_token_count = if semantic_tokens_provider {
                write_lsp_probe_payload_to_child_stdin(&mut stdin, &semantic_tokens_payload)
                    .await?;
                loop {
                    let Some(payload) =
                        read_lsp_payload_from_stdout(&mut stdout, &mut pending).await?
                    else {
                        return Err(
                            "LSP runtime probe stdout closed before semanticTokens response"
                                .to_string(),
                        );
                    };
                    let value = serde_json::from_str::<Value>(&payload).map_err(|e| {
                        format!(
                            "failed to parse LSP semanticTokens response: {e}; payload={}",
                            truncate_for_log(&payload, 2000)
                        )
                    })?;
                    if respond_to_lsp_probe_server_request_if_needed(&mut stdin, language, &value)
                        .await?
                    {
                        continue;
                    }
                    if value.get("id").and_then(Value::as_i64) != Some(3) {
                        continue;
                    }
                    if let Some(error) = value.get("error") {
                        return Err(format!("LSP semanticTokens returned error: {}", error));
                    }
                    let result = value.get("result").ok_or_else(|| {
                        "LSP semanticTokens response did not contain result".to_string()
                    })?;
                    break validate_lsp_semantic_tokens_probe_result(result)?;
                }
            } else {
                0
            };
            let semantic_tokens_request_ok = semantic_tokens_provider;

            let _ = write_lsp_probe_payload_to_child_stdin(&mut stdin, &shutdown_payload).await;
            let _ = write_lsp_probe_payload_to_child_stdin(&mut stdin, &exit_payload).await;

            return Ok(LspRuntimeProbeResult {
                language: language.to_string(),
                program: cfg.program.clone(),
                args: cfg.args.clone(),
                cwd: cfg.cwd.clone(),
                completion_provider,
                completion_item_count,
                completion_first_label,
                hover_provider,
                definition_provider,
                semantic_tokens_provider,
                semantic_tokens_request_ok,
                semantic_tokens_token_count,
                server_name: read_server_info(&value, "name"),
                server_version: read_server_info(&value, "version"),
            });
        }
    };

    let result = timeout(Duration::from_secs(25), probe_result)
        .await
        .map_err(|_| "LSP runtime probe timed out while waiting for initialize/completion/semanticTokens response".to_string())?;

    match &result {
        Ok(probe) => {
            info!(
                language = language,
                program = probe.program.as_str(),
                completion_provider = probe.completion_provider,
                completion_item_count = probe.completion_item_count,
                completion_first_label = probe.completion_first_label.as_deref().unwrap_or(""),
                hover_provider = probe.hover_provider,
                definition_provider = probe.definition_provider,
                semantic_tokens_provider = probe.semantic_tokens_provider,
                semantic_tokens_request_ok = probe.semantic_tokens_request_ok,
                semantic_tokens_token_count = probe.semantic_tokens_token_count,
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
            let cfg = match timeout(
                LSP_EDITOR_RUNTIME_PREPARE_TIMEOUT,
                prepare_lsp_command(&language),
            )
            .await
            {
                Ok(Ok(cfg)) => cfg,
                Ok(Err(err)) => {
                    warn!(
                        language = language.as_str(),
                        error = err.as_str(),
                        "LSP 配置准备失败"
                    );
                    let _ = addr.try_send(LspProcessFailed { error: err });
                    return;
                }
                Err(_) => {
                    let err = format!(
                        "LSP runtime prepare timed out after {}s; install or repair the LSP extension runtime from Settings",
                        LSP_EDITOR_RUNTIME_PREPARE_TIMEOUT.as_secs()
                    );
                    warn!(
                        language = language.as_str(),
                        timeout_secs = LSP_EDITOR_RUNTIME_PREPARE_TIMEOUT.as_secs(),
                        "LSP 配置准备超时，已快速关闭本次编辑器连接"
                    );
                    let _ = addr.try_send(LspProcessFailed { error: err });
                    return;
                }
            };
            let program_for_log = cfg.program.clone();
            let args_for_log = cfg.args.clone();
            let cwd_for_log = cfg.cwd.clone();
            let env_count_for_log = cfg.envs.len();
            let extension_id_for_process = cfg.extension_id.clone();
            let server_id_for_process = cfg.server_id.clone();
            info!(
                language = language.as_str(),
                extension_id = extension_id_for_process.as_deref().unwrap_or(""),
                server_id = server_id_for_process.as_str(),
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
                        extension_id = extension_id_for_process.as_deref().unwrap_or(""),
                        server_id = server_id_for_process.as_str(),
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

                    addr.do_send(LspChildStarted {
                        child,
                        stdin: stdin_arc,
                        extension_id: extension_id_for_process,
                        server_id: server_id_for_process,
                        process_id,
                    });
                }
                Err(e) => {
                    let error_message = format!(
                        "Failed to start LSP process: program={} error={}",
                        program_for_log, e
                    );
                    error!(
                        program = program_for_log.as_str(),
                        error = ?e,
                        "Failed to start LSP process"
                    );
                    let _ = addr.try_send(LspProcessFailed {
                        error: error_message,
                    });
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
                            extension_id = act.extension_id.as_deref().unwrap_or(""),
                            server_id = act.server_id.as_deref().unwrap_or(""),
                            status = status.to_string().as_str(),
                            "LSP server process exited"
                        );
                        unregister_lsp_extension_process(
                            act.extension_id.as_deref(),
                            act.process_id,
                            "process-exited",
                        );
                        act.child = None;
                        act.child_stdin = None;
                        act.process_id = None;
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
        unregister_lsp_extension_process(
            self.extension_id.as_deref(),
            self.process_id,
            "actor-stopped",
        );
        if let Some(mut child) = self.child.take() {
            actix::spawn(async move {
                let _ = child.kill().await;
            });
        }
        self.child_stdin = None;
        self.process_id = None;
    }
}

struct LspChildStarted {
    child: Child,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    extension_id: Option<String>,
    server_id: String,
    process_id: u32,
}

impl Message for LspChildStarted {
    type Result = ();
}

impl Handler<LspChildStarted> for LspWsSession {
    type Result = ();

    fn handle(&mut self, msg: LspChildStarted, ctx: &mut Self::Context) {
        self.extension_id = msg.extension_id.clone();
        self.server_id = Some(msg.server_id.clone());
        self.process_id = Some(msg.process_id);
        if let Some(extension_id) = msg.extension_id.as_deref() {
            register_lsp_extension_process(extension_id, msg.process_id, ctx.address());
        }
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

struct StopLspSession {
    reason: String,
}

impl Message for StopLspSession {
    type Result = ();
}

impl Handler<StopLspSession> for LspWsSession {
    type Result = ();

    fn handle(&mut self, msg: StopLspSession, ctx: &mut Self::Context) {
        info!(
            language = self.language.as_str(),
            extension_id = self.extension_id.as_deref().unwrap_or(""),
            server_id = self.server_id.as_deref().unwrap_or(""),
            process_id = self.process_id.unwrap_or(0),
            reason = msg.reason.as_str(),
            "stopping LSP session on extension mutation request"
        );

        unregister_lsp_extension_process(
            self.extension_id.as_deref(),
            self.process_id,
            "extension-mutation-stop",
        );
        self.child_stdin = None;
        self.process_id = None;

        if let Some(mut child) = self.child.take() {
            actix::spawn(async move {
                let _ = child.kill().await;
            });
        }

        ctx.close(Some(ws::CloseReason {
            code: ws::CloseCode::Normal,
            description: Some(websocket_close_description(
                "LSP extension is being updated or removed",
            )),
        }));
        ctx.stop();
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
    if let Some(items) = result.get("items").and_then(|value| value.as_array()) {
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
    let first_label = first.map(extract_completion_label).unwrap_or_default();
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
            let text_document = value
                .get("params")
                .and_then(|item| item.get("textDocument"));
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
