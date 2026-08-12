use std::path::Path;
use std::time::Duration;

use crate::settings::{ApiConfig, ApiFormat};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

const FAST_SCAN_TIMEOUT: Duration = Duration::from_secs(30);
const DEEP_SCAN_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanMode {
    Fast,
    Deep,
}

impl ScanMode {
    fn sidecar_name(&self) -> &'static str {
        match self {
            Self::Fast => "skillspector-fast",
            Self::Deep => "skillspector",
        }
    }

    fn arguments(&self, skill_path: &str) -> Vec<String> {
        vec![
            "scan".into(),
            skill_path.into(),
            "--format".into(),
            "json".into(),
        ]
    }

    fn timeout(&self) -> Duration {
        match self {
            Self::Fast => FAST_SCAN_TIMEOUT,
            Self::Deep => DEEP_SCAN_TIMEOUT,
        }
    }

    fn timeout_message(&self) -> &'static str {
        match self {
            Self::Fast => "快速扫描在 30 秒后超时。请检查 skill 中是否有异常大的文件。",
            Self::Deep => {
                "深度扫描在 3 分钟后超时。请检查模型服务、API URL 与网络连接，或改用快速扫描。"
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub score: u8,
    pub severity: String,
    pub recommendation: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IssueLocation {
    pub file: String,
    pub start_line: u32,
    pub end_line: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanIssue {
    pub id: String,
    pub category: Option<String>,
    pub severity: String,
    pub confidence: f64,
    pub location: IssueLocation,
    pub explanation: String,
    pub remediation: Option<String>,
    pub code_snippet: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanReport {
    pub risk_assessment: RiskAssessment,
    pub issues: Vec<ScanIssue>,
    pub execution_successful: bool,
    #[serde(default)]
    pub analysis_completeness: serde_json::Value,
}

fn user_error(message: impl Into<String>) -> String {
    message.into()
}

#[tauri::command]
pub async fn scan_skill(
    app: AppHandle,
    skill_path: String,
    mode: ScanMode,
) -> Result<ScanReport, String> {
    let directory = Path::new(&skill_path);
    let configuration = if matches!(mode, ScanMode::Deep) {
        Some(crate::settings::load(&app)?.deep_scan)
    } else {
        None
    };
    scan_directory(app, directory, mode, configuration.as_ref()).await
}

pub async fn scan_directory(
    app: AppHandle,
    directory: &Path,
    mode: ScanMode,
    config: Option<&ApiConfig>,
) -> Result<ScanReport, String> {
    if !directory.is_dir() {
        return Err(user_error(
            "请选择一个存在的 skill 文件夹，而不是单个 SKILL.md 文件。",
        ));
    }

    if matches!(mode, ScanMode::Deep) {
        let config = config
            .ok_or_else(|| user_error("深度扫描没有读取到模型配置，请先到设置页保存后重试。"))?;
        if config.api_key.trim().is_empty() || config.api_url.trim().is_empty() {
            return Err(user_error(
                "深度扫描尚未配置 API URL 或 API Key，请先到设置页完成配置。",
            ));
        }
    }
    let mut command = app
        .shell()
        .sidecar(mode.sidecar_name())
        .map_err(|error| user_error(format!("扫描组件不可用：{error}")))?
        .args(mode.arguments(&directory.to_string_lossy()));
    if let (ScanMode::Deep, Some(config)) = (&mode, config) {
        command = match config.format {
            ApiFormat::Openai => command
                .env("SKILLSPECTOR_PROVIDER", "openai")
                .env("OPENAI_API_KEY", &config.api_key)
                .env("OPENAI_BASE_URL", &config.api_url),
            ApiFormat::Anthropic => command
                .env("SKILLSPECTOR_PROVIDER", "anthropic")
                .env("ANTHROPIC_API_KEY", &config.api_key)
                .env("ANTHROPIC_BASE_URL", &config.api_url),
        };
        if !config.model.trim().is_empty() {
            command = command.env("SKILLSPECTOR_MODEL", &config.model);
        }
    }
    let timeout = mode.timeout();
    let timeout_message = mode.timeout_message();
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| user_error(timeout_message))?
        .map_err(|error| user_error(format!("无法启动扫描组件：{error}")))?;

    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| user_error("扫描组件返回了无法读取的结果。"))?;
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let report = serde_json::from_str::<ScanReport>(&stdout).map_err(|error| {
        let stdout = diagnostic_excerpt(&stdout);
        let stderr = diagnostic_excerpt(&stderr);
        let details = match (stdout.is_empty(), stderr.is_empty()) {
            (true, true) => "没有诊断信息。".to_string(),
            (false, true) => format!("扫描组件输出：{stdout}"),
            (true, false) => format!("扫描组件诊断：{stderr}"),
            (false, false) => format!("扫描组件输出：{stdout}；诊断：{stderr}"),
        };
        user_error(format!("扫描结果解析失败：{error}。{details}"))
    })?;

    if !report.execution_successful {
        return Err(user_error("扫描未完整完成，请查看 skill 文件后重试。"));
    }
    Ok(report)
}

fn diagnostic_excerpt(value: &str) -> String {
    const MAX_CHARS: usize = 800;
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= MAX_CHARS {
        compact
    } else {
        format!("{}…", compact.chars().take(MAX_CHARS).collect::<String>())
    }
}
