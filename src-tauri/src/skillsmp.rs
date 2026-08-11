use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::settings;

const SEARCH_URL: &str = "https://skillsmp.com/api/v1/skills/search";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: u8,
}

fn default_limit() -> u8 {
    20
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscoveredSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "whenToUse")]
    pub when_to_use: String,
    pub tags: Vec<String>,
    pub repo: String,
    pub path: String,
    pub default_branch: String,
    pub commit_sha: String,
    pub files: Vec<serde_json::Value>,
    pub repo_size_kb: u64,
    pub source: DiscoverySource,
    pub remote_source: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverySource {
    pub repository: String,
    pub skill_file_path: String,
    pub ref_name: String,
    pub blob_sha: String,
    pub raw_url: String,
    pub discovery_source: String,
}

#[derive(Deserialize)]
struct SearchResponse {
    success: bool,
    data: Option<SearchData>,
}

#[derive(Deserialize)]
struct SearchData {
    skills: Vec<SkillsMpItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillsMpItem {
    id: String,
    name: String,
    author: String,
    description: String,
    content_language: Option<String>,
    github_url: String,
}

#[tauri::command]
pub async fn search_skillsmp(
    app: AppHandle,
    request: SearchRequest,
) -> Result<Vec<DiscoveredSkill>, String> {
    let query = request.query.trim();
    if query.chars().count() < 2 {
        return Err("请输入至少两个字符后再搜索 SkillsMP。".into());
    }
    let api_key = settings::skillsmp_api_key(&app)?;
    let mut url =
        Url::parse(SEARCH_URL).map_err(|error| format!("SkillsMP 搜索地址无效：{error}"))?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("limit", &request.limit.clamp(1, 50).to_string())
        .append_pair("sortBy", "stars");
    let client = Client::builder()
        .user_agent("Super-Skill-Router/0.1")
        .build()
        .map_err(|error| format!("无法初始化 SkillsMP 客户端：{error}"))?;
    let response = client
        .get(url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|error| format!("无法访问 SkillsMP：{error}"))?;
    let status = response.status();
    let payload: SearchResponse = response
        .json()
        .await
        .map_err(|error| format!("SkillsMP 返回无法解析：{error}"))?;
    if !status.is_success() || !payload.success {
        return Err(format!("SkillsMP 搜索失败（HTTP {status}）。"));
    }

    Ok(payload
        .data
        .map(|data| data.skills)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| to_discovered_skill(item).ok())
        .collect())
}

#[tauri::command]
pub async fn import_skillsmp_command(
    _app: AppHandle,
    command: String,
) -> Result<DiscoveredSkill, String> {
    let (repo, requested_skill) = parse_install_command(&command)?;
    let client = Client::builder()
        .user_agent("Super-Skill-Router/0.1")
        .build()
        .map_err(|error| format!("无法初始化 GitHub 客户端：{error}"))?;
    let metadata_url = format!("https://api.github.com/repos/{repo}");
    let metadata: RepositoryMetadata = request_github_json(&client, &metadata_url).await?;
    let tree_url = format!(
        "https://api.github.com/repos/{repo}/git/trees/{}?recursive=1",
        metadata.default_branch
    );
    let tree: RepositoryTree = request_github_json(&client, &tree_url).await?;
    if tree.truncated {
        return Err(
            "该仓库的文件树过大，无法可靠解析 --skill；请使用 SkillsMP 搜索结果中的目录地址。"
                .into(),
        );
    }
    let mut matches = tree
        .tree
        .into_iter()
        .filter(|entry| entry.kind == "blob" && entry.path.ends_with("/SKILL.md"))
        .filter_map(|entry| {
            let directory = entry.path.strip_suffix("/SKILL.md")?;
            (directory.rsplit('/').next() == Some(requested_skill.as_str()))
                .then_some(directory.to_owned())
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    match matches.as_slice() {
        [path] => Ok(command_discovered_skill(&repo, &metadata.default_branch, path, &requested_skill)),
        [] => Err(format!("仓库中没有名为 `{requested_skill}` 且包含 SKILL.md 的目录。")),
        _ => Err(format!("仓库中有多个名为 `{requested_skill}` 的 skill 目录，无法安全选择。请从 SkillsMP 搜索结果中选择具体目录。")),
    }
}

#[derive(Deserialize)]
struct RepositoryMetadata {
    default_branch: String,
}

#[derive(Deserialize)]
struct RepositoryTree {
    #[serde(default)]
    truncated: bool,
    tree: Vec<TreeEntry>,
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
}

async fn request_github_json<T: serde::de::DeserializeOwned>(
    client: &Client,
    url: &str,
) -> Result<T, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("无法解析安装命令的 GitHub 仓库：{error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "无法解析安装命令的 GitHub 仓库（HTTP {}）。",
            response.status()
        ));
    }
    response
        .json()
        .await
        .map_err(|error| format!("GitHub 仓库响应无法解析：{error}"))
}

fn parse_install_command(command: &str) -> Result<(String, String), String> {
    let tokens: Vec<&str> = command.split_whitespace().collect();
    if tokens.len() < 6 || tokens[0] != "npx" || tokens[1] != "skills" || tokens[2] != "add" {
        return Err("仅支持 SkillsMP 页面提供的 `npx skills add <GitHub URL> --skill <名称>` 格式；该命令不会被执行。".into());
    }
    let repo = github_repo_from_url(tokens[3].trim_matches(['\'', '"']))?;
    let skill_position = tokens
        .iter()
        .position(|token| *token == "--skill")
        .ok_or_else(|| "安装命令缺少 --skill 参数。".to_string())?;
    let requested_skill = tokens
        .get(skill_position + 1)
        .ok_or_else(|| "安装命令的 --skill 参数缺少名称。".to_string())?
        .trim_matches(['\'', '"']);
    if requested_skill.is_empty()
        || requested_skill.contains(['/', '\\'])
        || requested_skill.chars().any(|value| value.is_control())
    {
        return Err("安装命令中的 skill 名称无效。".into());
    }
    Ok((repo, requested_skill.into()))
}

fn github_repo_from_url(value: &str) -> Result<String, String> {
    let url = Url::parse(value).map_err(|_| "安装命令中的 GitHub 地址无效。".to_string())?;
    if url.host_str() != Some("github.com") {
        return Err("安装命令必须指向 github.com 仓库。".into());
    }
    let parts: Vec<&str> = url
        .path_segments()
        .ok_or_else(|| "GitHub 地址缺少仓库路径。".to_string())?
        .collect();
    if parts.len() != 2
        || parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err("安装命令必须指向 GitHub 仓库根地址。".into());
    }
    Ok(format!(
        "{}/{}",
        parts[0],
        parts[1].trim_end_matches(".git")
    ))
}

fn command_discovered_skill(
    repo: &str,
    branch: &str,
    path: &str,
    requested_skill: &str,
) -> DiscoveredSkill {
    let github_url = format!("https://github.com/{repo}/tree/{branch}/{path}");
    DiscoveredSkill {
        id: format!("skillsmp-command:{repo}:{path}"),
        name: requested_skill.into(),
        description: format!("从 SkillsMP 安装命令解析：{repo}/{path}"),
        when_to_use:
            "从 SkillsMP 安装命令导入；下载时将锁定 GitHub commit SHA 并提取完整 skill 目录。"
                .into(),
        tags: vec!["SkillsMP 命令".into()],
        repo: repo.into(),
        path: path.into(),
        default_branch: branch.into(),
        commit_sha: String::new(),
        files: Vec::new(),
        repo_size_kb: 0,
        source: DiscoverySource {
            repository: repo.into(),
            skill_file_path: format!("{path}/SKILL.md"),
            ref_name: branch.into(),
            blob_sha: String::new(),
            raw_url: github_url,
            discovery_source: "skillsmp".into(),
        },
        remote_source: "skillsmp".into(),
    }
}

fn to_discovered_skill(item: SkillsMpItem) -> Result<DiscoveredSkill, String> {
    let location = parse_github_tree_url(&item.github_url)?;
    let mut tags = vec!["SkillsMP".into()];
    if !item.author.trim().is_empty() {
        tags.push(format!("作者: {}", item.author));
    }
    if let Some(language) = item
        .content_language
        .filter(|value| !value.trim().is_empty())
    {
        tags.push(language);
    }
    let skill_file_path = if location.path == "." {
        "SKILL.md".into()
    } else {
        format!("{}/SKILL.md", location.path)
    };
    Ok(DiscoveredSkill {
        id: format!("skillsmp:{}", item.id),
        name: item.name,
        description: item.description,
        when_to_use:
            "来自 SkillsMP 的远程发现结果；下载时将锁定 GitHub commit SHA 并提取完整 skill 目录。"
                .into(),
        tags,
        repo: location.repo.clone(),
        path: location.path.clone(),
        default_branch: location.reference.clone(),
        commit_sha: String::new(),
        files: Vec::new(),
        repo_size_kb: 0,
        source: DiscoverySource {
            repository: location.repo,
            skill_file_path,
            ref_name: location.reference,
            blob_sha: String::new(),
            raw_url: item.github_url,
            discovery_source: "skillsmp".into(),
        },
        remote_source: "skillsmp".into(),
    })
}

struct GithubTreeLocation {
    repo: String,
    reference: String,
    path: String,
}

fn parse_github_tree_url(value: &str) -> Result<GithubTreeLocation, String> {
    let url = Url::parse(value).map_err(|_| "SkillsMP 返回了无效的 GitHub 地址。".to_string())?;
    if url.host_str() != Some("github.com") {
        return Err("SkillsMP 返回的来源不是 github.com，无法下载完整 skill 目录。".into());
    }
    let parts: Vec<&str> = url
        .path_segments()
        .ok_or_else(|| "GitHub 地址缺少路径。".to_string())?
        .collect();
    if parts.len() < 5 || parts[2] != "tree" {
        return Err("SkillsMP 返回的 GitHub 地址不是 skill 目录地址。".into());
    }
    let repo = format!("{}/{}", parts[0], parts[1]);
    let reference = parts[3];
    let path = parts[4..].join("/");
    if repo.contains("..")
        || reference.is_empty()
        || path.is_empty()
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("SkillsMP 返回了不安全的 GitHub skill 路径。".into());
    }
    Ok(GithubTreeLocation {
        repo,
        reference: reference.into(),
        path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skillsmp_github_tree_location() {
        let location =
            parse_github_tree_url("https://github.com/openclaw/openclaw/tree/main/skills/nano-pdf")
                .expect("location");
        assert_eq!(location.repo, "openclaw/openclaw");
        assert_eq!(location.reference, "main");
        assert_eq!(location.path, "skills/nano-pdf");
    }

    #[test]
    fn rejects_non_tree_url() {
        assert!(parse_github_tree_url("https://github.com/openclaw/openclaw").is_err());
    }

    #[test]
    fn parses_skillsmp_install_command_without_executing_it() {
        let (repo, skill) = parse_install_command(
            "npx skills add https://github.com/anthropics/skills --skill frontend-design",
        )
        .expect("command");
        assert_eq!(repo, "anthropics/skills");
        assert_eq!(skill, "frontend-design");
    }
}
