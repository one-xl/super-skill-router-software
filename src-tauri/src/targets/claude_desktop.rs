use std::path::PathBuf;

use super::{InstallOutcome, InstalledSkill, LocalSkill, SkillTarget};
use crate::packager::package_for_claude_desktop;

pub struct ClaudeDesktopTarget {
    output_dir: PathBuf,
}
impl ClaudeDesktopTarget {
    pub fn with_output_dir(output_dir: PathBuf) -> Self {
        Self { output_dir }
    }
}
impl SkillTarget for ClaudeDesktopTarget {
    fn name(&self) -> &'static str {
        "Claude Desktop"
    }
    fn detect(&self) -> Option<PathBuf> {
        None
    }
    fn install_key(&self) -> String {
        "claude-desktop-account".into()
    }
    fn install(&self, skill: &LocalSkill) -> Result<InstallOutcome, String> {
        let zip_path = package_for_claude_desktop(skill, &self.output_dir)?;
        Ok(InstallOutcome::PackagedForUpload { zip_path })
    }
    fn uninstall(&self, _: &str) -> Result<(), String> {
        Err("Claude 桌面版 skill 存于账号侧，无法本地卸载。".into())
    }
    fn list_installed(&self) -> Result<Vec<InstalledSkill>, String> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn returns_a_packaged_for_upload_outcome() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        fs::create_dir_all(&source).expect("source");
        fs::write(source.join("SKILL.md"), "---\nname: demo\n---\n").expect("skill");
        let target = ClaudeDesktopTarget::with_output_dir(workspace.path().join("output"));
        let skill = LocalSkill {
            directory_name: "demo".into(),
            source_dir: source,
        };
        let InstallOutcome::PackagedForUpload { zip_path } =
            target.install(&skill).expect("package")
        else {
            panic!("expected upload package")
        };
        assert!(zip_path.is_file());
    }
}
