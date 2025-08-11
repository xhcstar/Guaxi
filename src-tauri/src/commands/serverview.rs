use crate::state::SharedState;
use tauri::{AppHandle, State};
use tracing::info;

#[tauri::command]
pub async fn init_backend_services(
    app_handle: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    info!("Initializing backend services");
    state.set_app_handle(app_handle);
    Ok(())
}