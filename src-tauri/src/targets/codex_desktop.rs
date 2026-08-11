use std::path::PathBuf;

use super::{
    CodexCliTarget, InstallOutcome, InstalledSkill, LocalSkill, SkillTarget, StagedUninstall,
};

pub struct CodexDesktopTarget {
    shared_home: CodexCliTarget,
}

impl CodexDesktopTarget {
    pub fn new() -> Self {
        Self {
            shared_home: CodexCliTarget::new(),
        }
    }
    #[cfg(test)]
    pub(crate) fn with_paths(home: PathBuf, codex_home: Option<PathBuf>) -> Self {
        Self {
            shared_home: CodexCliTarget::with_paths(home, codex_home),
        }
    }
}

impl SkillTarget for CodexDesktopTarget {
    fn name(&self) -> &'static str {
        "Codex Desktop"
    }
    fn detect(&self) -> Option<PathBuf> {
        self.shared_home.detect()
    }
    fn install_key(&self) -> String {
        self.shared_home.install_key()
    }
    fn install(&self, skill: &LocalSkill) -> Result<InstallOutcome, String> {
        self.shared_home.install(skill)
    }
    fn uninstall(&self, skill_name: &str) -> Result<(), String> {
        self.shared_home.uninstall(skill_name)
    }
    fn list_installed(&self) -> Result<Vec<InstalledSkill>, String> {
        self.shared_home.list_installed()
    }
    fn stage_uninstall(
        &self,
        skill_name: &str,
        transaction_id: &str,
    ) -> Result<Option<StagedUninstall>, String> {
        self.shared_home.stage_uninstall(skill_name, transaction_id)
    }
    fn read_skill_markdown(&self, skill_name: &str) -> Result<String, String> {
        self.shared_home.read_skill_markdown(skill_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uses_the_same_codex_home_as_cli() {
        let home = tempfile::tempdir().expect("home");
        let custom = home.path().join("custom");
        let target =
            CodexDesktopTarget::with_paths(home.path().to_path_buf(), Some(custom.clone()));
        assert_eq!(target.detect(), None);
        std::fs::create_dir_all(&custom).expect("custom codex home");
        assert_eq!(target.detect(), Some(custom.join("skills")));
    }
}
