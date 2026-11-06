use std::path::Path;

use async_trait::async_trait;
use russh_sftp::client::SftpSession;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::info;

use crate::commands::ssh_terminal::{connect_authenticated, ClientHandler};

use super::{RemoteEntry, SyncBackend, SyncConnection, normalize_key};

pub struct SftpBackend {
    session: SftpSession,
    _handle: russh::client::Handle<ClientHandler>,
    root: String,
}

impl SftpBackend {
    pub async fn connect(
        app: &tauri::AppHandle,
        connection: &SyncConnection,
        remote_root: &str,
    ) -> Result<Self, String> {
        if connection.sftp.profile_id.trim().is_empty() {
            return Err("SFTP 连接还没有选择 SSH 档案".to_string());
        }
        let handle = connect_authenticated(app, connection.sftp.profile_id.trim()).await?;
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| format!("打开 SFTP 通道失败：{}", e))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| format!("请求 SFTP 子系统失败：{}", e))?;
        let session = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| format!("建立 SFTP 会话失败：{}", e))?;

        let base = connection.sftp.root.trim().trim_end_matches('/');
        let extra = remote_root.trim().trim_matches('/');
        let mut root = if extra.is_empty() {
            base.to_string()
        } else if base.is_empty() {
            extra.to_string()
        } else {
            format!("{}/{}", base, extra)
        };
        if root.is_empty() {
            root = session.canonicalize(".").await.unwrap_or_else(|_| ".".to_string());
        }
        Ok(Self {
            session,
            _handle: handle,
            root,
        })
    }

    fn full(&self, key: &str) -> String {
        let key = normalize_key(key);
        if key.is_empty() {
            self.root.clone()
        } else {
            format!("{}/{}", self.root.trim_end_matches('/'), key)
        }
    }

    fn relative(&self, full: &str) -> String {
        let root = self.root.trim_end_matches('/');
        full.strip_prefix(root)
            .unwrap_or(full)
            .trim_start_matches('/')
            .to_string()
    }
}

#[async_trait]
impl SyncBackend for SftpBackend {
    async fn test(&self) -> Result<String, String> {
        let path = self.full("");
        if self.session.try_exists(&path).await.unwrap_or(false) {
            return Ok("SFTP 连接正常".to_string());
        }
        self.session
            .create_dir(&path)
            .await
            .map_err(|e| format!("SFTP 目录不可用：{}", e))?;
        Ok("SFTP 连接正常".to_string())
    }

    async fn list(&self) -> Result<Vec<RemoteEntry>, String> {
        let mut out = Vec::new();
        let mut queue = vec![self.full("")];
        let mut seen = std::collections::HashSet::new();
        while let Some(dir) = queue.pop() {
            if seen.len() > 5000 {
                return Err("SFTP 目录过多，请检查远端路径".to_string());
            }
            if !seen.insert(dir.clone()) {
                continue;
            }
            let entries = self
                .session
                .read_dir(&dir)
                .await
                .map_err(|e| format!("SFTP 列目录失败：{}", e))?;
            for entry in entries {
                let name = entry.file_name();
                if name == "." || name == ".." {
                    continue;
                }
                let full = format!("{}/{}", dir.trim_end_matches('/'), name);
                let meta = entry.metadata();
                if meta.is_dir() {
                    queue.push(full);
                } else {
                    let key = self.relative(&full);
                    if key.is_empty() {
                        continue;
                    }
                    let mtime_ms = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    out.push(RemoteEntry {
                        key,
                        size: meta.len(),
                        mtime_ms,
                    });
                }
            }
        }
        info!("[Sync][SFTP] listed {} files", out.len());
        Ok(out)
    }

    async fn stat(&self, key: &str) -> Result<Option<RemoteEntry>, String> {
        let path = self.full(key);
        if !self.session.try_exists(&path).await.unwrap_or(false) {
            return Ok(None);
        }
        let meta = self
            .session
            .metadata(&path)
            .await
            .map_err(|e| format!("SFTP 读取属性失败：{}", e))?;
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Ok(Some(RemoteEntry {
            key: key.to_string(),
            size: meta.len(),
            mtime_ms,
        }))
    }

    async fn get_file(&self, key: &str, local: &Path) -> Result<(), String> {
        let mut remote = self
            .session
            .open(self.full(key))
            .await
            .map_err(|e| format!("SFTP 打开远端文件失败：{}", e))?;
        if let Some(parent) = local.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        let mut file = tokio::fs::File::create(local)
            .await
            .map_err(|e| e.to_string())?;
        tokio::io::copy(&mut remote, &mut file)
            .await
            .map_err(|e| format!("SFTP 下载失败：{}", e))?;
        file.flush().await.map_err(|e| e.to_string())?;
        let _ = remote.shutdown().await;
        Ok(())
    }

    async fn put_file(&self, key: &str, local: &Path) -> Result<(), String> {
        let path = self.full(key);
        if let Some(parent) = key.rfind('/').map(|idx| &key[..idx]) {
            self.mkdir(parent).await?;
        }
        let mut source = tokio::fs::File::open(local)
            .await
            .map_err(|e| e.to_string())?;
        let mut remote = self
            .session
            .create(path)
            .await
            .map_err(|e| format!("SFTP 创建远端文件失败：{}", e))?;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let read = source.read(&mut buf).await.map_err(|e| e.to_string())?;
            if read == 0 {
                break;
            }
            remote
                .write_all(&buf[..read])
                .await
                .map_err(|e| format!("SFTP 上传失败：{}", e))?;
        }
        remote
            .shutdown()
            .await
            .map_err(|e| format!("SFTP 关闭远端文件失败：{}", e))?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), String> {
        let path = self.full(key);
        match self.session.remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) => {
                let text = error.to_string();
                if text.contains("NoSuchFile") || text.contains("no such file") {
                    Ok(())
                } else {
                    Err(format!("SFTP 删除失败：{}", text))
                }
            }
        }
    }

    async fn mkdir(&self, key: &str) -> Result<(), String> {
        let key = normalize_key(key);
        if key.is_empty() {
            return Ok(());
        }
        let mut acc = self.root.trim_end_matches('/').to_string();
        for seg in key.split('/').filter(|s| !s.is_empty()) {
            acc.push('/');
            acc.push_str(seg);
            if self.session.try_exists(&acc).await.unwrap_or(false) {
                continue;
            }
            let _ = self.session.create_dir(&acc).await;
        }
        Ok(())
    }

    fn concurrency(&self) -> usize {
        2
    }
}
