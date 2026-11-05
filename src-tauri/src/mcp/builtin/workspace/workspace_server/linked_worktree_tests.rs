//! File tool path validation with linked git worktrees (#2589).

use super::file_tool_manager;
use crate::mcp::builtin::utils::linked_worktrees::test_support::{add_worktree, main_repo};
use crate::mcp::builtin::utils::{display_workspace_path, SecurityError};
use std::path::Path;

fn file_in(dir: &Path) -> String {
    let file = dir.join("f.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();
    file.to_string_lossy().to_string()
}

#[test]
fn a_linked_worktree_path_is_accepted_for_read_and_write() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    let worktree = add_worktree(&repo, "worker");
    let manager = file_tool_manager(repo, true);
    let validator = manager.get_security_validator();

    let existing = file_in(&worktree);
    assert!(validator.validate_path_for_read(&existing).is_ok());
    assert!(validator.validate_path_for_write(&existing).is_ok());
    // A file that does not exist yet cannot be canonicalized, so use the plain
    // `C:\...` form an agent sends rather than the verbatim `\\?\C:\...` form.
    let new_file = display_workspace_path(&worktree.join("src").join("new.rs"));
    assert!(validator.validate_path_for_write(&new_file).is_ok());
}

#[test]
fn relative_paths_still_resolve_against_the_workspace_root() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    add_worktree(&repo, "worker");
    let manager = file_tool_manager(repo.clone(), true);

    let resolved = manager
        .get_security_validator()
        .validate_path_for_read("src/lib.rs")
        .unwrap();
    assert_eq!(
        resolved,
        repo.canonicalize().unwrap().join("src").join("lib.rs")
    );
}

#[test]
fn a_sibling_directory_that_is_not_a_worktree_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    add_worktree(&repo, "worker");
    let sibling = dir.path().join("repo.worktrees").join("plain");
    std::fs::create_dir_all(&sibling).unwrap();
    let manager = file_tool_manager(repo, true);

    let error = manager
        .get_security_validator()
        .validate_path_for_read(&file_in(&sibling))
        .unwrap_err();
    assert!(
        matches!(error, SecurityError::OutsideWorkspaceRoot(_)),
        "{error}"
    );
}

#[test]
fn a_forged_git_file_that_fails_the_back_link_check_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    let forged = dir.path().join("forged");
    std::fs::create_dir_all(&forged).unwrap();
    // Points at a real admin dir of the repo, which does not point back to `forged`.
    add_worktree(&repo, "worker");
    let admin = repo.join(".git").join("worktrees").join("worker");
    std::fs::write(
        forged.join(".git"),
        format!("gitdir: {}\n", admin.display()),
    )
    .unwrap();
    let manager = file_tool_manager(repo, true);

    let error = manager
        .get_security_validator()
        .validate_path_for_read(&file_in(&forged))
        .unwrap_err();
    assert!(
        matches!(error, SecurityError::OutsideWorkspaceRoot(_)),
        "{error}"
    );
}

#[test]
fn a_parent_segment_from_the_root_into_a_worktree_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    let worktree = add_worktree(&repo, "worker");
    file_in(&worktree);
    let manager = file_tool_manager(repo, true);

    let error = manager
        .get_security_validator()
        .validate_path_for_read("../repo.worktrees/worker/f.rs")
        .unwrap_err();
    assert!(matches!(error, SecurityError::PathTraversal(_)), "{error}");
}

#[test]
fn a_linked_worktree_root_accepts_the_main_worktree_and_its_siblings() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    let worker = add_worktree(&repo, "worker");
    let sibling = add_worktree(&repo, "worker-2");
    let manager = file_tool_manager(worker, true);
    let validator = manager.get_security_validator();

    assert!(validator.validate_path_for_read(&file_in(&repo)).is_ok());
    assert!(validator
        .validate_path_for_write(&file_in(&sibling))
        .is_ok());
}

#[test]
fn with_the_setting_off_a_worktree_path_gets_the_current_error() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    let worktree = add_worktree(&repo, "worker");
    let path = file_in(&worktree);

    let off = file_tool_manager(repo.clone(), false)
        .get_security_validator()
        .validate_path_for_read(&path)
        .unwrap_err();
    let single_root = crate::SecureFileManager::new_scoped_with_base_dir(repo)
        .get_security_validator()
        .validate_path_for_read(&path)
        .unwrap_err();
    assert!(
        matches!(off, SecurityError::OutsideWorkspaceRoot(_)),
        "{off}"
    );
    assert_eq!(off.to_string(), single_root.to_string());
}

#[cfg(unix)]
#[test]
fn a_symlink_in_a_worktree_that_points_outside_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    let worktree = add_worktree(&repo, "worker");
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    file_in(&outside);
    std::os::unix::fs::symlink(&outside, worktree.join("link")).unwrap();
    let manager = file_tool_manager(repo, true);

    let error = manager
        .get_security_validator()
        .validate_path_for_read(&worktree.join("link").join("f.rs").to_string_lossy())
        .unwrap_err();
    assert!(matches!(error, SecurityError::PathTraversal(_)), "{error}");
}

#[cfg(windows)]
#[test]
fn windows_verbatim_and_case_variants_of_a_worktree_path_are_accepted() {
    use crate::mcp::builtin::utils::strip_windows_verbatim_prefix;

    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    let worktree = add_worktree(&repo, "worker");
    file_in(&worktree);
    let manager = file_tool_manager(repo, true);
    let validator = manager.get_security_validator();

    let verbatim = worktree
        .canonicalize()
        .unwrap()
        .join("f.rs")
        .to_string_lossy()
        .to_string();
    assert!(verbatim.starts_with(r"\\?\"), "{verbatim}");
    assert!(validator.validate_path_for_read(&verbatim).is_ok());

    let plain = strip_windows_verbatim_prefix(&verbatim).to_string();
    let mut chars = plain.chars();
    let drive = chars.next().unwrap().to_ascii_lowercase();
    let case_variant =
        format!("{drive}{}", chars.as_str()).replace("repo.worktrees", "REPO.worktrees");
    assert!(
        validator.validate_path_for_read(&case_variant).is_ok(),
        "{case_variant}"
    );
}
