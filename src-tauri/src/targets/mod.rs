mod claude_code;
mod claude_desktop;
mod codex_cli;
mod codex_desktop;

use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub use claude_code::ClaudeCodeTarget;
pub use claude_desktop::ClaudeDesktopTarget;
pub use codex_cli::CodexCliTarget;
pub use codex_desktop::CodexDesktopTarget;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetId {
    ClaudeCode,
    CodexCli,
    CodexDesktop,
    ClaudeDesktop,
}

impl TargetId {
    pub fn label(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::CodexCli => "Codex CLI",
            Self::CodexDesktop => "Codex Desktop",
            Self::ClaudeDesktop => "Claude Desktop",
        }
    }
}

pub struct TargetAdapter {
    pub id: TargetId,
    pub target: Box<dyn SkillTarget>,
}

pub fn target_adapters() -> Vec<TargetAdapter> {
    target_adapters_with_upload_dir(
        windows_home()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Downloads")
            .join("Super Skill Router")
            .join("Claude Desktop Uploads"),
    )
}

pub fn target_adapters_with_upload_dir(upload_dir: PathBuf) -> Vec<TargetAdapter> {
    vec![
        TargetAdapter {
            id: TargetId::ClaudeCode,
            target: Box::new(ClaudeCodeTarget::new()),
        },
        TargetAdapter {
            id: TargetId::CodexCli,
            target: Box::new(CodexCliTarget::new()),
        },
        TargetAdapter {
            id: TargetId::CodexDesktop,
            target: Box::new(CodexDesktopTarget::new()),
        },
        TargetAdapter {
            id: TargetId::ClaudeDesktop,
            target: Box::new(ClaudeDesktopTarget::with_output_dir(upload_dir)),
        },
    ]
}

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
    fn install_key(&self) -> String;
    fn install(&self, skill: &LocalSkill) -> Result<InstallOutcome, String>;
    fn uninstall(&self, skill_name: &str) -> Result<(), String>;
    fn list_installed(&self) -> Result<Vec<InstalledSkill>, String>;
}

pub(crate) fn install_directory(
    skill: &LocalSkill,
    skills_root: &Path,
) -> Result<InstallOutcome, String> {
    validate_directory_name(&skill.directory_name)?;
    if !skill.source_dir.is_dir() || !skill.source_dir.join("SKILL.md").is_file() {
        return Err("下载内容不完整：skill 根目录必须包含 SKILL.md。".into());
    }
    ensure_writable(skills_root)?;
    let destination = skills_root.join(&skill.directory_name);
    let staging = skills_root.join(format!(
        ".{}.installing-{}",
        skill.directory_name,
        Uuid::new_v4()
    ));
    let backup = skills_root.join(format!(
        ".{}.backup-{}",
        skill.directory_name,
        Uuid::new_v4()
    ));
    if let Err(error) = copy_directory(&skill.source_dir, &staging) {
        let _ = remove_path(&staging);
        return Err(error);
    }
    let moved_existing = if destination.exists() {
        fs::rename(&destination, &backup).map_err(|error| {
            let _ = remove_path(&staging);
            format!("无法备份已有 skill，安装已取消：{error}")
        })?;
        true
    } else {
        false
    };
    if let Err(error) = fs::rename(&staging, &destination) {
        if moved_existing {
            let _ = fs::rename(&backup, &destination);
        }
        let _ = remove_path(&staging);
        return Err(format!("无法完成安装，已回滚：{error}"));
    }
    if moved_existing {
        remove_path(&backup).map_err(|error| format!("安装完成，但无法清理旧版本备份：{error}"))?;
    }
    Ok(InstallOutcome::Installed { path: destination })
}

pub(crate) fn uninstall_directory(skills_root: &Path, skill_name: &str) -> Result<(), String> {
    validate_directory_name(skill_name)?;
    let destination = skills_root.join(skill_name);
    if destination.exists() {
        remove_path(&destination).map_err(|error| format!("无法卸载 skill：{error}"))
    } else {
        Ok(())
    }
}

pub(crate) fn list_directories(skills_root: &Path) -> Result<Vec<InstalledSkill>, String> {
    if !skills_root.is_dir() {
        return Ok(Vec::new());
    }
    let entries =
        fs::read_dir(skills_root).map_err(|error| format!("无法读取 skills 目录：{error}"))?;
    let mut skills = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("无法读取 skill：{error}"))?;
        let path = entry.path();
        if path.is_dir() && path.join("SKILL.md").is_file() {
            skills.push(InstalledSkill {
                directory_name: entry.file_name().to_string_lossy().into_owned(),
                path,
            });
        }
    }
    skills.sort_by(|left, right| left.directory_name.cmp(&right.directory_name));
    Ok(skills)
}

fn ensure_writable(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|error| format!("无法创建 skills 目录：{error}"))?;
    let probe = root.join(format!(
        ".super-skill-router-write-check-{}",
        Uuid::new_v4()
    ));
    fs::write(&probe, b"check").map_err(|error| format!("skills 目录没有写入权限：{error}"))?;
    fs::remove_file(&probe).map_err(|error| format!("无法完成写入权限检查：{error}"))
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir(destination).map_err(|error| format!("无法创建安装临时目录：{error}"))?;
    for entry in fs::read_dir(source).map_err(|error| format!("无法读取 skill 文件：{error}"))?
    {
        let entry = entry.map_err(|error| format!("无法读取 skill 文件：{error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("无法读取 skill 文件属性：{error}"))?;
        let destination_path = destination.join(entry.file_name());
        if file_type.is_symlink() {
            return Err("skill 包含符号链接，无法安全安装。".into());
        }
        if file_type.is_dir() {
            copy_directory(&entry.path(), &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), destination_path)
                .map_err(|error| format!("无法复制 skill 文件：{error}"))?;
        }
    }
    Ok(())
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[derive(serde::Serialize)]
pub struct TargetDetection {
    pub id: TargetId,
    pub name: String,
    pub path: Option<PathBuf>,
    pub available: bool,
}

#[tauri::command]
pub fn detect_skill_targets() -> Vec<TargetDetection> {
    target_adapters()
        .into_iter()
        .map(|adapter| {
            let path = adapter.target.detect();
            TargetDetection {
                id: adapter.id,
                name: adapter.target.name().into(),
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
