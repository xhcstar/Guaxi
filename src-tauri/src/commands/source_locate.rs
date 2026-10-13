//! 源码定位：把调试信息里的源文件路径映射到**本机或远程编译机**上的真实源码。
//!
//! 现实里调试信息里的路径几乎从不等于源码在你机器上的路径：
//! - **out-of-tree / 原地构建**：产物就在工程目录里，路径基本对得上；
//! - **chroot 构建**：工程被挂到 chroot 内的 `/build`，调试信息里就是 `/build/usr/click/...`；
//! - **docker 构建**：容器内的构建根（`/build`、`/src`、`/work`…）同理；
//! - **源码根本不在本机**：源码躺在 Linux 编译机上，得用 SSH 过去读。
//!
//! 这里提供几种"规则"来消化这些差异，默认用 `auto` 逐个试：
//! 1. **直接**：`debug_path` 原样；
//! 2. **后缀匹配**（最有用）：从左侧逐段砍掉调试路径的前缀，看剩下的是否在源码根下存在
//!    —— 不需要知道构建根叫 `/build` 还是 `/src`；
//! 3. **前缀替换**：把常见构建根（`/build` `/src` `/work` `/root` `/home/*`）换成源码根；
//! 4. **按文件名搜索**：在源码根下按 basename 递归找（有深度与数量上限，找不到再报）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle};

use crate::commands::remote_terminal_tools::ssh_read_text_file;

/// docker/chroot 构建常见的容器内构建根。
const COMMON_BUILD_ROOTS: &[&str] = &[
    "/build", "/builddir", "/src", "/source", "/work", "/workdir", "/workspace", "/code", "/app",
    "/root", "/opt",
];

/// 搜索规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    Direct,
    Suffix,
    Prefix,
    Search,
    Auto,
}

fn parse_rule(s: &str) -> Rule {
    match s {
        "direct" => Rule::Direct,
        "suffix" => Rule::Suffix,
        "prefix" => Rule::Prefix,
        "search" => Rule::Search,
        _ => Rule::Auto,
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocateReq {
    /// 调试信息里的源文件路径（`-stack-info-frame` 的 fullname/file）。
    pub debug_path: String,
    /// 源码根：本机绝对路径，或远端编译机上的目录。
    pub root: String,
    /// 规则：auto（默认）| direct | suffix | prefix | search。
    #[serde(default)]
    pub rule: Option<String>,
    /// prefix 规则的目标前缀；留空则用 root。
    #[serde(default)]
    pub to_prefix: Option<String>,
    /// 给了就按"远端编译机"处理（用该 SSH 档案去探测/读取）。
    #[serde(default)]
    pub profile_id: Option<String>,
    /// 递归搜索的深度上限（默认 8）。
    #[serde(default)]
    pub max_depth: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocateResult {
    pub found: bool,
    /// 命中的路径（远端模式下是远端路径）。
    pub path: String,
    /// 命中的规则名，便于用户理解是怎么找到的。
    pub rule: String,
    /// 尝试过的候选路径（诊断用）。
    pub tried: Vec<String>,
    /// 给用户看的下一步建议。
    pub hint: String,
}

fn join(root: &str, rel: &str) -> String {
    let r = root.trim_end_matches(['/', '\\']);
    format!("{r}/{rel}")
}

/// 调试路径按"从左侧砍段"生成候选：`/build/usr/a/b.c` →
/// `build/usr/a/b.c`, `usr/a/b.c`, `a/b.c`, `b.c`。
fn suffix_candidates(debug_path: &str) -> Vec<String> {
    let norm = debug_path.replace('\\', "/");
    let parts: Vec<&str> = norm.split('/').filter(|p| !p.is_empty()).collect();
    let mut out = Vec::new();
    for skip in 1..parts.len() {
        out.push(parts[skip..].join("/"));
    }
    out
}

/// 常见构建根的替换候选：`/build/usr/a/b.c` → `<root>/usr/a/b.c`。
fn prefix_candidates(debug_path: &str, root_prefix: &str) -> Vec<String> {
    let norm = debug_path.replace('\\', "/");
    let mut out = Vec::new();
    for base in COMMON_BUILD_ROOTS {
        if let Some(rest) = norm.strip_prefix(base) {
            out.push(join(root_prefix, rest.trim_start_matches('/')));
        }
    }
    // 也试一下 root_prefix 直接拼 basename 的情况（有些只留了文件名）。
    if let Some(name) = norm.rsplit('/').next() {
        out.push(join(root_prefix, name));
    }
    out
}

/// 本机探测：返回第一个存在的候选。
fn probe_local(root: &str, cands: &[String]) -> Option<String> {
    for c in cands {
        let p = PathBuf::from(c);
        let abs = if p.is_absolute() { p } else { Path::new(root).join(c) };
        if abs.is_file() {
            return Some(abs.to_string_lossy().to_string());
        }
    }
    None
}

/// 本机按 basename 递归搜索（有深度/数量上限，避免在大工程里卡死）。
fn search_local(root: &str, name: &str, max_depth: u32) -> Vec<String> {
    let mut hits = Vec::new();
    let mut stack: Vec<(PathBuf, u32)> = vec![(PathBuf::from(root), 0)];
    let skip_dirs = ["node_modules", ".git", ".svn", "target", "build", "out", ".cache"];
    while let Some((dir, depth)) = stack.pop() {
        if depth > max_depth || hits.len() >= 5 {
            break;
        }
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for e in rd.flatten() {
            let p = e.path();
            let fname = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if fname.starts_with('.') || skip_dirs.contains(&fname.as_str()) {
                    continue;
                }
                stack.push((p, depth + 1));
            } else if fname == name {
                hits.push(p.to_string_lossy().to_string());
            }
        }
    }
    hits
}

/// 远端探测：一次 exec 里把所有候选都 `test -f` 一遍，省往返。
async fn probe_remote(
    app: &AppHandle,
    profile_id: &str,
    root: &str,
    cands: &[String],
) -> Result<Option<String>, String> {
    if cands.is_empty() {
        return Ok(None);
    }
    let list = cands
        .iter()
        .map(|c| {
            let p = if c.starts_with('/') { c.clone() } else { join(root, c) };
            format!("\"{}\"", p.replace('"', "\\\""))
        })
        .collect::<Vec<_>>()
        .join(" ");
    // 逐个 test -f，命中就打印并停止。
    let cmd = format!("for f in {list}; do if [ -f \"$f\" ]; then echo \"$f\"; break; fi; done");
    let out = crate::commands::remote_terminal_tools::ssh_exec_capture_quiet(app, profile_id, &cmd)
        .await?;
    let hit = out.lines().map(|l| l.trim()).find(|l| !l.is_empty());
    Ok(hit.map(|s| s.to_string()))
}

/// 远端按 basename 搜索（find，限深并排除常见产物目录）。
async fn search_remote(
    app: &AppHandle,
    profile_id: &str,
    root: &str,
    name: &str,
    max_depth: u32,
) -> Result<Vec<String>, String> {
    let cmd = format!(
        "find \"{root}\" -maxdepth {max_depth} -type d \\( -name node_modules -o -name .git -o -name .svn -o -name target -o -name out -o -name .cache \\) -prune -o -type f -name \"{name}\" -print 2>/dev/null | head -5"
    );
    let out = crate::commands::remote_terminal_tools::ssh_exec_capture_quiet(app, profile_id, &cmd)
        .await?;
    Ok(out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// 按规则定位源码。**不下发任何东西**，只做路径解析与存在性探测。
#[command]
pub async fn cmd_source_locate(app: AppHandle, req: LocateReq) -> Result<LocateResult, String> {
    let debug = req.debug_path.trim();
    if debug.is_empty() {
        return Err("调试路径为空".into());
    }
    let root = req.root.trim();
    if root.is_empty() {
        return Err("请先选择源码工程根目录".into());
    }
    let rule = parse_rule(req.rule.as_deref().unwrap_or("auto"));
    let to_prefix = req
        .to_prefix
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(root)
        .to_string();
    let depth = req.max_depth.unwrap_or(8);
    let name = debug
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(debug)
        .to_string();

    let mut tried: Vec<String> = Vec::new();
    let remote = req.profile_id.as_deref().filter(|s| !s.is_empty());

    // 组装候选（按规则；auto 会依次叠加更多策略）。
    let mut cands: Vec<String> = Vec::new();
    match rule {
        Rule::Direct => cands.push(debug.to_string()),
        Rule::Suffix => cands.extend(suffix_candidates(debug)),
        Rule::Prefix => cands.extend(prefix_candidates(debug, &to_prefix)),
        Rule::Search => {}
        Rule::Auto => {
            cands.push(debug.to_string());
            cands.extend(suffix_candidates(debug));
            cands.extend(prefix_candidates(debug, &to_prefix));
        }
    }
    // 去重保序
    let mut seen = std::collections::HashSet::new();
    cands.retain(|c| seen.insert(c.clone()));
    tried.extend(cands.iter().cloned());

    let hit = if let Some(pid) = remote {
        probe_remote(&app, pid, root, &cands).await?
    } else {
        probe_local(root, &cands)
    };

    if let Some(p) = hit {
        let used = if rule == Rule::Auto {
            if p == debug || p.replace('\\', "/") == debug.replace('\\', "/") {
                "直接匹配"
            } else if p.ends_with(&name) && !p.contains(&debug.replace('\\', "/")) {
                "后缀/前缀匹配"
            } else {
                "规则匹配"
            }
        } else {
            "指定规则"
        };
        return Ok(LocateResult {
            found: true,
            path: p,
            rule: used.to_string(),
            tried,
            hint: String::new(),
        });
    }

    // 都没命中：按 basename 搜一把（auto/search 都会走到这）。
    if matches!(rule, Rule::Auto | Rule::Search) {
        let hits = if let Some(pid) = remote {
            search_remote(&app, pid, root, &name, depth).await?
        } else {
            search_local(root, &name, depth)
        };
        if hits.len() == 1 {
            return Ok(LocateResult {
                found: true,
                path: hits[0].clone(),
                rule: "按文件名搜索".to_string(),
                tried,
                hint: String::new(),
            });
        }
        if hits.len() > 1 {
            return Ok(LocateResult {
                found: false,
                path: String::new(),
                rule: "按文件名搜索".to_string(),
                tried: hits,
                hint: format!(
                    "在源码根下找到 {name} 的多个匹配，无法确定用哪个。请把源码根缩小到具体子目录，或改用「前缀替换」规则。"
                ),
            });
        }
    }

    Ok(LocateResult {
        found: false,
        path: String::new(),
        rule: "未命中".to_string(),
        tried,
        hint: format!(
            "没能在 {root} 下找到 {debug}。\n\
             · 若源码在编译机上：把「源码来源」切到远端并选对目录；\n\
             · 若是 chroot/docker 构建：试「前缀替换」规则，把构建根（如 /build）换成源码根；\n\
             · 也可以直接用「按文件名搜索」。"
        ),
    })
}

/// 读源码正文：远端走 SSH，本地直接读文件。
#[command]
pub async fn cmd_source_read(
    app: AppHandle,
    path: String,
    profile_id: Option<String>,
    max_bytes: Option<u64>,
) -> Result<String, String> {
    match profile_id.filter(|s| !s.is_empty()) {
        Some(pid) => ssh_read_text_file(app, pid, path, max_bytes).await,
        None => {
            let cap = max_bytes.unwrap_or(1_048_576) as usize;
            let data = std::fs::read(&path).map_err(|e| format!("读取源码失败 {path}: {e}"))?;
            let cut = data.len().min(cap);
            Ok(String::from_utf8_lossy(&data[..cut]).into_owned())
        }
    }
}

/// 远端列目录（只列子目录，供"选远端源码根"用）。`ls -1p` 尾部带 `/` 的就是目录。
#[command]
pub async fn ssh_list_dirs(
    app: AppHandle,
    profile_id: String,
    path: String,
) -> Result<Vec<String>, String> {
    let p = if path.trim().is_empty() { ".".to_string() } else { path.trim().to_string() };
    let cmd = format!(
        "cd \"{}\" 2>/dev/null && pwd && ls -1p 2>/dev/null",
        p.replace('"', "\\\"")
    );
    let out = crate::commands::remote_terminal_tools::ssh_exec_capture_quiet(&app, &profile_id, &cmd)
        .await?;
    let mut lines = out.lines();
    let _cwd = lines.next();
    Ok(lines
        .map(|l| l.trim().to_string())
        .filter(|l| l.ends_with('/') && l.len() > 1)
        .map(|l| l.trim_end_matches('/').to_string())
        .collect())
}

// ============================ 测试（纯逻辑，可离线跑） ============================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffix_candidates_cover_chroot_docker_layouts() {
        // 截图里的真实情形：调试信息是 /build/usr/click/lib/libssl_cli/ssl_cli.c，
        // 而本地工程根下就是 usr/click/lib/libssl_cli/ssl_cli.c。
        let c = suffix_candidates("/build/usr/click/lib/libssl_cli/ssl_cli.c");
        assert!(
            c.contains(&"usr/click/lib/libssl_cli/ssl_cli.c".to_string()),
            "应能从左侧砍段得到相对工程根的路径: {c:?}"
        );
        assert!(c.contains(&"ssl_cli.c".to_string()));
        // docker 的 /src 布局同理
        let d = suffix_candidates("/src/app/main.c");
        assert!(d.contains(&"app/main.c".to_string()), "{d:?}");
    }

    #[test]
    fn prefix_candidates_replace_common_build_roots() {
        let c = prefix_candidates("/build/usr/a.c", "/repo");
        assert!(c.contains(&"/repo/usr/a.c".to_string()), "{c:?}");
        let d = prefix_candidates("/src/x/y.c", "/home/dev/proj");
        assert!(d.contains(&"/home/dev/proj/x/y.c".to_string()), "{d:?}");
    }

    #[test]
    fn probe_local_finds_file_by_suffix() {
        // 造一个临时工程：<tmp>/usr/click/lib/x.c
        let tmp = std::env::temp_dir().join(format!("guaxi_src_{}", std::process::id()));
        let deep = tmp.join("usr").join("click").join("lib");
        std::fs::create_dir_all(&deep).unwrap();
        let file = deep.join("x.c");
        std::fs::write(&file, "int main(){}").unwrap();

        let root = tmp.to_string_lossy().to_string();
        let cands = suffix_candidates("/build/usr/click/lib/x.c");
        let hit = probe_local(&root, &cands);
        assert!(hit.is_some(), "应能在工程根下按后缀匹配命中");
        assert!(hit.unwrap().replace('\\', "/").ends_with("usr/click/lib/x.c"));

        // 直接规则（原样路径）在这种布局下应该找不到
        assert!(probe_local(&root, &["/build/usr/click/lib/x.c".to_string()]).is_none());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn search_local_finds_by_basename() {
        let tmp = std::env::temp_dir().join(format!("guaxi_srch_{}", std::process::id()));
        let deep = tmp.join("a").join("b");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("needle.c"), "x").unwrap();
        let hits = search_local(&tmp.to_string_lossy(), "needle.c", 6);
        assert_eq!(hits.len(), 1, "{hits:?}");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
