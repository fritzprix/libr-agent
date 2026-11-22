//! `LIBRAGENT_BUILD_DIRTY` detection sequence, shared with `src-tauri/build.rs`
//! through the same `#[path]` include (#2963): a clean checkout — even after a
//! build rewrote tracked files with identical content — reports not-dirty,
//! while real uncommitted changes still report dirty.

use std::fs::{self, OpenOptions};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

#[path = "../../build_support/build_dirty.rs"]
mod build_dirty;

/// Runs `git` with test-local identity so the machine's global config
/// (GPG signing, aliases, hooks paths) cannot interfere.
fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo)
        .args([
            "-c",
            "user.email=build-dirty-test@example.invalid",
            "-c",
            "user.name=build-dirty-test",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .status()
        .expect("git is available on PATH");
    assert!(
        status.success(),
        "git {args:?} failed in {}",
        repo.display()
    );
}

/// A committed repository containing `tracked.txt` with `content`.
fn init_repo(content: &str) -> TempDir {
    let repo = TempDir::new().unwrap();
    git(repo.path(), &["init", "-b", "main", "."]);
    fs::write(repo.path().join("tracked.txt"), content).unwrap();
    git(repo.path(), &["add", "tracked.txt"]);
    git(repo.path(), &["commit", "-m", "initial"]);
    repo
}

/// Rewrite `tracked.txt` with byte-identical content, then push its mtime
/// forward — exactly what a build sync script does when it re-emits a file.
fn touch_with_identical_content(repo: &Path) {
    let file = repo.join("tracked.txt");
    let content = fs::read(&file).unwrap();
    fs::write(&file, &content).unwrap();
    let handle = OpenOptions::new().write(true).open(&file).unwrap();
    handle
        .set_modified(SystemTime::now() + Duration::from_secs(10))
        .unwrap();
}

#[test]
fn clean_checkout_reports_not_dirty() {
    let repo = init_repo("same bytes\n");
    assert!(!build_dirty::worktree_is_dirty(repo.path()));
}

#[test]
fn identical_rewrite_touching_only_mtime_reports_not_dirty() {
    // #2963: without `update-index --refresh`, `diff-index` compares the
    // index's cached stat data and can call this tree dirty even though no
    // content differs.
    let repo = init_repo("same bytes\n");
    touch_with_identical_content(repo.path());
    assert!(!build_dirty::worktree_is_dirty(repo.path()));
}

#[test]
fn real_uncommitted_change_reports_dirty() {
    let repo = init_repo("same bytes\n");
    fs::write(repo.path().join("tracked.txt"), "changed bytes\n").unwrap();
    assert!(build_dirty::worktree_is_dirty(repo.path()));
}

#[test]
fn staged_change_reports_dirty() {
    let repo = init_repo("same bytes\n");
    fs::write(repo.path().join("tracked.txt"), "staged bytes\n").unwrap();
    git(repo.path(), &["add", "tracked.txt"]);
    assert!(build_dirty::worktree_is_dirty(repo.path()));
}
