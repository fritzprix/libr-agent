//! Base-directory note for SKILL.md reads through a skill alias.

use crate::services::skill_service::extract_skill_alias_relative_path;
use std::path::Path;

/// Note naming the real base directory when `SKILL.md` is read via an alias such as
/// `@system-skills/<skill>/SKILL.md`: the container path in Docker sessions (only
/// system/user roots are mounted there), the host path otherwise. Shell commands cannot
/// resolve the alias, so the agent needs the real directory.
pub(super) fn skill_base_directory_note(
    path_str: &str,
    resolved_path: &Path,
    docker_session: bool,
) -> Option<String> {
    let (prefix, relative) = extract_skill_alias_relative_path(path_str)?;
    let mut parts = relative.split('/');
    let (skill_dir, file) = (parts.next()?, parts.next()?);
    if file != "SKILL.md" || parts.next().is_some() || skill_dir.is_empty() || skill_dir == "." {
        return None;
    }

    let container_dir = docker_session
        .then(|| {
            crate::session_isolation::container_path_for_skill_alias(&format!(
                "{prefix}/{skill_dir}"
            ))
        })
        .flatten();
    let (label, dir) = match container_dir {
        Some(dir) => ("Base directory (container)", dir),
        None => (
            "Base directory",
            resolved_path.parent()?.display().to_string(),
        ),
    };

    Some(format!(
        "{label}: `{dir}`\nUse this directory (not the `{prefix}` alias) in workspace__runShell commands; the alias only works in file tools."
    ))
}

#[cfg(test)]
mod tests {
    use super::skill_base_directory_note;
    use std::path::Path;

    #[test]
    fn docker_session_gets_the_container_base_directory() {
        let note = skill_base_directory_note(
            "@system-skills/skill-deployer/SKILL.md",
            Path::new("/host/data/system_skills/skill-deployer/SKILL.md"),
            true,
        )
        .expect("note");

        assert!(note
            .contains("Base directory (container): `/opt/libragent/skills/system/skill-deployer`"));
        assert!(!note.contains("/host/data"));
    }

    #[test]
    fn host_session_gets_the_host_base_directory() {
        let note = skill_base_directory_note(
            "@user-skills/my-skill/SKILL.md",
            Path::new("/host/data/user_skills/my-skill/SKILL.md"),
            false,
        )
        .expect("note");

        assert!(note.contains("Base directory: `/host/data/user_skills/my-skill`"));
        assert!(!note.contains("(container)"));
    }

    #[test]
    fn docker_session_falls_back_to_host_path_for_unmounted_roots() {
        let note = skill_base_directory_note(
            "@assistant-skills/helper/SKILL.md",
            Path::new("/host/data/assistants/a/skills/helper/SKILL.md"),
            true,
        )
        .expect("note");

        assert!(note.contains("Base directory: `/host/data/assistants/a/skills/helper`"));
    }

    #[test]
    fn other_reads_get_no_note() {
        let resolved = Path::new("/host/data/system_skills/x/references/a.md");
        assert!(
            skill_base_directory_note("@system-skills/x/references/a.md", resolved, false)
                .is_none()
        );
        assert!(
            skill_base_directory_note("@system-skills/x/SKILL.md.bak", resolved, false).is_none()
        );
        assert!(skill_base_directory_note("src/SKILL.md", resolved, false).is_none());
    }
}
