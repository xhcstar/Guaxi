use crate::state::SharedState;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{Emitter, State};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookDataLabRpcRequest {
    pub action: String,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub cell_id: Option<String>,
    #[serde(default)]
    pub cell_key: Option<String>,
    #[serde(default)]
    pub stable_id: Option<String>,
    #[serde(default)]
    pub variable_name: Option<String>,
    #[serde(default)]
    pub data_kind: Option<String>,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub source_label: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookDataLabRpcResponse {
    pub success: bool,
    pub request_id: String,
    pub delivered: bool,
    pub event_name: String,
    pub action: String,
    pub file_path: String,
    pub message: String,
}

const NOTEBOOK_DATALAB_RPC_EVENT: &str = "notebook-data-lab-rpc";

fn normalize_text(value: Option<&String>) -> String {
    value.map(|item| item.trim().to_string()).unwrap_or_default()
}

fn is_supported_action(action: &str) -> bool {
    matches!(
        action,
        "import-file"
            | "import-clipboard"
            | "import-inline-data"
            | "materialize-variable"
            | "writeback-variable"
            | "export-variable"
    )
}

// 设计说明：
// 1. 外部工具不应该直接操纵前端内部状态；后端 RPC 只负责校验请求并广播统一事件。
// 2. 真正的数据导入、物化、写回仍然由 Notebook 前端复用既有实验室链路执行，
//    这样 Tauri command、HTTP 接口和 UI 按钮走的是同一条业务路径。
// 3. 这里返回一个 request_id，便于后续扩展成“异步任务回执 / 状态跟踪”。
pub fn dispatch_notebook_data_lab_rpc(
    state: &SharedState,
    request: &NotebookDataLabRpcRequest,
    entrypoint: &str,
) -> Result<NotebookDataLabRpcResponse, String> {
    let action = request.action.trim().to_string();
    if action.is_empty() {
        return Err("action 不能为空".to_string());
    }
    if !is_supported_action(&action) {
        return Err(format!("不支持的数据实验室动作：{action}"));
    }

    let app_handle = state
        .get_app_handle()
        .ok_or_else(|| "AppHandle 尚未初始化，无法派发实验室 RPC".to_string())?;
    let request_id = format!("lab-rpc-{}", Uuid::new_v4());
    let file_path = normalize_text(request.file_path.as_ref());

    let payload = json!({
        "requestId": request_id,
        "entrypoint": entrypoint,
        "receivedAt": chrono::Local::now().to_rfc3339(),
        "action": action,
        "filePath": file_path,
        "cellId": normalize_text(request.cell_id.as_ref()),
        "cellKey": normalize_text(request.cell_key.as_ref()),
        "stableId": normalize_text(request.stable_id.as_ref()),
        "variableName": normalize_text(request.variable_name.as_ref()),
        "dataKind": normalize_text(request.data_kind.as_ref()),
        "sourceType": normalize_text(request.source_type.as_ref()),
        "sourceLabel": normalize_text(request.source_label.as_ref()),
        "text": request.text.clone().unwrap_or_default(),
        "clientId": normalize_text(request.client_id.as_ref()),
    });

    app_handle
        .emit(NOTEBOOK_DATALAB_RPC_EVENT, payload)
        .map_err(|error| format!("发送数据实验室 RPC 事件失败：{error}"))?;

    info!(
        event = NOTEBOOK_DATALAB_RPC_EVENT,
        entrypoint = entrypoint,
        request_id = request_id.as_str(),
        action = action.as_str(),
        file_path = file_path.as_str(),
        "[NotebookDataLabRpc] dispatched rpc event"
    );

    Ok(NotebookDataLabRpcResponse {
        success: true,
        request_id,
        delivered: true,
        event_name: NOTEBOOK_DATALAB_RPC_EVENT.to_string(),
        action: action.clone(),
        file_path,
        message: format!("数据实验室动作 {action} 已派发"),
    })
}

#[tauri::command]
pub async fn notebook_data_lab_rpc(
    request: NotebookDataLabRpcRequest,
    state: State<'_, SharedState>,
) -> Result<NotebookDataLabRpcResponse, String> {
    dispatch_notebook_data_lab_rpc(state.inner(), &request, "tauri-command")
}

pub fn notebook_data_lab_rpc_event_name() -> &'static str {
    NOTEBOOK_DATALAB_RPC_EVENT
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookDataLabHttpHealth {
    pub success: bool,
    pub event_name: String,
    pub message: String,
}

pub fn notebook_data_lab_health_response() -> NotebookDataLabHttpHealth {
    NotebookDataLabHttpHealth {
        success: true,
        event_name: NOTEBOOK_DATALAB_RPC_EVENT.to_string(),
        message: "Notebook Data Lab RPC endpoint is ready".to_string(),
    }
}
