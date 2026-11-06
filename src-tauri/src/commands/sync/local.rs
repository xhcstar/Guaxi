use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tracing::info;
use walkdir::WalkDir;

use super::{RemoteEntry, SyncBackend, SyncConnection, normalize_key};

/// 本地目录 / 网络映射盘 / 各网盘的同步文件夹。
/// 不依赖任何厂商 API：把目标指向 OneDrive、Dropbox、坚果云同步盘或 NAS 挂载点即可。
pub struct LocalBackend {
    root: PathBuf,
}

impl LocalBackend {
    pub fn new(connection: &SyncConnection, remote_root: &str) -> Result<Self, String> {
        let base = connection.local.root.trim();
        if base.is_empty() {
            return Err("本地同步目录不能为空".to_string());
        }
        let extra = normalize_key(remote_root);
        let root = if extra.is_empty() {
            PathBuf::from(base)
        } else {
            Path::new(base).join(extra)
        };
        Ok(Self { root })
    }

    fn full(&self, key: &str) -> PathBuf {
        self.root.join(normalize_key(key))
    }
}

#[async_trait]
impl SyncBackend for LocalBackend {
    async fn test(&self) -> Result<String, String> {
        tokio::fs::create_dir_all(&self.root)
            .await
            .map_err(|e| format!("本地目录不可用：{}", e))?;
        Ok(format!("本地目录可用：{}", self.root.to_string_lossy()))
    }

    async fn list(&self) -> Result<Vec<RemoteEntry>, String> {
        let root = self.root.clone();
        let entries = tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            if !root.is_dir() {
                return Ok(out);
            }
            for entry in WalkDir::new(&root)
                .follow_links(false)
                .into_iter()
                .filter_map(|item| item.ok())
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy();
                if super::is_temp_name(&name) {
                    continue;
                }
                let rel = match entry.path().strip_prefix(&root) {
                    Ok(value) => value.to_string_lossy().replace('\\', "/"),
                    Err(_) => continue,
                };
                let meta = match entry.metadata() {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                out.push(RemoteEntry {
                    key: rel,
                    size: meta.len(),
                    mtime_ms: super::mtime_ms(&meta),
                });
            }
            Ok::<Vec<RemoteEntry>, String>(out)
        })
        .await
        .map_err(|e| e.to_string())??;
        info!("[Sync][Local] listed {} files", entries.len());
        Ok(entries)
    }

    async fn stat(&self, key: &str) -> Result<Option<RemoteEntry>, String> {
        let path = self.full(key);
        match tokio::fs::metadata(&path).await {
            Ok(meta) if meta.is_file() => Ok(Some(RemoteEntry {
                key: key.to_string(),
                size: meta.len(),
                mtime_ms: super::mtime_ms(&meta),
            })),
            Ok(_) => Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    async fn get_file(&self, key: &str, local: &Path) -> Result<(), String> {
        let source = self.full(key);
        if let Some(parent) = local.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        let mut input = tokio::fs::File::open(&source)
            .await
            .map_err(|e| format!("读取本地同步文件失败：{}", e))?;
        let mut output = tokio::fs::File::create(local)
            .await
            .map_err(|e| e.to_string())?;
        tokio::io::copy(&mut input, &mut output)
            .await
            .map_err(|e| format!("复制同步文件失败：{}", e))?;
        output.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn put_file(&self, key: &str, local: &Path) -> Result<(), String> {
        let destination = self.full(key);
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        let mut input = tokio::fs::File::open(local)
            .await
            .map_err(|e| e.to_string())?;
        let mut output = tokio::fs::File::create(&destination)
            .await
            .map_err(|e| format!("写入本地同步文件失败：{}", e))?;
        tokio::io::copy(&mut input, &mut output)
            .await
            .map_err(|e| format!("复制同步文件失败：{}", e))?;
        output.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), String> {
        match tokio::fs::remove_file(self.full(key)).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("删除本地同步文件失败：{}", error)),
        }
    }

    async fn mkdir(&self, key: &str) -> Result<(), String> {
        if key.is_empty() {
            tokio::fs::create_dir_all(&self.root)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            tokio::fs::create_dir_all(self.full(key))
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::sync::{LocalConnection, SyncBackend, SyncConnection};

    #[tokio::test]
    async fn local_backend_roundtrip_keeps_relative_paths() {
        let base = std::env::temp_dir().join(format!("guaxi-local-sync-{}", uuid::Uuid::new_v4()));
        let source = base.join("source.txt");
        tokio::fs::create_dir_all(&base).await.expect("create base");
        tokio::fs::write(&source, b"hello sync").await.expect("write source");

        let connection = SyncConnection {
            kind: "local".to_string(),
            local: LocalConnection {
                root: base.join("remote").to_string_lossy().to_string(),
            },
            ..Default::default()
        };
        let backend = LocalBackend::new(&connection, "sub").expect("backend");
        backend
            .put_file("docs/a.txt", &source)
            .await
            .expect("put");

        let list = backend.list().await.expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].key, "docs/a.txt");

        let target = base.join("downloaded.txt");
        backend
            .get_file("docs/a.txt", &target)
            .await
            .expect("get");
        assert_eq!(
            tokio::fs::read(&target).await.expect("read"),
            b"hello sync"
        );

        backend.delete("docs/a.txt").await.expect("delete");
        assert!(backend.stat("docs/a.txt").await.expect("stat").is_none());
        let _ = tokio::fs::remove_dir_all(&base).await;
    }
}
