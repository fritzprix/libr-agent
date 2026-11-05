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

fn assert_outside(manager: &crate::services::SecureFileManager, path: &str) {
    let validator = manager.get_security_validator();
    for error in [
        validator.validate_path_for_read(path).unwrap_err(),
        validator.validate_path_for_write(path).unwrap_err(),
    ] {
        assert!(
            matches!(error, SecurityError::OutsideWorkspaceRoot(_)),
            "{error}"
        );
    }
}

#[test]
fn a_forged_git_file_at_the_root_cannot_add_another_repository() {
    let dir = tempfile::tempdir().unwrap();
    let other = main_repo(&dir.path().join("other"));
    let root = dir.path().canonicalize().unwrap().join("session");
    let fake = root.join("fake");
    std::fs::create_dir_all(&fake).unwrap();
    std::fs::write(root.join(".git"), format!("gitdir: {}\n", fake.display())).unwrap();
    std::fs::write(
        fake.join("commondir"),
        format!("{}\n", other.join(".git").display()),
    )
    .unwrap();

    assert_outside(&file_tool_manager(root, true), &file_in(&other));
}

#[test]
fn a_root_pointing_at_another_worktrees_admin_dir_cannot_add_that_repository() {
    let dir = tempfile::tempdir().unwrap();
    let other = main_repo(&dir.path().join("other"));
    let theirs = add_worktree(&other, "theirs");
    let root = dir.path().canonicalize().unwrap().join("session");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join(".git"),
        format!(
            "gitdir: {}\n",
            other
                .join(".git")
                .join("worktrees")
                .join("theirs")
                .display()
        ),
    )
    .unwrap();

    let manager = file_tool_manager(root, true);
    assert_outside(&manager, &file_in(&other));
    assert_outside(&manager, &file_in(&theirs));
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

fn assert_git_write_refused(manager: &crate::SecureFileManager, path: &Path) {
    let path = display_workspace_path(path);
    let error = manager
        .get_security_validator()
        .validate_path_for_write(&path)
        .unwrap_err();
    assert!(matches!(error, SecurityError::AccessDenied(_)), "{error}");
    assert!(error.to_string().contains("git metadata"), "{error}");
}

#[test]
fn a_linked_worktree_session_cannot_write_main_git_hooks() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    let worker = add_worktree(&repo, "worker");
    let manager = file_tool_manager(worker.clone(), true);

    assert_git_write_refused(&manager, &repo.join(".git").join("hooks").join("x"));
    assert_git_write_refused(&manager, &repo.join(".git").join("config"));
    assert_git_write_refused(&manager, &worker.join(".git"));
    assert!(manager
        .get_security_validator()
        .validate_path_for_write(&file_in(&worker))
        .is_ok());
}

#[test]
fn the_workspace_root_git_config_cannot_be_written() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    std::fs::write(repo.join(".git").join("config"), "[core]\n").unwrap();

    for allow in [true, false] {
        let manager = file_tool_manager(repo.clone(), allow);
        assert_git_write_refused(&manager, &repo.join(".git").join("config"));
        let relative_error = manager
            .get_security_validator()
            .validate_path_for_write(".git/config")
            .unwrap_err();
        assert!(matches!(relative_error, SecurityError::AccessDenied(_)));
        assert!(manager
            .get_security_validator()
            .validate_path_for_read(".git/config")
            .is_ok());
        assert!(manager
            .get_security_validator()
            .validate_path_for_write("src/new.rs")
            .is_ok());
    }
}

#[test]
fn a_worktree_admin_file_cannot_be_written() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    add_worktree(&repo, "worker");
    let manager = file_tool_manager(repo.clone(), true);

    let gitdir = repo
        .join(".git")
        .join("worktrees")
        .join("worker")
        .join("gitdir");
    assert_git_write_refused(&manager, &gitdir);
    assert!(manager
        .get_security_validator()
        .validate_path_for_read(&display_workspace_path(&gitdir))
        .is_ok());
}

#[cfg(unix)]
#[test]
fn a_symlink_into_git_metadata_is_refused_for_write() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    let worker = add_worktree(&repo, "worker");
    let hooks = repo.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    std::fs::write(hooks.join("pre-commit"), "#!/bin/sh\n").unwrap();
    std::os::unix::fs::symlink(&hooks, worker.join("hooks-link")).unwrap();
    std::os::unix::fs::symlink(hooks.join("pre-commit"), worker.join("pre-commit-link")).unwrap();
    let manager = file_tool_manager(worker.clone(), true);

    assert_git_write_refused(&manager, &worker.join("hooks-link").join("post-checkout"));
    assert_git_write_refused(&manager, &worker.join("hooks-link").join("pre-commit"));
    assert_git_write_refused(&manager, &worker.join("pre-commit-link"));
}

#[test]
fn a_separate_git_dir_inside_the_workspace_cannot_be_written() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap().join("repo");
    let gitdata = root.join(".gitdata");
    std::fs::create_dir_all(gitdata.join("hooks")).unwrap();
    std::fs::write(gitdata.join("config"), "[core]\n").unwrap();
    std::fs::write(root.join(".git"), "gitdir: .gitdata\n").unwrap();

    for allow in [true, false] {
        let manager = file_tool_manager(root.clone(), allow);
        assert_git_write_refused(&manager, &gitdata.join("config"));
        assert_git_write_refused(&manager, &gitdata.join("hooks").join("x"));
        assert!(manager
            .get_security_validator()
            .validate_path_for_write("src/new.rs")
            .is_ok());
    }
}

#[test]
fn files_named_like_git_metadata_stay_writable() {
    let dir = tempfile::tempdir().unwrap();
    let repo = main_repo(dir.path());
    let manager = file_tool_manager(repo, true);
    let validator = manager.get_security_validator();

    for path in [".gitignore", ".gitattributes", ".github/workflows/x.yml"] {
        assert!(validator.validate_path_for_write(path).is_ok(), "{path}");
    }
    assert!(validator
        .validate_path_for_write(".git::$INDEX_ALLOCATION/hooks/x")
        .is_err());
}
