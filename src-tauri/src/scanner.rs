use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

const SCAN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
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
    if !directory.is_dir() {
        return Err(user_error(
            "请选择一个存在的 skill 文件夹，而不是单个 SKILL.md 文件。",
        ));
    }

    let command = app
        .shell()
        .sidecar(mode.sidecar_name())
        .map_err(|error| user_error(format!("扫描组件不可用：{error}")))?
        .args(mode.arguments(&skill_path));
    let output = tokio::time::timeout(SCAN_TIMEOUT, command.output())
        .await
        .map_err(|_| {
            user_error("扫描在 30 秒后超时。可改用快速扫描，或检查 skill 中是否有异常大的文件。")
        })?
        .map_err(|error| user_error(format!("无法启动扫描组件：{error}")))?;

    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| user_error("扫描组件返回了无法读取的结果。"))?;
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let report = serde_json::from_str::<ScanReport>(&stdout).map_err(|error| {
        let details = if stderr.is_empty() {
            "没有诊断信息。"
        } else {
            &stderr
        };
        user_error(format!("扫描结果解析失败：{error}。{details}"))
    })?;

    if !report.execution_successful {
        return Err(user_error("扫描未完整完成，请查看 skill 文件后重试。"));
    }
    Ok(report)
}
