use crate::commands::settingview::{
    get_device_id, get_image_bed_root, get_knowledge_bases_root, get_resource_repo_sync_config,
};
use base64::Engine as _;
use blake3::Hasher;
use chrono::{DateTime, Duration, Utc};
use copia::{Sync, SyncBuilder};
use hex;
use iroh::SecretKey;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tracing::info;
use uuid::Uuid;
use walkdir::WalkDir;

const P2P_META_DIR: &str = ".guaxi-p2p";
const P2P_STATE_FILE: &str = "state.json";
const P2P_MANIFEST_FILE: &str = "manifest.json";
const P2P_TMP_DIR: &str = "tmp";
const P2P_BLOCK_SIZE: usize = 64 * 1024;
const RESOURCE_REPO_SYNC_PROGRESS_EVENT: &str = "resource_repo_sync_progress";

#[derive(Debug, Clone, Copy)]
enum P2PSyncTargetKind {
    ResourceRepo,
    KnowledgeBase,
}

#[derive(Debug, Clone)]
struct P2PSyncTarget {
    kind: P2PSyncTargetKind,
    key: &'static str,
    label: &'static str,
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceRepoSyncProgress {
    pub run_id: String,
    pub provider: String,
    pub target: String,
    pub phase: String,
    pub status: String,
    pub message: String,
    pub current: u64,
    pub total: u64,
    pub bytes_current: u64,
    pub bytes_total: u64,
    pub file_path: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct P2PTrustedPeer {
    pub peer_id: String,
    pub device_id: String,
    pub device_name: String,
    pub added_at: String,
    #[serde(default)]
    pub address_hint: String,
    #[serde(default)]
    pub online_status: String,
    #[serde(default)]
    pub status_detail: String,
    #[serde(default)]
    pub last_checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PPendingTicket {
    ticket_id: String,
    ticket: String,
    created_at: String,
    expires_at: Option<String>,
    used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct P2PStateFile {
    secret_key_hex: String,
    trusted_peers: Vec<P2PTrustedPeer>,
    pending_tickets: Vec<P2PPendingTicket>,
}

#[derive(Debug, Clone, Serialize)]
pub struct P2PStatus {
    pub enabled: bool,
    pub provider: String,
    pub device_id: String,
    pub device_name: String,
    pub peer_id: String,
    pub ticket_validity_mode: String,
    pub ticket_ttl_minutes: u64,
    pub auto_sync_resource_repo: bool,
    pub sync_interval_minutes: u64,
    pub trusted_peers: Vec<P2PTrustedPeer>,
    pub iroh_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct P2PTicketResponse {
    pub ticket: String,
    pub ticket_id: String,
    pub peer_id: String,
    pub device_id: String,
    pub device_name: String,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PTicketPayload {
    version: u32,
    ticket_id: String,
    peer_id: String,
    device_id: String,
    device_name: String,
    created_at: String,
    expires_at: Option<String>,
    #[serde(default)]
    address_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct P2PManifest {
    version: u32,
    target: String,
    device_id: String,
    updated_at: String,
    files: BTreeMap<String, P2PFileEntry>,
}

fn emit_sync_progress(app: Option<&AppHandle>, progress: ResourceRepoSyncProgress) {
    info!(
        run_id = progress.run_id,
        provider = progress.provider,
        target = progress.target,
        phase = progress.phase,
        status = progress.status,
        current = progress.current,
        total = progress.total,
        bytes_current = progress.bytes_current,
        bytes_total = progress.bytes_total,
        file_path = progress.file_path,
        message = progress.message,
        "[P2PSync][Progress]"
    );
    if let Some(app) = app {
        if let Err(e) = app.emit(RESOURCE_REPO_SYNC_PROGRESS_EVENT, progress) {
            info!("[P2PSync][Progress] emit failed: {}", e);
        }
    }
}

fn emit_p2p_progress(
    app: Option<&AppHandle>,
    run_id: &str,
    target: &P2PSyncTarget,
    phase: &str,
    status: &str,
    message: impl Into<String>,
    current: u64,
    total: u64,
    bytes_current: u64,
    bytes_total: u64,
    file_path: impl Into<String>,
) {
    emit_sync_progress(
        app,
        ResourceRepoSyncProgress {
            run_id: run_id.to_string(),
            provider: "p2p".to_string(),
            target: target.key.to_string(),
            phase: phase.to_string(),
            status: status.to_string(),
            message: message.into(),
            current,
            total,
            bytes_current,
            bytes_total,
            file_path: file_path.into(),
            at: Utc::now().to_rfc3339(),
        },
    );
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct P2PFileEntry {
    size: u64,
    modified_unix_ms: i64,
    blake3: String,
}

#[derive(Debug, Clone, Serialize, Default)]
struct P2PSyncStats {
    uploaded: u32,
    downloaded: u32,
    conflicts: u32,
    skipped: u32,
    verified: u32,
}

fn p2p_state_dir() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("获取程序路径失败: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "无法获取程序所在目录".to_string())?
        .join(P2P_META_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("创建 P2P 状态目录失败: {e}"))?;
    Ok(dir)
}

fn p2p_state_path() -> Result<PathBuf, String> {
    Ok(p2p_state_dir()?.join(P2P_STATE_FILE))
}

fn read_state_file() -> Result<P2PStateFile, String> {
    let path = p2p_state_path()?;
    if !path.exists() {
        return Ok(P2PStateFile::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取 P2P 状态失败: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析 P2P 状态失败: {e}"))
}

fn write_state_file(state: &P2PStateFile) -> Result<(), String> {
    let path = p2p_state_path()?;
    let raw = serde_json::to_string_pretty(state).map_err(|e| format!("序列化 P2P 状态失败: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("写入 P2P 状态失败: {e}"))?;
    info!(path = %path.to_string_lossy(), "[P2PSync] state file saved");
    Ok(())
}

fn ensure_secret_key(state: &mut P2PStateFile) -> Result<SecretKey, String> {
    if state.secret_key_hex.trim().is_empty() {
        let key = SecretKey::generate();
        state.secret_key_hex = hex::encode(key.to_bytes());
        write_state_file(state)?;
        info!("[P2PSync] generated local iroh secret key");
        return Ok(key);
    }

    let bytes = hex::decode(state.secret_key_hex.trim())
        .map_err(|e| format!("解析 P2P SecretKey 失败: {e}"))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "P2P SecretKey 长度不正确".to_string())?;
    Ok(SecretKey::from_bytes(&bytes))
}

fn p2p_device_name() -> String {
    let cfg = get_resource_repo_sync_config();
    let configured = cfg.p2p.device_name.trim();
    if !configured.is_empty() {
        return configured.to_string();
    }
    env::var("COMPUTERNAME")
        .or_else(|_| env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Guaxi Device".to_string())
}

fn p2p_config_values() -> (String, u64, bool, u64) {
    let cfg = get_resource_repo_sync_config();
    (
        cfg.p2p.ticket_validity_mode,
        cfg.p2p.ticket_ttl_minutes.max(1),
        cfg.p2p.auto_sync_resource_repo,
        cfg.p2p.sync_interval_minutes.max(30),
    )
}

fn encode_ticket(payload: &P2PTicketPayload) -> Result<String, String> {
    let raw = serde_json::to_vec(payload).map_err(|e| format!("序列化 P2P Ticket 失败: {e}"))?;
    Ok(format!(
        "guaxi-p2p:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw)
    ))
}

fn decode_ticket(ticket: &str) -> Result<P2PTicketPayload, String> {
    let body = ticket
        .trim()
        .strip_prefix("guaxi-p2p:")
        .ok_or_else(|| "P2P Ticket 格式不正确".to_string())?;
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|e| format!("解码 P2P Ticket 失败: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("解析 P2P Ticket 失败: {e}"))
}

fn target_for(kind: P2PSyncTargetKind) -> Result<P2PSyncTarget, String> {
    match kind {
        P2PSyncTargetKind::ResourceRepo => {
            let root = PathBuf::from(get_image_bed_root());
            fs::create_dir_all(&root).map_err(|e| format!("创建资源库目录失败: {e}"))?;
            Ok(P2PSyncTarget {
                kind,
                key: "resource_repo",
                label: "资源库",
                root,
            })
        }
        P2PSyncTargetKind::KnowledgeBase => {
            let root = PathBuf::from(get_knowledge_bases_root());
            fs::create_dir_all(&root).map_err(|e| format!("创建知识库目录失败: {e}"))?;
            Ok(P2PSyncTarget {
                kind,
                key: "knowledge_base",
                label: "知识库",
                root,
            })
        }
    }
}

fn manifest_path(target: &P2PSyncTarget) -> PathBuf {
    target.root.join(P2P_META_DIR).join(P2P_MANIFEST_FILE)
}

fn rel_path(root: &Path, path: &Path) -> Result<String, String> {
    let rel = path
        .strip_prefix(root)
        .map_err(|e| format!("计算相对路径失败: {e}"))?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn should_skip(target: &P2PSyncTarget, rel: &str) -> bool {
    let normalized = rel.replace('\\', "/");
    normalized == P2P_META_DIR
        || normalized.starts_with(&format!("{P2P_META_DIR}/"))
        || matches!(target.kind, P2PSyncTargetKind::KnowledgeBase)
            && (normalized == ".git" || normalized.starts_with(".git/"))
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| format!("打开文件失败 {}: {e}", path.display()))?;
    let mut hasher = Hasher::new();
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("读取文件失败 {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn file_mtime_ms(path: &Path) -> Result<i64, String> {
    let modified = fs::metadata(path)
        .map_err(|e| format!("读取文件元数据失败 {}: {e}", path.display()))?
        .modified()
        .map_err(|e| format!("读取文件修改时间失败 {}: {e}", path.display()))?;
    let duration = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("文件修改时间早于 UNIX_EPOCH {}: {e}", path.display()))?;
    Ok(duration.as_millis() as i64)
}

fn scan_manifest(target: &P2PSyncTarget) -> Result<P2PManifest, String> {
    scan_manifest_with_progress(target, None, "", false)
}

fn scan_manifest_with_progress(
    target: &P2PSyncTarget,
    app: Option<&AppHandle>,
    run_id: &str,
    emit_progress: bool,
) -> Result<P2PManifest, String> {
    let device_id = get_device_id();
    let mut total_files = 0_u64;
    let mut total_bytes = 0_u64;

    for entry in WalkDir::new(&target.root).follow_links(false) {
        let entry = entry.map_err(|e| format!("统计目录失败: {e}"))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = rel_path(&target.root, path)?;
        if should_skip(target, &rel) {
            continue;
        }
        let meta = fs::metadata(path).map_err(|e| format!("读取文件元数据失败 {}: {e}", path.display()))?;
        total_files += 1;
        total_bytes = total_bytes.saturating_add(meta.len());
    }

    if emit_progress {
        emit_p2p_progress(
            app,
            run_id,
            target,
            "scan",
            "running",
            format!("开始扫描 {}：{} 个文件，{} 字节", target.label, total_files, total_bytes),
            0,
            total_files,
            0,
            total_bytes,
            "",
        );
    }

    let mut files = BTreeMap::new();
    let mut scanned_files = 0_u64;
    let mut scanned_bytes = 0_u64;
    for entry in WalkDir::new(&target.root).follow_links(false) {
        let entry = entry.map_err(|e| format!("扫描目录失败: {e}"))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = rel_path(&target.root, path)?;
        if should_skip(target, &rel) {
            continue;
        }
        let meta = fs::metadata(path).map_err(|e| format!("读取文件元数据失败 {}: {e}", path.display()))?;
        let hash = hash_file(path)?;
        scanned_files += 1;
        scanned_bytes = scanned_bytes.saturating_add(meta.len());
        files.insert(
            rel.clone(),
            P2PFileEntry {
                size: meta.len(),
                modified_unix_ms: file_mtime_ms(path)?,
                blake3: hash,
            },
        );

        if emit_progress && (scanned_files == total_files || scanned_files == 1 || scanned_files % 25 == 0) {
            emit_p2p_progress(
                app,
                run_id,
                target,
                "scan",
                "running",
                format!("正在扫描 {}：{}/{} 个文件", target.label, scanned_files, total_files),
                scanned_files,
                total_files,
                scanned_bytes,
                total_bytes,
                rel,
            );
        }
    }
    info!(
        target = target.key,
        root = %target.root.to_string_lossy(),
        count = files.len(),
        bytes_total = total_bytes,
        "[P2PSync] local manifest scanned"
    );
    if emit_progress {
        emit_p2p_progress(
            app,
            run_id,
            target,
            "scan",
            "success",
            format!("扫描完成：{} 个文件", files.len()),
            scanned_files,
            total_files,
            scanned_bytes,
            total_bytes,
            "",
        );
    }
    Ok(P2PManifest {
        version: 1,
        target: target.key.to_string(),
        device_id,
        updated_at: Utc::now().to_rfc3339(),
        files,
    })
}

fn read_manifest(target: &P2PSyncTarget) -> Result<Option<P2PManifest>, String> {
    let path = manifest_path(target);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取 P2P manifest 失败: {e}"))?;
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| format!("解析 P2P manifest 失败: {e}"))
}

fn write_manifest(target: &P2PSyncTarget, manifest: &P2PManifest) -> Result<(), String> {
    let path = manifest_path(target);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 P2P manifest 目录失败: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(manifest).map_err(|e| format!("序列化 P2P manifest 失败: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("写入 P2P manifest 失败: {e}"))?;
    info!(target = target.key, path = %path.to_string_lossy(), "[P2PSync] manifest saved");
    Ok(())
}

fn conflict_path(original: &Path, device_name: &str) -> PathBuf {
    let ts = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let stem = original.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = original.extension().and_then(|s| s.to_str()).unwrap_or("");
    let name = if ext.is_empty() {
        format!("{stem}.conflict.{device_name}.{ts}")
    } else {
        format!("{stem}.conflict.{device_name}.{ts}.{ext}")
    };
    original.with_file_name(name)
}

/// 生产同步的写入入口：所有来自对端的文件都先进入临时文件，使用 copia 的块级差异
/// 生成目标内容，再做 BLAKE3 校验，最后 rename 原子替换，避免中断时污染原文件。
///
/// 当前函数先封装本地块级 patch 流程；接入 iroh stream 后，只需要把 `remote_path`
/// 替换成从对端流中读取 signature/delta 的数据源，落盘安全策略不变。
fn apply_block_delta_with_copia(
    target: &P2PSyncTarget,
    rel: &str,
    remote_path: &Path,
    expected_hash: &str,
) -> Result<(), String> {
    let dest = target.root.join(PathBuf::from(rel));
    let tmp_dir = target.root.join(P2P_META_DIR).join(P2P_TMP_DIR);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建 P2P 临时目录失败: {e}"))?;
    let tmp = tmp_dir.join(format!("{}.tmp", Uuid::new_v4()));

    let sync = SyncBuilder::new().block_size(P2P_BLOCK_SIZE).build();
    let basis = if dest.exists() {
        fs::read(&dest).map_err(|e| format!("读取 P2P basis 文件失败 {}: {e}", dest.display()))?
    } else {
        Vec::new()
    };
    let source =
        fs::read(remote_path).map_err(|e| format!("读取 P2P 对端文件失败 {}: {e}", remote_path.display()))?;

    let signature = sync
        .signature(Cursor::new(basis.as_slice()))
        .map_err(|e| format!("生成 copia 块签名失败 {rel}: {e}"))?;
    let delta = sync
        .delta(Cursor::new(source.as_slice()), &signature)
        .map_err(|e| format!("生成 copia delta 失败 {rel}: {e}"))?;

    let mut patched = Vec::with_capacity(source.len());
    sync.patch(Cursor::new(basis.as_slice()), &delta, &mut patched)
        .map_err(|e| format!("应用 copia patch 失败 {rel}: {e}"))?;

    let mut out = fs::File::create(&tmp)
        .map_err(|e| format!("创建 P2P 临时文件失败 {}: {e}", tmp.display()))?;
    out.write_all(&patched)
        .map_err(|e| format!("写入 P2P 临时文件失败 {}: {e}", tmp.display()))?;
    out.flush()
        .map_err(|e| format!("刷新 P2P 临时文件失败 {}: {e}", tmp.display()))?;

    let actual = hash_file(&tmp)?;
    if actual != expected_hash {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "P2P 块级同步校验失败：{} 期望 {} 实际 {}",
            rel, expected_hash, actual
        ));
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目标目录失败 {}: {e}", parent.display()))?;
    }
    fs::rename(&tmp, &dest).map_err(|e| format!("原子替换目标文件失败 {}: {e}", dest.display()))?;
    info!(
        target = target.key,
        rel,
        basis_bytes = basis.len(),
        source_bytes = source.len(),
        "[P2PSync] file applied with copia block delta"
    );
    Ok(())
}

fn run_copia_block_delta_self_check() -> Result<(), String> {
    let root = p2p_state_dir()?.join("self-check");
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(&root).map_err(|e| format!("创建 P2P 块级差异自检目录失败: {e}"))?;

    let target = P2PSyncTarget {
        kind: P2PSyncTargetKind::ResourceRepo,
        key: "p2p_self_check",
        label: "P2P 自检",
        root: root.clone(),
    };
    let basis = root.join("basis.txt");
    let source = root.join("source.txt");
    fs::write(&basis, b"guaxi p2p old block\nsame tail")
        .map_err(|e| format!("写入 P2P basis 自检文件失败: {e}"))?;
    fs::write(&source, b"guaxi p2p new block\nsame tail")
        .map_err(|e| format!("写入 P2P source 自检文件失败: {e}"))?;

    let expected_hash = hash_file(&source)?;
    apply_block_delta_with_copia(&target, "basis.txt", &source, &expected_hash)?;
    let final_hash = hash_file(&basis)?;
    if final_hash != expected_hash {
        return Err("P2P copia 块级差异自检结果不一致".to_string());
    }

    let _ = fs::remove_dir_all(&root);
    info!("[P2PSync] copia block-delta self check ok");
    Ok(())
}

fn verify_target(target: P2PSyncTarget) -> Result<String, String> {
    let manifest = scan_manifest(&target)?;
    let old = read_manifest(&target)?;
    let mut stats = P2PSyncStats::default();
    match old {
        Some(old) => {
            let keys: BTreeSet<String> = manifest
                .files
                .keys()
                .chain(old.files.keys())
                .cloned()
                .collect();
            for key in keys {
                match (manifest.files.get(&key), old.files.get(&key)) {
                    (Some(a), Some(b)) if a.blake3 == b.blake3 => stats.verified += 1,
                    _ => stats.conflicts += 1,
                }
            }
        }
        None => {
            stats.verified = manifest.files.len() as u32;
        }
    }
    write_manifest(&target, &manifest)?;
    Ok(format!(
        "{} P2P 校验完成：一致 {}，差异 {}",
        target.label, stats.verified, stats.conflicts
    ))
}

fn sync_target_local_plan(
    app: Option<&AppHandle>,
    run_id: &str,
    target: P2PSyncTarget,
) -> Result<String, String> {
    emit_p2p_progress(
        app,
        run_id,
        &target,
        "connect",
        "running",
        "确认本机 P2P 身份与块级差异链路",
        0,
        0,
        0,
        0,
        "",
    );
    run_copia_block_delta_self_check()?;

    let state = read_state_file()?;
    let trusted_peers = state.trusted_peers.len() as u64;
    let online_peers = state
        .trusted_peers
        .iter()
        .filter(|p| p.online_status == "online")
        .count() as u64;
    emit_p2p_progress(
        app,
        run_id,
        &target,
        "connect",
        if online_peers > 0 { "success" } else { "waiting" },
        format!("可信设备 {} 个，当前在线 {} 个", trusted_peers, online_peers),
        online_peers,
        trusted_peers,
        0,
        0,
        "",
    );

    let manifest = scan_manifest_with_progress(&target, app, run_id, true)?;
    let base = read_manifest(&target)?;
    let mut stats = P2PSyncStats::default();
    let mut planned_bytes = 0_u64;
    let mut planned_files = 0_u64;

    emit_p2p_progress(
        app,
        run_id,
        &target,
        "plan",
        "running",
        "正在计算同步计划",
        0,
        manifest.files.len() as u64,
        0,
        0,
        "",
    );

    if let Some(base) = base {
        let keys: BTreeSet<String> = manifest
            .files
            .keys()
            .chain(base.files.keys())
            .cloned()
            .collect();
        for key in keys {
            match (manifest.files.get(&key), base.files.get(&key)) {
                (Some(a), Some(b)) if a.blake3 == b.blake3 => stats.skipped += 1,
                (Some(a), Some(_)) => {
                    stats.uploaded += 1;
                    planned_files += 1;
                    planned_bytes = planned_bytes.saturating_add(a.size);
                }
                (Some(a), None) => {
                    stats.uploaded += 1;
                    planned_files += 1;
                    planned_bytes = planned_bytes.saturating_add(a.size);
                }
                (None, Some(_)) => {
                    stats.conflicts += 1;
                    let candidate = conflict_path(&target.root.join(PathBuf::from(&key)), &p2p_device_name());
                    info!(
                        target = target.key,
                        rel = key,
                        conflict_candidate = %candidate.to_string_lossy(),
                        "[P2PSync] local deletion is kept as conflict candidate; delete propagation requires confirmation"
                    );
                }
                (None, None) => stats.skipped += 1,
            }
        }
    } else {
        stats.uploaded = manifest.files.len() as u32;
        planned_files = manifest.files.len() as u64;
        planned_bytes = manifest.files.values().map(|f| f.size).sum();
    }

    emit_p2p_progress(
        app,
        run_id,
        &target,
        "plan",
        "success",
        format!(
            "同步计划完成：待发送 {} 个文件，{} 字节；冲突/删除待确认 {} 个",
            planned_files, planned_bytes, stats.conflicts
        ),
        planned_files,
        manifest.files.len() as u64,
        planned_bytes,
        planned_bytes,
        "",
    );

    emit_p2p_progress(
        app,
        run_id,
        &target,
        "transfer",
        if online_peers > 0 { "running" } else { "waiting" },
        if online_peers > 0 {
            "已生成 P2P 传输计划，等待 iroh 传输会话消费计划"
        } else {
            "没有可确认在线的可信设备，资源库内容尚未开始跨设备传输"
        },
        0,
        planned_files,
        0,
        planned_bytes,
        "",
    );

    write_manifest(&target, &manifest)?;
    emit_p2p_progress(
        app,
        run_id,
        &target,
        "complete",
        if online_peers > 0 { "success" } else { "waiting" },
        if online_peers > 0 {
            "本地同步计划已保存；传输层会继续按计划发送"
        } else {
            "本地同步计划已保存；请先让可信设备在线后再开始传输"
        },
        planned_files,
        planned_files,
        planned_bytes,
        planned_bytes,
        "",
    );
    Ok(format!(
        "{} P2P 同步计划完成：待发送 {} 个文件，{} 字节；删除/冲突需确认 {}，跳过 {}。{}",
        target.label,
        stats.uploaded,
        planned_bytes,
        stats.conflicts,
        stats.skipped,
        if online_peers > 0 {
            "检测到在线设备，等待传输会话执行。"
        } else {
            "当前没有可确认在线设备，尚未跨设备传输。"
        }
    ))
}

#[tauri::command]
pub async fn p2p_sync_status() -> Result<P2PStatus, String> {
    let cfg = get_resource_repo_sync_config();
    let mut state = read_state_file()?;
    let key = ensure_secret_key(&mut state)?;
    let peer_id = key.public().to_string();
    let (ticket_validity_mode, ticket_ttl_minutes, auto_sync_resource_repo, sync_interval_minutes) =
        p2p_config_values();
    Ok(P2PStatus {
        enabled: cfg.provider.trim().eq_ignore_ascii_case("p2p"),
        provider: cfg.provider,
        device_id: get_device_id(),
        device_name: p2p_device_name(),
        peer_id,
        ticket_validity_mode,
        ticket_ttl_minutes,
        auto_sync_resource_repo,
        sync_interval_minutes,
        trusted_peers: state.trusted_peers,
        iroh_version: "0.98.2".to_string(),
    })
}

#[tauri::command]
pub async fn p2p_sync_test_connection() -> Result<String, String> {
    let mut state = read_state_file()?;
    let key = ensure_secret_key(&mut state)?;
    run_copia_block_delta_self_check()?;
    let peer_id = key.public().to_string();
    let peers = state.trusted_peers.len();
    info!(peer_id, peers, "[P2PSync] iroh identity ready");
    Ok(format!("P2P 身份可用：Peer ID {}，可信设备 {} 个", peer_id, peers))
}

#[tauri::command]
pub async fn p2p_sync_create_ticket() -> Result<P2PTicketResponse, String> {
    let mut state = read_state_file()?;
    let key = ensure_secret_key(&mut state)?;
    let peer_id = key.public().to_string();
    let (mode, ttl, _, _) = p2p_config_values();
    let created_at = Utc::now();
    let expires_at = if mode.trim().eq_ignore_ascii_case("permanent") {
        None
    } else {
        Some((created_at + Duration::minutes(ttl as i64)).to_rfc3339())
    };

    let payload = P2PTicketPayload {
        version: 1,
        ticket_id: Uuid::new_v4().to_string(),
        peer_id: peer_id.clone(),
        device_id: get_device_id(),
        device_name: p2p_device_name(),
        created_at: created_at.to_rfc3339(),
        expires_at: expires_at.clone(),
        address_hint: String::new(),
    };
    let ticket = encode_ticket(&payload)?;
    state.pending_tickets.push(P2PPendingTicket {
        ticket_id: payload.ticket_id.clone(),
        ticket: ticket.clone(),
        created_at: payload.created_at.clone(),
        expires_at: expires_at.clone(),
        used: false,
    });
    write_state_file(&state)?;
    info!(
        peer_id,
        ticket_id = payload.ticket_id,
        expires_at = expires_at.clone().unwrap_or_else(|| "permanent".to_string()),
        "[P2PSync] created pairing ticket"
    );
    Ok(P2PTicketResponse {
        ticket,
        ticket_id: payload.ticket_id,
        peer_id,
        device_id: payload.device_id,
        device_name: payload.device_name,
        created_at: payload.created_at,
        expires_at,
    })
}

#[tauri::command]
pub async fn p2p_sync_join_ticket(ticket: String) -> Result<String, String> {
    let payload = decode_ticket(&ticket)?;
    if let Some(exp) = &payload.expires_at {
        let exp = DateTime::parse_from_rfc3339(exp)
            .map_err(|e| format!("解析 Ticket 过期时间失败: {e}"))?
            .with_timezone(&Utc);
        if Utc::now() > exp {
            return Err("P2P Ticket 已过期".to_string());
        }
    }

    let mut state = read_state_file()?;
    if !state.trusted_peers.iter().any(|p| p.peer_id == payload.peer_id) {
        state.trusted_peers.push(P2PTrustedPeer {
            peer_id: payload.peer_id.clone(),
            device_id: payload.device_id.clone(),
            device_name: payload.device_name.clone(),
            added_at: Utc::now().to_rfc3339(),
            address_hint: payload.address_hint.clone(),
            online_status: "unknown".to_string(),
            status_detail: if payload.address_hint.trim().is_empty() {
                "缺少 iroh 可拨号地址；请在对端启动 P2P 服务后重新生成新版 Ticket".to_string()
            } else {
                "尚未检测".to_string()
            },
            last_checked_at: String::new(),
        });
        write_state_file(&state)?;
        info!(
            peer_id = payload.peer_id,
            device_id = payload.device_id,
            device_name = payload.device_name,
            "[P2PSync] trusted peer added from ticket"
        );
    }
    Ok(format!("已信任设备：{}", payload.device_name))
}

#[tauri::command]
pub async fn p2p_sync_list_peers() -> Result<Vec<P2PTrustedPeer>, String> {
    Ok(read_state_file()?.trusted_peers)
}

#[tauri::command]
pub async fn p2p_sync_probe_peers() -> Result<Vec<P2PTrustedPeer>, String> {
    let mut state = read_state_file()?;
    let mut changed = false;
    let checked_at = Utc::now().to_rfc3339();

    // 先确认本机 P2P 基础能力正常，否则远端检测没有意义。
    run_copia_block_delta_self_check()?;

    for peer in &mut state.trusted_peers {
        peer.last_checked_at = checked_at.clone();
        if peer.address_hint.trim().is_empty() {
            peer.online_status = "unknown".to_string();
            peer.status_detail =
                "缺少 iroh 可拨号地址；请使用新版 Ticket 重新配对后再检测在线状态".to_string();
            changed = true;
            info!(
                peer_id = peer.peer_id,
                device_name = peer.device_name,
                "[P2PSync] peer probe skipped because address hint is empty"
            );
            continue;
        }

        // 这里保留真实 iroh 拨号检测的入口。当前版本的 Ticket 尚未携带完整 NodeAddr，
        // 所以不能把“有 Peer ID”伪装成“在线”。后续接入长驻 Endpoint 后在此处 connect。
        peer.online_status = "unknown".to_string();
        peer.status_detail = "已保存地址提示，但当前版本尚未启用长驻 iroh 拨号检测".to_string();
        changed = true;
    }

    if changed {
        write_state_file(&state)?;
    }
    Ok(state.trusted_peers)
}

#[tauri::command]
pub async fn p2p_sync_remove_peer(peer_id: String) -> Result<String, String> {
    let mut state = read_state_file()?;
    let before = state.trusted_peers.len();
    state.trusted_peers.retain(|p| p.peer_id != peer_id);
    write_state_file(&state)?;
    Ok(format!("已移除 {} 个设备", before.saturating_sub(state.trusted_peers.len())))
}

#[tauri::command]
pub async fn p2p_sync_resource_repo_now(app: AppHandle) -> Result<String, String> {
    let run_id = Uuid::new_v4().to_string();
    let target = target_for(P2PSyncTargetKind::ResourceRepo)?;
    info!(run_id, target = target.key, "[P2PSync] resource repo sync requested");
    sync_target_local_plan(Some(&app), &run_id, target)
}

#[tauri::command]
pub async fn p2p_sync_verify_resource_repo() -> Result<String, String> {
    verify_target(target_for(P2PSyncTargetKind::ResourceRepo)?)
}

#[tauri::command]
pub async fn p2p_sync_knowledge_base_now(app: AppHandle) -> Result<String, String> {
    let run_id = Uuid::new_v4().to_string();
    let target = target_for(P2PSyncTargetKind::KnowledgeBase)?;
    info!(run_id, target = target.key, "[P2PSync] knowledge base emergency sync requested");
    sync_target_local_plan(Some(&app), &run_id, target)
}

#[tauri::command]
pub async fn p2p_sync_verify_knowledge_base() -> Result<String, String> {
    verify_target(target_for(P2PSyncTargetKind::KnowledgeBase)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_roundtrip_preserves_expiry_policy() {
        let payload = P2PTicketPayload {
            version: 1,
            ticket_id: "ticket".to_string(),
            peer_id: "peer".to_string(),
            device_id: "device".to_string(),
            device_name: "Laptop".to_string(),
            created_at: "2026-06-05T00:00:00Z".to_string(),
            expires_at: None,
            address_hint: String::new(),
        };
        let encoded = encode_ticket(&payload).expect("encode ticket");
        let decoded = decode_ticket(&encoded).expect("decode ticket");
        assert_eq!(decoded.expires_at, None);
        assert_eq!(decoded.device_name, "Laptop");
    }

    #[test]
    fn skip_knowledge_git_and_p2p_meta() {
        let target = P2PSyncTarget {
            kind: P2PSyncTargetKind::KnowledgeBase,
            key: "knowledge_base",
            label: "知识库",
            root: PathBuf::from("x"),
        };
        assert!(should_skip(&target, ".git/config"));
        assert!(should_skip(&target, ".guaxi-p2p/manifest.json"));
        assert!(!should_skip(&target, "notes/a.md"));
    }

    #[test]
    fn copia_block_delta_apply_replaces_file_atomically() {
        let root = env::temp_dir().join(format!("guaxi-p2p-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let target = P2PSyncTarget {
            kind: P2PSyncTargetKind::ResourceRepo,
            key: "resource_repo",
            label: "资源库",
            root: root.clone(),
        };
        let local = root.join("a.txt");
        let remote = root.join("remote-a.txt");
        fs::write(&local, b"hello old block\nsame tail").expect("write local");
        fs::write(&remote, b"hello new block\nsame tail").expect("write remote");
        let expected = hash_file(&remote).expect("hash remote");

        apply_block_delta_with_copia(&target, "a.txt", &remote, &expected).expect("apply block delta");

        let content = fs::read(&local).expect("read local");
        assert_eq!(content, b"hello new block\nsame tail");
        let _ = fs::remove_dir_all(root);
    }
}
