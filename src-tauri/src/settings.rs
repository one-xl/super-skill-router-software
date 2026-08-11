use std::fs;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFormat {
    #[default]
    Openai,
    Anthropic,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiConfig {
    pub format: ApiFormat,
    pub api_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub deep_scan: ApiConfig,
    pub prompt: ApiConfig,
}

fn path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法确定设置目录：{error}"))?
        .join("settings.json"))
}
pub fn load(app: &AppHandle) -> Result<AppSettings, String> {
    let path = path(app)?;
    if !path.is_file() {
        return Ok(AppSettings::default());
    }
    serde_json::from_slice(&fs::read(&path).map_err(|error| format!("无法读取设置：{error}"))?)
        .map_err(|error| format!("设置格式无效：{error}"))
}
#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    load(&app)
}
#[tauri::command]
pub fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    let path = path(&app)?;
    let parent = path.parent().ok_or_else(|| "设置目录无效。".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建设置目录：{error}"))?;
    let data =
        serde_json::to_vec_pretty(&settings).map_err(|error| format!("无法保存设置：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, data).map_err(|error| format!("无法写入设置：{error}"))?;
    fs::rename(&temporary, &path).map_err(|error| format!("无法完成设置保存：{error}"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefineRequest {
    pub requirement: String,
    pub template_prompt: String,
}
#[tauri::command]
pub async fn refine_prompt(app: AppHandle, request: RefineRequest) -> Result<String, String> {
    let config = load(&app)?.prompt;
    if config.api_url.trim().is_empty() || config.api_key.trim().is_empty() {
        return Err("Prompt 精炼尚未配置 API URL 或 API Key，请先到设置页完成配置。".into());
    }
    let instructions = format!("将以下用户需求改写为可直接执行的结构化 Prompt。保留并完善现有章节；仅引用给出的 skill 名称、用途和触发场景，绝不添加 skill 全文。\n\n需求：{}\n\n模板 Prompt：{}", request.requirement, request.template_prompt);
    let client = reqwest::Client::new();
    let endpoint = match config.format {
        ApiFormat::Openai => suffix(&config.api_url, "/chat/completions"),
        ApiFormat::Anthropic => suffix(&config.api_url, "/messages"),
    };
    let response = match config.format {
        ApiFormat::Openai => client.post(endpoint).bearer_auth(&config.api_key).json(&json!({"model": default_model(&config, "gpt-4o-mini"), "messages": [{"role":"system","content":"You produce concise Chinese task prompts in Markdown."},{"role":"user","content":instructions}], "temperature":0.2})).send().await,
        ApiFormat::Anthropic => client.post(endpoint).header("x-api-key", &config.api_key).header("anthropic-version", "2023-06-01").json(&json!({"model": default_model(&config, "claude-3-5-haiku-latest"), "max_tokens":2000, "messages":[{"role":"user","content":instructions}]})).send().await,
    }.map_err(|error| format!("无法调用 Prompt API：{error}"))?;
    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|error| format!("Prompt API 返回无法解析：{error}"))?;
    if !status.is_success() {
        return Err(format!("Prompt API 请求失败（{status}）：{}", body));
    }
    let text = match config.format {
        ApiFormat::Openai => body
            .pointer("/choices/0/message/content")
            .and_then(serde_json::Value::as_str),
        ApiFormat::Anthropic => body
            .pointer("/content/0/text")
            .and_then(serde_json::Value::as_str),
    };
    text.map(str::to_owned)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Prompt API 没有返回可用文本。".into())
}
fn suffix(url: &str, suffix: &str) -> String {
    let url = url.trim_end_matches('/');
    if url.ends_with(suffix) {
        url.into()
    } else {
        format!("{url}{suffix}")
    }
}
fn default_model(config: &ApiConfig, fallback: &str) -> String {
    if config.model.trim().is_empty() {
        fallback.into()
    } else {
        config.model.clone()
    }
}
