use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::fetcher::{self, RemoteSkill};
use crate::scanner::{self, ScanMode, ScanReport};
use crate::targets::{ClaudeCodeTarget, InstallOutcome, LocalSkill, SkillTarget};

#[derive(Default)]
pub struct PendingInstallStore(Mutex<HashMap<String, LocalSkill>>);

#[derive(Serialize)]
pub struct PreparedInstall {
    pub token: String,
    pub directory_name: String,
    pub report: ScanReport,
}

#[tauri::command]
pub async fn prepare_claude_code_install(
    app: AppHandle,
    skill: RemoteSkill,
    pending: tauri::State<'_, PendingInstallStore>,
) -> Result<PreparedInstall, String> {
    let token = Uuid::new_v4().to_string();
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("无法确定安装缓存目录：{error}"))?
        .join("pending-installs")
        .join(&token);
    let downloaded = fetcher::download_skill(&skill, &cache).await?;
    let report =
        match scanner::scan_directory(app.clone(), &downloaded.source_dir, ScanMode::Fast).await {
            Ok(report) => report,
            Err(error) => {
                let _ = fs::remove_dir_all(&cache);
                return Err(error);
            }
        };
    let directory_name = downloaded.directory_name.clone();
    pending
        .0
        .lock()
        .map_err(|_| "安装状态不可用，请重试。".to_string())?
        .insert(token.clone(), downloaded);
    Ok(PreparedInstall {
        token,
        directory_name,
        report,
    })
}

#[tauri::command]
pub fn install_prepared_claude_code(
    token: String,
    pending: tauri::State<'_, PendingInstallStore>,
) -> Result<InstallOutcome, String> {
    let skill = pending
        .0
        .lock()
        .map_err(|_| "安装状态不可用，请重试。".to_string())?
        .get(&token)
        .cloned()
        .ok_or_else(|| "安装准备已过期，请重新下载并扫描。".to_string())?;
    let target = ClaudeCodeTarget::new();
    let outcome = target.install(&skill)?;
    let removed = pending
        .0
        .lock()
        .map_err(|_| "安装完成，但无法清理安装状态。".to_string())?
        .remove(&token);
    if let Some(downloaded) = removed {
        if let Some(parent) = downloaded.source_dir.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
    Ok(outcome)
}
