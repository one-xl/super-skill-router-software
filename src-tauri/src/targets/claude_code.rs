use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::{
    read_skill_markdown_from_directory, stage_uninstall_directory, validate_directory_name,
    windows_home, InstallOutcome, InstalledSkill, LocalSkill, SkillTarget, StagedUninstall,
};

pub struct ClaudeCodeTarget {
    home: Option<PathBuf>,
}

impl ClaudeCodeTarget {
    pub fn new() -> Self {
        Self {
            home: windows_home(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_home(home: PathBuf) -> Self {
        Self { home: Some(home) }
    }

    fn claude_root(&self) -> Result<PathBuf, String> {
        self.home
            .as_ref()
            .map(|home| home.join(".claude"))
            .ok_or_else(|| "无法确定 Windows 用户目录，无法定位 Claude Code。".into())
    }

    fn skills_root(&self) -> Result<PathBuf, String> {
        Ok(self.claude_root()?.join("skills"))
    }
}

impl SkillTarget for ClaudeCodeTarget {
    fn name(&self) -> &'static str {
        "Claude Code"
    }

    fn detect(&self) -> Option<PathBuf> {
        let root = self.claude_root().ok()?;
        root.is_dir().then_some(root.join("skills"))
    }

    fn install_key(&self) -> String {
        self.skills_root()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "claude-code".into())
    }

    fn install(&self, skill: &LocalSkill) -> Result<InstallOutcome, String> {
        validate_directory_name(&skill.directory_name)?;
        validate_source(&skill.source_dir)?;

        let skills_root = self.skills_root()?;
        ensure_writable(&skills_root)?;
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
            return Err(format!("无法完成 Claude Code 安装，已回滚：{error}"));
        }

        if moved_existing {
            remove_path(&backup)
                .map_err(|error| format!("安装完成，但无法清理旧版本备份：{error}"))?;
        }
        Ok(InstallOutcome::Installed { path: destination })
    }

    fn uninstall(&self, skill_name: &str) -> Result<(), String> {
        validate_directory_name(skill_name)?;
        let destination = self.skills_root()?.join(skill_name);
        if !destination.exists() {
            return Ok(());
        }
        remove_path(&destination).map_err(|error| format!("无法卸载 Claude Code skill：{error}"))
    }

    fn list_installed(&self) -> Result<Vec<InstalledSkill>, String> {
        let root = self.skills_root()?;
        if !root.is_dir() {
            return Ok(Vec::new());
        }
        let entries =
            fs::read_dir(&root).map_err(|error| format!("无法读取 Claude Code skills：{error}"))?;
        let mut skills = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| format!("无法读取 Claude Code skill：{error}"))?;
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

    fn stage_uninstall(
        &self,
        skill_name: &str,
        transaction_id: &str,
    ) -> Result<Option<StagedUninstall>, String> {
        stage_uninstall_directory(&self.skills_root()?, skill_name, transaction_id)
    }

    fn read_skill_markdown(&self, skill_name: &str) -> Result<String, String> {
        read_skill_markdown_from_directory(&self.skills_root()?, skill_name)
    }
}

fn validate_source(source: &Path) -> Result<(), String> {
    if !source.is_dir() || !source.join("SKILL.md").is_file() {
        return Err("下载内容不完整：skill 根目录必须包含 SKILL.md。".into());
    }
    Ok(())
}

fn ensure_writable(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("无法创建 Claude Code skills 目录：{error}"))?;
    let probe = root.join(format!(
        ".super-skill-router-write-check-{}",
        Uuid::new_v4()
    ));
    fs::write(&probe, b"check")
        .map_err(|error| format!("Claude Code skills 目录没有写入权限：{error}"))?;
    fs::remove_file(&probe).map_err(|error| format!("无法完成 Claude Code 写入权限检查：{error}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_existing_claude_directory() {
        let home = tempfile::tempdir().expect("temporary home");
        let target = ClaudeCodeTarget::with_home(home.path().to_path_buf());
        assert!(target.detect().is_none());
        fs::create_dir_all(home.path().join(".claude")).expect("claude directory");
        assert_eq!(
            target.detect(),
            Some(home.path().join(".claude").join("skills"))
        );
    }

    #[test]
    fn installs_and_uninstalls_the_complete_directory() {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let source = workspace.path().join("source-skill");
        fs::create_dir_all(source.join("scripts")).expect("script directory");
        fs::write(source.join("SKILL.md"), "---\nname: source\n---\n").expect("skill file");
        fs::write(source.join("scripts").join("helper.py"), "print('ok')").expect("script file");
        let target = ClaudeCodeTarget::with_home(workspace.path().join("home"));
        let skill = LocalSkill {
            directory_name: "source-skill".into(),
            source_dir: source,
        };

        let InstallOutcome::Installed { path } = target.install(&skill).expect("install") else {
            panic!("expected installed outcome")
        };
        assert!(path.join("SKILL.md").is_file());
        assert!(path.join("scripts").join("helper.py").is_file());
        assert_eq!(target.list_installed().expect("list").len(), 1);
        target.uninstall("source-skill").expect("uninstall");
        assert!(!path.exists());
    }

    #[test]
    #[ignore = "writes a temporary skill into the real Claude Code directory"]
    fn installs_into_the_real_claude_code_directory() {
        let target = ClaudeCodeTarget::new();
        assert!(
            target.detect().is_some(),
            "Claude Code directory must exist for this smoke test"
        );
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let source = workspace.path().join("m3-smoke-source");
        fs::create_dir_all(source.join("scripts")).expect("script directory");
        fs::write(
            source.join("SKILL.md"),
            "---\nname: super-skill-router-m3-smoke\ndescription: temporary M3 verification\n---\n",
        )
        .expect("skill file");
        fs::write(
            source.join("scripts").join("check.txt"),
            "complete directory",
        )
        .expect("nested file");
        let name = format!("super-skill-router-m3-smoke-{}", Uuid::new_v4());
        let skill = LocalSkill {
            directory_name: name.clone(),
            source_dir: source,
        };
        let InstallOutcome::Installed { path } = target.install(&skill).expect("real install")
        else {
            panic!("expected installed outcome")
        };
        assert!(path.join("SKILL.md").is_file());
        assert!(path.join("scripts").join("check.txt").is_file());
        target.uninstall(&name).expect("real uninstall");
        assert!(!path.exists());
    }
}
