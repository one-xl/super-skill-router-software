use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

use crate::fetcher::{self, RemoteSkill};
use crate::scanner::{self, ScanMode, ScanReport};
use crate::settings;
use crate::targets::{
    target_adapters_with_upload_dir, InstallOutcome, LocalSkill, TargetAdapter, TargetId,
};

#[derive(Default)]
pub struct PendingInstallStore(Mutex<HashMap<String, LocalSkill>>);

#[derive(Serialize)]
pub struct PreparedInstall {
    pub token: String,
    pub directory_name: String,
}

#[derive(Serialize)]
pub struct TargetInstallResult {
    pub target: TargetId,
    pub target_name: String,
    pub outcome: Option<InstallOutcome>,
    pub error: Option<String>,
    pub reused_physical_install: bool,
}

#[derive(Serialize)]
pub struct BatchInstallReport {
    pub results: Vec<TargetInstallResult>,
}

#[tauri::command]
pub async fn prepare_skill_install(
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
    let directory_name = downloaded.directory_name.clone();
    pending
        .0
        .lock()
        .map_err(|_| "安装状态不可用，请重试。".to_string())?
        .insert(token.clone(), downloaded);
    Ok(PreparedInstall {
        token,
        directory_name,
    })
}

#[tauri::command]
pub async fn scan_prepared_skill(
    app: AppHandle,
    token: String,
    mode: ScanMode,
    pending: tauri::State<'_, PendingInstallStore>,
) -> Result<ScanReport, String> {
    let skill = pending
        .0
        .lock()
        .map_err(|_| "安装状态不可用，请重试。".to_string())?
        .get(&token)
        .cloned()
        .ok_or_else(|| "安装准备已过期，请重新下载。".to_string())?;
    let configuration = if matches!(mode, ScanMode::Deep) {
        Some(settings::load(&app)?.deep_scan)
    } else {
        None
    };
    scanner::scan_directory(app, &skill.source_dir, mode, configuration.as_ref()).await
}

#[tauri::command]
pub fn install_prepared_skill(
    app: AppHandle,
    token: String,
    targets: Vec<TargetId>,
    pending: tauri::State<'_, PendingInstallStore>,
) -> Result<BatchInstallReport, String> {
    if targets.is_empty() {
        return Err("请至少选择一个部署目标。".into());
    }
    let skill = pending
        .0
        .lock()
        .map_err(|_| "安装状态不可用，请重试。".to_string())?
        .get(&token)
        .cloned()
        .ok_or_else(|| "安装准备已过期，请重新下载并扫描。".to_string())?;
    let upload_dir = app
        .path()
        .download_dir()
        .map_err(|error| format!("无法确定下载目录，无法创建 Claude Desktop 上传包：{error}"))?
        .join("Super Skill Router")
        .join("Claude Desktop Uploads");
    let report = deploy_to_adapters(
        &skill,
        &targets,
        target_adapters_with_upload_dir(upload_dir),
    );
    for result in &report.results {
        if let Some(InstallOutcome::PackagedForUpload { zip_path }) = &result.outcome {
            let _ = app.opener().reveal_item_in_dir(zip_path);
        }
    }
    let any_success = report.results.iter().any(|result| result.outcome.is_some());
    if any_success {
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
    }
    Ok(report)
}

#[tauri::command]
pub fn reveal_packaged_skill(app: AppHandle, zip_path: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(zip_path);
    if path
        .extension()
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("zip"))
        || !path.is_file()
    {
        return Err("上传包不存在或不是 zip 文件。".into());
    }
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|error| format!("无法打开上传包所在目录：{error}"))
}

fn deploy_to_adapters(
    skill: &LocalSkill,
    targets: &[TargetId],
    adapters: Vec<TargetAdapter>,
) -> BatchInstallReport {
    let mut physical_installs: HashMap<String, InstallOutcome> = HashMap::new();
    let mut results = Vec::new();
    for target_id in targets.iter().copied() {
        let Some(adapter) = adapters.iter().find(|adapter| adapter.id == target_id) else {
            results.push(TargetInstallResult {
                target: target_id,
                target_name: target_id.label().into(),
                outcome: None,
                error: Some("未知部署目标。".into()),
                reused_physical_install: false,
            });
            continue;
        };
        let install_key = adapter.target.install_key();
        if let Some(outcome) = physical_installs.get(&install_key) {
            results.push(TargetInstallResult {
                target: target_id,
                target_name: adapter.target.name().into(),
                outcome: Some(outcome.clone()),
                error: None,
                reused_physical_install: true,
            });
            continue;
        }
        match adapter.target.install(skill) {
            Ok(outcome) => {
                physical_installs.insert(install_key, outcome.clone());
                results.push(TargetInstallResult {
                    target: target_id,
                    target_name: adapter.target.name().into(),
                    outcome: Some(outcome),
                    error: None,
                    reused_physical_install: false,
                });
            }
            Err(error) => results.push(TargetInstallResult {
                target: target_id,
                target_name: adapter.target.name().into(),
                outcome: None,
                error: Some(error),
                reused_physical_install: false,
            }),
        }
    }
    BatchInstallReport { results }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use crate::targets::{InstalledSkill, SkillTarget};

    use super::*;

    struct MockTarget {
        name: &'static str,
        key: String,
        calls: Arc<AtomicUsize>,
    }

    impl SkillTarget for MockTarget {
        fn name(&self) -> &'static str {
            self.name
        }
        fn detect(&self) -> Option<PathBuf> {
            Some(PathBuf::from("C:/mock/skills"))
        }
        fn install_key(&self) -> String {
            self.key.clone()
        }
        fn install(&self, _: &LocalSkill) -> Result<InstallOutcome, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(InstallOutcome::Installed {
                path: PathBuf::from("C:/mock/skills/demo"),
            })
        }
        fn uninstall(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn list_installed(&self) -> Result<Vec<InstalledSkill>, String> {
            Ok(Vec::new())
        }
        fn stage_uninstall(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Option<crate::targets::StagedUninstall>, String> {
            Ok(None)
        }
        fn read_skill_markdown(&self, _: &str) -> Result<String, String> {
            Ok(String::new())
        }
    }

    #[test]
    fn deduplicates_shared_codex_physical_install() {
        let calls = Arc::new(AtomicUsize::new(0));
        let first = TargetAdapter {
            id: TargetId::CodexCli,
            target: Box::new(MockTarget {
                name: "Codex CLI",
                key: "shared-codex-home".into(),
                calls: calls.clone(),
            }),
        };
        let second = TargetAdapter {
            id: TargetId::CodexDesktop,
            target: Box::new(MockTarget {
                name: "Codex Desktop",
                key: "shared-codex-home".into(),
                calls: calls.clone(),
            }),
        };
        let skill = LocalSkill {
            directory_name: "demo".into(),
            source_dir: PathBuf::from("C:/source/demo"),
        };
        let report = deploy_to_adapters(
            &skill,
            &[TargetId::CodexCli, TargetId::CodexDesktop],
            vec![first, second],
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(report.results.len(), 2);
        assert!(!report.results[0].reused_physical_install);
        assert!(report.results[1].reused_physical_install);
        assert!(report.results.iter().all(|result| result.outcome.is_some()));
    }
}
