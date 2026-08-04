//! Windows PATH recovery for isolated processes.
//!
//! Kept as a standalone test binary: `tests/integration_tests.rs` is
//! `#![cfg(not(windows))]` due to WebView link crashes on Windows.

#![cfg(windows)]

use std::ffi::{OsStr, OsString};
use tauri_mcp_agent_lib::utils::env::{
    compose_windows_effective_path, get_effective_path, get_isolated_env,
};
use tauri_mcp_agent_lib::utils::windows_path_discovery::find_python_install_root;
use tauri_mcp_agent_lib::utils::windows_registry_path::get_windows_registry_path_os;

#[test]
fn test_effective_path_is_not_empty() {
    let effective_path = get_effective_path();
    assert!(
        !effective_path.trim().is_empty(),
        "effective PATH should never be empty"
    );
}

#[test]
fn test_isolated_env_uses_effective_path() {
    let effective_path = get_effective_path();
    let isolated_path = get_isolated_env()
        .into_iter()
        .find_map(|(key, value)| (key == "PATH").then_some(value))
        .expect("isolated env should always include PATH");

    assert_eq!(isolated_path, effective_path);
}

#[test]
fn test_effective_path_includes_discovered_python_scripts_when_present() {
    let Some(python_root) = find_python_install_root() else {
        return;
    };

    let scripts_dir = python_root.join("Scripts");
    if !scripts_dir.is_dir() {
        return;
    }

    let effective_path = get_effective_path();
    assert!(
        effective_path.contains(&scripts_dir.to_string_lossy().replace('/', "\\"))
            || effective_path.contains(scripts_dir.to_string_lossy().as_ref()),
        "effective PATH should prepend discovered Python Scripts directory"
    );
}

#[test]
fn test_registry_path_is_readable_and_non_empty() {
    let registry =
        get_windows_registry_path_os().expect("Windows registry Path should be readable");
    assert!(
        !registry.is_empty(),
        "registry Machine+User Path should not be empty"
    );
}

#[test]
fn test_compose_recovers_registry_dirs_missing_from_stripped_process_path() {
    let registry =
        get_windows_registry_path_os().expect("Windows registry Path should be readable");

    // Simulate Explorer/GUI-launched PATH that omits user tool directories.
    let stripped = OsString::from(r"C:\Windows\System32;C:\Windows");
    let effective = compose_windows_effective_path(None, Some(registry.clone()), Some(stripped));

    let effective_dirs: Vec<_> = std::env::split_paths(&effective).collect();
    for dir in std::env::split_paths(&registry) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        assert!(
            effective_dirs.iter().any(|existing| existing == &dir),
            "stripped process PATH recovery should keep registry entry: {}",
            dir.display()
        );
    }
}

#[test]
fn test_compose_keeps_discovered_dirs_ahead_of_registry() {
    let discovered = OsString::from(r"C:\Discovered\Python\Scripts");
    let registry = OsString::from(r"C:\Windows\System32;C:\Users\test\.cargo\bin");
    let current = OsString::from(r"C:\Windows");

    let effective = compose_windows_effective_path(Some(discovered), Some(registry), Some(current));
    let parts: Vec<_> = std::env::split_paths(&effective)
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    assert_eq!(
        parts.first().map(String::as_str),
        Some(r"C:\Discovered\Python\Scripts")
    );
    assert!(parts.iter().any(|p| p == r"C:\Users\test\.cargo\bin"));
    assert!(parts.iter().any(|p| p == r"C:\Windows\System32"));
}

#[test]
fn test_effective_path_includes_cargo_bin_when_present_on_user_path() {
    let Some(home) = std::env::var_os("USERPROFILE") else {
        return;
    };
    let cargo_bin = std::path::PathBuf::from(home).join(".cargo").join("bin");
    if !cargo_bin.is_dir() {
        return;
    }

    let registry = match get_windows_registry_path_os() {
        Some(path) => path,
        None => return,
    };

    let cargo_in_registry = std::env::split_paths(&registry).any(|dir| dir == cargo_bin);
    if !cargo_in_registry {
        // Installed but not on User PATH — registry recovery cannot invent it.
        return;
    }

    let effective = get_effective_path();
    let cargo_text = cargo_bin.to_string_lossy();
    assert!(
        effective
            .to_ascii_lowercase()
            .contains(&cargo_text.to_ascii_lowercase().replace('/', "\\"))
            || effective
                .to_ascii_lowercase()
                .contains(&cargo_text.to_ascii_lowercase()),
        "effective PATH should include registry .cargo\\bin when present: {cargo_text}"
    );
}

#[test]
fn test_effective_path_includes_discovered_cargo_bin_when_present() {
    let Some(home) = std::env::var_os("USERPROFILE") else {
        return;
    };
    let cargo_bin = std::path::PathBuf::from(home).join(".cargo").join("bin");
    if !cargo_bin.is_dir() {
        return;
    }

    let effective = get_effective_path();
    let cargo_text = cargo_bin.to_string_lossy();
    assert!(
        effective.contains(cargo_text.as_ref())
            || effective.contains(&cargo_text.replace('/', "\\")),
        "discovered PATH should include %USERPROFILE%\\.cargo\\bin when present; got: {effective}"
    );

    let isolated_path = get_isolated_env()
        .into_iter()
        .find_map(|(key, value)| (key.eq_ignore_ascii_case("PATH")).then_some(value))
        .expect("isolated env should always include PATH");
    assert!(
        isolated_path.contains(cargo_text.as_ref())
            || isolated_path.contains(&cargo_text.replace('/', "\\")),
        "isolated PATH must preserve discovered Cargo bin dir"
    );
}

#[test]
fn test_collect_windows_user_tool_dirs_includes_cargo_and_cli_managers() {
    use std::path::PathBuf;
    use tauri_mcp_agent_lib::utils::windows_path_discovery::collect_windows_user_tool_dirs;

    let root = std::env::temp_dir().join("libragent_user_tool_dirs_win_test");
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

    let missing = PathBuf::from(&profile).join(".fnm");
    assert!(
        found.iter().all(|p| p != &missing),
        "non-existent dirs must not be added"
    );

    let empty = collect_windows_user_tool_dirs(None, None, None, None);
    assert!(empty.is_empty());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_compose_prefers_registry_over_empty_process_path() {
    let registry = OsString::from(r"C:\Users\test\.cargo\bin");
    let effective = compose_windows_effective_path(None, Some(registry), None);
    let parts: Vec<_> = std::env::split_paths(&effective).collect();
    assert_eq!(parts.len(), 1);
    assert_eq!(
        parts[0].as_os_str(),
        OsStr::new(r"C:\Users\test\.cargo\bin")
    );
}
