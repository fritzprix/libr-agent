use std::fs;
use std::io;
use std::path::Path;

const SKILL_FILE_NAME: &str = "SKILL.md";

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

fn is_valid_bundled_skill_dir(path: &Path) -> bool {
    path.is_dir() && path.join(SKILL_FILE_NAME).is_file()
}

pub fn mirror_bundled_skills(source_dir: &Path, deployed_dir: &Path) -> io::Result<()> {
    if !source_dir.exists() {
        return Ok(());
    }

    if deployed_dir.exists() {
        fs::remove_dir_all(deployed_dir)?;
    }
    fs::create_dir_all(deployed_dir)?;

    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let src_path = entry.path();
        if !is_valid_bundled_skill_dir(&src_path) {
            continue;
        }

        copy_dir_recursive(&src_path, &deployed_dir.join(entry.file_name()))?;
    }

    Ok(())
}
