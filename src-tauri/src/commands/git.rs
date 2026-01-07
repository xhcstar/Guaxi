use git2::{
    build::CheckoutBuilder, BranchType, Cred, CredentialType, Delta, DiffFormat, DiffOptions,
    FetchOptions, IndexEntry, Oid, PushOptions, RemoteCallbacks, Repository, RepositoryOpenFlags,
    StatusOptions, StatusShow, Tree,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tracing::{debug, error, info, warn};
use urlencoding::encode;

use super::settingview;

fn is_ignored_git_path(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p == ".meta" || p.starts_with(".meta/") || p.contains("/.meta/")
}

fn normalize_remote_url(raw: &str) -> String {
    let mut s = raw.trim().to_string();
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
pub struct GitBranch {
    name: String,
    is_current: bool,
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
pub fn get_git_status(path: String) -> Result<Vec<GitChange>, String> {
    let repo =
        Repository::discover(&path).map_err(|e| format!("Failed to open repository: {}", e))?;

    let mut options = StatusOptions::new();
    options.include_untracked(true);
    options.recurse_untracked_dirs(true);
    options.show(StatusShow::IndexAndWorkdir);

    let statuses = repo
        .statuses(Some(&mut options))
        .map_err(|e| format!("Failed to get statuses: {}", e))?;

    let mut changes = Vec::new();

    for entry in statuses.iter() {
        let path = match entry.path() {
            Some(p) => p.to_string(),
            None => continue,
        };

        if is_ignored_git_path(&path) {
            continue;
        }

        // 更精确的状态判断
        let (status, staged) = if entry.status().is_index_new() {
            ("A", true) // 已暂存的新文件
        } else if entry.status().is_index_modified() {
            ("M", true) // 已暂存的修改
        } else if entry.status().is_index_deleted() {
            ("D", true) // 已暂存的删除
        } else if entry.status().is_wt_new() {
            ("?", false) // 未跟踪文件
        } else if entry.status().is_wt_modified() {
            ("M", false) // 工作区修改但未暂存
        } else if entry.status().is_wt_deleted() {
            ("D", false) // 工作区删除但未暂存
        } else if entry.status().is_wt_renamed() {
            ("R", false) // 重命名
        } else if entry.status().is_wt_typechange() {
            ("T", false) // 类型变更
        } else {
            (" ", false) // 其他状态
        };

        changes.push(GitChange {
            path,
            status: status.to_string(),
            lines_added: Vec::new(),
            lines_modified: Vec::new(),
            lines_deleted: Vec::new(),
            diff: String::new(),
            staged, // 添加 staged 字段
        });
    }

    Ok(changes)
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
        .unwrap_or_else(|| "master".to_string());

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

    let is_kb_repo = settingview::is_knowledge_bases_root_path(&path);
    let (kb_username, kb_token) = if is_kb_repo {
        settingview::get_knowledge_bases_remote_username_token()
    } else {
        (String::new(), String::new())
    };

    let cfg = repo.config().ok();

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |url, username_from_url, allowed| {
        if is_kb_repo
            && !kb_token.is_empty()
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

    let mut remote = repo.find_remote("origin").map_err(|e| {
        error!("Failed to find remote 'origin': {}", e);
        e.to_string()
    })?;

    let is_kb_repo = settingview::is_knowledge_bases_root_path(&path);
    let (kb_username, kb_token) = if is_kb_repo {
        settingview::get_knowledge_bases_remote_username_token()
    } else {
        (String::new(), String::new())
    };

    let cfg = repo.config().ok();

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |url, username_from_url, allowed| {
        if is_kb_repo
            && !kb_token.is_empty()
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
pub fn get_git_log(path: String, branch_name: String) -> Result<Vec<GitCommit>, String> {
    let repo = Repository::discover(&path).map_err(|e| e.to_string())?;

    let branch = repo
        .find_branch(&branch_name, BranchType::Local)
        .map_err(|e| format!("Failed to find branch {}: {}", branch_name, e))?;

    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;
    revwalk
        .push_ref(branch.get().name().ok_or("Invalid branch name")?)
        .map_err(|e| e.to_string())?;

    let mut commits = Vec::new();
    for oid in revwalk {
        let oid = oid.map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;

        let message = commit.message().unwrap_or_default().to_string();
        let summary = message.lines().next().unwrap_or_default().to_string();

        commits.push(GitCommit {
            id: oid.to_string(),
            message,
            summary,
            author: commit.author().name().unwrap_or_default().to_string(),
            date: commit.time().seconds().to_string(),
        });
    }

    Ok(commits)
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
        // 获取指定版本的文件内容
        let object = repo.revparse_single(&rev).map_err(|e| {
            error!("Failed to find revision {}: {}", rev, e);
            format!("Failed to find revision {}: {}", rev, e)
        })?;

        let tree = object.peel_to_tree().map_err(|e| {
            error!("Failed to peel to tree: {}", e);
            format!("Failed to peel to tree: {}", e)
        })?;

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
