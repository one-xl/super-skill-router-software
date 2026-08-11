use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use crate::targets::{
    commit_staged_uninstall, rollback_staged_uninstall, target_adapters, InstalledSkill,
    StagedUninstall, TargetId,
};

#[derive(Default)]
pub struct PendingUninstallStore(Mutex<HashMap<String, Vec<StagedUninstall>>>);

#[derive(Serialize)]
pub struct TargetSkillInventory {
    pub id: TargetId,
    pub name: String,
    pub skills: Vec<InstalledSkill>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct PreparedUninstall {
    pub token: String,
    pub staged_targets: Vec<TargetId>,
}

#[tauri::command]
pub fn list_installed_skills() -> Vec<TargetSkillInventory> {
    target_adapters()
        .into_iter()
        .map(|adapter| match adapter.target.list_installed() {
            Ok(skills) => TargetSkillInventory {
                id: adapter.id,
                name: adapter.target.name().into(),
                skills,
                error: None,
            },
            Err(error) => TargetSkillInventory {
                id: adapter.id,
                name: adapter.target.name().into(),
                skills: Vec::new(),
                error: Some(error),
            },
        })
        .collect()
}

#[tauri::command]
pub fn read_installed_skill_markdown(
    target: TargetId,
    directory_name: String,
) -> Result<String, String> {
    let adapters = target_adapters();
    let adapter = adapters
        .into_iter()
        .find(|adapter| adapter.id == target)
        .ok_or_else(|| "未知部署目标。".to_string())?;
    adapter.target.read_skill_markdown(&directory_name)
}

#[tauri::command]
pub fn prepare_skill_uninstall(
    directory_name: String,
    targets: Vec<TargetId>,
    pending: State<'_, PendingUninstallStore>,
) -> Result<PreparedUninstall, String> {
    if targets.is_empty() {
        return Err("请至少选择一个本地部署目标。".into());
    }
    let token = Uuid::new_v4().to_string();
    let adapters = target_adapters();
    let mut staged = Vec::new();
    let mut seen_keys = HashSet::new();
    let mut staged_targets = Vec::new();
    for target in targets {
        let Some(adapter) = adapters.iter().find(|adapter| adapter.id == target) else {
            rollback_all(&staged);
            return Err("未知部署目标。".into());
        };
        if !seen_keys.insert(adapter.target.install_key()) {
            staged_targets.push(target);
            continue;
        }
        match adapter.target.stage_uninstall(&directory_name, &token) {
            Ok(Some(item)) => {
                staged.push(item);
                staged_targets.push(target);
            }
            Ok(None) => staged_targets.push(target),
            Err(error) => {
                rollback_all(&staged);
                return Err(error);
            }
        }
    }
    pending
        .0
        .lock()
        .map_err(|_| "卸载状态不可用，请重试。".to_string())?
        .insert(token.clone(), staged);
    Ok(PreparedUninstall {
        token,
        staged_targets,
    })
}

#[tauri::command]
pub fn commit_skill_uninstall(
    token: String,
    pending: State<'_, PendingUninstallStore>,
) -> Result<(), String> {
    let staged = take(&token, &pending)?;
    for item in &staged {
        if let Err(error) = commit_staged_uninstall(item) {
            rollback_all(&staged);
            return Err(error);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn rollback_skill_uninstall(
    token: String,
    pending: State<'_, PendingUninstallStore>,
) -> Result<(), String> {
    rollback_all(&take(&token, &pending)?);
    Ok(())
}

fn take(token: &str, pending: &PendingUninstallStore) -> Result<Vec<StagedUninstall>, String> {
    pending
        .0
        .lock()
        .map_err(|_| "卸载状态不可用，请重试。".to_string())?
        .remove(token)
        .ok_or_else(|| "卸载准备已过期，请重试。".to_string())
}
fn rollback_all(staged: &[StagedUninstall]) {
    for item in staged.iter().rev() {
        let _ = rollback_staged_uninstall(item);
    }
}
