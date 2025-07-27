use git2::{
    build::CheckoutBuilder, BranchType, DiffFormat, DiffOptions, FetchOptions, Index, IndexEntry,
    ObjectType, PushOptions, RemoteCallbacks, Repository, RepositoryOpenFlags, StatusOptions,
    StatusShow, TreeEntry,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};
use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use urlencoding::encode;

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

    let mut remote = repo.find_remote("origin").map_err(|e| {
        error!("Failed to find remote 'origin': {}", e);
        e.to_string()
    })?;

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username, _| {
        let key_path = std::env::var("HOME")
            .map(|home| PathBuf::from(home).join(".ssh").join("id_rsa"))
            .unwrap_or_else(|_| PathBuf::from("id_rsa"));

        debug!("Using SSH key at: {:?}", key_path);

        git2::Cred::ssh_key(username.unwrap(), None, &key_path, None)
    });

    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);

    remote
        .push(&["refs/heads/*:refs/heads/*"], Some(&mut push_options))
        .map_err(|e| {
            error!("Failed to push: {}", e);
            e.to_string()
        })?;

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

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username, _| {
        let key_path = std::env::var("HOME")
            .map(|home| PathBuf::from(home).join(".ssh").join("id_rsa"))
            .unwrap_or_else(|_| PathBuf::from("id_rsa"));

        debug!("Using SSH key at: {:?}", key_path);

        git2::Cred::ssh_key(username.unwrap(), None, &key_path, None)
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
pub fn checkout_branch(path: String, branch_name: String) -> Result<(), String> {
    info!("Checking out branch: {} for path: {}", branch_name, path);
    let repo = Repository::discover(&path).map_err(|e| {
        error!("Failed to discover repository: {}", e);
        e.to_string()
    })?;

    let branch = repo
        .find_branch(&branch_name, BranchType::Local)
        .map_err(|e| {
            error!("Failed to find branch {}: {}", branch_name, e);
            e.to_string()
        })?;

    let reference = branch.into_reference();
    let commit = repo
        .reference_to_annotated_commit(&reference)
        .map_err(|e| {
            error!("Failed to get annotated commit: {}", e);
            e.to_string()
        })?;

    let commit_obj = repo.find_commit(commit.id()).map_err(|e| {
        error!("Failed to find commit: {}", e);
        e.to_string()
    })?;

    repo.checkout_tree(
        &commit_obj.as_object(),
        Some(CheckoutBuilder::default().force()),
    )
    .map_err(|e| {
        error!("Failed to checkout tree: {}", e);
        e.to_string()
    })?;

    repo.set_head(reference.name().unwrap()).map_err(|e| {
        error!("Failed to set HEAD: {}", e);
        e.to_string()
    })?;

    info!("Checkout successful");
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
        let mut index_entry = IndexEntry {
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

    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(|e| {
            error!("Failed to add all to index: {}", e);
            e.to_string()
        })?;

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
) -> Result<(), String> {
    // 检查窗口是否已存在
    if app.get_webview_window(&label).is_some() {
        return Err(format!("窗口 {} 已存在", label));
    }

    info!("workspace_path: {}", workspace_path);

    // 构建URL查询参数
    let url = WebviewUrl::App(
        format!(
            "/git?workspacePath={}",
            encode(&workspace_path),
        )
        .into(),
    );

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