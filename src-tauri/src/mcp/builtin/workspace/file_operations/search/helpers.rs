use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

pub(super) const BINARY_SNIFF_BYTES: usize = 8 * 1024;
pub(super) const MAX_SEARCH_CONTENT_FILE_SIZE: usize = 5 * 1024 * 1024;
pub(super) const SKIPPED_SEARCH_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "dist",
    "target",
    ".next",
    "coverage",
];

pub(super) enum SearchEntrySkipReason {
    Gitignored,
    HeavyweightDirectory,
}

pub(super) struct ScopedGitignoreMatcher {
    search_root: PathBuf,
    matchers: Vec<(PathBuf, ignore::gitignore::Gitignore)>,
}

impl ScopedGitignoreMatcher {
    fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let scope_target = if is_dir {
            path
        } else {
            path.parent().unwrap_or(&self.search_root)
        };

        let mut scoped_ancestors = Vec::new();
        for ancestor in scope_target.ancestors() {
            if !ancestor.starts_with(&self.search_root) {
                break;
            }
            scoped_ancestors.push(ancestor);
            if ancestor == self.search_root {
                break;
            }
        }
        scoped_ancestors.reverse();

        let mut ignored = false;
        for ancestor in scoped_ancestors {
            if let Some((_, matcher)) = self.matchers.iter().find(|(scope, _)| scope == ancestor) {
                let matched = matcher.matched(path, is_dir);
                if matched.is_ignore() {
                    ignored = true;
                } else if matched.is_whitelist() {
                    ignored = false;
                }
            }
        }

        ignored
    }
}

pub(super) fn matches_glob(pattern: &glob::Pattern, path: &Path, file_name: Option<&str>) -> bool {
    if let Some(name) = file_name {
        if pattern.matches(name) {
            return true;
        }
    }
    let path_str = path.to_string_lossy();
    if pattern.matches(&path_str) {
        return true;
    }
    #[cfg(target_os = "windows")]
    if path_str.contains('\\') {
        let normalized = path_str.replace('\\', "/");
        if pattern.matches(&normalized) {
            return true;
        }
    }
    false
}

pub(super) fn classify_search_entry_skip(
    root: &Path,
    entry: &walkdir::DirEntry,
    gitignore: Option<&ScopedGitignoreMatcher>,
) -> Option<SearchEntrySkipReason> {
    if entry.depth() == 0 || entry.path() == root {
        return None;
    }

    if let Some(gitignore_matcher) = gitignore {
        if gitignore_matcher.is_ignored(entry.path(), entry.file_type().is_dir()) {
            return Some(SearchEntrySkipReason::Gitignored);
        }
    }

    if entry.file_type().is_dir() {
        return entry
            .file_name()
            .to_str()
            .filter(|name| SKIPPED_SEARCH_DIR_NAMES.contains(name))
            .map(|_| SearchEntrySkipReason::HeavyweightDirectory);
    }

    None
}

pub(super) fn effective_search_content_file_size_limit() -> usize {
    crate::config::max_file_size().min(MAX_SEARCH_CONTENT_FILE_SIZE)
}

pub(super) async fn is_probably_binary_file(path: &Path) -> bool {
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut buffer = [0u8; BINARY_SNIFF_BYTES];
    let bytes_read = match file.read(&mut buffer).await {
        Ok(bytes_read) => bytes_read,
        Err(_) => return false,
    };

    buffer[..bytes_read].contains(&0)
}

fn add_gitignore_matcher(
    scope_dir: &Path,
    gitignore_path: &Path,
    matchers: &mut Vec<(PathBuf, ignore::gitignore::Gitignore)>,
) {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(scope_dir);
    if builder.add(gitignore_path).is_none() {
        if let Ok(matcher) = builder.build() {
            matchers.push((scope_dir.to_path_buf(), matcher));
        }
    }
}

pub(super) fn build_gitignore_matcher(search_root: &Path) -> Option<ScopedGitignoreMatcher> {
    use walkdir::WalkDir;

    if !search_root.is_dir() {
        return None;
    }

    let mut matchers = Vec::new();
    let root_gitignore = search_root.join(".gitignore");
    if root_gitignore.is_file() {
        add_gitignore_matcher(search_root, &root_gitignore, &mut matchers);
    }

    let walker = WalkDir::new(search_root)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }

            entry.depth() == 0
                || entry
                    .file_name()
                    .to_str()
                    .map(|name| !SKIPPED_SEARCH_DIR_NAMES.contains(&name))
                    .unwrap_or(true)
        })
        .filter_map(Result::ok);

    for entry in walker {
        if entry.file_type().is_file()
            && entry.file_name() == OsStr::new(".gitignore")
            && entry.path() != root_gitignore
        {
            if let Some(scope_dir) = entry.path().parent() {
                add_gitignore_matcher(scope_dir, entry.path(), &mut matchers);
            }
        }
    }

    if matchers.is_empty() {
        return None;
    }

    Some(ScopedGitignoreMatcher {
        search_root: search_root.to_path_buf(),
        matchers,
    })
}
