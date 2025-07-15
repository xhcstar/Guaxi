use git2::{
    build::CheckoutBuilder, BranchType, DiffOptions, FetchOptions, PushOptions, RemoteCallbacks,
    Repository, Status, StatusOptions, StatusShow,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct GitChange {
    path: String,
    status: String,
    lines_added: Vec<u32>,
    lines_modified: Vec<u32>,
    lines_deleted: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitBranch {
    name: String,
    is_current: bool,
}

#[tauri::command]
pub fn get_git_root(path: String) -> Result<String, String> {
    Repository::discover(&path)
        .map_err(|e| format!("Not a git repository: {}", e))
        .map(|repo| repo.path().parent().unwrap().to_str().unwrap().to_string())
}

#[tauri::command]
pub fn get_git_status(path: String) -> Result<Vec<GitChange>, String> {
    let repo = Repository::discover(&path).map_err(|e| e.to_string())?;
    let mut options = StatusOptions::new();
    options.include_untracked(true);
    options.recurse_untracked_dirs(true);
    options.show(StatusShow::IndexAndWorkdir);

    let statuses = repo
        .statuses(Some(&mut options))
        .map_err(|e| e.to_string())?;
    let mut changes = Vec::new();

    for entry in statuses.iter() {
        // 安全处理 path() 可能返回 None 的情况
        let path = match entry.path() {
            Some(p) => p.to_string(),
            None => continue, // 跳过没有路径的条目
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

        let diff = repo
            .diff_index_to_workdir(None, Some(DiffOptions::new().context_lines(0)))
            .map_err(|e| e.to_string())?;

        let mut lines_added = Vec::new();
        let mut lines_modified = Vec::new();
        let mut lines_deleted = Vec::new();

        diff.foreach(
            &mut |delta, _| {
                if let Some(new_file) = delta.new_file().path() {
                    new_file.to_str().map_or(false, |p| p == path)
                } else {
                    false
                }
            },
            None,
            None,
            Some(&mut |delta, maybe_hunk, line| {
                if let Some(new_file) = delta.new_file().path() {
                    if new_file.to_str().map_or(false, |p| p == path) {
                        match line.origin() {
                            '+' => lines_added.push(line.new_lineno().unwrap_or(0)),
                            '-' => lines_deleted.push(line.old_lineno().unwrap_or(0)),
                            ' ' => (),
                            _ => lines_modified
                                .push(line.new_lineno().unwrap_or(line.old_lineno().unwrap_or(0))),
                        }
                    }
                }
                true
            }),
        )
        .map_err(|e| e.to_string())?;

        changes.push(GitChange {
            path,
            status,
            lines_added,
            lines_modified,
            lines_deleted,
        });
    }

    Ok(changes)
}

#[tauri::command]
pub fn get_current_branch(path: String) -> Result<String, String> {
    let repo = Repository::discover(&path).map_err(|e| e.to_string())?;
    let head = repo.head().map_err(|e| e.to_string())?;
    head.shorthand()
        .map(|s| s.to_string())
        .ok_or_else(|| "No branch found".to_string())
}

#[tauri::command]
pub fn list_branches(path: String) -> Result<Vec<GitBranch>, String> {
    let repo = Repository::discover(&path).map_err(|e| e.to_string())?;
    let branches = repo
        .branches(Some(BranchType::Local))
        .map_err(|e| e.to_string())?;
    let current_branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()));

    let mut result = Vec::new();
    for branch in branches {
        let (branch, _) = branch.map_err(|e| e.to_string())?;
        let name = branch
            .name()
            .map_err(|e| e.to_string())?
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

    Ok(result)
}

#[tauri::command]
pub fn git_commit(path: String, message: String) -> Result<(), String> {
    let repo = Repository::discover(&path).map_err(|e| e.to_string())?;
    let mut index = repo.index().map_err(|e| e.to_string())?;
    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;

    let signature = repo.signature().map_err(|e| e.to_string())?;
    let head = repo.head().map_err(|e| e.to_string())?;
    let parent_commit = repo
        .find_commit(head.target().unwrap())
        .map_err(|e| e.to_string())?;

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        &[&parent_commit],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn git_push(path: String) -> Result<(), String> {
    let repo = Repository::discover(&path).map_err(|e| e.to_string())?;
    let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username, _| {
        git2::Cred::ssh_key(
            username.unwrap(),
            None,
            Path::new(&format!("{}/.ssh/id_rsa", std::env::var("HOME").unwrap())),
            None,
        )
    });

    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);

    remote
        .push(&["refs/heads/*:refs/heads/*"], Some(&mut push_options))
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn git_pull(path: String) -> Result<(), String> {
    let repo = Repository::discover(&path).map_err(|e| e.to_string())?;
    let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username, _| {
        git2::Cred::ssh_key(
            username.unwrap(),
            None,
            Path::new(&format!("{}/.ssh/id_rsa", std::env::var("HOME").unwrap())),
            None,
        )
    });

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    remote
        .fetch(
            &["refs/heads/*:refs/heads/*"],
            Some(&mut fetch_options),
            None,
        )
        .map_err(|e| e.to_string())?;

    let fetch_head = repo
        .find_reference("FETCH_HEAD")
        .map_err(|e| e.to_string())?;
    let fetch_commit = repo
        .reference_to_annotated_commit(&fetch_head)
        .map_err(|e| e.to_string())?;

    let analysis = repo
        .merge_analysis(&[&fetch_commit])
        .map_err(|e| e.to_string())?;
    if analysis.0.is_up_to_date() {
        return Ok(());
    } else if analysis.0.is_fast_forward() {
        let mut reference = repo
            .find_reference("refs/heads/main")
            .map_err(|e| e.to_string())?;
        reference
            .set_target(fetch_commit.id(), "Fast-forward")
            .map_err(|e| e.to_string())?;
        repo.set_head("refs/heads/main")
            .map_err(|e| e.to_string())?;
        repo.checkout_head(Some(CheckoutBuilder::default().force()))
            .map_err(|e| e.to_string())?;
    } else {
        return Err("Merge required".into());
    }

    Ok(())
}

#[tauri::command]
pub fn checkout_branch(path: String, branch_name: String) -> Result<(), String> {
    let repo = Repository::discover(&path).map_err(|e| e.to_string())?;
    let branch = repo
        .find_branch(&branch_name, BranchType::Local)
        .map_err(|e| e.to_string())?;
    let reference = branch.into_reference();
    let commit = repo
        .reference_to_annotated_commit(&reference)
        .map_err(|e| e.to_string())?;

    let commit_obj = repo.find_commit(commit.id()).map_err(|e| e.to_string())?;

    repo.checkout_tree(
        &commit_obj.as_object(),
        Some(CheckoutBuilder::default().force()),
    )
    .map_err(|e| e.to_string())?;

    repo.set_head(reference.name().unwrap())
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn create_branch(path: String, branch_name: String) -> Result<(), String> {
    let repo = Repository::discover(&path).map_err(|e| e.to_string())?;
    let head = repo.head().map_err(|e| e.to_string())?;
    let commit = repo
        .find_commit(head.target().unwrap())
        .map_err(|e| e.to_string())?;

    repo.branch(&branch_name, &commit, false)
        .map_err(|e| e.to_string())?;

    Ok(())
}
