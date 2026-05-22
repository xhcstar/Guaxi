use git2::{
    build::CheckoutBuilder, BranchType, Delta, DiffFormat, DiffOptions, IndexEntry, Oid, Repository,
    RepositoryOpenFlags, StatusOptions, StatusShow, Tree,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tracing::{debug, error, info, warn};
use urlencoding::encode;

const HUNK_LINE_MATCH_TOLERANCE: u32 = 1;

fn is_ignored_git_path(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p == ".meta" || p.starts_with(".meta/") || p.contains("/.meta/") || is_cellmeta_git_path(&p)
}

fn is_cellmeta_git_path(path: &str) -> bool {
    path.replace('\\', "/").to_ascii_lowercase().ends_with(".cellmeta")
}

fn git_output_message(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        format!("{}\n{}", stdout.trim_end(), stderr.trim_start())
    }
}

fn flush_pending_deletes(pending_deletes: &mut u32, deleted: &mut BTreeSet<u32>, anchor: u32) {
    if *pending_deletes > 0 {
        deleted.insert(anchor.max(1));
        *pending_deletes = 0;
    }
}

fn collect_line_changes_from_diff(diff: &git2::Diff) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut added = BTreeSet::<u32>::new();
    let mut modified = BTreeSet::<u32>::new();
    let mut deleted = BTreeSet::<u32>::new();

    let mut pending_deletes: u32 = 0;
    let mut current_new_line: u32 = 1;
    let mut current_hunk_new_start: u32 = 1;

    let _ = diff.foreach(
        &mut |_delta, _| true,
        None,
        None,
        Some(&mut |_delta, hunk, line| {
            if let Some(hunk) = hunk {
                let hunk_new_start = hunk.new_start() as u32;
                if hunk_new_start != current_hunk_new_start {
                    flush_pending_deletes(
                        &mut pending_deletes,
                        &mut deleted,
                        current_new_line.max(1),
                    );
                    current_hunk_new_start = hunk_new_start;
                    current_new_line = current_hunk_new_start.max(1);
                }
            }

            match line.origin() {
                '+' => {
                    let ln = line.new_lineno().unwrap_or(current_new_line);
                    if pending_deletes > 0 {
                        modified.insert(ln);
                        pending_deletes = pending_deletes.saturating_sub(1);
                    } else {
                        added.insert(ln);
                    }
                    current_new_line = ln.saturating_add(1);
                }
                '-' => {
                    pending_deletes = pending_deletes.saturating_add(1);
                }
                ' ' => {
                    flush_pending_deletes(&mut pending_deletes, &mut deleted, current_new_line.max(1));
                    let ln = line.new_lineno().unwrap_or(current_new_line);
                    current_new_line = ln.saturating_add(1);
                }
                _ => {}
            }
            true
        }),
    );

    flush_pending_deletes(&mut pending_deletes, &mut deleted, current_new_line.max(1));

    (
        added.into_iter().collect(),
        modified.into_iter().collect(),
        deleted.into_iter().collect(),
    )
}

#[cfg(any())]
fn normalize_remote_url(raw: &str) -> String {
    let mut s = raw
        .trim()
        .trim_matches(|c: char| matches!(c, '`' | '"' | '\''))
        .to_string();
    if s.is_empty() {
        return s;
    }

    if let Some((left, _)) = s.split_once('#') {
        s = left.to_string();
    }
    if let Some((left, _)) = s.split_once('?') {
        s = left.to_string();
    }

    while s.ends_with('/') {
        s.pop();
    }

    if (s.starts_with("http://") || s.starts_with("https://")) && !s.ends_with(".git") {
        s.push_str(".git");
    }

    s
}

#[cfg(any())]
fn default_http_username(url: &str, username_from_url: Option<&str>) -> String {
    if let Some(u) = username_from_url {
        if !u.trim().is_empty() {
            return u.to_string();
        }
    }

    let u = url.to_ascii_lowercase();
    if u.contains("github.com") {
        return "x-access-token".to_string();
    }
    if u.contains("gitlab") {
        return "oauth2".to_string();
    }

    "git".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitCommit {
    id: String,
    message: String,
    author: String,
    date: String,
    summary: String,
    files_changed: usize,
    insertions: usize,
    deletions: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitChange {
    path: String,
    status: String,
    lines_added: Vec<u32>,
    lines_modified: Vec<u32>,
    lines_deleted: Vec<u32>,
    diff: String,
    staged: bool, // 新增字段，表示是否已暂存
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitDiffHunk {
    index: u32,
    header: String,
    old_start: u32,
    old_count: u32,
    new_start: u32,
    new_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitBranch {
    name: String,
    is_current: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHeadMarkers {
    local_head: Option<String>,
    remote_head: Option<String>,
    remote_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitFile {
    path: String,
    status: String, // "A", "M", "D", etc.
    additions: usize,
    deletions: usize,
}

#[allow(dead_code)]
#[tauri::command]
pub fn is_git_repository(path: String) -> bool {
    match Repository::discover(&path) {
        Ok(_) => true,
        Err(e) => {
            warn!("Path is not a git repository: {}", e);
            false
        }
    }
}

#[tauri::command]
pub fn get_git_root(path: String) -> Result<String, String> {
    // 使用 NO_SEARCH 标志禁止向上查找父目录
    let repo = Repository::open_ext(
        &path,
        RepositoryOpenFlags::NO_SEARCH, // 关键标志，禁止向上查找
        Vec::<&Path>::new(),            // 不需要额外的 git 目录
    )
    .map_err(|e| format!("Failed to open repository: {}", e))?;

    repo.workdir()
        .ok_or("Bare repository has no working directory".to_string())?
        .to_str()
        .ok_or("Path contains invalid UTF-8".to_string())
        .map(|s| s.to_string())
}

#[tauri::command]
pub async fn get_git_status(path: String) -> Result<Vec<GitChange>, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let repo =
            Repository::discover(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

        let repo_root: PathBuf = repo
            .workdir()
            .unwrap_or_else(|| Path::new(&path))
            .to_path_buf();

        let head_tree = match repo.head().ok().and_then(|h| h.peel_to_tree().ok()) {
            Some(t) => t,
            None => {
                let builder = repo
                    .treebuilder(None)
                    .map_err(|e| format!("Failed to create tree builder: {}", e))?;
                let oid = builder
                    .write()
                    .map_err(|e| format!("Failed to write empty tree: {}", e))?;
                repo.find_tree(oid)
                    .map_err(|e| format!("Failed to find empty tree: {}", e))?
            }
        };

        let mut index = repo
            .index()
            .map_err(|e| format!("Failed to get repository index: {}", e))?;
        let index_tree_oid = index
            .write_tree()
            .map_err(|e| format!("Failed to write index tree: {}", e))?;
        let index_tree = repo
            .find_tree(index_tree_oid)
            .map_err(|e| format!("Failed to find index tree: {}", e))?;

        let mut options = StatusOptions::new();
        options.include_untracked(true);
        options.recurse_untracked_dirs(true);
        options.show(StatusShow::IndexAndWorkdir);

        let statuses = repo
            .statuses(Some(&mut options))
            .map_err(|e| format!("Failed to get statuses: {}", e))?;

        let mut changes = Vec::new();

        for entry in statuses.iter() {
            let rel_path = match entry.path() {
                Some(p) => p.to_string(),
                None => continue,
            };

            if is_ignored_git_path(&rel_path) {
                continue;
            }

            let mut append_change = |status: &str, staged: bool| -> Result<(), String> {
                let mut lines_added: Vec<u32> = Vec::new();
                let mut lines_modified: Vec<u32> = Vec::new();
                let mut lines_deleted: Vec<u32> = Vec::new();

                if status == "?" {
                    let abs = repo_root.join(Path::new(&rel_path));
                    if let Ok(content) = std::fs::read_to_string(&abs) {
                        let line_count = content.lines().count() as u32;
                        if line_count > 0 {
                            lines_added = (1..=line_count).collect();
                        }
                    }
                } else if status == "A" || status == "M" || status == "D" {
                    let mut diff_options = DiffOptions::new();
                    diff_options.context_lines(0);
                    diff_options.pathspec(rel_path.clone());

                    let diff = if staged {
                        repo.diff_tree_to_tree(
                            Some(&head_tree),
                            Some(&index_tree),
                            Some(&mut diff_options),
                        )
                        .map_err(|e| format!("Failed to create staged diff: {}", e))?
                    } else {
                        repo.diff_index_to_workdir(Some(&index), Some(&mut diff_options))
                            .map_err(|e| format!("Failed to create workdir diff: {}", e))?
                    };

                    let (a, m, d) = collect_line_changes_from_diff(&diff);

                    if status == "M" {
                        let mut all = BTreeSet::<u32>::new();
                        all.extend(a);
                        all.extend(m);
                        all.extend(d);
                        lines_modified = all.into_iter().collect();

                        if lines_modified.is_empty() {
                            lines_modified.push(1);
                        }
                    } else if status == "A" {
                        lines_added = a;
                        if lines_added.is_empty() {
                            let abs = repo_root.join(Path::new(&rel_path));
                            if let Ok(content) = std::fs::read_to_string(&abs) {
                                let line_count = content.lines().count() as u32;
                                if line_count > 0 {
                                    lines_added = (1..=line_count).collect();
                                }
                            }
                        }
                        if lines_added.is_empty() {
                            lines_added.push(1);
                        }
                    } else if status == "D" {
                        lines_deleted = d;
                        if lines_deleted.is_empty() {
                            lines_deleted.push(1);
                        }
                    }
                }

                changes.push(GitChange {
                    path: rel_path.clone(),
                    status: status.to_string(),
                    lines_added,
                    lines_modified,
                    lines_deleted,
                    diff: String::new(),
                    staged,
                });
                Ok(())
            };

            // 设计说明：同一路径可能同时存在“已暂存”和“未暂存”两份差异。
            // VS Code 会在两个分组各显示一个条目；这里不能用 else-if 折叠掉工作区状态。
            if entry.status().is_index_new() {
                append_change("A", true)?;
            } else if entry.status().is_index_modified() {
                append_change("M", true)?;
            } else if entry.status().is_index_deleted() {
                append_change("D", true)?;
            }

            if entry.status().is_wt_new() {
                append_change("?", false)?;
            } else if entry.status().is_wt_modified() {
                append_change("M", false)?;
            } else if entry.status().is_wt_deleted() {
                append_change("D", false)?;
            } else if entry.status().is_wt_renamed() {
                append_change("R", false)?;
            } else if entry.status().is_wt_typechange() {
                append_change("T", false)?;
            }
        }

        Ok(changes)
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

#[tauri::command]
pub fn git_read_file(path: String, file_path: String) -> Result<String, String> {
    let full_path = Path::new(&path).join(file_path);
    std::fs::read_to_string(&full_path).map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
pub fn get_current_branch(path: String) -> Result<String, String> {
    info!("Getting current branch for path: {}", path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let head = repo.head().map_err(|e| {
        error!("Failed to get HEAD: {}", e);
        e.to_string()
    })?;

    head.shorthand().map(|s| s.to_string()).ok_or_else(|| {
        error!("No branch found");
        "No branch found".to_string()
    })
}

#[tauri::command]
pub fn get_git_head_markers(path: String) -> Result<GitHeadMarkers, String> {
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let head = repo.head().map_err(|e| {
        error!("Failed to get HEAD: {}", e);
        e.to_string()
    })?;

    let local_head = head.target().map(|oid| oid.to_string());

    let mut remote_head: Option<String> = None;
    let mut remote_ref: Option<String> = None;

    if let Some(branch_name) = head.shorthand() {
        let origin_refname = format!("refs/remotes/origin/{}", branch_name);
        if let Ok(origin_ref) = repo.find_reference(origin_refname.as_str()) {
            remote_ref = Some(format!("origin/{}", branch_name));
            remote_head = origin_ref.target().map(|oid| oid.to_string());
        } else if let Ok(local_branch) = repo.find_branch(branch_name, BranchType::Local) {
            if let Ok(upstream_branch) = local_branch.upstream() {
                if let Some(name) = upstream_branch.get().name() {
                    remote_ref = Some(name.trim_start_matches("refs/remotes/").to_string());
                }
                let upstream_ref = upstream_branch.into_reference();
                remote_head = upstream_ref.target().map(|oid| oid.to_string());
            }
        }
    }

    Ok(GitHeadMarkers {
        local_head,
        remote_head,
        remote_ref,
    })
}

#[tauri::command]
pub fn list_branches(path: String) -> Result<Vec<GitBranch>, String> {
    info!("Listing branches for path: {}", path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let branches = repo.branches(Some(BranchType::Local)).map_err(|e| {
        error!("Failed to list branches: {}", e);
        e.to_string()
    })?;

    let current_branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()));

    debug!("Current branch: {:?}", current_branch);

    let mut result = Vec::new();
    for branch in branches {
        let (branch, _) = branch.map_err(|e| {
            error!("Failed to process branch: {}", e);
            e.to_string()
        })?;
        let name = branch
            .name()
            .map_err(|e| {
                error!("Failed to get branch name: {}", e);
                e.to_string()
            })?
            .unwrap_or_default()
            .to_string();
        let name_clone = name.clone();
        result.push(GitBranch {
            name,
            is_current: current_branch
                .as_ref()
                .map_or(false, |cb| cb == &name_clone),
        });
    }

    info!("Found {} branches", result.len());
    Ok(result)
}

#[tauri::command]
pub fn git_commit(path: String, message: String) -> Result<(), String> {
    info!("Committing changes to path: {}", path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let mut index = repo.index().map_err(|e| {
        error!("Failed to get repository index: {}", e);
        e.to_string()
    })?;

    let tree_id = index.write_tree().map_err(|e| {
        error!("Failed to write tree: {}", e);
        e.to_string()
    })?;

    let tree = repo.find_tree(tree_id).map_err(|e| {
        error!("Failed to find tree: {}", e);
        e.to_string()
    })?;

    let signature = repo.signature().map_err(|e| {
        error!("Failed to get signature: {}", e);
        e.to_string()
    })?;

    let head = repo.head().map_err(|e| {
        error!("Failed to get HEAD: {}", e);
        e.to_string()
    })?;

    let parent_commit = repo.find_commit(head.target().unwrap()).map_err(|e| {
        error!("Failed to find parent commit: {}", e);
        e.to_string()
    })?;

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        &[&parent_commit],
    )
    .map_err(|e| {
        error!("Failed to commit: {}", e);
        e.to_string()
    })?;

    info!("Commit successful");
    Ok(())
}

#[derive(Serialize)]
pub struct AutoCellmetaCommitResult {
    committed: bool,
    path_count: usize,
    message: String,
}

#[tauri::command]
pub fn auto_commit_cellmeta_changes(path: String) -> Result<AutoCellmetaCommitResult, String> {
    info!("[CellmetaAutoCommit] checking repository: {}", path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("[CellmetaAutoCommit] failed to discover repository: {}", e);
        e.to_string()
    })?;

    // 设计说明：
    // 1. `.cellmeta` 是 Notebook/探索者模式的侧车元数据文件，需要被 Git 记录，但不应该打扰用户的普通文件树和 Git 面板。
    // 2. 这里先只收集 `.cellmeta` 路径，再用带 pathspec 的 `git commit --only` 提交，避免把用户已经暂存的普通文件一起提交。
    // 3. 在修改索引前先校验签名配置，降低自动提交失败后留下内部暂存项的概率。
    repo.signature().map_err(|e| {
        error!("[CellmetaAutoCommit] git signature is not configured: {}", e);
        format!("Git signature is not configured: {}", e)
    })?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);
    opts.show(StatusShow::IndexAndWorkdir);

    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| {
        error!("[CellmetaAutoCommit] failed to get statuses: {}", e);
        e.to_string()
    })?;

    let mut paths = BTreeSet::<String>::new();
    for entry in statuses.iter() {
        let Some(rel_path) = entry.path() else {
            continue;
        };
        if is_cellmeta_git_path(rel_path) {
            paths.insert(rel_path.replace('\\', "/"));
        }
    }

    let paths: Vec<String> = paths.into_iter().collect();
    if paths.is_empty() {
        debug!("[CellmetaAutoCommit] no .cellmeta changes found");
        return Ok(AutoCellmetaCommitResult {
            committed: false,
            path_count: 0,
            message: "no .cellmeta changes".to_string(),
        });
    }

    let repo_dir = repo
        .workdir()
        .ok_or_else(|| "Bare repository has no working directory".to_string())?;

    info!(
        "[CellmetaAutoCommit] staging .cellmeta paths before commit: count={}",
        paths.len()
    );
    let add_output = std::process::Command::new("git")
        .current_dir(repo_dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .arg("add")
        .arg("--")
        .args(&paths)
        .output()
        .map_err(|e| format!("无法执行 git add 自动暂存 .cellmeta：{}", e))?;

    if !add_output.status.success() {
        let msg = git_output_message(&add_output);
        error!("[CellmetaAutoCommit] git add failed: {}", msg);
        return Err(format!("自动暂存 .cellmeta 失败：{}", msg));
    }

    let diff_output = std::process::Command::new("git")
        .current_dir(repo_dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .arg("diff")
        .arg("--cached")
        .arg("--quiet")
        .arg("--")
        .args(&paths)
        .output()
        .map_err(|e| format!("无法检查 .cellmeta 暂存差异：{}", e))?;

    if diff_output.status.success() {
        debug!(
            "[CellmetaAutoCommit] .cellmeta paths had no staged diff after git add: count={}",
            paths.len()
        );
        return Ok(AutoCellmetaCommitResult {
            committed: false,
            path_count: paths.len(),
            message: "no staged .cellmeta diff".to_string(),
        });
    }

    if diff_output.status.code() != Some(1) {
        let msg = git_output_message(&diff_output);
        error!("[CellmetaAutoCommit] git diff --cached failed: {}", msg);
        return Err(format!("检查 .cellmeta 暂存差异失败：{}", msg));
    }

    let commit_message = "chore(cellmeta): auto-save notebook metadata";
    info!(
        "[CellmetaAutoCommit] committing .cellmeta paths: count={}, message={}",
        paths.len(),
        commit_message
    );
    let commit_output = std::process::Command::new("git")
        .current_dir(repo_dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .arg("commit")
        .arg("--quiet")
        .arg("--only")
        .arg("-m")
        .arg(commit_message)
        .arg("--")
        .args(&paths)
        .output()
        .map_err(|e| format!("无法执行 git commit 自动提交 .cellmeta：{}", e))?;

    if !commit_output.status.success() {
        let msg = git_output_message(&commit_output);
        error!("[CellmetaAutoCommit] git commit failed: {}", msg);
        // 提交失败时把 `.cellmeta` 从索引里撤回，避免用户在 Git 面板里看不到的内部文件
        // 长时间停留在 staged 状态；工作区文件本身会保留，下一次定时检查仍可重试。
        let reset_output = std::process::Command::new("git")
            .current_dir(repo_dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .arg("reset")
            .arg("-q")
            .arg("HEAD")
            .arg("--")
            .args(&paths)
            .output();
        if let Ok(output) = reset_output {
            if !output.status.success() {
                warn!(
                    "[CellmetaAutoCommit] failed to unstage .cellmeta after commit failure: {}",
                    git_output_message(&output)
                );
            }
        }
        return Err(format!("自动提交 .cellmeta 失败：{}", msg));
    }

    info!(
        "[CellmetaAutoCommit] committed .cellmeta changes successfully: count={}",
        paths.len()
    );
    Ok(AutoCellmetaCommitResult {
        committed: true,
        path_count: paths.len(),
        message: commit_message.to_string(),
    })
}

#[tauri::command]
pub fn git_push(path: String) -> Result<(), String> {
    info!("Pushing changes to remote for path: {}", path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let current_branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "main".to_string());

    let repo_dir = repo.workdir().unwrap_or_else(|| Path::new(&path));
    let output = std::process::Command::new("git")
        .current_dir(repo_dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .arg("push")
        .arg("-u")
        .arg("origin")
        .arg(&current_branch)
        .output()
        .map_err(|e| format!("无法执行 git（请确认已安装 Git 且已加入 PATH）：{}", e))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = if stdout.trim().is_empty() {
            stderr.trim().to_string()
        } else if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("{}\n{}", stdout.trim_end(), stderr.trim_start())
        };
        return Err(format!("git push 失败（{}）：{}", current_branch, msg));
    }

    return Ok(());

    #[cfg(any())]
    {
    let mut remote = repo.find_remote("origin").map_err(|e| {
        error!("Failed to find remote 'origin': {}", e);
        e.to_string()
    })?;

    if let Some(url) = remote.url() {
        let normalized = normalize_remote_url(url);
        if normalized != url {
            info!(
                origin_url_before = url,
                origin_url_after = normalized.as_str(),
                "normalize origin url for pull"
            );
            if let Err(e) = repo.remote_set_url("origin", normalized.as_str()) {
                warn!("Failed to normalize origin url: {}", e);
            } else {
                remote = repo.find_remote("origin").map_err(|e| {
                    error!("Failed to find remote 'origin': {}", e);
                    e.to_string()
                })?;
            }
        }
    }

    let origin_url = remote
        .url()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "".to_string());

    let is_kb_repo = false;
    let (kb_username, kb_token) = (String::new(), String::new());
    let kb_token_present = !kb_token.trim().is_empty();

    info!(
        origin_url = origin_url.as_str(),
        current_branch = current_branch.as_str(),
        is_kb_repo,
        kb_username_present = !kb_username.trim().is_empty(),
        kb_token_present,
        "git_push context"
    );

    let cfg = repo.config().ok();

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |url, username_from_url, allowed| {
        info!(
            url,
            username_from_url = ?username_from_url,
            allowed = ?allowed,
            is_kb_repo,
            kb_token_present,
            "git_push credentials callback"
        );

        if is_kb_repo
            && kb_token_present
            && allowed.contains(CredentialType::USER_PASS_PLAINTEXT)
        {
            let username = if !kb_username.is_empty() {
                kb_username.clone()
            } else if url.to_ascii_lowercase().contains("gitee.com") {
                return Err(git2::Error::from_str(
                    "Gitee HTTPS 推送/拉取使用访问令牌时需要填写远程仓库用户名（你的 Gitee 账号或邮箱）",
                ));
            } else {
                default_http_username(url, username_from_url)
            };
            return Cred::userpass_plaintext(&username, &kb_token);
        }

        if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
            if let Some(cfg) = cfg.as_ref() {
                if let Ok(c) = Cred::credential_helper(cfg, url, username_from_url) {
                    return Ok(c);
                }
                let u = default_http_username(url, username_from_url);
                if let Ok(c) = Cred::credential_helper(cfg, url, Some(u.as_str())) {
                    return Ok(c);
                }
            }
        }

        if allowed.contains(CredentialType::SSH_KEY) {
            let key_path = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".ssh").join("id_rsa"))
                .unwrap_or_else(|_| PathBuf::from("id_rsa"));

            let username = username_from_url.unwrap_or("git");
            debug!("Using SSH key at: {:?}", key_path);
            return Cred::ssh_key(username, None, &key_path, None);
        }

        Cred::default()
    });

    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);

    let refspec = format!("refs/heads/{0}:refs/heads/{0}", current_branch);

    remote.push(&[refspec.as_str()], Some(&mut push_options)).map_err(|e| {
        error!("Failed to push: {}", e);
        if origin_url.is_empty() {
            e.to_string()
        } else {
            format!("{} (origin: {})", e, origin_url)
        }
    })?;

    if let Ok(mut branch) = repo.find_branch(&current_branch, BranchType::Local) {
        let upstream = format!("origin/{}", current_branch);
        if let Err(e) = branch.set_upstream(Some(upstream.as_str())) {
            warn!("Failed to set upstream for {}: {}", current_branch, e);
        }
    }

    info!("Push successful");
    Ok(())
    }
}

#[tauri::command]
pub fn git_pull(path: String) -> Result<(), String> {
    info!("Pulling changes for path: {}", path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    // 获取当前分支名称
    let current_branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "main".to_string());

    let repo_dir = repo.workdir().unwrap_or_else(|| Path::new(&path));
    let output = std::process::Command::new("git")
        .current_dir(repo_dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .arg("pull")
        .arg("--ff-only")
        .arg("origin")
        .arg(&current_branch)
        .output()
        .map_err(|e| format!("无法执行 git（请确认已安装 Git 且已加入 PATH）：{}", e))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = if stdout.trim().is_empty() {
            stderr.trim().to_string()
        } else if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("{}\n{}", stdout.trim_end(), stderr.trim_start())
        };
        return Err(format!("git pull 失败（{}）：{}", current_branch, msg));
    }

    return Ok(());

    #[cfg(any())]
    {
    let mut remote = repo.find_remote("origin").map_err(|e| {
        error!("Failed to find remote 'origin': {}", e);
        e.to_string()
    })?;

    if let Some(url) = remote.url() {
        let normalized = normalize_remote_url(url);
        if normalized != url {
            if let Err(e) = repo.remote_set_url("origin", normalized.as_str()) {
                warn!("Failed to normalize origin url: {}", e);
            } else {
                remote = repo.find_remote("origin").map_err(|e| {
                    error!("Failed to find remote 'origin': {}", e);
                    e.to_string()
                })?;
            }
        }
    }

    let origin_url = remote
        .url()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "".to_string());

    let is_kb_repo = false;
    let (kb_username, kb_token) = (String::new(), String::new());
    let kb_token_present = !kb_token.trim().is_empty();

    info!(
        origin_url = origin_url.as_str(),
        current_branch = current_branch.as_str(),
        is_kb_repo,
        kb_username_present = !kb_username.trim().is_empty(),
        kb_token_present,
        "git_pull context"
    );

    let cfg = repo.config().ok();

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |url, username_from_url, allowed| {
        info!(
            url,
            username_from_url = ?username_from_url,
            allowed = ?allowed,
            is_kb_repo,
            kb_token_present,
            "git_pull credentials callback"
        );

        if is_kb_repo
            && kb_token_present
            && allowed.contains(CredentialType::USER_PASS_PLAINTEXT)
        {
            let username = if !kb_username.is_empty() {
                kb_username.clone()
            } else if url.to_ascii_lowercase().contains("gitee.com") {
                return Err(git2::Error::from_str(
                    "Gitee HTTPS 推送/拉取使用访问令牌时需要填写远程仓库用户名（你的 Gitee 账号或邮箱）",
                ));
            } else {
                default_http_username(url, username_from_url)
            };
            return Cred::userpass_plaintext(&username, &kb_token);
        }

        if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
            if let Some(cfg) = cfg.as_ref() {
                if let Ok(c) = Cred::credential_helper(cfg, url, username_from_url) {
                    return Ok(c);
                }
                let u = default_http_username(url, username_from_url);
                if let Ok(c) = Cred::credential_helper(cfg, url, Some(u.as_str())) {
                    return Ok(c);
                }
            }
        }

        if allowed.contains(CredentialType::SSH_KEY) {
            let key_path = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".ssh").join("id_rsa"))
                .unwrap_or_else(|_| PathBuf::from("id_rsa"));

            let username = username_from_url.unwrap_or("git");
            debug!("Using SSH key at: {:?}", key_path);
            return Cred::ssh_key(username, None, &key_path, None);
        }

        Cred::default()
    });

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    remote
        .fetch(
            &["refs/heads/*:refs/heads/*"],
            Some(&mut fetch_options),
            None,
        )
        .map_err(|e| {
            error!("Failed to fetch: {}", e);
            e.to_string()
        })?;

    // 安全处理 FETCH_HEAD
    let fetch_head = match repo.find_reference("FETCH_HEAD") {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to find FETCH_HEAD: {}", e);
            return Err(format!("No changes to pull: {}", e));
        }
    };

    let fetch_commit = repo
        .reference_to_annotated_commit(&fetch_head)
        .map_err(|e| {
            error!("Failed to get annotated commit: {}", e);
            e.to_string()
        })?;

    let analysis = repo.merge_analysis(&[&fetch_commit]).map_err(|e| {
        error!("Failed to analyze merge: {}", e);
        e.to_string()
    })?;

    if analysis.0.is_up_to_date() {
        info!("Already up-to-date");
        return Ok(());
    } else if analysis.0.is_fast_forward() {
        info!("Performing fast-forward merge");
        let reference_name = format!("refs/heads/{}", current_branch);
        let mut reference = repo.find_reference(&reference_name).map_err(|e| {
            error!("Failed to find branch: {}", e);
            e.to_string()
        })?;

        reference
            .set_target(fetch_commit.id(), "Fast-forward")
            .map_err(|e| {
                error!("Failed to set target: {}", e);
                e.to_string()
            })?;

        repo.set_head(&reference_name).map_err(|e| {
            error!("Failed to set HEAD: {}", e);
            e.to_string()
        })?;

        repo.checkout_head(Some(CheckoutBuilder::default().force()))
            .map_err(|e| {
                error!("Failed to checkout HEAD: {}", e);
                e.to_string()
            })?;
    } else {
        error!("Merge required but not implemented");
        return Err("Merge required".into());
    }

    info!("Pull successful");
    Ok(())
    }
}

#[tauri::command]
pub fn create_branch(path: String, branch_name: String) -> Result<(), String> {
    info!("Creating branch: {} for path: {}", branch_name, path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let head = repo.head().map_err(|e| {
        error!("Failed to get HEAD: {}", e);
        e.to_string()
    })?;

    let commit = repo.find_commit(head.target().unwrap()).map_err(|e| {
        error!("Failed to find commit: {}", e);
        e.to_string()
    })?;

    repo.branch(&branch_name, &commit, false).map_err(|e| {
        error!("Failed to create branch: {}", e);
        e.to_string()
    })?;

    info!("Branch created successfully");
    Ok(())
}

#[allow(dead_code)]
#[tauri::command]
pub fn get_staged_files(path: String) -> Result<Vec<GitChange>, String> {
    info!("Getting staged files for path: {}", path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let mut index = repo.index().map_err(|e| {
        error!("Failed to get index: {}", e);
        e.to_string()
    })?;

    let tree = repo
        .find_tree(index.write_tree().map_err(|e| {
            error!("Failed to write tree: {}", e);
            e.to_string()
        })?)
        .map_err(|e| {
            error!("Failed to find tree: {}", e);
            e.to_string()
        })?;

    let head = repo.head().map_err(|e| {
        error!("Failed to get HEAD: {}", e);
        e.to_string()
    })?;

    let head_tree = head.peel_to_tree().map_err(|e| {
        error!("Failed to peel to tree: {}", e);
        e.to_string()
    })?;

    let diff = repo
        .diff_tree_to_tree(Some(&head_tree), Some(&tree), None)
        .map_err(|e| {
            error!("Failed to create diff: {}", e);
            e.to_string()
        })?;

    let mut staged_files = Vec::new();

    diff.foreach(
        &mut |delta, _| {
            if let Some(new_file) = delta.new_file().path() {
                staged_files.push(new_file.to_str().unwrap_or_default().to_string());
            }
            true
        },
        None,
        None,
        None,
    )
    .map_err(|e| {
        error!("Failed to process diff: {}", e);
        e.to_string()
    })?;

    info!("Found {} staged files", staged_files.len());

    Ok(staged_files
        .iter()
        .map(|path| GitChange {
            path: path.clone(),
            status: "S".to_string(),
            lines_added: Vec::new(),
            lines_modified: Vec::new(),
            lines_deleted: Vec::new(),
            diff: String::new(),
            staged: true, // 添加 staged 字段并设置为 true
        })
        .collect())
}

#[tauri::command]
pub fn unstage_file(path: String, file_path: String) -> Result<(), String> {
    let repo =
        Repository::discover(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

    // 获取HEAD提交
    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?;
    let head_commit = head
        .peel_to_commit()
        .map_err(|e| format!("Failed to peel to commit: {}", e))?;
    let head_tree = head_commit
        .tree()
        .map_err(|e| format!("Failed to get tree: {}", e))?;

    let mut index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e))?;

    // 从索引中移除文件
    index
        .remove_path(Path::new(&file_path))
        .map_err(|e| format!("Failed to remove path from index: {}", e))?;

    // 如果文件在HEAD中存在，则恢复到HEAD版本
    if let Ok(entry) = head_tree.get_path(Path::new(&file_path)) {
        // 创建 IndexEntry
        let index_entry = IndexEntry {
            ctime: git2::IndexTime::new(0, 0),
            mtime: git2::IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: entry.filemode() as u32,
            uid: 0,
            gid: 0,
            file_size: 0,
            id: entry.id(),
            flags: 0,
            flags_extended: 0,
            path: file_path.as_bytes().to_vec(),
        };

        let blob = repo
            .find_blob(entry.id())
            .map_err(|e| format!("Failed to find blob: {}", e))?;

        // 使用正确的 IndexEntry 参数
        index
            .add_frombuffer(&index_entry, blob.content())
            .map_err(|e| format!("Failed to add from buffer: {}", e))?;
    }

    // 写入索引
    index
        .write()
        .map_err(|e| format!("Failed to write index: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn get_git_log(
    path: String,
    branch_name: String,
    limit: Option<usize>,
) -> Result<Vec<GitCommit>, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let max = match limit.unwrap_or(200) {
            0 => 200,
            n if n > 2000 => 2000,
            n => n,
        };

        let repo = Repository::discover(&path).map_err(|e| e.to_string())?;

        let branch = repo
            .find_branch(&branch_name, BranchType::Local)
            .map_err(|e| format!("Failed to find branch {}: {}", branch_name, e))?;

        let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;
        revwalk
            .push_ref(branch.get().name().ok_or("Invalid branch name")?)
            .map_err(|e| e.to_string())?;

        let mut commits = Vec::new();
        for oid in revwalk.take(max) {
            let oid = oid.map_err(|e| e.to_string())?;
            let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;

            let message = commit.message().unwrap_or_default().to_string();
            let summary = message.lines().next().unwrap_or_default().to_string();

            let parent_tree = if commit.parent_count() > 0 {
                Some(
                    commit
                        .parent(0)
                        .map_err(|e| e.to_string())?
                        .tree()
                        .map_err(|e| e.to_string())?,
                )
            } else {
                None
            };
            let commit_tree = commit.tree().map_err(|e| e.to_string())?;
            let diff = repo
                .diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)
                .map_err(|e| e.to_string())?;

            let stats = diff.stats().map_err(|e| e.to_string())?;

            commits.push(GitCommit {
                id: oid.to_string(),
                message,
                summary,
                author: commit.author().name().unwrap_or_default().to_string(),
                date: commit.time().seconds().to_string(),
                files_changed: stats.files_changed(),
                insertions: stats.insertions(),
                deletions: stats.deletions(),
            });
        }

        Ok(commits)
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

#[tauri::command]
pub fn stage_file(path: String, file_path: String) -> Result<(), String> {
    let repo =
        Repository::discover(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

    let mut index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e))?;

    // 添加文件到索引
    index
        .add_path(Path::new(&file_path))
        .map_err(|e| format!("Failed to add path to index: {}", e))?;

    // 写入索引
    index
        .write()
        .map_err(|e| format!("Failed to write index: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn stage_all(path: String) -> Result<(), String> {
    info!("Staging all changes in path: {}", path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let mut index = repo.index().map_err(|e| {
        error!("Failed to get index: {}", e);
        e.to_string()
    })?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);
    opts.show(StatusShow::IndexAndWorkdir);

    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| {
        error!("Failed to get statuses: {}", e);
        e.to_string()
    })?;

    for entry in statuses.iter() {
        let Some(p) = entry.path() else {
            continue;
        };
        if is_ignored_git_path(p) {
            continue;
        }

        let rel = Path::new(p);
        let abs = repo
            .workdir()
            .ok_or_else(|| "Bare repository has no working directory".to_string())?
            .join(rel);

        if abs.is_dir() {
            continue;
        }

        if entry.status().is_wt_deleted() {
            if let Err(e) = index.remove_path(rel) {
                warn!("Failed to remove path from index {}: {}", rel.display(), e);
            }
            continue;
        }

        if let Err(e) = index.add_path(rel) {
            warn!("Skipping invalid path {}: {}", rel.display(), e);
        }
    }

    index.write().map_err(|e| {
        error!("Failed to write index: {}", e);
        e.to_string()
    })?;

    info!("All changes staged successfully");
    Ok(())
}

#[tauri::command]
pub fn unstage_all(path: String) -> Result<(), String> {
    info!("Unstaging all changes in path: {}", path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let head = repo.head().map_err(|e| {
        error!("Failed to get HEAD: {}", e);
        e.to_string()
    })?;

    let head_tree = head.peel_to_tree().map_err(|e| {
        error!("Failed to peel to tree: {}", e);
        e.to_string()
    })?;

    let mut index = repo.index().map_err(|e| {
        error!("Failed to get index: {}", e);
        e.to_string()
    })?;

    // 重置整个索引到 HEAD
    index.read_tree(&head_tree).map_err(|e| {
        error!("Failed to read tree into index: {}", e);
        e.to_string()
    })?;

    index.write().map_err(|e| {
        error!("Failed to write index: {}", e);
        e.to_string()
    })?;

    info!("All changes unstaged successfully");
    Ok(())
}

#[tauri::command]
pub fn discard_file(path: String, file_path: String) -> Result<(), String> {
    info!(
        "Discarding changes for file: {} in path: {}",
        file_path, path
    );
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let head = repo.head().map_err(|e| {
        error!("Failed to get HEAD: {}", e);
        e.to_string()
    })?;

    let head_tree = head.peel_to_tree().map_err(|e| {
        error!("Failed to peel to tree: {}", e);
        e.to_string()
    })?;

    let mut checkout_builder = CheckoutBuilder::new();
    checkout_builder.path(Path::new(&file_path));
    checkout_builder.force();

    repo.checkout_tree(head_tree.as_object(), Some(&mut checkout_builder))
        .map_err(|e| {
            error!("Failed to checkout tree: {}", e);
            e.to_string()
        })?;

    info!("File changes discarded successfully");
    Ok(())
}

#[tauri::command]
pub fn discard_all(path: String) -> Result<(), String> {
    info!("Discarding all changes in path: {}", path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let mut checkout_builder = CheckoutBuilder::new();
    checkout_builder.force();

    repo.checkout_head(Some(&mut checkout_builder))
        .map_err(|e| {
            error!("Failed to checkout HEAD: {}", e);
            e.to_string()
        })?;

    info!("All changes discarded successfully");
    Ok(())
}

#[tauri::command]
pub fn discard_hunk(
    path: String,
    file_path: String,
    line: u32,
    hunk_index: Option<u32>,
    original_start_line: Option<u32>,
    original_end_line: Option<u32>,
    modified_start_line: Option<u32>,
    modified_end_line: Option<u32>,
    reverse: bool,
) -> Result<(), String> {
    info!(
        "Discarding hunk for file: {} in path: {}, line: {}, hunk_index: {:?}, original_range: {:?}-{:?}, modified_range: {:?}-{:?}, reverse: {}",
        file_path,
        path,
        line,
        hunk_index,
        original_start_line,
        original_end_line,
        modified_start_line,
        modified_end_line,
        reverse
    );
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let index = repo.index().map_err(|e| {
        error!("Failed to get index for hunk discard: {}", e);
        e.to_string()
    })?;

    let mut diff_options = DiffOptions::new();
    diff_options.pathspec(file_path.clone());
    diff_options.include_untracked(true);
    diff_options.recurse_untracked_dirs(true);
    diff_options.context_lines(0);

    // 设计说明：撤销未暂存 hunk 必须比较“索引 -> 工作区”，不能比较
    // “HEAD -> 工作区”。否则文件已经部分暂存时，点击未暂存视图里的单块撤销
    // 会把已暂存块也纳入候选 patch，看起来像整文件被撤销。
    let diff = repo
        .diff_index_to_workdir(Some(&index), Some(&mut diff_options))
        .map_err(|e| {
            error!("Failed to get diff for file: {}", e);
            e.to_string()
        })?;

    let normalized_path = file_path.replace('\\', "/");

    struct HunkState {
        target_header: Option<String>,
        target_status: Option<Delta>,
        best_match_distance: u32,
        range_matched: bool,
        hunk_count: u32,
    }

    let state = RefCell::new(HunkState {
        target_header: None,
        target_status: None,
        best_match_distance: u32::MAX,
        range_matched: false,
        hunk_count: 0,
    });

    diff.foreach(
        &mut |_delta, _| true,
        None,
        Some(&mut |delta, hunk| {
            let mut state = state.borrow_mut();
            let current_index = state.hunk_count;
            state.hunk_count = state.hunk_count.saturating_add(1);

            let ranges = UnifiedHunkRanges {
                old_start: hunk.old_start() as u32,
                old_count: hunk.old_lines() as u32,
                new_start: hunk.new_start() as u32,
                new_count: hunk.new_lines() as u32,
            };
            let distance = line_distance_to_unified_hunk(line, ranges);
            let range_matches = unified_hunk_matches_selection(
                ranges,
                original_start_line,
                original_end_line,
                modified_start_line,
                modified_end_line,
            );
            if hunk_index == Some(current_index) {
                // 设计说明：前端知道当前 Monaco diff change 的顺序，后端优先按顺序取
                // git hunk，避免行号范围过宽时把同一文件的多个 hunk 一起撤销。
                state.range_matched = true;
                state.best_match_distance = distance;
                state.target_header = Some(String::from_utf8_lossy(hunk.header()).to_string());
                state.target_status = Some(delta.status());
                return true;
            }
            if hunk_index.is_some() {
                return true;
            }
            // 设计说明：Monaco 的当前差异块按钮可能锚在插入块前一行；
            // 撤销差异块优先使用前端传入的可视 diff 范围；范围没有命中时，
            // 再退回“精确优先，允许 1 行邻近”的单行策略，和暂存保持一致。
            if range_matches
                && (!state.range_matched || distance < state.best_match_distance)
            {
                // 设计说明：Monaco 的可视范围在少数场景可能覆盖多个 git hunk。
                // 这里仍只选择离当前光标锚点最近的一块，保证胶囊按钮是块级操作。
                state.range_matched = true;
                state.best_match_distance = distance;
                state.target_header = Some(String::from_utf8_lossy(hunk.header()).to_string());
                state.target_status = Some(delta.status());
            } else if !state.range_matched
                && distance <= HUNK_LINE_MATCH_TOLERANCE
                && distance < state.best_match_distance
            {
                state.best_match_distance = distance;
                state.target_header = Some(String::from_utf8_lossy(hunk.header()).to_string());
                state.target_status = Some(delta.status());
            }
            true
        }),
        None,
    )
    .map_err(|e| {
        error!("Failed to inspect diff hunks: {}", e);
        e.to_string()
    })?;

    let state = state.into_inner();
    let Some(target_header) = state.target_header else {
        error!(
            "No matching hunk found for file: {}, line: {}, hunk_count: {}",
            normalized_path, line, state.hunk_count
        );
        return Err("撤销差异块失败：未找到匹配的差异块".to_string());
    };

    if state.range_matched {
        debug!(
            "Matched hunk for discard by index/range: file={}, line={}, hunk_index={:?}, header={}",
            normalized_path,
            line,
            hunk_index,
            target_header.trim()
        );
    } else if state.best_match_distance == 0 {
        debug!(
            "Matched hunk for discard by exact line: file={}, line={}",
            normalized_path, line
        );
    } else {
        warn!(
            "Matched hunk for discard by nearby anchor: file={}, line={}, distance={}",
            normalized_path, line, state.best_match_distance
        );
    }

    let mut patch = format!("diff --git a/{0} b/{0}\n", normalized_path);
    match state.target_status.unwrap_or(Delta::Modified) {
        Delta::Added | Delta::Untracked => {
            patch.push_str("--- /dev/null\n");
            patch.push_str(&format!("+++ b/{}\n", normalized_path));
        }
        Delta::Deleted => {
            patch.push_str(&format!("--- a/{}\n", normalized_path));
            patch.push_str("+++ /dev/null\n");
        }
        _ => {
            patch.push_str(&format!("--- a/{}\n", normalized_path));
            patch.push_str(&format!("+++ b/{}\n", normalized_path));
        }
    }
    let selected_hunk_lines = RefCell::new(Vec::<String>::new());
    let appended_header = RefCell::new(false);
    diff.foreach(
        &mut |_delta, _| true,
        None,
        None,
        Some(&mut |_delta, hunk, line_info| {
            if let Some(current_hunk) = hunk {
                let header = String::from_utf8_lossy(current_hunk.header());
                if header != target_header.as_str() {
                    return true;
                }
                let header_text = header.to_string();
                if !*appended_header.borrow() {
                    selected_hunk_lines.borrow_mut().push(header_text);
                    *appended_header.borrow_mut() = true;
                }
            }
            let origin = line_info.origin();
            // Keep '\ No newline at end of file' metadata lines; omitting them
            // can make zero-context patches fail to apply in reverse.
            if origin != ' ' && origin != '+' && origin != '-' && origin != '\\' {
                return true;
            }
            if let Ok(content) = std::str::from_utf8(line_info.content()) {
                let trimmed = content.trim_end_matches(|c| c == '\n' || c == '\r');
                selected_hunk_lines
                    .borrow_mut()
                    .push(format!("{}{}", origin, trimmed));
            }
            true
        }),
    )
    .map_err(|e| {
        error!("Failed to iterate diff hunks: {}", e);
        e.to_string()
    })?;

    let selected_hunk_lines = selected_hunk_lines.into_inner();
    if selected_hunk_lines.is_empty() {
        error!("Matched hunk produced an empty patch for line {}", line);
        return Err("撤销差异块失败：未找到匹配的差异块".to_string());
    }
    let precise_hunk = crop_hunk_to_selection(
        &selected_hunk_lines,
        original_start_line,
        original_end_line,
        modified_start_line,
        modified_end_line,
        "撤销",
    )?;
    for item in precise_hunk {
        patch.push_str(&item);
        patch.push('\n');
    }
    info!(
        "Discard hunk patch prepared: file={}, hunk_index={:?}, patch_lines={}, patch_preview={}",
        normalized_path,
        hunk_index,
        patch.lines().count(),
        patch.lines().take(8).collect::<Vec<_>>().join(" | ")
    );

    let repo_dir = repo.workdir().unwrap_or_else(|| Path::new(&path));
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(repo_dir)
        .arg("apply")
        .arg("--whitespace=nowarn")
        .arg("--unidiff-zero")
        .arg("--inaccurate-eof")
        .arg("--recount")
        .arg("-p1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if reverse {
        cmd.arg("--reverse");
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("无法执行 git（请确认已安装 Git 且已加入 PATH）：{}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(patch.as_bytes())
            .map_err(|e| format!("写入补丁失败：{}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("执行 git apply 失败：{}", e))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = if stdout.trim().is_empty() {
            stderr.trim().to_string()
        } else if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("{}\n{}", stdout.trim_end(), stderr.trim_start())
        };
        error!("git apply failed: {}", msg);
        return Err(format!("撤销差异块失败：{}", msg));
    }

    info!("Hunk discarded successfully");
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct UnifiedHunkRanges {
    old_start: u32,
    old_count: u32,
    new_start: u32,
    new_count: u32,
}

fn parse_unified_hunk_range_part(header: &str, prefix: char) -> Option<(u32, u32)> {
    let part = header
        .split_whitespace()
        .find(|part| part.starts_with(prefix))?
        .trim_start_matches(prefix);
    let mut parts = part.split(',');
    let start = parts.next()?.parse::<u32>().ok()?;
    let count = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1);
    Some((start, count))
}

fn parse_unified_hunk_ranges(header: &str) -> Option<UnifiedHunkRanges> {
    let (old_start, old_count) = parse_unified_hunk_range_part(header, '-')?;
    let (new_start, new_count) = parse_unified_hunk_range_part(header, '+')?;
    Some(UnifiedHunkRanges {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

fn unified_hunk_range_end(start: u32, count: u32) -> u32 {
    start.saturating_add(count.saturating_sub(1)).max(start)
}

fn line_distance_to_unified_range(line: u32, start: u32, count: u32) -> u32 {
    let end = unified_hunk_range_end(start, count);
    if line < start {
        start - line
    } else if line > end {
        line - end
    } else {
        0
    }
}

fn line_distance_to_unified_hunk(line: u32, ranges: UnifiedHunkRanges) -> u32 {
    let old_distance = line_distance_to_unified_range(line, ranges.old_start, ranges.old_count);
    let new_distance = line_distance_to_unified_range(line, ranges.new_start, ranges.new_count);
    old_distance.min(new_distance)
}

fn normalized_selection_range(start: Option<u32>, end: Option<u32>) -> Option<(u32, u32)> {
    let start = start.filter(|value| *value > 0)?;
    let end = end.filter(|value| *value > 0).unwrap_or(start);
    Some((start.min(end), start.max(end)))
}

fn range_overlaps_unified_range(selection: Option<(u32, u32)>, start: u32, count: u32) -> bool {
    let Some((selection_start, selection_end)) = selection else {
        return false;
    };
    let range_end = unified_hunk_range_end(start, count);
    selection_start <= range_end && selection_end >= start
}

fn unified_hunk_matches_selection(
    ranges: UnifiedHunkRanges,
    original_start_line: Option<u32>,
    original_end_line: Option<u32>,
    modified_start_line: Option<u32>,
    modified_end_line: Option<u32>,
) -> bool {
    let original_range = normalized_selection_range(original_start_line, original_end_line);
    let modified_range = normalized_selection_range(modified_start_line, modified_end_line);
    range_overlaps_unified_range(original_range, ranges.old_start, ranges.old_count)
        || range_overlaps_unified_range(modified_range, ranges.new_start, ranges.new_count)
}

fn range_contains_line(start: Option<u32>, end: Option<u32>, line: u32) -> bool {
    let Some((start, end)) = normalized_selection_range(start, end) else {
        return false;
    };
    line >= start && line <= end
}

fn has_precise_hunk_range(
    original_start_line: Option<u32>,
    original_end_line: Option<u32>,
    modified_start_line: Option<u32>,
    modified_end_line: Option<u32>,
) -> bool {
    normalized_selection_range(original_start_line, original_end_line).is_some()
        || normalized_selection_range(modified_start_line, modified_end_line).is_some()
}

fn format_unified_range(prefix: char, start: u32, count: u32) -> String {
    if count == 1 {
        format!("{}{}", prefix, start)
    } else {
        format!("{}{},{}", prefix, start, count)
    }
}

fn crop_hunk_to_selection(
    hunk_lines: &[String],
    original_start_line: Option<u32>,
    original_end_line: Option<u32>,
    modified_start_line: Option<u32>,
    modified_end_line: Option<u32>,
    action_name: &str,
) -> Result<Vec<String>, String> {
    let Some(header) = hunk_lines.first() else {
        return Err(format!("{}差异块失败：空的差异块", action_name));
    };
    let Some(ranges) = parse_unified_hunk_ranges(header) else {
        return Err(format!("{}差异块失败：无法解析差异块范围", action_name));
    };

    if !has_precise_hunk_range(
        original_start_line,
        original_end_line,
        modified_start_line,
        modified_end_line,
    ) {
        warn!(
            "No precise Monaco hunk range received; refusing to use the full git hunk: action={}, header={}",
            action_name,
            header.trim()
        );
        return Err(format!(
            "{}差异块失败：未收到精确的差异块范围，请重新点击目标块",
            action_name
        ));
    }

    let mut old_line = ranges.old_start.max(1);
    let mut new_line = ranges.new_start.max(1);
    let mut selected_lines = Vec::<String>::new();
    let mut old_start: Option<u32> = None;
    let mut new_start: Option<u32> = None;
    let mut old_count = 0u32;
    let mut new_count = 0u32;
    let mut last_selected = false;

    for raw_line in hunk_lines.iter().skip(1) {
        let origin = raw_line.chars().next().unwrap_or('\0');
        match origin {
            '-' => {
                let current_old = old_line;
                old_line = old_line.saturating_add(1);
                if range_contains_line(original_start_line, original_end_line, current_old) {
                    old_start = Some(old_start.unwrap_or(current_old));
                    old_count = old_count.saturating_add(1);
                    selected_lines.push(raw_line.clone());
                    last_selected = true;
                } else {
                    last_selected = false;
                }
            }
            '+' => {
                let current_new = new_line;
                new_line = new_line.saturating_add(1);
                if range_contains_line(modified_start_line, modified_end_line, current_new) {
                    new_start = Some(new_start.unwrap_or(current_new));
                    new_count = new_count.saturating_add(1);
                    selected_lines.push(raw_line.clone());
                    last_selected = true;
                } else {
                    last_selected = false;
                }
            }
            ' ' => {
                let current_old = old_line;
                let current_new = new_line;
                old_line = old_line.saturating_add(1);
                new_line = new_line.saturating_add(1);
                let selected = range_contains_line(original_start_line, original_end_line, current_old)
                    || range_contains_line(modified_start_line, modified_end_line, current_new);
                if selected {
                    old_start = Some(old_start.unwrap_or(current_old));
                    new_start = Some(new_start.unwrap_or(current_new));
                    old_count = old_count.saturating_add(1);
                    new_count = new_count.saturating_add(1);
                    selected_lines.push(raw_line.clone());
                    last_selected = true;
                } else {
                    last_selected = false;
                }
            }
            '\\' => {
                if last_selected {
                    selected_lines.push(raw_line.clone());
                }
            }
            _ => {}
        }
    }

    if selected_lines.is_empty() {
        warn!(
            "Precise hunk crop produced no lines: action={}, header={}, original_range={:?}-{:?}, modified_range={:?}-{:?}",
            action_name,
            header.trim(),
            original_start_line,
            original_end_line,
            modified_start_line,
            modified_end_line
        );
        return Err(format!("{}差异块失败：未找到当前胶囊对应的变更行", action_name));
    }

    let old_start = old_start.unwrap_or(ranges.old_start);
    let new_start = new_start.unwrap_or(ranges.new_start);
    let cropped_header = format!(
        "@@ {} {} @@",
        format_unified_range('-', old_start, old_count),
        format_unified_range('+', new_start, new_count)
    );
    let mut result = Vec::with_capacity(selected_lines.len() + 1);
    result.push(cropped_header);
    result.extend(selected_lines);
    debug!(
        "Cropped hunk for {}: old_start={}, old_count={}, new_start={}, new_count={}, lines={}",
        action_name,
        old_start,
        old_count,
        new_start,
        new_count,
        result.len().saturating_sub(1)
    );
    Ok(result)
}

#[tauri::command]
pub fn stage_hunk(
    path: String,
    file_path: String,
    line: u32,
    hunk_index: Option<u32>,
    original_start_line: Option<u32>,
    original_end_line: Option<u32>,
    modified_start_line: Option<u32>,
    modified_end_line: Option<u32>,
) -> Result<(), String> {
    info!(
        "Staging hunk for file: {} in path: {}, line: {}, hunk_index: {:?}, original_range: {:?}-{:?}, modified_range: {:?}-{:?}",
        file_path,
        path,
        line,
        hunk_index,
        original_start_line,
        original_end_line,
        modified_start_line,
        modified_end_line
    );
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;
    let repo_dir = repo.workdir().unwrap_or_else(|| Path::new(&path));
    let normalized_path = file_path.replace('\\', "/");

    let diff_output = std::process::Command::new("git")
        .current_dir(repo_dir)
        .arg("diff")
        .arg("--unified=0")
        .arg("--")
        .arg(&normalized_path)
        .output()
        .map_err(|e| format!("无法执行 git diff：{}", e))?;

    if !diff_output.status.success() {
        let stderr = String::from_utf8_lossy(&diff_output.stderr);
        return Err(format!("暂存差异块失败：{}", stderr.trim()));
    }

    let diff_text = String::from_utf8_lossy(&diff_output.stdout);
    let mut header_lines = Vec::<String>::new();
    let mut current_hunk = Vec::<String>::new();
    let mut matched_hunk = Vec::<String>::new();
    let mut fallback_hunk = Vec::<String>::new();
    let mut in_hunk = false;
    let mut hunk_count = 0u32;
    let mut best_match_distance = u32::MAX;
    let mut current_match_distance = u32::MAX;
    let mut current_range_matches = false;
    let mut range_matched = false;
    let mut index_matched = false;

    for raw_line in diff_text.lines() {
        if raw_line.starts_with("@@ ") {
            // 设计说明：Monaco 对纯插入/纯删除块可能会把工具栏锚到相邻行，
            // 而 git diff --unified=0 的 hunk 头只记录真实变更行号。这里优先使用
            // 前端传入的可视 diff 范围，范围没有命中时再使用单行邻近兜底。
            if in_hunk {
                let current_index = hunk_count.saturating_sub(1);
                if hunk_index == Some(current_index) {
                    index_matched = true;
                    matched_hunk = current_hunk.clone();
                    best_match_distance = current_match_distance;
                } else if hunk_index.is_none()
                    && current_range_matches
                    && (!range_matched || current_match_distance < best_match_distance)
                {
                    // 设计说明：即使前端可视范围覆盖多个 git hunk，也只暂存
                    // 离当前光标锚点最近的一块，避免块级按钮退化成整文件暂存。
                    range_matched = true;
                    matched_hunk = current_hunk.clone();
                    best_match_distance = current_match_distance;
                } else if !range_matched
                    && hunk_index.is_none()
                    && current_match_distance <= HUNK_LINE_MATCH_TOLERANCE
                    && current_match_distance < best_match_distance
                {
                    fallback_hunk = current_hunk.clone();
                    best_match_distance = current_match_distance;
                }
            }
            in_hunk = true;
            hunk_count = hunk_count.saturating_add(1);
            current_hunk.clear();
            current_hunk.push(raw_line.to_string());
            if let Some(ranges) = parse_unified_hunk_ranges(raw_line) {
                current_match_distance = line_distance_to_unified_hunk(line, ranges);
                current_range_matches = unified_hunk_matches_selection(
                    ranges,
                    original_start_line,
                    original_end_line,
                    modified_start_line,
                    modified_end_line,
                );
            } else {
                current_match_distance = u32::MAX;
                current_range_matches = false;
            }
        } else if in_hunk {
            current_hunk.push(raw_line.to_string());
        } else {
            header_lines.push(raw_line.to_string());
        }
    }

    if in_hunk {
        let current_index = hunk_count.saturating_sub(1);
        if hunk_index == Some(current_index) {
            index_matched = true;
            matched_hunk = current_hunk;
            best_match_distance = current_match_distance;
        } else if hunk_index.is_none()
            && current_range_matches
            && (!range_matched || current_match_distance < best_match_distance)
        {
            range_matched = true;
            matched_hunk = current_hunk;
            best_match_distance = current_match_distance;
        } else if !range_matched
            && hunk_index.is_none()
            && current_match_distance <= HUNK_LINE_MATCH_TOLERANCE
            && current_match_distance < best_match_distance
        {
            fallback_hunk = current_hunk;
            best_match_distance = current_match_distance;
        }
    }

    if hunk_index.is_some() && !index_matched {
        warn!(
            "Requested hunk index not found for staging: file={}, hunk_index={:?}, hunk_count={}",
            normalized_path, hunk_index, hunk_count
        );
    }

    if matched_hunk.is_empty() && !fallback_hunk.is_empty() {
        matched_hunk = fallback_hunk;
    }

    if matched_hunk.is_empty() {
        warn!(
            "No matching hunk found for file: {}, line: {}, hunk_count: {}",
            normalized_path, line, hunk_count
        );
        return Err(format!("暂存差异块失败：未找到第 {} 行附近的差异块", line));
    }

    if index_matched {
        debug!(
            "Matched hunk for staging by index: file={}, line={}, hunk_index={:?}, header={}",
            normalized_path,
            line,
            hunk_index,
            matched_hunk.first().map(String::as_str).unwrap_or("").trim()
        );
    } else if range_matched {
        debug!(
            "Matched hunk for staging by visual range: file={}, line={}, header={}",
            normalized_path,
            line,
            matched_hunk.first().map(String::as_str).unwrap_or("").trim()
        );
    } else if best_match_distance == 0 {
        debug!(
            "Matched hunk for staging by exact line: file={}, line={}",
            normalized_path, line
        );
    } else {
        warn!(
            "Matched hunk for staging by nearby anchor: file={}, line={}, distance={}",
            normalized_path, line, best_match_distance
        );
    }

    let mut patch = String::new();
    for item in header_lines {
        patch.push_str(&item);
        patch.push('\n');
    }
    let precise_hunk = crop_hunk_to_selection(
        &matched_hunk,
        original_start_line,
        original_end_line,
        modified_start_line,
        modified_end_line,
        "暂存",
    )?;
    for item in precise_hunk {
        patch.push_str(&item);
        patch.push('\n');
    }
    info!(
        "Stage hunk patch prepared: file={}, hunk_index={:?}, patch_lines={}, patch_preview={}",
        normalized_path,
        hunk_index,
        patch.lines().count(),
        patch.lines().take(8).collect::<Vec<_>>().join(" | ")
    );

    let mut child = std::process::Command::new("git")
        .current_dir(repo_dir)
        .arg("apply")
        .arg("--cached")
        .arg("--whitespace=nowarn")
        .arg("--unidiff-zero")
        .arg("--recount")
        .arg("-p1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法执行 git apply --cached：{}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(patch.as_bytes())
            .map_err(|e| format!("写入暂存补丁失败：{}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("执行 git apply --cached 失败：{}", e))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = if stdout.trim().is_empty() {
            stderr.trim().to_string()
        } else if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("{}\n{}", stdout.trim_end(), stderr.trim_start())
        };
        error!("git apply --cached failed: {}", msg);
        return Err(format!("暂存差异块失败：{}", msg));
    }

    info!("Hunk staged successfully");
    Ok(())
}

#[tauri::command]
pub fn unstage_hunk(
    path: String,
    file_path: String,
    line: u32,
    hunk_index: Option<u32>,
    original_start_line: Option<u32>,
    original_end_line: Option<u32>,
    modified_start_line: Option<u32>,
    modified_end_line: Option<u32>,
) -> Result<(), String> {
    info!(
        "Unstaging hunk for file: {} in path: {}, line: {}, hunk_index: {:?}, original_range: {:?}-{:?}, modified_range: {:?}-{:?}",
        file_path,
        path,
        line,
        hunk_index,
        original_start_line,
        original_end_line,
        modified_start_line,
        modified_end_line
    );
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;
    let repo_dir = repo.workdir().unwrap_or_else(|| Path::new(&path));
    let normalized_path = file_path.replace('\\', "/");
    if hunk_index.is_none() {
        warn!(
            "Refusing to unstage hunk without precise hunk index: file={}, line={}, original_range={:?}-{:?}, modified_range={:?}-{:?}",
            normalized_path,
            line,
            original_start_line,
            original_end_line,
            modified_start_line,
            modified_end_line
        );
        return Err("取消暂存差异块失败：缺少精确的差异块索引，请重新点击目标块".to_string());
    }

    let diff_output = std::process::Command::new("git")
        .current_dir(repo_dir)
        .arg("diff")
        .arg("--cached")
        .arg("--unified=0")
        .arg("--")
        .arg(&normalized_path)
        .output()
        .map_err(|e| format!("无法执行 git diff --cached：{}", e))?;

    if !diff_output.status.success() {
        let stderr = String::from_utf8_lossy(&diff_output.stderr);
        return Err(format!("取消暂存差异块失败：{}", stderr.trim()));
    }

    let diff_text = String::from_utf8_lossy(&diff_output.stdout);
    let mut header_lines = Vec::<String>::new();
    let mut current_hunk = Vec::<String>::new();
    let mut matched_hunk = Vec::<String>::new();
    let mut fallback_hunk = Vec::<String>::new();
    let mut in_hunk = false;
    let mut hunk_count = 0u32;
    let mut best_match_distance = u32::MAX;
    let mut current_match_distance = u32::MAX;
    let mut current_range_matches = false;
    let mut range_matched = false;
    let mut index_matched = false;

    for raw_line in diff_text.lines() {
        if raw_line.starts_with("@@ ") {
            if in_hunk {
                let current_index = hunk_count.saturating_sub(1);
                if hunk_index == Some(current_index) {
                    index_matched = true;
                    matched_hunk = current_hunk.clone();
                    best_match_distance = current_match_distance;
                } else if hunk_index.is_none()
                    && current_range_matches
                    && (!range_matched || current_match_distance < best_match_distance)
                {
                    // 设计说明：已暂存视图同样保持块级操作，只取消暂存当前胶囊对应的 hunk。
                    range_matched = true;
                    matched_hunk = current_hunk.clone();
                    best_match_distance = current_match_distance;
                } else if !range_matched
                    && hunk_index.is_none()
                    && current_match_distance <= HUNK_LINE_MATCH_TOLERANCE
                    && current_match_distance < best_match_distance
                {
                    fallback_hunk = current_hunk.clone();
                    best_match_distance = current_match_distance;
                }
            }
            in_hunk = true;
            hunk_count = hunk_count.saturating_add(1);
            current_hunk.clear();
            current_hunk.push(raw_line.to_string());
            if let Some(ranges) = parse_unified_hunk_ranges(raw_line) {
                current_match_distance = line_distance_to_unified_hunk(line, ranges);
                current_range_matches = unified_hunk_matches_selection(
                    ranges,
                    original_start_line,
                    original_end_line,
                    modified_start_line,
                    modified_end_line,
                );
            } else {
                current_match_distance = u32::MAX;
                current_range_matches = false;
            }
        } else if in_hunk {
            current_hunk.push(raw_line.to_string());
        } else {
            header_lines.push(raw_line.to_string());
        }
    }

    if in_hunk {
        let current_index = hunk_count.saturating_sub(1);
        if hunk_index == Some(current_index) {
            index_matched = true;
            matched_hunk = current_hunk;
            best_match_distance = current_match_distance;
        } else if hunk_index.is_none()
            && current_range_matches
            && (!range_matched || current_match_distance < best_match_distance)
        {
            range_matched = true;
            matched_hunk = current_hunk;
            best_match_distance = current_match_distance;
        } else if !range_matched
            && hunk_index.is_none()
            && current_match_distance <= HUNK_LINE_MATCH_TOLERANCE
            && current_match_distance < best_match_distance
        {
            fallback_hunk = current_hunk;
            best_match_distance = current_match_distance;
        }
    }

    if hunk_index.is_some() && !index_matched {
        warn!(
            "Requested hunk index not found for unstaging: file={}, hunk_index={:?}, hunk_count={}",
            normalized_path, hunk_index, hunk_count
        );
    }

    if matched_hunk.is_empty() && !fallback_hunk.is_empty() {
        matched_hunk = fallback_hunk;
    }

    if matched_hunk.is_empty() {
        warn!(
            "No matching staged hunk found for file: {}, line: {}, hunk_count: {}",
            normalized_path, line, hunk_count
        );
        return Err(format!("取消暂存差异块失败：未找到第 {} 行附近的差异块", line));
    }

    debug!(
        "Matched hunk for unstaging: file={}, line={}, hunk_index={:?}, header={}, index_matched={}, range_matched={}, distance={}",
        normalized_path,
        line,
        hunk_index,
        matched_hunk.first().map(String::as_str).unwrap_or("").trim(),
        index_matched,
        range_matched,
        best_match_distance
    );

    let mut patch = String::new();
    for item in header_lines {
        patch.push_str(&item);
        patch.push('\n');
    }
    let precise_hunk = crop_hunk_to_selection(
        &matched_hunk,
        original_start_line,
        original_end_line,
        modified_start_line,
        modified_end_line,
        "取消暂存",
    )?;
    for item in precise_hunk {
        patch.push_str(&item);
        patch.push('\n');
    }
    info!(
        "Unstage hunk patch prepared: file={}, hunk_index={:?}, patch_lines={}, patch_preview={}",
        normalized_path,
        hunk_index,
        patch.lines().count(),
        patch.lines().take(8).collect::<Vec<_>>().join(" | ")
    );

    let mut child = std::process::Command::new("git")
        .current_dir(repo_dir)
        .arg("apply")
        .arg("--cached")
        .arg("--reverse")
        .arg("--whitespace=nowarn")
        .arg("--unidiff-zero")
        .arg("--recount")
        .arg("-p1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法执行 git apply --cached --reverse：{}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(patch.as_bytes())
            .map_err(|e| format!("写入取消暂存补丁失败：{}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("执行 git apply --cached --reverse 失败：{}", e))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = if stdout.trim().is_empty() {
            stderr.trim().to_string()
        } else if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("{}\n{}", stdout.trim_end(), stderr.trim_start())
        };
        error!("git apply --cached --reverse failed: {}", msg);
        return Err(format!("取消暂存差异块失败：{}", msg));
    }

    info!("Hunk unstaged successfully");
    Ok(())
}

#[tauri::command]
pub fn get_commit_diff(path: String, commit_id: String) -> Result<String, String> {
    info!("Getting diff for commit: {} in path: {}", commit_id, path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let commit = repo
        .find_commit(git2::Oid::from_str(&commit_id).map_err(|e| {
            error!("Failed to parse commit ID: {}", e);
            e.to_string()
        })?)
        .map_err(|e| {
            error!("Failed to find commit: {}", e);
            e.to_string()
        })?;

    // 获取父提交（如果有）
    let parent = if commit.parent_count() > 0 {
        Some(commit.parent(0).map_err(|e| {
            error!("Failed to get parent commit: {}", e);
            e.to_string()
        })?)
    } else {
        None
    };

    // 获取父提交的树对象
    let parent_tree = parent.as_ref().and_then(|p| p.tree().ok());

    // 获取当前提交的树对象
    let commit_tree = commit.tree().map_err(|e| {
        error!("Failed to get commit tree: {}", e);
        e.to_string()
    })?;

    // 设置差异选项
    let mut diff_options = DiffOptions::new();
    diff_options.context_lines(3); // 显示3行上下文
    diff_options.show_binary(true); // 显示二进制文件差异
    diff_options.include_untracked(true); // 包含未跟踪文件

    // 生成差异
    let diff = repo
        .diff_tree_to_tree(
            parent_tree.as_ref(),
            Some(&commit_tree),
            Some(&mut diff_options),
        )
        .map_err(|e| {
            error!("Failed to generate diff: {}", e);
            e.to_string()
        })?;

    // 格式化差异输出
    let mut output = String::new();
    diff.print(DiffFormat::Patch, |_, _, line| {
        if let Ok(content) = std::str::from_utf8(line.content()) {
            output.push_str(content);
        }
        true
    })
    .map_err(|e| {
        error!("Failed to print diff: {}", e);
        e.to_string()
    })?;

    info!("Successfully generated diff for commit: {}", commit_id);
    Ok(output)
}

#[tauri::command]
pub async fn create_git_window(
    app: tauri::AppHandle,
    label: String,
    title: String,
    workspace_path: String,
    theme: Option<String>,
) -> Result<(), String> {
    // 检查窗口是否已存在
    if app.get_webview_window(&label).is_some() {
        return Err(format!("窗口 {} 已存在", label));
    }

    info!("workspace_path: {}", workspace_path);

    let mut url_qs = format!("workspacePath={}", encode(&workspace_path));

    let theme = theme.unwrap_or_default().trim().to_lowercase();
    if theme == "dark" || theme == "light" {
        url_qs.push_str("&theme=");
        url_qs.push_str(encode(&theme).as_ref());
    }

    let url = WebviewUrl::App(format!("/git?{}", url_qs).into());

    info!("url: {}", url);
    info!("正在打开路径: {}", workspace_path);

    // 创建窗口
    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1200.0, 800.0)
        .min_inner_size(800.0, 600.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    // 短暂延迟后显示窗口
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn checkout_branch(path: String, branch_name: String) -> Result<(), String> {
    info!("Checking out branch: {} for path: {}", branch_name, path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    // 查找本地分支
    let branch = match repo.find_branch(&branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(_) => {
            // 如果本地分支不存在，尝试查找远程分支
            let remote_branch_name = format!("origin/{}", branch_name);
            repo.find_branch(&remote_branch_name, BranchType::Remote)
                .map_err(|e| {
                    error!("Failed to find branch {}: {}", branch_name, e);
                    format!("Branch {} not found: {}", branch_name, e)
                })?
        }
    };

    let reference = branch.into_reference();
    let commit = repo
        .reference_to_annotated_commit(&reference)
        .map_err(|e| {
            error!("Failed to get annotated commit: {}", e);
            e.to_string()
        })?;

    // 设置检出选项
    let mut checkout_builder = CheckoutBuilder::new();
    checkout_builder.force(); // 强制检出，丢弃本地修改
    checkout_builder.remove_untracked(true); // 删除未跟踪的文件

    // 获取 commit 对象并转换为 Object
    let commit_obj = repo.find_commit(commit.id()).map_err(|e| {
        error!("Failed to find commit: {}", e);
        e.to_string()
    })?;

    // 使用 commit 对象进行检出
    repo.checkout_tree(&commit_obj.as_object(), Some(&mut checkout_builder))
        .map_err(|e| {
            error!("Failed to checkout tree: {}", e);
            e.to_string()
        })?;

    // 设置HEAD指向新分支
    repo.set_head(reference.name().unwrap()).map_err(|e| {
        error!("Failed to set HEAD: {}", e);
        e.to_string()
    })?;

    info!("Successfully checked out branch: {}", branch_name);
    Ok(())
}

#[tauri::command]
pub fn get_file_content(
    path: String,
    file_path: String,
    revision: Option<String>,
) -> Result<String, String> {
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to open repository at {}: {}", path, e);
        format!("Failed to open repository: {}", e)
    })?;

    // 处理路径分隔符问题
    let file_path = Path::new(&file_path);
    debug!("Getting content for file: {}", file_path.display());

    // 获取指定版本或工作目录的文件内容
    let content = if let Some(rev) = revision {
        debug!("Getting content from revision: {}", rev);
        // 设计说明：Git 面板需要分别渲染已暂存和未暂存 diff。
        // INDEX 表示当前暂存区快照；未暂存视图用 INDEX -> 工作区，
        // 已暂存视图用 HEAD -> INDEX，才能在部分暂存后显示两份独立条目。
        let tree = if rev.eq_ignore_ascii_case("INDEX") {
            let mut index = repo.index().map_err(|e| {
                error!("Failed to get index for file content: {}", e);
                format!("Failed to get index: {}", e)
            })?;
            let tree_id = index.write_tree().map_err(|e| {
                error!("Failed to write index tree for file content: {}", e);
                format!("Failed to write index tree: {}", e)
            })?;
            repo.find_tree(tree_id).map_err(|e| {
                error!("Failed to find index tree for file content: {}", e);
                format!("Failed to find index tree: {}", e)
            })?
        } else {
            // 获取指定版本的文件内容
            let object = repo.revparse_single(&rev).map_err(|e| {
                error!("Failed to find revision {}: {}", rev, e);
                format!("Failed to find revision {}: {}", rev, e)
            })?;

            object.peel_to_tree().map_err(|e| {
                error!("Failed to peel to tree: {}", e);
                format!("Failed to peel to tree: {}", e)
            })?
        };

        let entry = match tree.get_path(file_path) {
            Ok(entry) => entry,
            Err(e) => {
                if e.code() == git2::ErrorCode::NotFound {
                    return Ok(String::new());
                }
                error!("Failed to get file in revision {}: {}", file_path.display(), e);
                return Err(format!("Failed to get file in revision: {}", file_path.display()));
            }
        };

        let blob = repo.find_blob(entry.id()).map_err(|e| {
            error!("Failed to find blob: {}", e);
            format!("Failed to find blob: {}", e)
        })?;

        String::from_utf8(blob.content().to_vec()).map_err(|e| {
            error!("Failed to convert blob content to UTF-8: {}", e);
            format!("Failed to convert blob content to UTF-8: {}", e)
        })?
    } else {
        debug!("Getting content from working directory");
        // 获取工作目录的文件内容
        let full_path = Path::new(&path).join(file_path);
        match std::fs::read_to_string(&full_path) {
            Ok(v) => v,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(String::new());
                }
                error!("Failed to read file {}: {}", full_path.display(), e);
                return Err(format!("Failed to read file: {}", e));
            }
        }
    };

    debug!("Successfully retrieved content for {}", file_path.display());
    Ok(content)
}

#[tauri::command]
pub fn get_file_diff(path: String, file_path: String) -> Result<String, String> {
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    // 获取HEAD树
    let head = repo.head().map_err(|e| {
        error!("Failed to get HEAD: {}", e);
        e.to_string()
    })?;
    let head_tree = head.peel_to_tree().map_err(|e| {
        error!("Failed to peel to tree: {}", e);
        e.to_string()
    })?;

    // 获取索引
    let mut index = repo.index().map_err(|e| {
        error!("Failed to get index: {}", e);
        e.to_string()
    })?;
    let index_tree = repo
        .find_tree(index.write_tree().map_err(|e| {
            error!("Failed to write tree: {}", e);
            e.to_string()
        })?)
        .map_err(|e| {
            error!("Failed to find tree: {}", e);
            e.to_string()
        })?;

    // 设置差异选项
    let mut diff_options = DiffOptions::new();
    diff_options.pathspec(file_path.clone());
    diff_options.context_lines(3);

    // 生成差异
    let diff = repo
        .diff_tree_to_tree(Some(&head_tree), Some(&index_tree), Some(&mut diff_options))
        .map_err(|e| {
            error!("Failed to generate diff: {}", e);
            e.to_string()
        })?;

    // 格式化差异输出
    let mut output = String::new();
    diff.print(DiffFormat::Patch, |_, _, line| {
        if let Ok(content) = std::str::from_utf8(line.content()) {
            output.push_str(content);
        }
        true
    })
    .map_err(|e| {
        error!("Failed to print diff: {}", e);
        e.to_string()
    })?;

    Ok(output)
}

#[tauri::command]
pub fn get_file_diff_hunks(
    path: String,
    file_path: String,
    staged: bool,
) -> Result<Vec<GitDiffHunk>, String> {
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository for diff hunks: {}", e);
        e.to_string()
    })?;
    let repo_dir = repo.workdir().unwrap_or_else(|| Path::new(&path));
    let normalized_path = file_path.replace('\\', "/");

    let mut command = std::process::Command::new("git");
    command
        .current_dir(repo_dir)
        .arg("diff")
        .arg("--unified=0");
    if staged {
        command.arg("--cached");
    }
    let output = command
        .arg("--")
        .arg(&normalized_path)
        .output()
        .map_err(|e| format!("无法执行 git diff 获取差异块：{}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(
            "git diff --unified=0 failed for hunks: file={}, staged={}, stderr={}",
            normalized_path,
            staged,
            stderr.trim()
        );
        return Err(format!("获取差异块失败：{}", stderr.trim()));
    }

    let diff_text = String::from_utf8_lossy(&output.stdout);
    let mut hunks = Vec::<GitDiffHunk>::new();
    for raw_line in diff_text.lines() {
        if !raw_line.starts_with("@@ ") {
            continue;
        }
        if let Some(ranges) = parse_unified_hunk_ranges(raw_line) {
            hunks.push(GitDiffHunk {
                index: hunks.len() as u32,
                header: raw_line.to_string(),
                old_start: ranges.old_start,
                old_count: ranges.old_count,
                new_start: ranges.new_start,
                new_count: ranges.new_count,
            });
        } else {
            warn!(
                "Failed to parse git diff hunk header: file={}, staged={}, header={}",
                normalized_path,
                staged,
                raw_line
            );
        }
    }

    info!(
        "Loaded git diff hunks: file={}, staged={}, count={}",
        normalized_path,
        staged,
        hunks.len()
    );
    Ok(hunks)
}

#[tauri::command]
pub fn get_commit_files(path: String, commit_id: String) -> Result<Vec<CommitFile>, String> {
    let repo =
        Repository::discover(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

    let commit = repo
        .find_commit(Oid::from_str(&commit_id).map_err(|e| format!("Invalid commit ID: {}", e))?)
        .map_err(|e| format!("Failed to find commit: {}", e))?;

    // 获取父提交（如果有）
    let parent = if commit.parent_count() > 0 {
        Some(
            commit
                .parent(0)
                .map_err(|e| format!("Failed to get parent commit: {}", e))?,
        )
    } else {
        None
    };

    // 获取父提交的树对象
    let parent_tree = parent.as_ref().and_then(|p| p.tree().ok());

    // 获取当前提交的树对象
    let commit_tree = commit
        .tree()
        .map_err(|e| format!("Failed to get commit tree: {}", e))?;

    // 生成差异
    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)
        .map_err(|e| format!("Failed to generate diff: {}", e))?;

    let mut files = Vec::new();

    // 处理差异统计
    diff.foreach(
        &mut |delta, _| {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();

            let status = match delta.status() {
                Delta::Added => "A",
                Delta::Modified => "M",
                Delta::Deleted => "D",
                Delta::Renamed => "R",
                Delta::Copied => "C",
                Delta::Ignored => "I",
                Delta::Untracked => "?",
                Delta::Typechange => "T",
                Delta::Unreadable => "U",
                _ => " ",
            }
            .to_string();

            // 计算增删行数
            let mut additions = 0;
            let mut deletions = 0;
            delta.new_file().id().is_zero().then(|| additions += 1);
            delta.old_file().id().is_zero().then(|| deletions += 1);

            files.push(CommitFile {
                path,
                status,
                additions,
                deletions,
            });
            true
        },
        None,
        None,
        None,
    )
    .map_err(|e| format!("Failed to process diff: {}", e))?;

    Ok(files)
}

#[tauri::command]
pub fn get_commit_file_diff(
    path: String,      // 仓库路径
    commit_id: String, // 提交ID
    file_path: String, // 文件路径
) -> Result<(String, String), String> {
    // 1. 打开Git仓库
    let repo = Repository::discover(&path).map_err(|e| format!("无法打开仓库: {}", e))?;

    // 2. 查找指定提交
    let commit = repo
        .find_commit(Oid::from_str(&commit_id).map_err(|e| format!("无效的提交ID: {}", e))?)
        .map_err(|e| format!("无法找到提交: {}", e))?;

    // 3. 获取父提交(用于对比)
    let parent = if commit.parent_count() > 0 {
        Some(
            commit
                .parent(0)
                .map_err(|e| format!("无法获取父提交: {}", e))?,
        )
    } else {
        None
    };

    // 4. 获取文件在两个提交中的内容
    let get_file_content = |tree: Option<&Tree>| -> Result<String, String> {
        match tree.and_then(|t| t.get_path(Path::new(&file_path)).ok()) {
            Some(entry) => {
                let blob = repo
                    .find_blob(entry.id())
                    .map_err(|e| format!("无法找到blob: {}", e))?;
                String::from_utf8(blob.content().to_vec())
                    .map_err(|e| format!("文件内容解码失败: {}", e))
            }
            None => Ok(String::new()), // 文件不存在返回空
        }
    };

    // 5. 获取原始内容(父提交)和修改后内容(当前提交)
    let original = if let Some(p) = parent.as_ref() {
        get_file_content(Some(&p.tree().map_err(|e| format!("无法获取父树: {}", e))?))?
    } else {
        String::new()
    };

    let modified = get_file_content(Some(
        &commit
            .tree()
            .map_err(|e| format!("无法获取提交树: {}", e))?,
    ))?;

    Ok((original, modified))
}
