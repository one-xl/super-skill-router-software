use std::path::PathBuf;

use super::{
    install_directory, list_directories, uninstall_directory, windows_home, InstallOutcome,
    InstalledSkill, LocalSkill, SkillTarget,
};

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
    fn install_key(&self) -> String {
        self.skills_root()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| "codex-home".into())
    }
    fn install(&self, skill: &LocalSkill) -> Result<InstallOutcome, String> {
        let root = self
            .skills_root()
            .ok_or_else(|| "无法确定 CODEX_HOME，无法部署 Codex CLI。".to_string())?;
        install_directory(skill, &root)
    }
    fn uninstall(&self, skill_name: &str) -> Result<(), String> {
        let root = self
            .skills_root()
            .ok_or_else(|| "无法确定 CODEX_HOME，无法卸载 Codex CLI。".to_string())?;
        uninstall_directory(&root, skill_name)
    }
    fn list_installed(&self) -> Result<Vec<InstalledSkill>, String> {
        let root = self
            .skills_root()
            .ok_or_else(|| "无法确定 CODEX_HOME，无法读取 Codex CLI skills。".to_string())?;
        list_directories(&root)
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

    #[test]
    fn installs_and_uninstalls_complete_skill_under_custom_codex_home() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        std::fs::create_dir_all(source.join("references")).expect("references");
        std::fs::write(source.join("SKILL.md"), "---\nname: demo\n---\n").expect("skill");
        std::fs::write(source.join("references").join("guide.md"), "full directory")
            .expect("reference");
        let codex_home = workspace.path().join("custom-codex");
        let target =
            CodexCliTarget::with_paths(workspace.path().join("home"), Some(codex_home.clone()));
        let skill = LocalSkill {
            directory_name: "demo-directory".into(),
            source_dir: source,
        };
        let InstallOutcome::Installed { path } = target.install(&skill).expect("install") else {
            panic!("expected installation")
        };
        assert_eq!(path, codex_home.join("skills").join("demo-directory"));
        assert!(path.join("references").join("guide.md").is_file());
        assert_eq!(target.list_installed().expect("list").len(), 1);
        target.uninstall("demo-directory").expect("uninstall");
        assert!(!path.exists());
    }

    #[test]
    #[ignore = "writes a temporary skill into the real CODEX_HOME directory"]
    fn installs_into_the_real_default_codex_home() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("source");
        std::fs::create_dir_all(source.join("scripts")).expect("scripts");
        std::fs::write(source.join("SKILL.md"), "---\nname: m4 smoke\n---\n").expect("skill");
        std::fs::write(
            source.join("scripts").join("check.txt"),
            "complete directory",
        )
        .expect("nested file");
        let target = CodexCliTarget::new();
        let root = target.skills_root().expect("CODEX_HOME root");
        let name = format!("super-skill-router-m4-smoke-{}", uuid::Uuid::new_v4());
        let skill = LocalSkill {
            directory_name: name.clone(),
            source_dir: source,
        };
        let InstallOutcome::Installed { path } = target.install(&skill).expect("real install")
        else {
            panic!("expected installation")
        };
        assert!(path.starts_with(&root));
        assert!(path.join("scripts").join("check.txt").is_file());
        target.uninstall(&name).expect("real uninstall");
        assert!(!path.exists());
    }
}
