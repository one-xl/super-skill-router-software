use std::fs::{self, File};
use std::io;
use std::path::{Component, Path};

use zip::ZipArchive;

use crate::targets::LocalSkill;

const MAX_ENTRIES: usize = 5_000;
const MAX_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ImportedSkill {
    pub skill: LocalSkill,
    pub display_name: String,
}

pub fn import_archive(
    archive_path: &Path,
    destination_parent: &Path,
) -> Result<ImportedSkill, String> {
    if !archive_path.is_file()
        || archive_path
            .extension()
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("zip"))
    {
        return Err("请选择一个 .zip 格式的 skill 压缩包。".into());
    }

    let file =
        File::open(archive_path).map_err(|error| format!("无法打开 skill 压缩包：{error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("skill 压缩包无效：{error}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(format!(
            "skill 压缩包包含超过 {MAX_ENTRIES} 个文件，已拒绝导入。"
        ));
    }

    let mut total_size = 0_u64;
    let mut roots = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("无法读取压缩包条目：{error}"))?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| "skill 压缩包包含不安全的文件路径。".to_string())?;
        if is_symlink(&entry) {
            return Err("skill 压缩包包含符号链接，无法安全导入。".into());
        }
        total_size = total_size.saturating_add(entry.size());
        if total_size > MAX_UNCOMPRESSED_BYTES {
            return Err("skill 压缩包解压后超过 512 MB，已拒绝导入。".into());
        }
        if !entry.is_dir() && enclosed.file_name().is_some_and(|name| name == "SKILL.md") {
            roots.push(
                enclosed
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .to_path_buf(),
            );
        }
    }
    roots.sort();
    roots.dedup();
    let root = match roots.as_slice() {
        [root] => root.clone(),
        [] => return Err("压缩包中未找到 SKILL.md，不能作为 skill 导入。".into()),
        _ => return Err("压缩包中包含多个 SKILL.md。请一次只导入一个完整 skill。".into()),
    };

    let directory_name = directory_name(archive_path, &root)?;
    let destination = destination_parent.join(&directory_name);
    fs::create_dir_all(destination_parent)
        .map_err(|error| format!("无法创建导入缓存目录：{error}"))?;
    fs::create_dir(&destination).map_err(|error| format!("无法创建 skill 临时目录：{error}"))?;

    let extraction = extract_root(&mut archive, &root, &destination);
    if let Err(error) = extraction {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }
    let skill_file = destination.join("SKILL.md");
    let content = fs::read_to_string(&skill_file)
        .map_err(|error| format!("无法读取导入 skill 的 SKILL.md：{error}"))?;
    Ok(ImportedSkill {
        skill: LocalSkill {
            directory_name,
            source_dir: destination,
        },
        display_name: frontmatter_name(&content).unwrap_or_else(|| "本地导入 skill".into()),
    })
}

fn extract_root(
    archive: &mut ZipArchive<File>,
    root: &Path,
    destination: &Path,
) -> Result<(), String> {
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("无法读取压缩包条目：{error}"))?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| "skill 压缩包包含不安全的文件路径。".to_string())?;
        if is_symlink(&entry) {
            return Err("skill 压缩包包含符号链接，无法安全导入。".into());
        }
        let Ok(relative) = enclosed.strip_prefix(root) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err("skill 压缩包包含不安全的文件路径。".into());
        }
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|error| format!("无法解压 skill 目录：{error}"))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建 skill 文件目录：{error}"))?;
        }
        let mut output =
            File::create(&target).map_err(|error| format!("无法解压 skill 文件：{error}"))?;
        io::copy(&mut entry, &mut output)
            .map_err(|error| format!("无法写入 skill 文件：{error}"))?;
    }
    if !destination.join("SKILL.md").is_file() {
        return Err("导入后的 skill 根目录缺少 SKILL.md。".into());
    }
    Ok(())
}

fn is_symlink(entry: &zip::read::ZipFile<'_>) -> bool {
    entry
        .unix_mode()
        .is_some_and(|mode| mode & 0o170000 == 0o120000)
}

fn directory_name(archive_path: &Path, root: &Path) -> Result<String, String> {
    let original = root
        .file_name()
        .or_else(|| archive_path.file_stem())
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let normalized: String = original
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
        return Err("无法从压缩包生成安全的 skill 目录名。".into());
    }
    Ok(normalized)
}

fn frontmatter_name(markdown: &str) -> Option<String> {
    let mut lines = markdown.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        let line = line.trim();
        if line == "---" {
            break;
        }
        if let Some(value) = line.strip_prefix("name:") {
            let value = value.trim().trim_matches(['\'', '"']);
            if !value.is_empty() {
                return Some(value.into());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    use super::*;

    fn write_archive(path: &Path, files: &[(&str, &str)]) {
        let file = File::create(path).expect("archive");
        let mut writer = ZipWriter::new(file);
        for (name, content) in files {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .expect("entry");
            writer.write_all(content.as_bytes()).expect("content");
        }
        writer.finish().expect("finish");
    }

    #[test]
    fn imports_the_complete_skill_tree_from_a_wrapped_archive() {
        let workspace = tempfile::tempdir().expect("workspace");
        let archive = workspace.path().join("download.zip");
        write_archive(
            &archive,
            &[
                ("example/SKILL.md", "---\nname: example\n---\n"),
                ("example/scripts/tool.py", "print('ok')"),
                ("example/references/guide.md", "guide"),
            ],
        );
        let imported =
            import_archive(&archive, workspace.path().join("out").as_path()).expect("import");
        assert_eq!(imported.skill.directory_name, "example");
        assert_eq!(imported.display_name, "example");
        assert!(imported.skill.source_dir.join("scripts/tool.py").is_file());
        assert!(imported
            .skill
            .source_dir
            .join("references/guide.md")
            .is_file());
    }

    #[test]
    fn rejects_archives_with_multiple_skills() {
        let workspace = tempfile::tempdir().expect("workspace");
        let archive = workspace.path().join("multiple.zip");
        write_archive(
            &archive,
            &[("one/SKILL.md", "one"), ("two/SKILL.md", "two")],
        );
        assert!(import_archive(&archive, workspace.path()).is_err());
    }

    #[test]
    fn ignores_metadata_outside_the_skill_root() {
        let workspace = tempfile::tempdir().expect("workspace");
        let archive = workspace.path().join("metadata.zip");
        write_archive(
            &archive,
            &[
                ("example/SKILL.md", "---\nname: example\n---\n"),
                ("__MACOSX/example/.DS_Store", "metadata"),
            ],
        );
        let imported =
            import_archive(&archive, workspace.path().join("out").as_path()).expect("import");
        assert!(imported.skill.source_dir.join("SKILL.md").is_file());
        assert!(!imported.skill.source_dir.join("__MACOSX").exists());
    }
}
