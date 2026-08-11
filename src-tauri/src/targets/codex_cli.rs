use std::path::PathBuf;

use super::{windows_home, InstallOutcome, InstalledSkill, LocalSkill, SkillTarget};

pub struct CodexCliTarget {
    home: Option<PathBuf>,
    codex_home: Option<PathBuf>,
}

impl CodexCliTarget {
    pub fn new() -> Self {
        Self {
            home: windows_home(),
            codex_home: std::env::var_os("CODEX_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
        }
    }
    #[cfg(test)]
    pub(crate) fn with_paths(home: PathBuf, codex_home: Option<PathBuf>) -> Self {
        Self {
            home: Some(home),
            codex_home,
        }
    }
    pub(crate) fn skills_root(&self) -> Option<PathBuf> {
        self.codex_home
            .clone()
            .or_else(|| self.home.as_ref().map(|home| home.join(".codex")))
            .map(|root| root.join("skills"))
    }
}

impl SkillTarget for CodexCliTarget {
    fn name(&self) -> &'static str {
        "Codex CLI"
    }
    fn detect(&self) -> Option<PathBuf> {
        let root = self.skills_root()?;
        if root.parent().is_some_and(|parent| parent.is_dir()) {
            Some(root)
        } else {
            None
        }
    }
    fn install(&self, _: &LocalSkill) -> Result<InstallOutcome, String> {
        Err("Codex CLI 部署将在 M4 提供。".into())
    }
    fn uninstall(&self, _: &str) -> Result<(), String> {
        Err("Codex CLI 卸载将在 M4 提供。".into())
    }
    fn list_installed(&self) -> Result<Vec<InstalledSkill>, String> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefers_custom_codex_home() {
        let home = tempfile::tempdir().expect("home");
        let custom = home.path().join("L-codex-home");
        let target = CodexCliTarget::with_paths(home.path().to_path_buf(), Some(custom.clone()));
        assert_eq!(target.skills_root(), Some(custom.join("skills")));
    }
    #[test]
    fn falls_back_to_user_codex_home() {
        let home = tempfile::tempdir().expect("home");
        let target = CodexCliTarget::with_paths(home.path().to_path_buf(), None);
        assert_eq!(
            target.skills_root(),
            Some(home.path().join(".codex").join("skills"))
        );
    }
}
