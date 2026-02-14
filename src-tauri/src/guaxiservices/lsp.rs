use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use actix::prelude::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use parking_lot::Mutex as ParkingMutex;
use regex::Regex;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use tokio::fs as tokio_fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{error, info, warn};
use walkdir::WalkDir;
use zip::ZipArchive;
use flate2::read::GzDecoder;
use tar::Archive;

#[derive(Debug, Clone)]
struct LspCommandConfig {
    program: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    cwd: Option<String>,
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

#[derive(Debug, Clone)]
struct LspLanguageEntry {
    language: &'static str,
    server: &'static str,
    bin_name: &'static str,
    args: &'static [&'static str],
    auto_download: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LspServerStatus {
    pub language: String,
    pub server: String,
    pub supported: bool,
    pub installed: bool,
    pub installing: bool,
    pub install_dir: String,
    pub bin_path: Option<String>,
    pub message: Option<String>,
}

const LSP_LANGUAGES: &[LspLanguageEntry] = &[
    LspLanguageEntry {
        language: "cpp",
        server: "clangd",
        bin_name: "clangd",
        args: &["--enable-config"],
        auto_download: true,
    },
    LspLanguageEntry {
        language: "c",
        server: "clangd",
        bin_name: "clangd",
        args: &["--enable-config"],
        auto_download: true,
    },
    LspLanguageEntry {
        language: "rust",
        server: "rust-analyzer",
        bin_name: "rust-analyzer",
        args: &[],
        auto_download: true,
    },
    LspLanguageEntry {
        language: "lua",
        server: "lua-language-server",
        bin_name: "lua-language-server",
        args: &[],
        auto_download: true,
    },
    LspLanguageEntry {
        language: "latex",
        server: "texlab",
        bin_name: "texlab",
        args: &[],
        auto_download: true,
    },
    LspLanguageEntry {
        language: "python",
        server: "ruff",
        bin_name: "ruff",
        args: &["server"],
        auto_download: true,
    },
    LspLanguageEntry {
        language: "shell",
        server: "shellcheck",
        bin_name: "shellcheck",
        args: &[],
        auto_download: true,
    },
    LspLanguageEntry {
        language: "javascript",
        server: "biome",
        bin_name: "biome",
        args: &["lsp-proxy"],
        auto_download: true,
    },
    LspLanguageEntry {
        language: "typescript",
        server: "biome",
        bin_name: "biome",
        args: &["lsp-proxy"],
        auto_download: true,
    },
    LspLanguageEntry {
        language: "html",
        server: "html-ls",
        bin_name: "html-ls",
        args: &["--stdio"],
        auto_download: false,
    },
    LspLanguageEntry {
        language: "markdown",
        server: "marksman",
        bin_name: "marksman",
        args: &["server"],
        auto_download: true,
    },
    LspLanguageEntry {
        language: "java",
        server: "jdtls",
        bin_name: "jdtls",
        args: &[],
        auto_download: false,
    },
];

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

fn lsp_language_entry(language: &str) -> Option<&'static LspLanguageEntry> {
    LSP_LANGUAGES.iter().find(|entry| entry.language == language)
}

fn build_lsp_command(entry: &LspLanguageEntry, program: String) -> LspCommandConfig {
    LspCommandConfig {
        program,
        args: entry.args.iter().map(|v| v.to_string()).collect(),
        envs: vec![],
        cwd: None,
    }
}

fn resolve_lsp_binary(name: &str) -> String {
    let file_name = lsp_file_name(name);

    if let Some(path) = find_lsp_binary_in_server(name, &file_name) {
        return path.to_string_lossy().into_owned();
    }

    let base_dir = exe_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut candidates = vec![
        base_dir.join("lsp").join(&file_name),
        base_dir.join("utils").join("lsp").join(&file_name),
    ];

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        candidates.push(
            PathBuf::from(manifest_dir)
                .join("utils")
                .join("lsp")
                .join(&file_name),
        );
    }

    for p in candidates {
        if p.exists() {
            return p.to_string_lossy().into_owned();
        }
    }

    file_name
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

fn build_lsp_status(entry: &LspLanguageEntry) -> LspServerStatus {
    let server_dir = lsp_server_dir(entry.server).ok();
    let bin_file = lsp_file_name(entry.bin_name);
    let bin_path = server_dir
        .as_ref()
        .and_then(|dir| find_file_by_name(dir, &bin_file));
    let installed = bin_path.is_some();
    let supported = entry.auto_download && lsp_binary_spec(entry.server).is_some();
    let installing = LSP_INSTALLING.lock().contains(entry.server);
    let mut message = None;

    if !supported {
        message = Some("需要手动配置运行环境".to_string());
    } else if LSP_INSTALL_FAILED.lock().contains(entry.server) {
        message = Some("上次安装失败".to_string());
    }

    LspServerStatus {
        language: entry.language.to_string(),
        server: entry.server.to_string(),
        supported,
        installed,
        installing,
        install_dir: server_dir
            .map(|dir| dir.to_string_lossy().to_string())
            .unwrap_or_default(),
        bin_path: bin_path.map(|p| p.to_string_lossy().to_string()),
        message,
    }
}

pub async fn get_lsp_server_statuses() -> Vec<LspServerStatus> {
    LSP_LANGUAGES.iter().map(build_lsp_status).collect()
}

pub async fn install_lsp_server(
    language: &str,
    token: Option<String>,
) -> Result<LspServerStatus, String> {
    let entry = lsp_language_entry(language)
        .ok_or_else(|| format!("不支持的语言: {}", language))?;

    if lsp_binary_spec(entry.server).is_none() {
        return Err(format!("当前语言暂不支持自动下载: {}", language));
    }

    let _ = ensure_lsp_binary(entry.server, entry.bin_name, token.as_deref()).await?;
    Ok(build_lsp_status(entry))
}

async fn prepare_lsp_command(language: &str) -> Result<LspCommandConfig, String> {
    let entry = lsp_language_entry(language)
        .ok_or_else(|| format!("Unsupported language for LSP: {}", language))?;

    let program = if entry.auto_download && lsp_binary_spec(entry.server).is_some() {
        ensure_lsp_binary(entry.server, entry.bin_name, None)
            .await
            .map(|p| p.to_string_lossy().to_string())?
    } else {
        resolve_lsp_binary(entry.server)
    };

    Ok(build_lsp_command(entry, program))
}

pub struct LspWsSession {
    language: String,
    child: Option<Child>,
    child_stdin: Option<Arc<Mutex<ChildStdin>>>,
}

impl LspWsSession {
    pub fn new(language: String) -> Self {
        Self {
            language,
            child: None,
            child_stdin: None,
        }
    }
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
                    let _ = addr.try_send(LspProcessFailed);
                    return;
                }
            };

            let program_for_log = cfg.program.clone();
            let mut cmd = Command::new(cfg.program);
            cmd.args(cfg.args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());

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

                    let stdin_arc = stdin.map(|s| Arc::new(Mutex::new(s)));

                    if let Some(mut stdout) = stdout {
                        let addr_clone = addr.clone();
                        actix::spawn(async move {
                            let mut buf = vec![0u8; 8192];
                            let mut pending = Vec::<u8>::new();
                            loop {
                                match stdout.read(&mut buf).await {
                                    Ok(0) => break,
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

                                            if let Ok(data) = String::from_utf8(payload) {
                                                addr_clone.do_send(LspStdoutMessage { data });
                                            }
                                        }

                                        if pending.len() > 1024 * 1024 {
                                            pending.clear();
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        });
                    }

                    addr.do_send(LspChildStarted { child, stdin: stdin_arc });
                }
                Err(e) => {
                    error!(
                        program = program_for_log.as_str(),
                        error = ?e,
                        "Failed to start LSP process"
                    );
                    let _ = addr.try_send(LspProcessFailed);
                }
            }
        });

        ctx.run_interval(Duration::from_secs(30), |_act, ctx| {
            ctx.ping(b"ping");
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
        self.child_stdin = msg.stdin;
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
        ctx.text(msg.data);
    }
}

struct LspProcessFailed;

impl Message for LspProcessFailed {
    type Result = ();
}

impl Handler<LspProcessFailed> for LspWsSession {
    type Result = ();

    fn handle(&mut self, _msg: LspProcessFailed, ctx: &mut Self::Context) {
        ctx.close(None);
        ctx.stop();
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for LspWsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                if let Some(stdin_arc) = &self.child_stdin {
                    let payload = text.into_bytes();
                    let stdin_clone = stdin_arc.clone();
                    actix::spawn(async move {
                        let mut guard = stdin_clone.lock().await;
                        let header = format!("Content-Length: {}\r\n\r\n", payload.len());
                        let _ = guard.write_all(header.as_bytes()).await;
                        let _ = guard.write_all(&payload).await;
                        let _ = guard.flush().await;
                    });
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