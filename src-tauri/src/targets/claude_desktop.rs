use std::path::PathBuf;

use super::{InstallOutcome, InstalledSkill, LocalSkill, SkillTarget};

pub struct ClaudeDesktopTarget;
impl ClaudeDesktopTarget {
    pub fn new() -> Self {
        Self
    }
}
impl SkillTarget for ClaudeDesktopTarget {
    fn name(&self) -> &'static str {
        "Claude Desktop"
    }
    fn detect(&self) -> Option<PathBuf> {
        None
    }
    fn install(&self, _: &LocalSkill) -> Result<InstallOutcome, String> {
        Err("Claude 桌面版没有本地 skills 目录；请在 M5 使用 zip 打包和上传引导。".into())
    }
    fn uninstall(&self, _: &str) -> Result<(), String> {
        Err("Claude 桌面版 skill 存于账号侧，无法本地卸载。".into())
    }
    fn list_installed(&self) -> Result<Vec<InstalledSkill>, String> {
        Ok(Vec::new())
    }
}
