use std::fs;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager};

const CREDENTIAL_SERVICE: &str = "Super Skill Router";
const DEEP_SCAN_SECRET: &str = "deep-scan-api-key";
const PROMPT_SECRET: &str = "prompt-api-key";
const SKILLSMP_SECRET: &str = "skillsmp-api-key";
pub const DEFAULT_RECOVERY_TEXT: &str = "继续并恢复todo-list";

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
    #[serde(default)]
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub api_key_configured: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsMpConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_key_configured: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationConfig {
    #[serde(default)]
    pub auto_inject_after_refine: bool,
    #[serde(default)]
    pub start_codex_recovery_monitor_on_launch: bool,
    #[serde(default = "default_recovery_text")]
    pub recovery_text: String,
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self {
            auto_inject_after_refine: false,
            start_codex_recovery_monitor_on_launch: false,
            recovery_text: default_recovery_text(),
        }
    }
}

fn default_recovery_text() -> String {
    DEFAULT_RECOVERY_TEXT.to_string()
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub deep_scan: ApiConfig,
    pub prompt: ApiConfig,
    #[serde(default)]
    pub skills_mp: SkillsMpConfig,
    #[serde(default)]
    pub automation: AutomationConfig,
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
    let bytes = fs::read(&path).map_err(|error| format!("无法读取设置：{error}"))?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(AppSettings::default());
    }
    let json = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let mut settings: AppSettings =
        serde_json::from_slice(json).map_err(|error| format!("设置格式无效：{error}"))?;

    // Migrate keys saved by versions before Credential Manager support.
    let had_legacy_secret = !settings.deep_scan.api_key.is_empty()
        || !settings.prompt.api_key.is_empty()
        || !settings.skills_mp.api_key.is_empty();
    migrate_secret(&settings.deep_scan.api_key, DEEP_SCAN_SECRET)?;
    migrate_secret(&settings.prompt.api_key, PROMPT_SECRET)?;
    migrate_secret(&settings.skills_mp.api_key, SKILLSMP_SECRET)?;
    settings.deep_scan.api_key = read_secret(DEEP_SCAN_SECRET)?.unwrap_or_default();
    settings.deep_scan.api_key_configured = !settings.deep_scan.api_key.is_empty();
    settings.prompt.api_key = read_secret(PROMPT_SECRET)?.unwrap_or_default();
    settings.prompt.api_key_configured = !settings.prompt.api_key.is_empty();
    settings.skills_mp.api_key = read_secret(SKILLSMP_SECRET)?.unwrap_or_default();
    settings.skills_mp.api_key_configured = !settings.skills_mp.api_key.is_empty();
    if had_legacy_secret {
        write_settings(&path, &settings)?;
    }
    Ok(settings)
}
#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    let mut settings = load(&app)?;
    settings.deep_scan.api_key.clear();
    settings.prompt.api_key.clear();
    settings.skills_mp.api_key.clear();
    Ok(settings)
}
#[tauri::command]
pub fn save_settings(app: AppHandle, mut settings: AppSettings) -> Result<(), String> {
    let path = path(&app)?;
    if !settings.deep_scan.api_key.trim().is_empty() {
        write_secret(DEEP_SCAN_SECRET, &settings.deep_scan.api_key)?;
    }
    if !settings.prompt.api_key.trim().is_empty() {
        write_secret(PROMPT_SECRET, &settings.prompt.api_key)?;
    }
    if !settings.skills_mp.api_key.trim().is_empty() {
        write_secret(SKILLSMP_SECRET, &settings.skills_mp.api_key)?;
    }
    settings.automation.recovery_text = if settings.automation.recovery_text.trim().is_empty() {
        DEFAULT_RECOVERY_TEXT.to_string()
    } else {
        settings.automation.recovery_text.trim().to_string()
    };
    let mut stored = settings;
    stored.deep_scan.api_key.clear();
    stored.deep_scan.api_key_configured = read_secret(DEEP_SCAN_SECRET)?.is_some();
    stored.prompt.api_key.clear();
    stored.prompt.api_key_configured = read_secret(PROMPT_SECRET)?.is_some();
    stored.skills_mp.api_key.clear();
    stored.skills_mp.api_key_configured = read_secret(SKILLSMP_SECRET)?.is_some();
    write_settings(&path, &stored)
}

pub fn skillsmp_api_key(app: &AppHandle) -> Result<String, String> {
    let api_key = load(app)?.skills_mp.api_key;
    if api_key.trim().is_empty() {
        Err("SkillsMP API Key 尚未配置，请先到设置页保存。".into())
    } else {
        Ok(api_key)
    }
}

fn write_settings(path: &std::path::Path, settings: &AppSettings) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "设置目录无效。".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建设置目录：{error}"))?;
    let mut stored = settings.clone();
    stored.deep_scan.api_key.clear();
    stored.prompt.api_key.clear();
    stored.skills_mp.api_key.clear();
    let data =
        serde_json::to_vec_pretty(&stored).map_err(|error| format!("无法保存设置：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, data).map_err(|error| format!("无法写入设置：{error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("无法完成设置保存：{error}"))
}

fn credential(name: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(CREDENTIAL_SERVICE, name)
        .map_err(|error| format!("无法访问 Windows Credential Manager：{error}"))
}

fn read_secret(name: &str) -> Result<Option<String>, String> {
    match credential(name)?.get_password() {
        Ok(value) if !value.is_empty() => Ok(Some(value)),
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!(
            "无法从 Windows Credential Manager 读取密钥：{error}"
        )),
    }
}

fn write_secret(name: &str, value: &str) -> Result<(), String> {
    credential(name)?
        .set_password(value)
        .map_err(|error| format!("无法保存密钥到 Windows Credential Manager：{error}"))
}

fn migrate_secret(value: &str, name: &str) -> Result<(), String> {
    if !value.trim().is_empty() {
        write_secret(name, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_default_automation_to_disabled() {
        let settings: AppSettings = serde_json::from_value(json!({
            "deepScan": { "format": "openai", "apiUrl": "", "model": "" },
            "prompt": { "format": "openai", "apiUrl": "", "model": "" },
            "skillsMp": {}
        }))
        .expect("parse legacy settings");

        assert!(!settings.automation.auto_inject_after_refine);
        assert!(!settings.automation.start_codex_recovery_monitor_on_launch);
        assert_eq!(settings.automation.recovery_text, DEFAULT_RECOVERY_TEXT);
    }

    #[test]
    fn settings_parser_accepts_a_utf8_bom() {
        let bytes = [
            b"\xEF\xBB\xBF".as_slice(),
            br#"{"deepScan":{"format":"openai","apiUrl":"","model":""},"prompt":{"format":"openai","apiUrl":"","model":""}}"#,
        ]
        .concat();
        let json = bytes.strip_prefix(b"\xEF\xBB\xBF").expect("strip BOM");
        let settings: AppSettings = serde_json::from_slice(json).expect("parse settings");
        assert_eq!(settings.automation.recovery_text, DEFAULT_RECOVERY_TEXT);
    }

    #[test]
    fn windows_credential_manager_round_trip() {
        let name = format!("test-{}", uuid::Uuid::new_v4());
        write_secret(&name, "credential-test-value").expect("write credential");
        let loaded = read_secret(&name).expect("read credential");
        assert_eq!(loaded.as_deref(), Some("credential-test-value"));
        credential(&name)
            .expect("open credential")
            .delete_credential()
            .expect("remove credential");
    }
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
