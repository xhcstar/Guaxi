use git2::{
    build::CheckoutBuilder, BranchType, DiffFormat, DiffOptions, FetchOptions, PushOptions,
    RemoteCallbacks, Repository, StatusOptions, StatusShow, RepositoryOpenFlags
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};

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
        RepositoryOpenFlags::NO_SEARCH,  // 关键标志，禁止向上查找
        Vec::<&Path>::new()             // 不需要额外的 git 目录
    ).map_err(|e| format!("Failed to open repository: {}", e))?;
    
    repo.workdir()
        .ok_or("Bare repository has no working directory".to_string())?
        .to_str()
        .ok_or("Path contains invalid UTF-8".to_string())
        .map(|s| s.to_string())
}

#[tauri::command]
pub fn get_git_status(path: String) -> Result<Vec<GitChange>, String> {
    // 同样使用 NO_SEARCH 标志
    let repo = Repository::open_ext(
        &path,
        RepositoryOpenFlags::NO_SEARCH,
        Vec::<&Path>::new()
    ).map_err(|e| format!("Failed to open repository: {}", e))?;
 
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
            Some(p) => Path::new(p).to_str().unwrap_or("").to_string(),
            None => continue,
        };

        let status = match entry.status() {
            s if s.is_index_new() => "A",
            s if s.is_index_modified() => "M",
            s if s.is_index_deleted() => "D",
            s if s.is_wt_new() => "?",
            s if s.is_wt_modified() => "M",
            s if s.is_wt_deleted() => "D",
            _ => " ",
        }
        .to_string();

        let diff = match get_file_diff_safe(&repo, &path) {
            Ok(d) => d,
            Err(e) => {
                warn!("Failed to get diff for {}: {}", path, e);
                String::new()
            }
        };

        changes.push(GitChange {
            path,
            status,
            lines_added: Vec::new(),
            lines_modified: Vec::new(),
            lines_deleted: Vec::new(),
            diff,
        });
    }

    Ok(changes)
}

fn get_file_diff_safe(repo: &Repository, path: &str) -> Result<String, String> {
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return Ok(String::new()),
    };

    let head_tree = match head.peel_to_tree() {
        Ok(t) => t,
        Err(_) => return Ok(String::new()),
    };

    let mut diff_opts = DiffOptions::new();
    diff_opts.context_lines(3);
    diff_opts.pathspec(path);

    let diff = repo
        .diff_tree_to_workdir(Some(&head_tree), Some(&mut diff_opts))
        .map_err(|e| format!("Diff error: {}", e))?;

    let mut output = String::new();
    diff.print(DiffFormat::Patch, |_, _, line| {
        if let Ok(content) = std::str::from_utf8(line.content()) {
            output.push_str(content);
        }
        true
    })
    .map_err(|e| format!("Diff print error: {}", e))?;

    Ok(output)
}

#[tauri::command]
pub fn get_file_diff(path: String, file_path: String) -> Result<String, String> {
    let repo = Repository::discover(&path).map_err(|e| e.to_string())?;
    get_file_diff_safe(&repo, &file_path)
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
        })
        .collect())
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
