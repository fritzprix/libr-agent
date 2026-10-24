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
use super::scanning::parse_skill_metadata;
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

const MAX_OUTBOX_ENTRIES: usize = 32;
const MAX_SKILL_FILES: usize = 1_000;
const MAX_SKILL_BYTES: u64 = 16 * 1024 * 1024;

/// The container controls everything under the outbox. Nothing is followed: a
/// symlink, a swapped-in file or a special file fails the copy.
#[derive(Default)]
struct CopyBudget {
    files: usize,
    bytes: u64,
}

#[cfg(unix)]
fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}

#[cfg(not(unix))]
fn same_file(_: &fs::Metadata, _: &fs::Metadata) -> bool {
    true
}

fn copy_file_no_follow(src: &Path, dst: &Path, listed: &fs::Metadata) -> Result<u64, String> {
    let mut input = fs::File::open(src).map_err(|e| e.to_string())?;
    let opened = input.metadata().map_err(|e| e.to_string())?;
    if !opened.is_file() || !same_file(listed, &opened) {
        return Err("a file changed while it was being copied".to_string());
    }
    let mut output = fs::File::create_new(dst).map_err(|e| e.to_string())?;
    std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())
}

fn copy_tree_no_follow(src: &Path, dst: &Path, budget: &mut CopyBudget) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let listed = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        let target = dst.join(entry.file_name());
        let file_type = listed.file_type();
        if file_type.is_symlink() {
            return Err("symlinks are not allowed in a skill".to_string());
        } else if file_type.is_dir() {
            copy_tree_no_follow(&path, &target, budget)?;
        } else if file_type.is_file() {
            budget.files += 1;
            if budget.files > MAX_SKILL_FILES {
                return Err("too many files in the skill".to_string());
            }
            budget.bytes += copy_file_no_follow(&path, &target, &listed)?;
            if budget.bytes > MAX_SKILL_BYTES {
                return Err("skill is too large".to_string());
            }
        } else {
            return Err("special files are not allowed in a skill".to_string());
        }
    }
    Ok(())
}

fn install_one(entry: &Path, name: &str, user_skills: &Path) -> Result<(), String> {
    fs::create_dir_all(user_skills).map_err(|e| e.to_string())?;
    let staging = user_skills.join(format!(".staging-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        let staged = staging.join(name);
        copy_tree_no_follow(entry, &staged, &mut CopyBudget::default())?;
        // The copy is the only thing that gets validated and installed.
        if WalkDir::new(&staged)
            .into_iter()
            .filter_map(Result::ok)
            .any(|e| e.path_is_symlink())
        {
            return Err("symlinks are not allowed in a skill".to_string());
        }
        let skill_file = staged.join(SKILL_FILE_NAME);
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

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
}

/// Writes `<name>.status` without following a link the container may have planted.
fn write_status(outbox: &Path, name: &str, status: &str) {
    let path = outbox.join(format!("{name}.status"));
    let _ = fs::remove_file(&path);
    if let Ok(mut file) = fs::File::create_new(&path) {
        let _ = std::io::Write::write_all(&mut file, status.as_bytes());
    }
}

/// Installs every staged skill under `<workspace>/.libragent/skill-outbox`
/// into `user_skills`. Returns `(name, outcome)` per entry.
///
/// `.libragent` and the outbox must be real directories: the workspace is
/// writable from the container, and a link there would point at host paths.
pub fn install_outbox_into(
    workspace: &Path,
    user_skills: &Path,
) -> Vec<(String, Result<(), String>)> {
    let outbox: PathBuf = workspace.join(SKILL_OUTBOX_RELATIVE_DIR);
    if !is_real_dir(&workspace.join(".libragent")) || !is_real_dir(&outbox) {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(&outbox) else {
        return Vec::new();
    };
    let mut outcomes = Vec::new();
    for entry in entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .take(MAX_OUTBOX_ENTRIES)
    {
        let path = entry.path();
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
        write_status(&outbox, &name, &status);
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

    #[cfg(unix)]
    #[test]
    fn symlinked_outbox_or_libragent_dir_is_ignored_and_nothing_is_deleted() {
        let root = tempfile::tempdir().unwrap();
        let skills = root.path().join("user_skills");
        let host_dir = root.path().join("host-projects");
        fs::create_dir_all(host_dir.join("proj")).unwrap();
        fs::write(
            host_dir.join("proj").join(SKILL_FILE_NAME),
            GOOD.replace("demo", "proj"),
        )
        .unwrap();

        // outbox -> host dir
        let ws = root.path().join("ws");
        fs::create_dir_all(ws.join(".libragent")).unwrap();
        std::os::unix::fs::symlink(&host_dir, ws.join(SKILL_OUTBOX_RELATIVE_DIR)).unwrap();
        assert!(install_outbox_into(&ws, &skills).is_empty());

        // .libragent -> dir containing skill-outbox/proj
        let ws2 = root.path().join("ws2");
        fs::create_dir_all(&ws2).unwrap();
        let libragent = root.path().join("fake-libragent");
        fs::create_dir_all(libragent.join("skill-outbox/proj")).unwrap();
        std::os::unix::fs::symlink(&libragent, ws2.join(".libragent")).unwrap();
        assert!(install_outbox_into(&ws2, &skills).is_empty());

        assert!(host_dir.join("proj").join(SKILL_FILE_NAME).is_file());
        assert!(libragent.join("skill-outbox/proj").is_dir());
        assert!(!host_dir.join("proj.status").exists());
        assert!(!skills.exists());
    }

    #[cfg(unix)]
    #[test]
    fn planted_status_link_is_not_followed_and_file_links_are_not_copied() {
        let root = tempfile::tempdir().unwrap();
        let (ws, skills) = (root.path().join("ws"), root.path().join("user_skills"));
        let victim = root.path().join("victim.txt");
        fs::write(&victim, "keep").unwrap();
        let secret = root.path().join("secret.txt");
        fs::write(&secret, "secret").unwrap();

        let staged = stage(&ws, "demo", GOOD);
        std::os::unix::fs::symlink(
            &victim,
            ws.join(SKILL_OUTBOX_RELATIVE_DIR).join("demo.status"),
        )
        .unwrap();
        install_outbox_into(&ws, &skills);
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep");
        assert_eq!(status(&ws, "demo"), "installed");

        let staged2 = stage(&ws, "linked", &GOOD.replace("demo", "linked"));
        std::os::unix::fs::symlink(&secret, staged2.join("link.txt")).unwrap();
        install_outbox_into(&ws, &skills);
        assert!(status(&ws, "linked").contains("symlink"));
        assert!(!skills.join("linked").exists());
        let _ = staged;
    }

    #[test]
    fn oversized_skills_and_extra_entries_are_bounded() {
        let root = tempfile::tempdir().unwrap();
        let (ws, skills) = (root.path().join("ws"), root.path().join("user_skills"));
        let staged = stage(&ws, "demo", GOOD);
        for i in 0..=MAX_SKILL_FILES {
            fs::write(staged.join(format!("f{i}.txt")), "x").unwrap();
        }
        install_outbox_into(&ws, &skills);
        assert!(status(&ws, "demo").contains("too many files"));
        assert!(!skills.join("demo").exists());

        for i in 0..(MAX_OUTBOX_ENTRIES + 5) {
            stage(&ws, &format!("s{i}"), "bad");
        }
        assert_eq!(install_outbox_into(&ws, &skills).len(), MAX_OUTBOX_ENTRIES);
    }

    #[test]
    fn empty_or_missing_outbox_is_a_noop() {
        let root = tempfile::tempdir().unwrap();
        assert!(install_outbox_into(root.path(), &root.path().join("user_skills")).is_empty());
    }
}
