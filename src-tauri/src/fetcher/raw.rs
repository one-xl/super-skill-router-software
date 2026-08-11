use std::path::Path;

use reqwest::{Client, Url};

use super::{remote_path, validate_relative_path, RemoteSkill};

pub async fn download(
    client: &Client,
    skill: &RemoteSkill,
    destination: &Path,
) -> Result<(), String> {
    std::fs::create_dir(destination).map_err(|error| format!("无法创建下载目录：{error}"))?;
    for file in &skill.files {
        let remote_file = remote_path(skill, &file.path)?;
        let url = raw_url(&skill.repo, &skill.commit_sha, &remote_file)?;
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|error| format!("无法下载 {}：{error}", file.path))?;
        if !response.status().is_success() {
            return Err(format!(
                "下载 {} 失败（HTTP {}）。",
                file.path,
                response.status()
            ));
        }
        let content = response
            .bytes()
            .await
            .map_err(|error| format!("无法读取 {}：{error}", file.path))?;
        if content.len() as u64 != file.size {
            return Err(format!("下载 {} 的大小与索引不符。", file.path));
        }
        let target = destination.join(&file.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建文件目录：{error}"))?;
        }
        std::fs::write(target, content)
            .map_err(|error| format!("无法写入 {}：{error}", file.path))?;
    }
    Ok(())
}

fn raw_url(repo: &str, sha: &str, path: &str) -> Result<Url, String> {
    validate_relative_path(path)?;
    let mut url = Url::parse("https://raw.githubusercontent.com/")
        .map_err(|error| format!("无法创建下载地址：{error}"))?;
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| "无法创建下载地址。".to_string())?;
    segments.pop_if_empty();
    segments.extend(repo.split('/'));
    segments.push(sha);
    segments.extend(path.split('/'));
    drop(segments);
    Ok(url)
}
