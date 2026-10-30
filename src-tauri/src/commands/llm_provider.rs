//! 大模型供应商的统一配置与解析。
//!
//! 普通对话（`chatview`）、Codex Agent（`codex_agent`）与内置 Python Agent（`agent_server`）
//! 以前各自硬编码了一份供应商地址、模型名和密钥字段，改一处很容易漏掉另外两处。
//! 本模块把这份清单收敛成唯一的 `PROVIDERS`，并为三条链路提供同一套解析结果：
//!
//! - `resolve_chat`：给“对话”模式解析某个 modelid 的入口（含自定义与 cc-switch）。
//! - `resolve_agent`：给 Codex Agent 解析供应商（cc-switch 优先，其次自定义）。
//! - `agent_model_registry`：给 Python Agent 的 `GUAXI_LLM_CONFIG` 生成注册表。
//!
//! 设置页的“服务商 API / cc-switch 代理”两个 Tab 对应这里的两类来源，密钥字段仍由
//! `settingview::LlmConfig` 持有，本模块只负责“从配置里挑出该用哪一份”。

use serde::Serialize;
use serde_json::{json, Value};

use crate::commands::settingview::{self, LlmConfig};

/// 一个内置服务商。`id` 与 `ChatModel` 的 id 一一对应，前端模型选择器也按它传值。
pub struct ProviderSpec {
    pub id: i32,
    pub name: &'static str,
    /// OpenAI 兼容根地址（写 codex 配置、Python Agent 注册表时用）。
    pub base_url: &'static str,
    /// “对话”模式使用的模型名。
    pub chat_model: &'static str,
    /// Agent 侧默认模型名（设置页填了 Codex 模型时以设置为准）。
    pub agent_model: &'static str,
}

pub const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec {
        id: 0,
        name: "ChatGPT",
        base_url: "https://api.openai.com/v1",
        chat_model: "gpt-4o-mini",
        agent_model: "gpt-4o",
    },
    ProviderSpec {
        id: 1,
        name: "DeepSeek",
        base_url: "https://api.deepseek.com/v1",
        chat_model: "deepseek-chat",
        agent_model: "deepseek-chat",
    },
    ProviderSpec {
        id: 2,
        name: "Qwen",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        chat_model: "qwen-plus",
        agent_model: "qwen-plus",
    },
    ProviderSpec {
        id: 3,
        name: "Kimi",
        base_url: "https://api.moonshot.cn/v1",
        chat_model: "moonshot-v1-8k",
        agent_model: "moonshot-v1-8k",
    },
    ProviderSpec {
        id: 4,
        name: "豆包",
        base_url: "https://ark.cn-beijing.volces.com/api/v3",
        chat_model: "doubao-pro-32k",
        agent_model: "doubao-pro-32k",
    },
    ProviderSpec {
        id: 5,
        name: "硅基流动",
        base_url: "https://api.siliconflow.cn/v1",
        chat_model: "deepseek-ai/DeepSeek-R1-0528-Qwen3-8B",
        agent_model: "deepseek-ai/DeepSeek-R1-0528-Qwen3-8B",
    },
];

pub fn spec(id: i32) -> Option<&'static ProviderSpec> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// 从配置里取某个内置服务商的密钥。
pub fn api_key_for(llm: &LlmConfig, id: i32) -> String {
    let raw: &str = match id {
        0 => &llm.chatgpt_api_key,
        1 => &llm.deepseek_api_key,
        2 => &llm.qwen_api_key,
        3 => &llm.kimi_api_key,
        4 => &llm.doubao_api_key,
        5 => &llm.siliconflow_api_key,
        _ => "",
    };
    raw.trim().to_string()
}

/// 将用户填写的 base_url 规整为完整的 chat/completions 端点。
/// 兼容三种写法：填到根(https://x.com)、填到 /v1、填完整 /v1/chat/completions。
pub fn normalize_openai_base_url(base: &str) -> String {
    let b = base.trim().trim_end_matches('/');
    if b.is_empty() {
        return String::new();
    }
    if b.ends_with("/chat/completions") {
        b.to_string()
    } else if b.ends_with("/v1") {
        format!("{}/chat/completions", b)
    } else {
        format!("{}/v1/chat/completions", b)
    }
}

/// 内置服务商的完整对话端点。
///
/// DeepSeek 沿用历史上的 `https://api.deepseek.com/chat/completions`（它能同时接受根路径与
/// `/v1` 路径）；其余都是 `base_url + /chat/completions`。
fn provider_chat_endpoint(spec: &ProviderSpec) -> String {
    if spec.id == 1 {
        return "https://api.deepseek.com/chat/completions".to_string();
    }
    format!("{}/chat/completions", spec.base_url.trim_end_matches('/'))
}

/// 一条链路上已经解析好的供应商。
#[derive(Serialize, Clone, Debug)]
pub struct ResolvedProvider {
    /// `provider`（内置服务商）/ `custom` / `cc-switch`
    pub source: String,
    pub label: String,
    /// OpenAI 兼容根地址；cc-switch 与自定义保留用户填写的原样（codex 需要根地址）。
    pub base_url: String,
    /// 完整 chat/completions 端点（对话链路使用）。
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
}

fn custom_provider(llm: &LlmConfig) -> Result<ResolvedProvider, String> {
    let endpoint = normalize_openai_base_url(&llm.custom_base_url);
    if endpoint.is_empty() {
        return Err("自定义供应商 Base URL 未配置".to_string());
    }
    let model = llm.custom_model.trim().to_string();
    if model.is_empty() {
        return Err("自定义供应商模型名未配置".to_string());
    }
    Ok(ResolvedProvider {
        source: "custom".to_string(),
        label: "自定义供应商".to_string(),
        base_url: llm.custom_base_url.trim().to_string(),
        endpoint,
        api_key: llm.custom_api_key.trim().to_string(),
        model,
    })
}

fn cc_switch_provider(llm: &LlmConfig) -> Result<ResolvedProvider, String> {
    if !llm.cc_switch_enabled {
        return Err("cc-switch 代理未启用：请在设置 → 大模型 → cc-switch 代理里开启".to_string());
    }
    // 对话链路只支持 Chat Completions；代理只暴露 Responses 时给出可读提示。
    if llm.cc_switch_wire_api.trim() == "responses" {
        return Err(
            "cc-switch 代理当前配置为 responses 协议，普通对话需要 Chat Completions；请把协议改为 chat"
                .to_string(),
        );
    }
    let endpoint = normalize_openai_base_url(&llm.cc_switch_base_url);
    if endpoint.is_empty() {
        return Err("cc-switch 代理地址未配置".to_string());
    }
    let model = llm.cc_switch_model.trim().to_string();
    if model.is_empty() {
        return Err("cc-switch 模型名未配置".to_string());
    }
    Ok(ResolvedProvider {
        source: "cc-switch".to_string(),
        label: "cc-switch 代理".to_string(),
        base_url: llm.cc_switch_base_url.trim().to_string(),
        endpoint,
        api_key: llm.cc_switch_api_key.trim().to_string(),
        model,
    })
}

/// 解析“对话”模式要用的供应商。
///
/// 只处理 OpenAI 兼容的三类来源：内置服务商（0..=5）、自定义（10）、cc-switch（11）。
/// 本地模型与网页模型有自己的启动/抓取链路，不走这里。
pub fn resolve_chat(modelid: i32) -> Result<ResolvedProvider, String> {
    let llm = settingview::get_llm_config();
    match modelid {
        10 => custom_provider(&llm),
        11 => cc_switch_provider(&llm),
        id => {
            let Some(spec) = spec(id) else {
                return Err(format!("模型 {} 不走 OpenAI 兼容接口", id));
            };
            let api_key = api_key_for(&llm, id);
            if api_key.is_empty() {
                return Err(format!("{} API Key 未配置", spec.name));
            }
            Ok(ResolvedProvider {
                source: "provider".to_string(),
                label: spec.name.to_string(),
                base_url: spec.base_url.to_string(),
                endpoint: provider_chat_endpoint(spec),
                api_key,
                model: spec.chat_model.to_string(),
            })
        }
    }
}

/// 解析 Codex Agent 要用的供应商：cc-switch 优先，其次自定义 OpenAI 兼容配置。
///
/// 走 cc-switch 时 Guaxi 只记一个入口地址，具体用哪家由 cc-switch 决定；
/// codex 需要的是根地址而不是 chat/completions 端点，所以 `base_url` 保持用户填写的原样。
///
/// 注意 wire_api 不在这里选：codex 0.149 起只接受 Responses 协议，设置页里的
/// `cc_switch_wire_api` 只影响 Guaxi 自己的普通对话链路。
pub fn resolve_agent() -> Result<ResolvedProvider, String> {
    let llm = settingview::get_llm_config();

    let mut provider = if llm.cc_switch_enabled && !llm.cc_switch_base_url.trim().is_empty() {
        ResolvedProvider {
            source: "cc-switch".to_string(),
            label: "cc-switch 代理".to_string(),
            base_url: llm.cc_switch_base_url.trim().to_string(),
            endpoint: normalize_openai_base_url(&llm.cc_switch_base_url),
            api_key: llm.cc_switch_api_key.trim().to_string(),
            model: llm.cc_switch_model.trim().to_string(),
        }
    } else if !llm.custom_base_url.trim().is_empty() {
        ResolvedProvider {
            source: "custom".to_string(),
            label: "自定义供应商".to_string(),
            base_url: llm.custom_base_url.trim().to_string(),
            endpoint: normalize_openai_base_url(&llm.custom_base_url),
            api_key: llm.custom_api_key.trim().to_string(),
            model: llm.custom_model.trim().to_string(),
        }
    } else {
        return Err(
            "Agent 没有可用的模型供应商：请在设置 → 大模型里启用 cc-switch 代理，或填写自定义 Base URL"
                .to_string(),
        );
    };

    // 设置页显式填了 codex 专用模型时以它为准，方便 Agent 用比普通对话更强的模型。
    if !llm.codex_model.trim().is_empty() {
        provider.model = llm.codex_model.trim().to_string();
    }
    if provider.model.is_empty() {
        return Err(format!(
            "Agent 缺少模型名：请在设置 → 大模型里填写 Codex 模型或 {} 的模型名",
            provider.label
        ));
    }
    Ok(provider)
}

/// 生成 Python Agent 的模型注册表（`GUAXI_LLM_CONFIG`）。
///
/// 与 Rust 侧共用同一份 `PROVIDERS`，避免两边模型名漂移；本地模型与 cc-switch 也一并登记，
/// 这样内置 Agent 能跟随用户在模型选择器里选的同一档。
pub fn agent_model_registry() -> Value {
    let llm = settingview::get_llm_config();

    let mut models: Vec<Value> = PROVIDERS
        .iter()
        .map(|spec| {
            json!({
                "id": spec.id,
                "name": spec.name,
                "base_url": spec.base_url,
                "api_key": api_key_for(&llm, spec.id),
                "model_name": spec.agent_model,
                "supports_tools": true,
            })
        })
        .collect();

    models.push(json!({
        "id": 6,
        "name": "LocalDeepSeek",
        "base_url": "http://127.0.0.1:18080/v1",
        "api_key": "not-needed",
        "model_name": "local-deepseek",
        "supports_tools": true,
    }));
    models.push(json!({
        "id": 9,
        "name": "LocalQwen3",
        "base_url": "http://127.0.0.1:18082/v1",
        "api_key": "not-needed",
        "model_name": "local-qwen3",
        "supports_tools": true,
    }));

    let custom_model = llm.custom_model.trim().to_string();
    models.push(json!({
        "id": 10,
        "name": "Custom",
        "base_url": llm.custom_base_url.trim(),
        "api_key": llm.custom_api_key.trim(),
        "model_name": custom_model,
        "supports_tools": true,
    }));

    // cc-switch 只在启用时登记：没开代理时填一个本地地址只会让 Agent 连不上。
    if llm.cc_switch_enabled && !llm.cc_switch_base_url.trim().is_empty() {
        models.push(json!({
            "id": 11,
            "name": "cc-switch",
            "base_url": llm.cc_switch_base_url.trim(),
            "api_key": llm.cc_switch_api_key.trim(),
            "model_name": llm.cc_switch_model.trim(),
            "supports_tools": true,
        }));
    }

    Value::Array(models)
}
