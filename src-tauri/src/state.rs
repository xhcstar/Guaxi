use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tracing::info;

#[derive(Clone)]
pub struct SharedState {
    app_handle: Arc<Mutex<Option<AppHandle>>>,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            app_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        info!("Setting AppHandle in SharedState");
        let mut guard = self.app_handle.lock().unwrap();
        *guard = Some(handle);
    }

    pub fn get_app_handle(&self) -> Option<AppHandle> {
        info!("Getting AppHandle from SharedState");
        let guard = self.app_handle.lock().unwrap();
        guard.clone()
    }
}