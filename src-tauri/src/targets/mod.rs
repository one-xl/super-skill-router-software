mod claude_code;
mod claude_desktop;
mod codex_cli;
mod codex_desktop;

use std::path::PathBuf;

pub use claude_code::ClaudeCodeTarget;
pub use claude_desktop::ClaudeDesktopTarget;
pub use codex_cli::CodexCliTarget;
pub use codex_desktop::CodexDesktopTarget;

#[derive(Clone, Debug)]
pub struct LocalSkill {
    pub directory_name: String,
    pub source_dir: PathBuf,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Serialize)]
pub struct InstalledSkill {
    pub directory_name: String,
    pub path: PathBuf,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallOutcome {
    Installed { path: PathBuf },
    PackagedForUpload { zip_path: PathBuf },
}

#[allow(dead_code)]
pub trait SkillTarget {
    fn name(&self) -> &'static str;
    fn detect(&self) -> Option<PathBuf>;
    fn install(&self, skill: &LocalSkill) -> Result<InstallOutcome, String>;
    fn uninstall(&self, skill_name: &str) -> Result<(), String>;
    fn list_installed(&self) -> Result<Vec<InstalledSkill>, String>;
}

#[derive(serde::Serialize)]
pub struct TargetDetection {
    pub name: String,
    pub path: Option<PathBuf>,
    pub available: bool,
}

#[tauri::command]
pub fn detect_skill_targets() -> Vec<TargetDetection> {
    let targets: Vec<Box<dyn SkillTarget>> = vec![
        Box::new(ClaudeCodeTarget::new()),
        Box::new(CodexCliTarget::new()),
        Box::new(CodexDesktopTarget::new()),
        Box::new(ClaudeDesktopTarget::new()),
    ];
    targets
        .into_iter()
        .map(|target| {
            let path = target.detect();
            TargetDetection {
                name: target.name().into(),
                available: path.is_some(),
                path,
            }
        })
        .collect()
}

pub(crate) fn windows_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            Some(PathBuf::from(drive).join(path))
        })
}

pub(crate) fn validate_directory_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':'])
        || name.chars().any(|character| character.is_control())
    {
        return Err("skill 目录名无效，无法安全部署。".into());
    }
    Ok(())
}
