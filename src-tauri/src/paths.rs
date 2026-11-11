//! 用户数据根目录。
//!
//! 历史写法是把配置、行程计划、任务、资料库都放在 exe 同级目录（绿色版思路）。
//! 但 Windows 的 NSIS 安装器是"覆盖安装 = 先清安装目录"，用户升级一次，
//! `config/guaxi_config.json` 和 `repos/` 就一起没了 —— 表现就是"重装后配置和计划全丢"。
//!
//! 现在统一收口到这里：
//! - 默认放用户级目录（Windows: `%APPDATA%\Guaxi`），卸载程序不会碰它；
//! - exe 同级放一个 `portable.marker` 空文件，则继续按绿色版走 exe 目录；
//! - 首次启动把旧位置的数据复制过来（不删原件），用户随时能回退。

use std::fs;
use std::path::{Path, PathBuf};

/// 用户目录下的一级目录名。刻意不用 bundle identifier：
/// Tauri 卸载器删除应用数据时清的是 `<APPDATA>/<identifier>`，换个名字才躲得开。
const APP_DIR_NAME: &str = "Guaxi";
/// 绿色版标记文件。
const PORTABLE_MARKER: &str = "portable.marker";

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn is_portable() -> bool {
    exe_dir().join(PORTABLE_MARKER).is_file()
}

fn platform_base() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata);
        }
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(profile).join("AppData").join("Roaming");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Library").join("Application Support");
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(data);
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local").join("share");
        }
    }
    std::env::temp_dir()
}

/// 用户数据根。所有会变的用户数据都应该落在它下面。
pub fn data_root() -> PathBuf {
    if is_portable() {
        return exe_dir();
    }
    let dir = platform_base().join(APP_DIR_NAME);
    let _ = fs::create_dir_all(&dir);
    dir
}

/// 配置目录（`guaxi_config.json`、flipboard 源、任务配置等都在这里）。
pub fn config_dir() -> PathBuf {
    let dir = data_root().join("config");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// 数据根下的子目录，按需创建。
pub fn sub_dir(name: &str) -> PathBuf {
    let dir = data_root().join(name);
    let _ = fs::create_dir_all(&dir);
    dir
}

/// 各业务库（todo / resume / 知识库 / 索引）共用的 `repos` 目录。
pub fn repos_dir() -> PathBuf {
    sub_dir("repos")
}

fn copy_dir_all(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if !target.exists() {
            // 只补缺，不覆盖用户在新位置已经动过的文件。
            let _ = fs::copy(entry.path(), &target);
        }
    }
    Ok(())
}

/// 目录总大小。只用来判断"要不要同步搬"，误差无所谓。
fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        total += if meta.is_dir() {
            dir_size(&entry.path())
        } else {
            meta.len()
        };
    }
    total
}

/// 超过这个体积的目录改到后台复制，别让首次启动干等。
const SYNC_COPY_LIMIT_BYTES: u64 = 300 * 1024 * 1024;

/// NSIS 默认把当前用户版装到 `%LOCALAPPDATA%\<productName>`。
/// 用户换了个目录安装时，旧数据还躺在原处，这里把它也当成候选来源。
fn legacy_install_dirs() -> Vec<PathBuf> {
    #[allow(unused_mut)]
    let mut dirs = vec![exe_dir()];
    #[cfg(target_os = "windows")]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let guess = PathBuf::from(local).join("guaxi");
            if !dirs.iter().any(|dir| dir == &guess) && guess.is_dir() {
                dirs.push(guess);
            }
        }
    }
    dirs
}

/// 把旧位置（exe 同级）里缺失的数据复制到用户目录。
///
/// 只做复制，不动旧文件：迁移出问题时用户还能从安装目录捞回来。
pub fn migrate_legacy() {
    if is_portable() {
        return;
    }
    let root = data_root();
    if root == exe_dir() {
        return;
    }

    for legacy in legacy_install_dirs() {
        // 配置：单个 JSON 最关键，同步搬完再放行，否则 load_config 会读到一份默认值。
        let legacy_config = legacy.join("config");
        if legacy_config.is_dir() {
            let _ = copy_dir_all(&legacy_config, &root.join("config"));
        }

        // 行程计划、任务等数据。体积正常时同步复制；很大（缓存/原始网页）就丢后台。
        for name in ["repos", "task"] {
            let from = legacy.join(name);
            if !from.is_dir() {
                continue;
            }
            let target = root.join(name);
            if dir_size(&from) > SYNC_COPY_LIMIT_BYTES {
                std::thread::spawn(move || {
                    let _ = copy_dir_all(&from, &target);
                });
            } else {
                let _ = copy_dir_all(&from, &target);
            }
        }

        // 资料库基本都是文档，可能很大，一律放后台。
        let legacy_libraries = legacy.join("Libraries");
        if legacy_libraries.is_dir() {
            let target = root.join("Libraries");
            if !target.exists() {
                std::thread::spawn(move || {
                    let _ = copy_dir_all(&legacy_libraries, &target);
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_root_is_not_exe_dir_without_marker() {
        // 测试环境没有 portable.marker，数据根应该落在用户目录而不是安装目录。
        if is_portable() {
            return;
        }
        assert_ne!(data_root(), exe_dir());
    }

    #[test]
    fn config_dir_ends_with_config() {
        assert!(config_dir().ends_with("config"));
    }
}
