use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Cached PATH prefix discovered from common Python/pip/pipx install locations.
/// GUI-launched Windows apps often inherit a stripped PATH that omits user-local
/// tool directories where `jupyter`, `pip`, and pipx shims are installed.
pub fn get_windows_discovered_path_os() -> Option<OsString> {
    static DISCOVERED: OnceLock<Option<OsString>> = OnceLock::new();
    DISCOVERED
        .get_or_init(|| {
            let paths = discover_windows_tool_paths();
            if paths.is_empty() {
                None
            } else {
                std::env::join_paths(paths).ok()
            }
        })
        .clone()
}

/// Returns the root directory of the first valid non-Store Python installation found.
pub fn find_python_install_root() -> Option<PathBuf> {
    find_python_in_standard_locations().or_else(find_python_in_host_path)
}

fn find_python_in_standard_locations() -> Option<PathBuf> {
    for path in standard_python_roots().into_iter().flatten() {
        if let Some(root) = find_python_in_directory(&path) {
            return Some(root);
        }
    }

    None
}

/// Scans the host process PATH for a `python.exe` directory (e.g. `C:\Python312`).
/// Uses the raw inherited PATH, not the augmented effective PATH.
fn find_python_in_host_path() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| find_python_in_path_env(&path))
}

fn find_python_in_path_env(path: &OsStr) -> Option<PathBuf> {
    for dir in std::env::split_paths(path) {
        if is_windows_store_python_shim(&dir) {
            continue;
        }
        if dir.join("python.exe").exists() {
            return Some(dir);
        }
    }

    None
}

fn find_python_in_directory(base: &Path) -> Option<PathBuf> {
    if base.join("python.exe").exists() {
        return Some(base.to_path_buf());
    }

    if !base.is_dir() {
        return None;
    }

    let Ok(entries) = std::fs::read_dir(base) else {
        return None;
    };

    for entry in entries.flatten() {
        let subpath = entry.path();
        if subpath.join("python.exe").exists() {
            return Some(subpath);
        }
    }

    None
}

fn is_windows_store_python_shim(path: &Path) -> bool {
    path.to_string_lossy()
        .to_ascii_lowercase()
        .contains("windowsapps")
}

fn discover_windows_tool_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(python_root) = find_python_install_root() {
        push_unique_dir(&mut paths, &python_root);
        push_unique_dir(&mut paths, &python_root.join("Scripts"));
        push_unique_dir(&mut paths, &python_root.join("Library").join("bin"));
    }

    if let Ok(appdata) = std::env::var("APPDATA") {
        collect_versioned_scripts_dirs(&PathBuf::from(appdata).join("Python"), &mut paths);
    }

    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        collect_versioned_scripts_dirs(
            &PathBuf::from(localappdata).join("Programs").join("Python"),
            &mut paths,
        );
    }

    if let Ok(profile) = std::env::var("USERPROFILE") {
        push_unique_dir(
            &mut paths,
            &PathBuf::from(profile).join(".local").join("bin"),
        );
    }

    paths
}

fn collect_versioned_scripts_dirs(base: &Path, paths: &mut Vec<PathBuf>) {
    if !base.is_dir() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };

    for entry in entries.flatten() {
        push_unique_dir(paths, &entry.path().join("Scripts"));
    }
}

fn push_unique_dir(paths: &mut Vec<PathBuf>, path: &Path) {
    if path.is_dir() && !paths.iter().any(|existing| existing == path) {
        paths.push(path.to_path_buf());
    }
}

fn standard_python_roots() -> Vec<Option<PathBuf>> {
    vec![
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|path| PathBuf::from(path).join("Anaconda3")),
        std::env::var("ProgramData")
            .ok()
            .map(|path| PathBuf::from(path).join("Anaconda3")),
        std::env::var("USERPROFILE")
            .ok()
            .map(|path| PathBuf::from(path).join("anaconda3")),
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|path| PathBuf::from(path).join("Programs").join("Python")),
        std::env::var("ProgramFiles")
            .ok()
            .map(|path| PathBuf::from(path).join("Python")),
        std::env::var("ProgramFiles(x86)")
            .ok()
            .map(|path| PathBuf::from(path).join("Python")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_unique_dir_deduplicates() {
        let mut paths = Vec::new();
        let dir = std::env::temp_dir();

        push_unique_dir(&mut paths, &dir);
        push_unique_dir(&mut paths, &dir);

        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn test_discover_windows_tool_paths_is_idempotent_via_cache() {
        let first = get_windows_discovered_path_os();
        let second = get_windows_discovered_path_os();
        assert_eq!(first, second);
    }

    #[test]
    fn test_find_python_in_path_env_skips_windowsapps_shim() {
        let python_root = std::env::temp_dir().join("libragent_path_env_python");
        let _ = std::fs::remove_dir_all(&python_root);
        std::fs::create_dir_all(&python_root).expect("create temp python root");
        std::fs::write(python_root.join("python.exe"), b"").expect("create fake python.exe");

        let path = format!(
            "C:\\Users\\test\\AppData\\Local\\Microsoft\\WindowsApps;{}",
            python_root.display()
        );
        let found = find_python_in_path_env(OsStr::new(&path));
        assert_eq!(found, Some(python_root.clone()));

        let _ = std::fs::remove_dir_all(&python_root);
    }

    #[test]
    fn test_find_python_in_directory_checks_versioned_subdirs() {
        let base = std::env::temp_dir().join("libragent_python_discovery_test");
        let versioned = base.join("Python312");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(versioned.join("Scripts")).expect("create test python tree");

        let fake_python = versioned.join("python.exe");
        std::fs::write(&fake_python, b"").expect("create fake python.exe");

        let found = find_python_in_directory(&base);
        assert_eq!(found, Some(versioned));

        let _ = std::fs::remove_dir_all(&base);
    }
}
