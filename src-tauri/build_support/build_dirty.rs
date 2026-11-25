use std::path::Path;
use std::process::Command;

/// True when the repository at `repo_dir` has uncommitted changes to tracked
/// files (staged or unstaged). Feeds `LIBRAGENT_BUILD_DIRTY` (#2744).
///
/// `git diff-index` consults the index's cached stat data, so a file whose
/// mtime alone moved — a build sync script rewriting identical content, for
/// example — can make a clean checkout report dirty (#2963). Refreshing the
/// index first makes the result depend only on content differences. A build
/// from a clean checkout therefore reports not-dirty, and a build with real
/// uncommitted changes still reports dirty.
pub fn worktree_is_dirty(repo_dir: &Path) -> bool {
    // The refresh exit status is ignored on purpose: it is non-zero exactly
    // when there are real changes, which `diff-index` below then reports.
    run_git(repo_dir, &["update-index", "-q", "--refresh"]);
    !run_git(repo_dir, &["diff-index", "--quiet", "HEAD", "--"])
}

/// Runs `git` in `repo_dir`, returning whether it exited successfully.
fn run_git(repo_dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .current_dir(repo_dir)
        .args(args)
        .status()
        .is_ok_and(|status| status.success())
}
