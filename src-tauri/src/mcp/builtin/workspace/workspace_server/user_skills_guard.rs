//! File tools never write into the user skills folder; `skill.py deploy` is the
//! single allowed writer. Only the refusal names `skill.py deploy`: reads and
//! unrelated paths get no extra guidance.

use crate::mcp::builtin::utils::relative_path_under_base;
use std::path::{Path, PathBuf};

/// Marker substring for user-skills write rejection (recovery guidance + tests).
pub const USER_SKILLS_WRITE_REJECTED_MARKER: &str = "User skills are read-only";

pub fn user_skills_write_rejected_error() -> String {
    format!(
        "{USER_SKILLS_WRITE_REJECTED_MARKER} for file tools. Edit a workspace copy \
         (`skill.py new` / `skill.py clone`), then publish it with \
         `python3 <skill_base>/scripts/skill.py deploy <dir>` using workspace__runShell."
    )
}

/// Canonicalizes the deepest existing ancestor and re-appends the missing tail,
/// so not-yet-created targets and symlinked parents resolve consistently.
fn resolve_lenient(path: &Path) -> PathBuf {
    let mut tail = Vec::new();
    let mut current = path;
    loop {
        if let Ok(resolved) = current.canonicalize() {
            return tail.iter().rev().fold(resolved, |acc, part| acc.join(part));
        }
        match (current.parent(), current.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_owned());
                current = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// True when `target` is the user skills folder or inside it.
pub fn is_inside_user_skills(target: &Path, user_skills_dir: &Path) -> bool {
    relative_path_under_base(&resolve_lenient(target), &resolve_lenient(user_skills_dir)).is_some()
}

/// `Err` with the refusal message when `target` is inside the user skills folder.
pub fn refuse_user_skills_write(target: &Path) -> Result<(), String> {
    match crate::services::skill_service::get_user_skills_directory() {
        Ok(dir) if is_inside_user_skills(target, &dir) => Err(user_skills_write_rejected_error()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_paths_inside_user_skills() {
        let root = tempfile::tempdir().unwrap();
        let skills = root.path().join("user_skills");
        std::fs::create_dir_all(skills.join("demo")).unwrap();

        assert!(is_inside_user_skills(
            &skills.join("demo/SKILL.md"),
            &skills
        ));
        assert!(is_inside_user_skills(
            &skills.join("new-skill/SKILL.md"),
            &skills
        ));
        assert!(is_inside_user_skills(&skills, &skills));
    }

    #[test]
    fn ignores_sibling_and_lookalike_paths() {
        let root = tempfile::tempdir().unwrap();
        let skills = root.path().join("user_skills");
        std::fs::create_dir_all(&skills).unwrap();

        assert!(!is_inside_user_skills(
            &root.path().join("user_skills_backup/x"),
            &skills
        ));
        assert!(!is_inside_user_skills(
            &root.path().join("system_skills/x"),
            &skills
        ));
        assert!(!is_inside_user_skills(
            &root.path().join("workspace/x"),
            &skills
        ));
    }

    #[test]
    fn dotdot_and_symlink_escapes_resolve_to_the_real_location() {
        let root = tempfile::tempdir().unwrap();
        let skills = root.path().join("user_skills");
        std::fs::create_dir_all(skills.join("demo")).unwrap();
        std::fs::create_dir_all(root.path().join("workspace")).unwrap();

        let dotdot = root.path().join("workspace/../user_skills/demo/SKILL.md");
        assert!(is_inside_user_skills(&dotdot, &skills));

        #[cfg(unix)]
        {
            let link = root.path().join("workspace/skills-link");
            std::os::unix::fs::symlink(&skills, &link).unwrap();
            assert!(is_inside_user_skills(&link.join("demo/SKILL.md"), &skills));
        }
    }

    #[test]
    fn refusal_names_deploy() {
        let message = user_skills_write_rejected_error();
        assert!(message.contains(USER_SKILLS_WRITE_REJECTED_MARKER));
        assert!(message.contains("skill.py deploy"));
    }
}
