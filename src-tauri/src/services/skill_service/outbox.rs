//! Host-side handoff for `skill.py deploy` inside Docker sessions.
//!
//! The container's user skills mount is read-only, so `skill.py` stages the
//! validated skill in `<workspace>/.libragent/skill-outbox/<name>/`. After a
//! shell command in a Docker session finishes, the host picks up each staged
//! skill, validates it again, installs it into the user skills folder and
//! leaves `<name>.status` next to the outbox entry for the agent to read.
//! No endpoint or approval UI is involved: the workspace folder is the only
//! channel, and only `<user skills>/<name>/` is ever written.

use super::cache::invalidate_skill_scan_cache;
use super::contracts::SKILL_FILE_NAME;
use super::directories::get_user_skills_directory;
use super::scanning::{copy_dir_recursive, parse_skill_metadata};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const SKILL_OUTBOX_RELATIVE_DIR: &str = ".libragent/skill-outbox";

fn valid_skill_dir_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

fn install_one(entry: &Path, name: &str, user_skills: &Path) -> Result<(), String> {
    if WalkDir::new(entry)
        .into_iter()
        .filter_map(Result::ok)
        .any(|e| e.path_is_symlink())
    {
        return Err("symlinks are not allowed in a skill".to_string());
    }
    let skill_file = entry.join(SKILL_FILE_NAME);
    if !skill_file.is_file() {
        return Err(format!("{SKILL_FILE_NAME} is missing"));
    }
    let metadata = parse_skill_metadata(&skill_file)?;
    if metadata.name != name {
        return Err(format!(
            "frontmatter name '{}' does not match folder '{name}'",
            metadata.name
        ));
    }

    fs::create_dir_all(user_skills).map_err(|e| e.to_string())?;
    let staging = user_skills.join(format!(".staging-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        let staged = staging.join(name);
        copy_dir_recursive(entry, &staged)?;
        let target = user_skills.join(name);
        let previous = staging.join("previous");
        let had_previous = target.exists();
        if had_previous {
            fs::rename(&target, &previous).map_err(|e| e.to_string())?;
        }
        if let Err(error) = fs::rename(&staged, &target) {
            if had_previous {
                let _ = fs::rename(&previous, &target);
            }
            return Err(error.to_string());
        }
        Ok(())
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

/// Installs every staged skill under `<workspace>/.libragent/skill-outbox`
/// into `user_skills`. Returns `(name, outcome)` per entry.
pub fn install_outbox_into(
    workspace: &Path,
    user_skills: &Path,
) -> Vec<(String, Result<(), String>)> {
    let outbox: PathBuf = workspace.join(SKILL_OUTBOX_RELATIVE_DIR);
    let Ok(entries) = fs::read_dir(&outbox) else {
        return Vec::new();
    };
    let mut outcomes = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let outcome = if valid_skill_dir_name(&name) {
            install_one(&path, &name, user_skills)
        } else {
            Err("invalid skill folder name".to_string())
        };
        let status = match &outcome {
            Ok(()) => "installed".to_string(),
            Err(reason) => format!("rejected: {reason}"),
        };
        let _ = fs::write(outbox.join(format!("{name}.status")), status);
        let _ = fs::remove_dir_all(&path);
        outcomes.push((name, outcome));
    }
    outcomes
}

/// [`install_outbox_into`] for the app's user skills folder; refreshes skill caches on change.
pub fn install_outbox(workspace: &Path) -> Vec<(String, Result<(), String>)> {
    let Ok(user_skills) = get_user_skills_directory() else {
        return Vec::new();
    };
    let outcomes = install_outbox_into(workspace, &user_skills);
    if outcomes.iter().any(|(_, outcome)| outcome.is_ok()) {
        invalidate_skill_scan_cache();
    }
    outcomes
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str =
        "---\nname: demo\ndescription: \"Use when testing the outbox.\"\n---\n# Demo\n";

    fn stage(workspace: &Path, name: &str, skill_md: &str) -> PathBuf {
        let dir = workspace.join(SKILL_OUTBOX_RELATIVE_DIR).join(name);
        fs::create_dir_all(dir.join("scripts")).unwrap();
        fs::write(dir.join(SKILL_FILE_NAME), skill_md).unwrap();
        fs::write(dir.join("scripts/run.py"), "print(1)").unwrap();
        dir
    }

    fn status(workspace: &Path, name: &str) -> String {
        fs::read_to_string(
            workspace
                .join(SKILL_OUTBOX_RELATIVE_DIR)
                .join(format!("{name}.status")),
        )
        .unwrap()
    }

    #[test]
    fn installs_staged_skill_and_overwrites_previous() {
        let root = tempfile::tempdir().unwrap();
        let (ws, skills) = (root.path().join("ws"), root.path().join("user_skills"));
        fs::create_dir_all(skills.join("demo")).unwrap();
        fs::write(skills.join("demo/old.txt"), "old").unwrap();
        let staged = stage(&ws, "demo", GOOD);

        let outcomes = install_outbox_into(&ws, &skills);
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].1.is_ok());
        assert!(skills.join("demo/scripts/run.py").is_file());
        assert!(!skills.join("demo/old.txt").exists());
        assert!(!staged.exists());
        assert_eq!(status(&ws, "demo"), "installed");
        assert_eq!(
            fs::read_dir(&skills).unwrap().count(),
            1,
            "no staging leftovers"
        );
    }

    #[test]
    fn rejects_invalid_names_mismatched_frontmatter_and_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let (ws, skills) = (root.path().join("ws"), root.path().join("user_skills"));
        stage(&ws, "Bad_Name", GOOD);
        stage(&ws, "other", GOOD);
        let linked = stage(&ws, "linked", &GOOD.replace("demo", "linked"));
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/hostname", linked.join("link.txt")).unwrap();
        #[cfg(not(unix))]
        let _ = linked;

        let outcomes = install_outbox_into(&ws, &skills);
        assert_eq!(outcomes.len(), 3);
        for name in ["Bad_Name", "other"] {
            assert!(status(&ws, name).starts_with("rejected"), "{name}");
        }
        #[cfg(unix)]
        assert!(status(&ws, "linked").contains("symlink"));
        assert!(!skills.join("other").exists());
        assert!(!skills.join("Bad_Name").exists());
    }

    #[test]
    fn empty_or_missing_outbox_is_a_noop() {
        let root = tempfile::tempdir().unwrap();
        assert!(install_outbox_into(root.path(), &root.path().join("user_skills")).is_empty());
    }
}
