//! 验证「合并内容 + hunk」这条新链路和旧的 `git diff` 子进程口径一致。
//!
//! 为什么值得单独测：两侧文件内容是后端逐字节读出来的（可能含 CRLF），
//! 而 hunk 的范围要给胶囊按钮定位、还要回传给 stage_hunk / discard_hunk。
//! 只要两边口径（是否忽略行尾、上下文行数）不一致，胶囊就会落到错行，
//! 用户点「暂存这个差异块」时暂存的却是另一块。
//!
//! 这里直接构造一个临时仓库，分别用两条路径算 hunk，要求逐项相等。

use std::fs;
use std::path::Path;
use std::process::Command;

use git2::{DiffOptions, Repository};
use tempfile::TempDir;

/// 旧的实现：调用 git 子进程解析 `@@` 头。作为对照基准。
fn hunks_via_git_cli(repo_dir: &Path, file_path: &str) -> Vec<(u32, u32, u32, u32)> {
    let output = Command::new("git")
        .current_dir(repo_dir)
        .arg("diff")
        .arg("--unified=0")
        .arg("--")
        .arg(file_path)
        .output()
        .expect("run git diff");
    assert!(output.status.success(), "git diff failed");

    let text = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();
    for line in text.lines() {
        if !line.starts_with("@@ ") {
            continue;
        }
        // 形如 `@@ -1,2 +1,3 @@`
        let mut old = (0u32, 1u32);
        let mut new = (0u32, 1u32);
        for part in line.split_whitespace() {
            let parse = |raw: &str, slot: &mut (u32, u32)| {
                let body = &raw[1..];
                let mut it = body.split(',');
                if let Some(start) = it.next().and_then(|v| v.parse::<u32>().ok()) {
                    slot.0 = start;
                }
                slot.1 = it.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(1);
            };
            if part.starts_with('-') && part.len() > 1 {
                parse(part, &mut old);
            } else if part.starts_with('+') && part.len() > 1 {
                parse(part, &mut new);
            }
        }
        out.push((old.0, old.1, new.0, new.1));
    }
    out
}

/// 新实现：libgit2 的 diff + 0 行上下文，切出 hunk 范围。
fn hunks_via_libgit2(repo_dir: &Path, file_path: &str) -> Vec<(u32, u32, u32, u32)> {
    let repo = Repository::discover(repo_dir).expect("open repo");
    let index = repo.index().expect("index");
    let mut options = DiffOptions::new();
    options.context_lines(0);
    options.pathspec(file_path);
    let diff = repo
        .diff_index_to_workdir(Some(&index), Some(&mut options))
        .expect("diff index to workdir");

    let mut out = Vec::new();
    let _ = diff.foreach(
        &mut |_, _| true,
        None,
        Some(&mut |_delta, hunk| {
            out.push((
                hunk.old_start() as u32,
                hunk.old_lines() as u32,
                hunk.new_start() as u32,
                hunk.new_lines() as u32,
            ));
            true
        }),
        None,
    );
    out
}

fn init_repo(dir: &Path) {
    let run = |args: &[&str]| {
        let status = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            status.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&status.stderr)
        );
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "user.name", "test"]);
    run(&["config", "core.autocrlf", "false"]);
}

#[test]
fn libgit2_hunks_match_git_cli() {
    let workspace = TempDir::new().expect("temp dir");
    let root = workspace.path();
    init_repo(root);

    let file = root.join("sample.txt");
    let original: String = (1..=40).map(|i| format!("line {}\n", i)).collect();
    fs::write(&file, &original).expect("write original");

    let run = |args: &[&str]| {
        let status = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .expect("run git");
        assert!(status.status.success(), "git {:?}", args);
    };
    run(&["add", "."]);
    run(&["commit", "-qm", "init", "--no-gpg-sign"]);

    // 制造三处彼此分离的改动：修改 3、删除 10、在 25 后插入两行。
    let mut lines: Vec<String> = (1..=40).map(|i| format!("line {}\n", i)).collect();
    lines[2] = "line 3 CHANGED\n".to_string();
    lines.remove(9);
    lines.insert(24, "inserted A\n".to_string());
    lines.insert(25, "inserted B\n".to_string());
    fs::write(&file, lines.concat()).expect("write modified");

    let expected = hunks_via_git_cli(root, "sample.txt");
    let actual = hunks_via_libgit2(root, "sample.txt");

    assert!(!expected.is_empty(), "sanity: git CLI should report hunks");
    assert_eq!(
        actual, expected,
        "libgit2 切出的 hunk 范围必须和 `git diff --unified=0` 完全一致\n实际: {:?}\n期望: {:?}",
        actual, expected
    );
}

#[test]
fn crlf_file_hunks_match_git_cli() {
    // Windows 上仓库里同时存在 CRLF 与 LF 是很常见的情况。
    // 这里故意让工作区文件是 CRLF、HEAD 里是 LF，确认两边口径仍然一致
    // （即：不忽略行尾差异，hunk 覆盖的是真实的行）。
    let workspace = TempDir::new().expect("temp dir");
    let root = workspace.path();
    init_repo(root);

    let file = root.join("crlf.txt");
    let lf: String = (1..=20).map(|i| format!("row {}\n", i)).collect();
    fs::write(&file, &lf).expect("write lf");

    let run = |args: &[&str]| {
        let status = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .expect("run git");
        assert!(status.status.success(), "git {:?}", args);
    };
    run(&["add", "."]);
    run(&["commit", "-qm", "init", "--no-gpg-sign"]);

    // 换成 CRLF，并改掉第 5 行。
    let crlf: String = (1..=20)
        .map(|i| {
            if i == 5 {
                "row 5 CRLF-CHANGED\r\n".to_string()
            } else {
                format!("row {}\r\n", i)
            }
        })
        .collect();
    fs::write(&file, crlf).expect("write crlf");

    let expected = hunks_via_git_cli(root, "crlf.txt");
    let actual = hunks_via_libgit2(root, "crlf.txt");
    assert_eq!(actual, expected, "CRLF 文件的 hunk 也必须与 git CLI 一致");
}
