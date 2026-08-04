use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Cached PATH prefix discovered from common user/dev tool install locations.
/// GUI-launched Windows apps often inherit a stripped PATH that omits user-local
/// directories for Python/pip/pipx, Cargo/Rust, Node package managers, and similar CLIs.
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

    let user_profile = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    let localappdata = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);

    if let Some(ref appdata) = appdata {
        collect_versioned_scripts_dirs(&appdata.join("Python"), &mut paths);
    }

    if let Some(ref localappdata) = localappdata {
        collect_versioned_scripts_dirs(&localappdata.join("Programs").join("Python"), &mut paths);
    }

    for dir in collect_windows_user_tool_dirs(
        user_profile.as_deref(),
        appdata.as_deref(),
        localappdata.as_deref(),
        program_files.as_deref(),
    ) {
        push_unique_dir(&mut paths, &dir);
    }

    paths
}

/// Well-known Windows user/dev CLI install dirs (Cargo, Node managers, Scoop, etc.).
/// Only existing directories are returned so isolated PATH stays lean.
pub fn collect_windows_user_tool_dirs(
    user_profile: Option<&Path>,
    appdata: Option<&Path>,
    localappdata: Option<&Path>,
    program_files: Option<&Path>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(profile) = user_profile {
        candidates.push(profile.join(".local").join("bin"));
        // rustup / cargo / many rustup-installed CLIs (often also hosts `uv`)
        candidates.push(profile.join(".cargo").join("bin"));
        candidates.push(profile.join(".bun").join("bin"));
        candidates.push(profile.join(".volta").join("bin"));
        candidates.push(profile.join("scoop").join("shims"));
        // fnm default root may hold current Node shims depending on install mode
        candidates.push(profile.join(".fnm"));
    }

    if let Some(appdata) = appdata {
        candidates.push(appdata.join("npm"));
        candidates.push(appdata.join("pnpm"));
        candidates.push(appdata.join("fnm"));
    }

    if let Some(localappdata) = localappdata {
        // Default PNPM_HOME on Windows
        candidates.push(localappdata.join("pnpm"));
        candidates.push(localappdata.join("Programs").join("nodejs"));
        candidates.push(localappdata.join("fnm"));
    }

    if let Some(program_files) = program_files {
        candidates.push(program_files.join("nodejs"));
    }

    let mut paths = Vec::new();
    for candidate in candidates {
        push_unique_dir(&mut paths, &candidate);
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

    #[test]
    fn test_collect_windows_user_tool_dirs_includes_cargo_and_cli_managers() {
        let root = std::env::temp_dir().join("libragent_user_tool_dirs_test");
        let _ = std::fs::remove_dir_all(&root);

        let profile = root.join("profile");
        let appdata = root.join("appdata");
        let localappdata = root.join("localappdata");
        let program_files = root.join("program_files");

        let expected = [
            profile.join(".cargo").join("bin"),
            profile.join(".bun").join("bin"),
            profile.join(".volta").join("bin"),
            profile.join("scoop").join("shims"),
            profile.join(".local").join("bin"),
            appdata.join("npm"),
            appdata.join("pnpm"),
            localappdata.join("pnpm"),
            localappdata.join("Programs").join("nodejs"),
            program_files.join("nodejs"),
        ];

        for dir in &expected {
            std::fs::create_dir_all(dir).expect("create tool dir");
        }

        // Missing dirs must be skipped
        let missing_fnm = profile.join(".fnm");
        assert!(!missing_fnm.exists());

        let found = collect_windows_user_tool_dirs(
            Some(&profile),
            Some(&appdata),
            Some(&localappdata),
            Some(&program_files),
        );

        for dir in &expected {
            assert!(
                found.iter().any(|p| p == dir),
                "expected {} in discovered tool dirs: {found:?}",
                dir.display()
            );
        }
        assert!(
            found.iter().all(|p| p != &missing_fnm),
            "non-existent dirs must not be added"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_collect_windows_user_tool_dirs_skips_missing_roots() {
        let found = collect_windows_user_tool_dirs(None, None, None, None);
        assert!(found.is_empty());
    }
}
