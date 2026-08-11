use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::targets::LocalSkill;

pub fn package_for_claude_desktop(
    skill: &LocalSkill,
    output_dir: &Path,
) -> Result<PathBuf, String> {
    if !skill.source_dir.is_dir() || !skill.source_dir.join("SKILL.md").is_file() {
        return Err("无法打包：skill 根目录缺少 SKILL.md。".into());
    }
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("无法创建 Claude Desktop 上传目录：{error}"))?;
    let archive_path = output_dir.join(format!("{}-{}.zip", skill.directory_name, Uuid::new_v4()));
    let file = File::create(&archive_path).map_err(|error| format!("无法创建上传 zip：{error}"))?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    if let Err(error) =
        add_directory_contents(&mut writer, &skill.source_dir, &skill.source_dir, options)
    {
        let _ = fs::remove_file(&archive_path);
        return Err(error);
    }
    writer
        .finish()
        .map_err(|error| format!("无法完成上传 zip：{error}"))?;
    Ok(archive_path)
}

fn add_directory_contents(
    writer: &mut ZipWriter<File>,
    root: &Path,
    current: &Path,
    options: SimpleFileOptions,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|error| format!("无法读取待打包文件：{error}"))?
    {
        let entry = entry.map_err(|error| format!("无法读取待打包文件：{error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("无法读取待打包文件属性：{error}"))?;
        if file_type.is_symlink() {
            return Err("skill 包含符号链接，无法安全打包。".into());
        }
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "无法生成 zip 内文件路径。".to_string())?;
        let name = relative.to_string_lossy().replace('\\', "/");
        if file_type.is_dir() {
            writer
                .add_directory(format!("{name}/"), options)
                .map_err(|error| format!("无法写入 zip 目录：{error}"))?;
            add_directory_contents(writer, root, &path, options)?;
        } else if file_type.is_file() {
            writer
                .start_file(&name, options)
                .map_err(|error| format!("无法写入 zip 文件：{error}"))?;
            let mut source =
                File::open(&path).map_err(|error| format!("无法打开待打包文件：{error}"))?;
            let mut buffer = Vec::new();
            source
                .read_to_end(&mut buffer)
                .map_err(|error| format!("无法读取待打包文件：{error}"))?;
            writer
                .write_all(&buffer)
                .map_err(|error| format!("无法写入 zip 内容：{error}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn packages_the_complete_skill_tree_without_an_extra_root_directory() {
        let workspace = tempfile::tempdir().expect("workspace");
        let source = workspace.path().join("skill-folder");
        fs::create_dir_all(source.join("scripts")).expect("scripts");
        fs::create_dir_all(source.join("references")).expect("references");
        fs::write(source.join("SKILL.md"), "---\nname: demo\n---\n").expect("skill");
        fs::write(source.join("scripts").join("tool.py"), "print('ok')").expect("script");
        fs::write(source.join("references").join("guide.md"), "guide").expect("reference");
        let skill = LocalSkill {
            directory_name: "skill-folder".into(),
            source_dir: source,
        };

        let archive = package_for_claude_desktop(&skill, workspace.path().join("out").as_path())
            .expect("package");
        let bytes = fs::read(archive).expect("archive bytes");
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
        let mut names = (0..zip.len())
            .map(|index| zip.by_index(index).expect("entry").name().to_string())
            .collect::<Vec<_>>();
        names.sort();
        assert!(names.contains(&"SKILL.md".to_string()));
        assert!(names.contains(&"scripts/tool.py".to_string()));
        assert!(names.contains(&"references/guide.md".to_string()));
        assert!(!names.iter().any(|name| name.starts_with("skill-folder/")));
    }
}
