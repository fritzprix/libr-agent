use std::path::{Component, Path, PathBuf};

/// Normalize user-provided path string before security and traversal checks.
///
/// Handles:
/// - Home shortcuts (`~`, `~/...`, `$HOME/...`, `${HOME}/...`) → absolute home path
/// - `file://` URLs via proper URL→path conversion (preserves Unix absolute paths)
/// - Leading slashes/backslashes before Windows drive letters (e.g. `/C:/path`, `\C:\path`)
/// - MSYS2/Git Bash POSIX-style paths on Windows only (e.g. `/c/Users/path` → `c:/Users/path`)
pub fn normalize_user_path(user_path: &str) -> String {
    let s = user_path.trim();

    // Expand home shortcuts before joining under workspace base_dir. Without this,
    // `~/foo` is treated as a relative path and creates a literal `~` directory.
    if crate::utils::security::is_home_pseudo_path(s) {
        let expanded = crate::utils::security::expand_home_pseudo(s);
        return normalize_os_path_string(&expanded.to_string_lossy().replace('\\', "/"));
    }

    // Prefer proper file URL parsing so Unix `file:///home/...` keeps its leading `/`
    // and Windows `file:///C:/...` / `file://localhost/C:/...` resolve correctly.
    if s.len() >= 7
        && s.get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file://"))
    {
        if let Ok(parsed) = url::Url::parse(s) {
            if parsed.scheme().eq_ignore_ascii_case("file") {
                if let Ok(path) = parsed.to_file_path() {
                    let with_forward_slashes = path.to_string_lossy().replace('\\', "/");
                    return normalize_os_path_string(&with_forward_slashes);
                }
            }
        }
    }

    normalize_os_path_string(&s.replace('\\', "/"))
}

/// Apply OS-oriented path heuristics after separators are normalized to `/`.
fn normalize_os_path_string(normalized: &str) -> String {
    // Strip leading slashes when followed by a Windows drive letter
    // (e.g. `/C:/foo`, `///C:/foo`, and backslash forms already converted to `/`).
    let trimmed_leading = normalized.trim_start_matches('/');
    if trimmed_leading.len() >= 2 {
        let bytes = trimmed_leading.as_bytes();
        if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            let is_sep_or_end = trimmed_leading.len() == 2 || bytes[2] == b'/' || bytes[2] == b'.';
            if is_sep_or_end {
                return trimmed_leading.to_string();
            }
        }
    }

    // MSYS2 / Git Bash POSIX-style paths are Windows-only. On Unix, `/c/...` is a
    // legitimate absolute path under `/c` and must not be rewritten.
    #[cfg(windows)]
    if normalized.len() >= 3 {
        let bytes = normalized.as_bytes();
        if bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b'/' {
            let drive = bytes[1] as char;
            let rest = &normalized[3..];
            return format!("{drive}:/{rest}");
        }
    }

    normalized.to_string()
}

/// Helper to check if a path starts with a base path, safely handling Windows UNC prefixes and case-insensitivity.
pub fn path_starts_with(path: &Path, base: &Path) -> bool {
    relative_path_under_base(path, base).is_some()
}

/// Strip Windows extended-length (`\\?\` / `//?/`) prefix for display, openers,
/// docker, and agent metadata.
///
/// Rust `Path::canonicalize()` on Windows yields `\\?\C:\...`. Call sites that
/// normalize separators to `/` turn that into `//?/C:/...`. Both forms break
/// `tauri_plugin_opener` / Explorer; strip them back to a normal drive path.
///
/// Leaves `\\?\UNC\...` and `//?/UNC/...` unchanged (network paths need the
/// verbatim form). Note: ShellExecute / `tauri_plugin_opener` may still reject
/// verbatim UNC; LibrAgent open paths are local drive workspaces today.
pub fn strip_windows_verbatim_prefix(path: &str) -> &str {
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        if rest.starts_with(r"UNC\") || rest.starts_with("UNC/") {
            return path;
        }
        return rest;
    }
    if let Some(rest) = path.strip_prefix("//?/") {
        if rest.starts_with("UNC/") || rest.starts_with(r"UNC\") {
            return path;
        }
        return rest;
    }
    path
}

/// Human-facing workspace path: strip Windows verbatim prefix, keep the rest.
pub fn display_workspace_path(path: &Path) -> String {
    strip_windows_verbatim_prefix(&path.to_string_lossy()).to_string()
}

/// True when two paths refer to the same directory (Windows verbatim/case aware).
pub fn workspace_paths_equivalent(left: &Path, right: &Path) -> bool {
    matches!(
        relative_path_under_base(left, right),
        Some(rel) if rel.as_os_str().is_empty()
    )
}

/// Return the relative suffix of `path` under `base`, or `None` if `path` is not under `base`.
///
/// Handles Windows verbatim vs normal drive prefixes and case-insensitive component matching.
/// An exact match returns an empty `PathBuf` (callers may treat that as `"."`).
///
/// Returns `None` if the relative suffix contains `..` (parent-dir) components — a lexical
/// prefix match alone is not enough to consider the path "under" the base.
pub fn relative_path_under_base(path: &Path, base: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut path_comps = path.components();
        let mut base_comps = base.components();

        loop {
            match (path_comps.next(), base_comps.next()) {
                (Some(p), Some(b)) => {
                    if !components_equal_ignore_case(&p, &b) {
                        return None;
                    }
                }
                // Base exhausted: keep the already-fetched path component (do not drop it).
                (Some(p), None) => {
                    return clean_relative_components(std::iter::once(p).chain(path_comps));
                }
                (None, None) => return Some(PathBuf::new()),
                (None, Some(_)) => return None,
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let relative = path.strip_prefix(base).ok()?;
        clean_relative_components(relative.components())
    }
}

/// Build a relative path from components, skipping `.` and rejecting `..`.
fn clean_relative_components<'a, I>(components: I) -> Option<PathBuf>
where
    I: Iterator<Item = Component<'a>>,
{
    let mut relative = PathBuf::new();
    for component in components {
        match component {
            Component::CurDir => {}
            Component::Normal(name) => relative.push(name),
            Component::ParentDir => return None,
            // Prefix/RootDir must not appear in a relative suffix.
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    Some(relative)
}

#[cfg(target_os = "windows")]
#[allow(clippy::manual_ignore_case_cmp)]
fn components_equal_ignore_case(c1: &Component, c2: &Component) -> bool {
    use std::path::Prefix;

    match (c1, c2) {
        (Component::Prefix(p1), Component::Prefix(p2)) => match (p1.kind(), p2.kind()) {
            (Prefix::VerbatimDisk(d1), Prefix::VerbatimDisk(d2)) => {
                d1.to_ascii_uppercase() == d2.to_ascii_uppercase()
            }
            (Prefix::Disk(d1), Prefix::Disk(d2)) => {
                d1.to_ascii_uppercase() == d2.to_ascii_uppercase()
            }
            (Prefix::VerbatimDisk(d1), Prefix::Disk(d2)) => {
                d1.to_ascii_uppercase() == d2.to_ascii_uppercase()
            }
            (Prefix::Disk(d1), Prefix::VerbatimDisk(d2)) => {
                d1.to_ascii_uppercase() == d2.to_ascii_uppercase()
            }
            (kind1, kind2) => kind1 == kind2,
        },
        (Component::RootDir, Component::RootDir) => true,
        (Component::CurDir, Component::CurDir) => true,
        (Component::ParentDir, Component::ParentDir) => true,
        (Component::Normal(s1), Component::Normal(s2)) => {
            let s1_str = s1.to_string_lossy();
            let s2_str = s2.to_string_lossy();
            s1_str.eq_ignore_ascii_case(&s2_str)
        }
        _ => false,
    }
}
