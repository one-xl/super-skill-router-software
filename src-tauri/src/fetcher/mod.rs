mod archive;
mod raw;

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::targets::LocalSkill;

#[derive(Clone, Debug, Deserialize)]
pub struct RemoteSkill {
    pub name: String,
    pub repo: String,
    pub path: String,
    #[serde(default)]
    pub default_branch: String,
    pub commit_sha: String,
    pub files: Vec<RemoteFile>,
    #[serde(default)]
    pub remote_source: String,
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

/// Use SkillsMP's directory manifest as the primary source, while pinning every raw download
/// to the resolved commit SHA. A codeload subtree extraction remains the failure fallback.
pub async fn download_skillsmp_skill(
    skill: &RemoteSkill,
    destination_parent: &Path,
    api_key: Option<&str>,
) -> Result<(LocalSkill, String), String> {
    if skill.remote_source != "skillsmp" {
        return Err("该远程 skill 的下载来源无效。".into());
    }
    validate_discovered_skill(skill)?;
    let directory_name = install_directory_name(skill)?;
    let destination = destination_parent.join(&directory_name);
    std::fs::create_dir_all(destination_parent)
        .map_err(|error| format!("无法创建下载缓存目录：{error}"))?;
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Super-Skill-Router/0.1")
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(|error| format!("无法初始化下载客户端：{error}"))?;
    let commit_sha = resolve_commit_sha(&client, skill).await?;
    let manifest_download =
        download_from_skillsmp_manifest(&client, skill, &commit_sha, api_key, &destination).await;
    if let Err(manifest_error) = manifest_download {
        let _ = remove_directory(&destination);
        archive::download_and_extract_tree(&client, skill, &commit_sha, &destination)
            .await
            .map_err(|archive_error| {
                format!(
                    "SkillsMP 目录接口下载失败：{manifest_error}；GitHub 归档回退也失败：{archive_error}"
                )
            })?;
    }
    if !destination.join("SKILL.md").is_file() {
        let _ = remove_directory(&destination);
        return Err("下载内容不完整：SkillsMP 指向的目录中缺少 SKILL.md。".into());
    }
    Ok((
        LocalSkill {
            directory_name,
            source_dir: destination,
        },
        commit_sha,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillsMpContentsResponse {
    files: Vec<SkillsMpContentFile>,
    #[serde(default)]
    limit_reason: Option<String>,
    #[serde(default)]
    skipped_files: u64,
    #[serde(default)]
    truncated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillsMpContentFile {
    path: String,
    size: u64,
    raw_url: String,
}

async fn download_from_skillsmp_manifest(
    client: &reqwest::Client,
    skill: &RemoteSkill,
    commit_sha: &str,
    api_key: Option<&str>,
    destination: &Path,
) -> Result<(), String> {
    let manifest = fetch_skillsmp_manifest(client, skill, api_key).await?;
    validate_skillsmp_manifest(skill, &manifest)?;
    let pinned = RemoteSkill {
        name: skill.name.clone(),
        repo: skill.repo.clone(),
        path: skill.path.clone(),
        default_branch: skill.default_branch.clone(),
        commit_sha: commit_sha.into(),
        files: manifest
            .files
            .into_iter()
            .map(|file| RemoteFile {
                path: file.path,
                size: file.size,
            })
            .collect(),
        remote_source: skill.remote_source.clone(),
    };
    if let Err(raw_error) = raw::download(client, &pinned, destination).await {
        let _ = remove_directory(&destination.to_path_buf());
        archive::download_and_extract(client, &pinned, destination)
            .await
            .map_err(|archive_error| {
                format!(
                    "按 SkillsMP 文件清单下载失败：{raw_error}；归档回退也失败：{archive_error}"
                )
            })?;
    }
    verify_download(&pinned, destination)
}

async fn fetch_skillsmp_manifest(
    client: &reqwest::Client,
    skill: &RemoteSkill,
    api_key: Option<&str>,
) -> Result<SkillsMpContentsResponse, String> {
    let mut repo_parts = skill.repo.split('/');
    let owner = repo_parts
        .next()
        .ok_or_else(|| "SkillsMP 来源缺少仓库所有者。".to_string())?;
    let repo = repo_parts
        .next()
        .ok_or_else(|| "SkillsMP 来源缺少仓库名称。".to_string())?;
    let mut url = reqwest::Url::parse("https://skillsmp.com/api/github-contents")
        .map_err(|error| format!("无法创建 SkillsMP 目录接口地址：{error}"))?;
    url.query_pairs_mut()
        .append_pair("owner", owner)
        .append_pair("repo", repo)
        .append_pair("path", skill.path.trim_matches('/'))
        .append_pair("branch", &skill.default_branch);
    let mut request = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::ORIGIN, "https://skillsmp.com")
        .header(reqwest::header::REFERER, "https://skillsmp.com/");
    if let Some(api_key) = api_key.filter(|value| !value.trim().is_empty()) {
        request = request.bearer_auth(api_key);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("无法调用 SkillsMP 目录接口：{error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("SkillsMP 目录接口返回 HTTP {status}。"));
    }
    response
        .json()
        .await
        .map_err(|error| format!("SkillsMP 目录清单无法解析：{error}"))
}

fn validate_skillsmp_manifest(
    skill: &RemoteSkill,
    manifest: &SkillsMpContentsResponse,
) -> Result<(), String> {
    if manifest.truncated || manifest.skipped_files > 0 || manifest.limit_reason.is_some() {
        return Err(format!(
            "SkillsMP 返回的目录清单不完整（跳过 {} 个文件，原因：{}）。",
            manifest.skipped_files,
            manifest.limit_reason.as_deref().unwrap_or("已截断")
        ));
    }
    if manifest.files.is_empty() {
        return Err("SkillsMP 返回了空目录清单。".into());
    }
    for file in &manifest.files {
        validate_relative_path(&file.path)?;
        validate_skillsmp_raw_url(skill, file)?;
    }
    if !manifest.files.iter().any(|file| file.path == "SKILL.md") {
        return Err("SkillsMP 目录清单缺少 SKILL.md。".into());
    }
    Ok(())
}

fn validate_skillsmp_raw_url(
    skill: &RemoteSkill,
    file: &SkillsMpContentFile,
) -> Result<(), String> {
    let url = reqwest::Url::parse(&file.raw_url)
        .map_err(|_| format!("SkillsMP 返回了无效文件地址：{}", file.path))?;
    if url.host_str() != Some("raw.githubusercontent.com") {
        return Err(format!(
            "SkillsMP 文件地址不是 GitHub raw 地址：{}",
            file.path
        ));
    }
    let expected_suffix = remote_path(skill, &file.path)?;
    let expected_prefix = format!("/{}/{}/", skill.repo, skill.default_branch);
    let actual_path = url.path().trim_start_matches('/');
    let expected_path = format!(
        "{}/{}/{}",
        skill.repo, skill.default_branch, expected_suffix
    );
    if !url.path().starts_with(&expected_prefix) || actual_path != expected_path {
        return Err(format!(
            "SkillsMP 文件地址与目标 skill 不一致：{}",
            file.path
        ));
    }
    Ok(())
}

async fn resolve_commit_sha(
    client: &reqwest::Client,
    skill: &RemoteSkill,
) -> Result<String, String> {
    let mut url = reqwest::Url::parse("https://api.github.com/")
        .map_err(|error| format!("无法创建 GitHub 元数据地址：{error}"))?;
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| "无法创建 GitHub 元数据地址。".to_string())?;
    segments.pop_if_empty();
    segments.push("repos");
    segments.extend(skill.repo.split('/'));
    segments.push("commits");
    segments.push(&skill.default_branch);
    drop(segments);
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("无法解析 SkillsMP 结果的 GitHub commit：{error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "无法解析 SkillsMP 结果的 GitHub commit（HTTP {}）。",
            response.status()
        ));
    }
    #[derive(Deserialize)]
    struct CommitResponse {
        sha: String,
    }
    let resolved: CommitResponse = response
        .json()
        .await
        .map_err(|error| format!("GitHub commit 响应无法解析：{error}"))?;
    if resolved.sha.len() < 7 || resolved.sha.chars().any(|value| !value.is_ascii_hexdigit()) {
        return Err("GitHub 返回的 commit SHA 无效。".into());
    }
    Ok(resolved.sha)
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

fn validate_discovered_skill(skill: &RemoteSkill) -> Result<(), String> {
    if skill.repo.split('/').count() != 2
        || skill.repo.chars().any(|character| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/'))
        })
    {
        return Err("SkillsMP 返回的仓库名称无效。".into());
    }
    if skill.default_branch.trim().is_empty()
        || skill
            .default_branch
            .chars()
            .any(|character| character.is_control())
    {
        return Err("SkillsMP 返回的分支名称无效。".into());
    }
    let root = skill.path.trim_matches('/');
    if root.is_empty()
        || root
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("SkillsMP 返回的 skill 路径无效。".into());
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
            default_branch: "main".into(),
            commit_sha: "abcdef0123456789".into(),
            files: vec![RemoteFile {
                path: "SKILL.md".into(),
                size: 1,
            }],
            remote_source: String::new(),
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

    #[test]
    fn accepts_complete_skillsmp_manifest() {
        let skill = skill("skills/frontend-design");
        let manifest = SkillsMpContentsResponse {
            files: vec![SkillsMpContentFile {
                path: "SKILL.md".into(),
                size: 8_260,
                raw_url: "https://raw.githubusercontent.com/owner/repository/main/skills/frontend-design/SKILL.md".into(),
            }],
            limit_reason: None,
            skipped_files: 0,
            truncated: false,
        };
        assert!(validate_skillsmp_manifest(&skill, &manifest).is_ok());
    }

    #[test]
    fn rejects_truncated_skillsmp_manifest() {
        let skill = skill("skills/frontend-design");
        let manifest = SkillsMpContentsResponse {
            files: Vec::new(),
            limit_reason: Some("file_limit".into()),
            skipped_files: 2,
            truncated: true,
        };
        assert!(validate_skillsmp_manifest(&skill, &manifest).is_err());
    }

    #[tokio::test]
    async fn downloads_complete_directory_at_commit_sha() {
        let download_root = tempfile::tempdir().expect("download root");
        let skill = RemoteSkill {
            name: "bazi".into(),
            repo: "jinchenma94/bazi-skill".into(),
            path: ".".into(),
            default_branch: "main".into(),
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
            remote_source: String::new(),
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
