mod archive;
mod raw;

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::targets::LocalSkill;

#[derive(Clone, Debug, Deserialize)]
pub struct RemoteSkill {
    pub name: String,
    pub repo: String,
    pub path: String,
    pub commit_sha: String,
    pub files: Vec<RemoteFile>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RemoteFile {
    pub path: String,
    pub size: u64,
}

pub async fn download_skill(
    skill: &RemoteSkill,
    destination_parent: &Path,
) -> Result<LocalSkill, String> {
    validate_remote_skill(skill)?;
    let directory_name = install_directory_name(skill)?;
    let destination = destination_parent.join(&directory_name);
    std::fs::create_dir_all(destination_parent)
        .map_err(|error| format!("无法创建下载缓存目录：{error}"))?;

    let client = reqwest::Client::builder()
        .user_agent("Super-Skill-Router/0.1")
        .build()
        .map_err(|error| format!("无法初始化下载客户端：{error}"))?;

    if let Err(raw_error) = raw::download(&client, skill, &destination).await {
        let _ = remove_directory(&destination);
        archive::download_and_extract(&client, skill, &destination)
            .await
            .map_err(|archive_error| {
                format!("按文件清单下载失败：{raw_error}；codeload 回退下载也失败：{archive_error}")
            })?;
    }

    if let Err(error) = verify_download(skill, &destination) {
        let _ = remove_directory(&destination);
        return Err(error);
    }
    Ok(LocalSkill {
        directory_name,
        source_dir: destination,
    })
}

fn verify_download(skill: &RemoteSkill, destination: &Path) -> Result<(), String> {
    if !destination.join("SKILL.md").is_file() {
        return Err("下载内容不完整：缺少 SKILL.md。".into());
    }
    for file in &skill.files {
        let path = destination.join(&file.path);
        let actual_size = std::fs::metadata(&path)
            .map_err(|error| format!("下载内容不完整：缺少 {}：{error}", file.path))?
            .len();
        if actual_size != file.size {
            return Err(format!("下载 {} 的大小与索引不符。", file.path));
        }
    }
    Ok(())
}

pub fn install_directory_name(skill: &RemoteSkill) -> Result<String, String> {
    let source_path = skill.path.trim_matches('/');
    let candidate = if source_path.is_empty() || source_path == "." {
        skill.repo.rsplit('/').next().unwrap_or_default()
    } else {
        source_path.rsplit('/').next().unwrap_or_default()
    };
    let normalized: String = candidate
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect();
    if normalized.is_empty() || normalized == "." || normalized == ".." {
        return Err(format!("无法从 skill 路径生成安全目录名：{}", skill.name));
    }
    Ok(normalized)
}

pub(crate) fn remote_path(skill: &RemoteSkill, relative_file: &str) -> Result<String, String> {
    validate_relative_path(relative_file)?;
    let root = skill.path.trim_matches('/');
    if root.is_empty() || root == "." {
        Ok(relative_file.into())
    } else {
        Ok(format!("{root}/{relative_file}"))
    }
}

pub(crate) fn validate_relative_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("索引包含不安全的文件路径：{value}"));
    }
    Ok(())
}

fn validate_remote_skill(skill: &RemoteSkill) -> Result<(), String> {
    if skill.repo.split('/').count() != 2
        || skill.repo.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/'))
        })
    {
        return Err("索引中的仓库名称无效。".into());
    }
    if skill.commit_sha.len() < 7
        || skill
            .commit_sha
            .chars()
            .any(|character| !character.is_ascii_hexdigit())
    {
        return Err("索引中的 commit SHA 无效。".into());
    }
    if skill.files.is_empty() {
        return Err("索引没有该 skill 的完整文件清单。".into());
    }
    for file in &skill.files {
        validate_relative_path(&file.path)?;
    }
    Ok(())
}

pub(crate) fn remove_directory(path: &PathBuf) -> std::io::Result<()> {
    if path.exists() {
        std::fs::remove_dir_all(path)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn skill(path: &str) -> RemoteSkill {
        RemoteSkill {
            name: "demo".into(),
            repo: "owner/repository".into(),
            path: path.into(),
            commit_sha: "abcdef0123456789".into(),
            files: vec![RemoteFile {
                path: "SKILL.md".into(),
                size: 1,
            }],
        }
    }
    #[test]
    fn preserves_nested_directory_name() {
        assert_eq!(
            install_directory_name(&skill("skills/pdf-processor")).expect("name"),
            "pdf-processor"
        );
    }
    #[test]
    fn uses_repository_name_for_root_skill() {
        assert_eq!(
            install_directory_name(&skill(".")).expect("name"),
            "repository"
        );
    }
    #[test]
    fn rejects_parent_traversal() {
        assert!(validate_relative_path("scripts/../../outside").is_err());
    }

    #[tokio::test]
    async fn downloads_complete_directory_at_commit_sha() {
        let download_root = tempfile::tempdir().expect("download root");
        let skill = RemoteSkill {
            name: "bazi".into(),
            repo: "jinchenma94/bazi-skill".into(),
            path: ".".into(),
            commit_sha: "bdd7f863d4450bf0e2fac84579ad6b45cfdfa25c".into(),
            files: vec![
                RemoteFile {
                    path: "SKILL.md".into(),
                    size: 10_753,
                },
                RemoteFile {
                    path: "references/classical-texts.md".into(),
                    size: 7_385,
                },
            ],
        };
        let downloaded = download_skill(&skill, download_root.path())
            .await
            .expect("download complete skill");
        assert!(downloaded.source_dir.join("SKILL.md").is_file());
        assert!(downloaded
            .source_dir
            .join("references")
            .join("classical-texts.md")
            .is_file());
    }
}
