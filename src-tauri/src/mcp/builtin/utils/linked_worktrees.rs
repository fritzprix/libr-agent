//! Linked git worktrees of the repository at a workspace root (#2589).
//!
//! Found by reading the `.git` metadata files only; no `git` process is started.

use std::path::{Path, PathBuf};

/// Resolves `value` against `base` unless it is absolute.
fn resolve(base: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

/// The directory named by the `gitdir: <dir>` line of a `.git` file.
fn read_gitdir_file(dot_git: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(dot_git).ok()?;
    let dir = text.lines().next()?.strip_prefix("gitdir:")?.trim();
    Some(resolve(dot_git.parent()?, dir))
}

/// The canonical common git dir of the repository at `root`, and whether `root` is
/// itself a linked worktree.
///
/// A linked root's `.git` file and `commondir` can be written by the file tools, so the
/// common dir is trusted only when it registers `root`: the admin dir must be
/// `<common>/worktrees/<name>` and must link back to `root`.
fn common_dir(root: &Path) -> Option<(PathBuf, bool)> {
    let dot_git = root.join(".git");
    if std::fs::metadata(&dot_git).ok()?.is_dir() {
        return Some((dot_git.canonicalize().ok()?, false));
    }
    let admin = read_gitdir_file(&dot_git)?.canonicalize().ok()?;
    let commondir = std::fs::read_to_string(admin.join("commondir")).ok()?;
    let common = resolve(&admin, commondir.trim()).canonicalize().ok()?;
    if admin.parent()? != common.join("worktrees") {
        return None;
    }
    if registered_worktree(&admin)? != root.canonicalize().ok()? {
        return None;
    }
    Some((common, true))
}

/// The canonical worktree directory registered in `admin`
/// (`<commondir>/worktrees/<name>`), if its `.git` file links back to `admin`.
/// A prunable worktree whose directory is gone yields `None`.
fn registered_worktree(admin: &Path) -> Option<PathBuf> {
    let gitdir = std::fs::read_to_string(admin.join("gitdir")).ok()?;
    let dot_git = resolve(admin, gitdir.trim());
    if !std::fs::metadata(&dot_git).ok()?.is_file() {
        return None;
    }
    let back_link = read_gitdir_file(&dot_git)?.canonicalize().ok()?;
    if back_link != admin.canonicalize().ok()? {
        return None;
    }
    dot_git.parent()?.canonicalize().ok()
}

/// The git dirs that git uses for the repository at `root`: the `.git` dir, or the
/// `gitdir:` target of a `.git` file plus its `commondir` target. Unlike `common_dir`,
/// nothing here is checked for trust, because this list is used to deny writes.
pub fn git_metadata_dirs(root: &Path) -> Vec<PathBuf> {
    let dot_git = root.join(".git");
    let Ok(metadata) = std::fs::metadata(&dot_git) else {
        return Vec::new();
    };
    if metadata.is_dir() {
        return vec![dot_git];
    }
    let Some(gitdir) = read_gitdir_file(&dot_git) else {
        return Vec::new();
    };
    let commondir = std::fs::read_to_string(gitdir.join("commondir"))
        .ok()
        .map(|common| resolve(&gitdir, common.trim()));
    std::iter::once(gitdir).chain(commondir).collect()
}

/// Canonical roots of the other worktrees of the repository at `root`: its linked
/// worktrees and, when `root` is a linked worktree, the main worktree. Empty when
/// `root` is not in a git repository.
pub fn linked_worktree_roots(root: &Path) -> Vec<PathBuf> {
    let Some((common, root_is_linked)) = common_dir(root) else {
        return Vec::new();
    };
    let canonical_root = root.canonicalize().ok();
    let mut roots = Vec::new();
    if root_is_linked && common.file_name().is_some_and(|name| name == ".git") {
        if let Some(main) = common.parent() {
            roots.push(main.to_path_buf());
        }
    }
    let Ok(entries) = std::fs::read_dir(common.join("worktrees")) else {
        return roots;
    };
    for entry in entries.flatten() {
        if let Some(worktree) = registered_worktree(&entry.path()) {
            if canonical_root.as_ref() != Some(&worktree) && !roots.contains(&worktree) {
                roots.push(worktree);
            }
        }
    }
    roots
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};

    /// A main worktree at `<parent>/repo` with a real `.git` dir. `parent` is
    /// canonicalized so paths of files that do not exist yet use the same form as the
    /// canonical roots (Windows temp dirs may use 8.3 short names).
    pub fn main_repo(parent: &Path) -> PathBuf {
        std::fs::create_dir_all(parent).unwrap();
        let repo = parent.canonicalize().unwrap().join("repo");
        std::fs::create_dir_all(repo.join(".git").join("worktrees")).unwrap();
        repo
    }

    /// Registers a linked worktree `<parent>/repo.worktrees/<name>` of `repo`.
    pub fn add_worktree(repo: &Path, name: &str) -> PathBuf {
        let parent = repo.parent().unwrap();
        let worktree = parent.join("repo.worktrees").join(name);
        let admin = repo.join(".git").join("worktrees").join(name);
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::create_dir_all(&admin).unwrap();
        std::fs::write(
            admin.join("gitdir"),
            format!("{}\n", worktree.join(".git").display()),
        )
        .unwrap();
        std::fs::write(admin.join("commondir"), "../..\n").unwrap();
        std::fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", admin.display()),
        )
        .unwrap();
        worktree
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{add_worktree, main_repo};
    use super::*;

    fn canonical(path: &Path) -> PathBuf {
        path.canonicalize().unwrap()
    }

    #[test]
    fn a_main_worktree_lists_its_linked_worktrees() {
        let dir = tempfile::tempdir().unwrap();
        let repo = main_repo(dir.path());
        let a = add_worktree(&repo, "a");
        let b = add_worktree(&repo, "b");

        let mut roots = linked_worktree_roots(&repo);
        roots.sort();
        let mut expected = vec![canonical(&a), canonical(&b)];
        expected.sort();
        assert_eq!(roots, expected);
    }

    #[test]
    fn a_linked_worktree_lists_the_main_worktree_and_its_siblings() {
        let dir = tempfile::tempdir().unwrap();
        let repo = main_repo(dir.path());
        let a = add_worktree(&repo, "a");
        let b = add_worktree(&repo, "b");

        let roots = linked_worktree_roots(&a);
        assert!(roots.contains(&canonical(&repo)));
        assert!(roots.contains(&canonical(&b)));
        assert!(!roots.contains(&canonical(&a)), "the root is not listed");
    }

    #[test]
    fn a_worktree_whose_git_file_does_not_link_back_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let repo = main_repo(dir.path());
        let forged = add_worktree(&repo, "forged");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(
            forged.join(".git"),
            format!("gitdir: {}\n", elsewhere.display()),
        )
        .unwrap();

        assert!(linked_worktree_roots(&repo).is_empty());
    }

    #[test]
    fn a_forged_git_file_at_a_non_git_root_adds_no_other_repository() {
        let dir = tempfile::tempdir().unwrap();
        let other = main_repo(&dir.path().join("other"));
        let root = dir.path().join("session");
        let fake = root.join("fake");
        std::fs::create_dir_all(&fake).unwrap();
        std::fs::write(root.join(".git"), format!("gitdir: {}\n", fake.display())).unwrap();
        std::fs::write(
            fake.join("commondir"),
            format!("{}\n", other.join(".git").display()),
        )
        .unwrap();

        assert!(linked_worktree_roots(&root).is_empty());
    }

    #[test]
    fn a_git_file_pointing_at_another_worktrees_admin_dir_adds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let other = main_repo(&dir.path().join("other"));
        add_worktree(&other, "theirs");
        let root = dir.path().join("session");
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

        assert!(linked_worktree_roots(&root).is_empty());
    }

    #[test]
    fn a_prunable_worktree_and_a_plain_directory_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let repo = main_repo(dir.path());
        let gone = add_worktree(&repo, "gone");
        std::fs::remove_dir_all(&gone).unwrap();

        assert!(linked_worktree_roots(&repo).is_empty());
        assert!(linked_worktree_roots(dir.path()).is_empty());
    }
}
