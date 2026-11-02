//! Skill base directory resolution for skill files read through an alias.
//!
//! Skills write `<skill-base-dir>` in their command snippets because they cannot know
//! where the skill is installed. This module resolves that placeholder to the real
//! absolute directory (the container path in Docker sessions, the host path otherwise)
//! so the agent never has to copy or guess the path.

use crate::mcp::builtin::utils::strip_windows_verbatim_prefix;
use crate::services::skill_service::extract_skill_alias_relative_path;
use std::path::Path;

/// Placeholder that skills use for their own installation directory.
pub(super) const SKILL_BASE_DIR_TOKEN: &str = "<skill-base-dir>";

/// Split a path string into its root (`/`, `\\`, `C:\`, or a verbatim UNC prefix) and the
/// remaining components.
fn split_path_root(text: &str) -> (&str, &str) {
    // A verbatim UNC path must keep its prefix: network paths need the verbatim form.
    for prefix in [r"\\?\UNC\", "//?/UNC/"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return text.split_at(text.len() - rest.len());
        }
    }
    let bytes = text.as_bytes();
    if bytes.len() >= 3 && bytes[1] == b':' && matches!(bytes[2], b'/' | b'\\') {
        return text.split_at(3);
    }
    match text.as_bytes().first() {
        Some(b'/') | Some(b'\\') => text.split_at(1),
        _ => ("", text),
    }
}

/// Host skill root for a resolved skill file path, dropping the `trailing_len` components
/// (the file name and any nested directories) that `nested` already counts.
///
/// Windows canonicalization (`SecurityValidator`) yields verbatim paths such as
/// `\\?\C:\...`. Win32 does not normalize `/` inside a verbatim path, so a substituted
/// command could not open the file; the prefix is stripped. Components are split on both
/// separators so a Windows path resolves identically on any host platform.
fn host_skill_dir(resolved_path: &Path, trailing_len: usize) -> Option<String> {
    let resolved = resolved_path.to_string_lossy();
    let text = strip_windows_verbatim_prefix(&resolved);
    let (root, rest) = split_path_root(text);
    let mut parts: Vec<&str> = rest
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    parts.truncate(parts.len().checked_sub(trailing_len)?);
    if parts.is_empty() {
        return None;
    }
    let separator = if root.ends_with('\\') { "\\" } else { "/" };
    Some(format!("{root}{}", parts.join(separator)))
}

/// Alias prefix, the real directory that owns a skill file read through an alias, and
/// whether that directory is the container path.
///
/// Only `<skill>/SKILL.md` and `<skill>/references/*.md` are resolved; other reads keep
/// the placeholder untouched.
fn skill_alias_base_dir(
    path_str: &str,
    resolved_path: &Path,
    docker_session: bool,
) -> Option<(&'static str, String, bool)> {
    let (prefix, relative) = extract_skill_alias_relative_path(path_str)?;
    let mut parts = relative.split(['/', '\\']);
    let skill_dir = parts.next()?;
    let nested: Vec<&str> = parts.collect();
    let is_supported = match nested.as_slice() {
        ["SKILL.md"] => true,
        ["references", name] => name.ends_with(".md"),
        _ => false,
    };
    if !is_supported || skill_dir.is_empty() || skill_dir == "." {
        return None;
    }

    let container_dir = docker_session
        .then(|| {
            crate::session_isolation::container_path_for_skill_alias(&format!(
                "{prefix}/{skill_dir}"
            ))
        })
        .flatten();
    if let Some(dir) = container_dir {
        return Some((prefix, dir, true));
    }

    Some((prefix, host_skill_dir(resolved_path, nested.len())?, false))
}

/// Replace `<skill-base-dir>` in `content` with the real skill directory.
///
/// Returns `true` when the placeholder was found and replaced.
pub(super) fn substitute_skill_base_dir(
    path_str: &str,
    resolved_path: &Path,
    docker_session: bool,
    content: &mut String,
) -> bool {
    if !content.contains(SKILL_BASE_DIR_TOKEN) {
        return false;
    }
    let Some((_, dir, _)) = skill_alias_base_dir(path_str, resolved_path, docker_session) else {
        return false;
    };
    *content = content.replace(SKILL_BASE_DIR_TOKEN, &dir);
    true
}

/// Note naming the real base directory when `SKILL.md` is read via an alias such as
/// `@system-skills/<skill>/SKILL.md`: the container path in Docker sessions (only
/// system/user roots are mounted there), the host path otherwise. Shell commands cannot
/// resolve the alias, so the agent needs the real directory.
///
/// Skills that already use `<skill-base-dir>` get the substitution instead, so they do
/// not also need this note.
pub(super) fn skill_base_directory_note(
    path_str: &str,
    resolved_path: &Path,
    docker_session: bool,
    placeholder_resolved: bool,
) -> Option<String> {
    if placeholder_resolved {
        return None;
    }
    let (prefix, relative) = extract_skill_alias_relative_path(path_str)?;
    let mut parts = relative.split(['/', '\\']);
    let (skill_dir, file) = (parts.next()?, parts.next()?);
    if file != "SKILL.md" || parts.next().is_some() || skill_dir.is_empty() || skill_dir == "." {
        return None;
    }

    let (_, dir, used_container_path) =
        skill_alias_base_dir(path_str, resolved_path, docker_session)?;
    let label = if used_container_path {
        "Base directory (container)"
    } else {
        "Base directory"
    };

    Some(format!(
        "{label}: `{dir}`\nUse this directory (not the `{prefix}` alias) in workspace__runShell commands; the alias only works in file tools."
    ))
}

#[cfg(test)]
mod tests {
    use super::{skill_base_directory_note, substitute_skill_base_dir};
    use std::path::Path;

    fn substituted(
        alias: &str,
        resolved: &str,
        docker_session: bool,
        body: &str,
    ) -> Option<String> {
        let mut content = body.to_string();
        substitute_skill_base_dir(alias, Path::new(resolved), docker_session, &mut content)
            .then_some(content)
    }

    #[test]
    fn skill_md_placeholder_uses_the_host_directory() {
        let content = substituted(
            "@user-skills/my-skill/SKILL.md",
            "/host/data/user_skills/my-skill/SKILL.md",
            false,
            "run `<skill-base-dir>/scripts/convert.py`",
        )
        .expect("substituted");

        assert_eq!(
            content,
            "run `/host/data/user_skills/my-skill/scripts/convert.py`"
        );
        assert!(!content.contains("<skill-base-dir>"));
    }

    #[test]
    fn windows_verbatim_host_path_is_stripped_and_usable() {
        // `SecurityValidator` canonicalizes its base directory, so a Windows host path
        // arrives verbatim. Win32 does not normalize `/` inside `\\?\`, so the substituted
        // command must get a plain drive path with backslashes.
        let content = substituted(
            "@system-skills/docs/SKILL.md",
            r"\\?\C:\data\system_skills\docs\SKILL.md",
            false,
            r#"& "<venv>" "<skill-base-dir>/scripts/convert.py" --in "a.md" --to pdf"#,
        )
        .expect("substituted");

        assert_eq!(
            content,
            r#"& "<venv>" "C:\data\system_skills\docs/scripts/convert.py" --in "a.md" --to pdf"#
        );
        assert!(!content.contains(r"\\?\"), "{content}");
        assert!(!content.contains("<skill-base-dir>"), "{content}");
    }

    #[test]
    fn windows_verbatim_nested_reference_resolves_to_the_skill_root() {
        let content = substituted(
            "@system-skills/docs/references/md-to-pdf.md",
            r"\\?\C:\data\system_skills\docs\references\md-to-pdf.md",
            false,
            "<skill-base-dir>",
        )
        .expect("substituted");

        assert_eq!(content, r"C:\data\system_skills\docs");
    }

    #[test]
    fn windows_forward_slash_verbatim_prefix_is_stripped() {
        let content = substituted(
            "@system-skills/docs/SKILL.md",
            "//?/C:/data/system_skills/docs/SKILL.md",
            false,
            "<skill-base-dir>",
        )
        .expect("substituted");

        assert_eq!(content, "C:/data/system_skills/docs");
        assert!(!content.contains("//?/"), "{content}");
    }

    #[test]
    fn windows_verbatim_unc_path_keeps_its_prefix() {
        // Network paths need the verbatim form, so the prefix must survive.
        let content = substituted(
            "@system-skills/docs/SKILL.md",
            r"\\?\UNC\server\share\docs\SKILL.md",
            false,
            "<skill-base-dir>",
        )
        .expect("substituted");

        assert_eq!(content, r"\\?\UNC\server\share\docs");
    }

    #[test]
    fn skill_md_placeholder_uses_the_container_directory_in_docker_sessions() {
        let content = substituted(
            "@system-skills/docs/SKILL.md",
            "/host/data/system_skills/docs/SKILL.md",
            true,
            "<skill-base-dir>/scripts/convert.py",
        )
        .expect("substituted");

        assert_eq!(
            content,
            "/opt/libragent/skills/system/docs/scripts/convert.py"
        );
    }

    #[test]
    fn references_placeholder_resolves_to_the_skill_root() {
        let content = substituted(
            "@system-skills/docs/references/md-to-pdf.md",
            "/host/data/system_skills/docs/references/md-to-pdf.md",
            false,
            "\"<skill-base-dir>/scripts/convert.py\"",
        )
        .expect("substituted");

        assert_eq!(
            content,
            "\"/host/data/system_skills/docs/scripts/convert.py\""
        );
    }

    #[test]
    fn docker_references_use_the_container_root() {
        let content = substituted(
            "@system-skills/docs/references/to-md.md",
            "/host/data/system_skills/docs/references/to-md.md",
            true,
            "<skill-base-dir>",
        )
        .expect("substituted");

        assert_eq!(content, "/opt/libragent/skills/system/docs");
    }

    #[test]
    fn files_without_the_placeholder_are_untouched() {
        assert!(substituted(
            "@system-skills/docs/SKILL.md",
            "/host/data/system_skills/docs/SKILL.md",
            false,
            "no placeholder here",
        )
        .is_none());
    }

    #[test]
    fn unsupported_skill_files_keep_the_placeholder() {
        for alias in [
            "@system-skills/docs/scripts/convert.py",
            "@system-skills/docs/references/nested/deep.md",
            "@system-skills/docs/README.md",
            "@system-skills/SKILL.md",
            "src/SKILL.md",
        ] {
            assert!(
                substituted(
                    alias,
                    "/host/data/system_skills/docs/SKILL.md",
                    false,
                    "<skill-base-dir>"
                )
                .is_none(),
                "{alias} must not be substituted"
            );
        }
    }

    #[test]
    fn docker_session_gets_the_container_base_directory() {
        let note = skill_base_directory_note(
            "@system-skills/skill-deployer/SKILL.md",
            Path::new("/host/data/system_skills/skill-deployer/SKILL.md"),
            true,
            false,
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
            false,
        )
        .expect("note");

        assert!(note.contains("Base directory: `/host/data/assistants/a/skills/helper`"));
    }

    #[test]
    fn windows_backslash_alias_paths_get_the_note() {
        let note = skill_base_directory_note(
            "@system-skills\\skill-deployer\\SKILL.md",
            Path::new("C:/data/system_skills/skill-deployer/SKILL.md"),
            true,
            false,
        )
        .expect("note");

        assert!(note
            .contains("Base directory (container): `/opt/libragent/skills/system/skill-deployer`"));
    }

    #[test]
    fn placeholder_substitution_suppresses_the_note() {
        assert!(skill_base_directory_note(
            "@system-skills/docs/SKILL.md",
            Path::new("/host/data/system_skills/docs/SKILL.md"),
            false,
            true,
        )
        .is_none());
    }

    #[test]
    fn other_reads_get_no_note() {
        let resolved = Path::new("/host/data/system_skills/x/references/a.md");
        assert!(skill_base_directory_note(
            "@system-skills/x/references/a.md",
            resolved,
            false,
            false
        )
        .is_none());
        assert!(
            skill_base_directory_note("@system-skills/x/SKILL.md.bak", resolved, false, false)
                .is_none()
        );
        assert!(skill_base_directory_note("src/SKILL.md", resolved, false, false).is_none());
    }
}
