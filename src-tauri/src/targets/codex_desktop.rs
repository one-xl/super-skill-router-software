use std::path::PathBuf;

use super::{CodexCliTarget, InstallOutcome, InstalledSkill, LocalSkill, SkillTarget};

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
    fn install(&self, _: &LocalSkill) -> Result<InstallOutcome, String> {
        Err("Codex 桌面版与 Codex CLI 共用 CODEX_HOME；部署将在 M4 合并执行。".into())
    }
    fn uninstall(&self, _: &str) -> Result<(), String> {
        Err("Codex 桌面版卸载将在 M4 提供。".into())
    }
    fn list_installed(&self) -> Result<Vec<InstalledSkill>, String> {
        Ok(Vec::new())
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
