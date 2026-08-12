use std::io::Cursor;
use std::path::Path;

use reqwest::Client;
use zip::ZipArchive;

use super::{remote_path, remove_directory, request_error_details, RemoteSkill};

pub async fn download_and_extract(
    client: &Client,
    skill: &RemoteSkill,
    destination: &Path,
) -> Result<(), String> {
    let url = format!(
        "https://codeload.github.com/{}/zip/{}",
        skill.repo, skill.commit_sha
    );
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("无法下载仓库归档：{}", request_error_details(&error)))?;
    if !response.status().is_success() {
        return Err(format!("下载仓库归档失败（HTTP {}）。", response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("无法读取仓库归档：{error}"))?;
    std::fs::create_dir(destination).map_err(|error| format!("无法创建回退下载目录：{error}"))?;

    let extracted = extract_selected_files(bytes.as_ref(), skill, destination);
    if let Err(error) = extracted {
        let _ = remove_directory(&destination.to_path_buf());
        return Err(error);
    }
    Ok(())
}

pub async fn download_and_extract_tree(
    client: &Client,
    skill: &RemoteSkill,
    commit_sha: &str,
    destination: &Path,
) -> Result<(), String> {
    let url = format!(
        "https://codeload.github.com/{}/zip/{commit_sha}",
        skill.repo
    );
    let response = client.get(url).send().await.map_err(|error| {
        format!(
            "无法下载 SkillsMP 来源的仓库归档：{}",
            request_error_details(&error)
        )
    })?;
    if !response.status().is_success() {
        return Err(format!(
            "下载 SkillsMP 来源的仓库归档失败（HTTP {}）。",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("无法读取 SkillsMP 来源的仓库归档：{error}"))?;
    std::fs::create_dir(destination).map_err(|error| format!("无法创建回退下载目录：{error}"))?;
    let extracted = extract_skill_tree(bytes.as_ref(), skill, destination);
    if let Err(error) = extracted {
        let _ = remove_directory(&destination.to_path_buf());
        return Err(error);
    }
    Ok(())
}

fn extract_selected_files(
    bytes: &[u8],
    skill: &RemoteSkill,
    destination: &Path,
) -> Result<(), String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("仓库归档不是有效 zip：{error}"))?;
    let expected: Vec<String> = skill
        .files
        .iter()
        .map(|file| remote_path(skill, &file.path))
        .collect::<Result<_, _>>()?;
    let mut found = vec![false; expected.len()];
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("无法读取归档条目：{error}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(enclosed) = entry.enclosed_name() else {
            return Err("归档包含不安全路径。".into());
        };
        let mut components = enclosed.components();
        let _top_level = components
            .next()
            .ok_or_else(|| "归档条目缺少顶层目录。".to_string())?;
        let relative = components.as_path().to_string_lossy().replace('\\', "/");
        let Some(expected_index) = expected.iter().position(|value| value == &relative) else {
            continue;
        };
        let target = destination.join(&skill.files[expected_index].path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建文件目录：{error}"))?;
        }
        let mut output =
            std::fs::File::create(target).map_err(|error| format!("无法创建归档文件：{error}"))?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|error| format!("无法解压归档文件：{error}"))?;
        found[expected_index] = true;
    }
    if found.iter().any(|present| !present) {
        return Err("归档中缺少索引声明的 skill 文件。".into());
    }
    Ok(())
}

fn extract_skill_tree(bytes: &[u8], skill: &RemoteSkill, destination: &Path) -> Result<(), String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("仓库归档不是有效 zip：{error}"))?;
    let root = skill.path.trim_matches('/');
    let prefix = format!("{root}/");
    let mut extracted_files = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("无法读取归档条目：{error}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(enclosed) = entry.enclosed_name() else {
            return Err("归档包含不安全路径。".into());
        };
        let mut components = enclosed.components();
        let _top_level = components
            .next()
            .ok_or_else(|| "归档条目缺少顶层目录。".to_string())?;
        let relative = components.as_path().to_string_lossy().replace('\\', "/");
        let Some(target_relative) = relative.strip_prefix(&prefix) else {
            continue;
        };
        if target_relative.is_empty() {
            continue;
        }
        let target = destination.join(target_relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建文件目录：{error}"))?;
        }
        let mut output =
            std::fs::File::create(target).map_err(|error| format!("无法创建归档文件：{error}"))?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|error| format!("无法解压归档文件：{error}"))?;
        extracted_files += 1;
    }
    if extracted_files == 0 {
        return Err("归档中没有 SkillsMP 指向的 skill 目录。".into());
    }
    Ok(())
}
