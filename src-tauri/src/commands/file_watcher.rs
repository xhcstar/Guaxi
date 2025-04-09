use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileEvent {
    kind: String,
    path: String,
    old_path: Option<String>,
}

pub struct FileWatcher {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
}

impl FileWatcher {
    pub fn new() -> Self {
        info!("Initializing new FileWatcher instance");
        Self {
            watcher: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn watch(&self, app: AppHandle, path: String) -> Result<(), String> {
        info!("Starting to watch path: {}", path);

        let path = Path::new(&path).to_path_buf();
        if !path.exists() {
            let err_msg = format!("Path does not exist: {}", path.display());
            error!("{}", err_msg);
            return Err(err_msg);
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => {
                info!("Successfully created file watcher");
                w
            }
            Err(e) => {
                let err_msg = format!("Failed to create file watcher: {}", e);
                error!("{}", err_msg);
                return Err(err_msg);
            }
        };

        if let Err(e) = watcher.watch(&path, RecursiveMode::Recursive) {
            let err_msg = format!("Failed to watch path {}: {}", path.display(), e);
            error!("{}", err_msg);
            return Err(err_msg);
        }

        *self.watcher.lock().await = Some(watcher);
        info!("Successfully started watching path: {}", path.display());

        tokio::spawn(async move {
            info!("Starting file change event listener");
            while let Ok(event) = rx.recv() {
                match event {
                    Ok(event) => {
                        debug!("Received raw file event: {:?}", event);

                        let file_event = match event.kind {
                            notify::EventKind::Create(_) => {
                                info!("File created: {:?}", event.paths);
                                FileEvent {
                                    kind: "Create".to_string(),
                                    path: event.paths[0].to_string_lossy().to_string(),
                                    old_path: None,
                                }
                            }
                            notify::EventKind::Remove(_) => {
                                warn!("File removed: {:?}", event.paths);
                                FileEvent {
                                    kind: "Remove".to_string(),
                                    path: event.paths[0].to_string_lossy().to_string(),
                                    old_path: None,
                                }
                            }
                            notify::EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
                                info!("File renamed: {:?}", event.paths);
                                let old_path = event.paths.get(1).map(|p| {
                                    let path = p.to_string_lossy().to_string();
                                    info!("Old path: {}", path);
                                    path
                                });
                                FileEvent {
                                    kind: "Rename".to_string(),
                                    path: event.paths[0].to_string_lossy().to_string(),
                                    old_path,
                                }
                            }
                            _ => {
                                debug!("Ignoring non-relevant file event: {:?}", event.kind);
                                continue;
                            }
                        };

                        if let Err(e) = app.emit("file_change", &file_event) {
                            error!("Failed to emit file_change event: {}", e);
                        } else {
                            debug!("Successfully emitted file event: {:?}", file_event);
                        }
                    }
                    Err(e) => {
                        error!("Error receiving file event: {}", e);
                    }
                }
            }
            warn!("File event receiver channel has been closed");
        });

        Ok(())
    }

    pub async fn stop(&self) {
        info!("Stopping file watcher");
        if let Some(watcher) = self.watcher.lock().await.take() {
            drop(watcher);
            info!("File watcher stopped successfully");
        }
    }
}

#[tauri::command]
pub async fn start_file_watching(
    app: AppHandle,
    path: String,
    file_watcher: tauri::State<'_, FileWatcher>,
) -> Result<(), String> {
    info!("Received start_file_watching command for path: {}", path);
    file_watcher.watch(app, path).await
}

#[tauri::command]
pub async fn stop_file_watching(file_watcher: tauri::State<'_, FileWatcher>) -> Result<(), String> {
    info!("Received stop_file_watching command");
    file_watcher.stop().await;
    Ok(())
}
