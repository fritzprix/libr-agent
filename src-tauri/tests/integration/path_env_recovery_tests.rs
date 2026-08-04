use tauri_mcp_agent_lib::utils::env::{get_effective_path, get_isolated_env};

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

#[cfg(windows)]
#[test]
fn test_effective_path_includes_discovered_python_scripts_when_present() {
    use tauri_mcp_agent_lib::utils::windows_path_discovery::find_python_install_root;

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

#[cfg(windows)]
#[test]
fn test_effective_path_includes_cargo_bin_when_present() {
    use std::path::PathBuf;

    let Ok(profile) = std::env::var("USERPROFILE") else {
        return;
    };

    let cargo_bin = PathBuf::from(profile).join(".cargo").join("bin");
    if !cargo_bin.is_dir() {
        return;
    }

    let effective_path = get_effective_path();
    let cargo_bin_str = cargo_bin.to_string_lossy();
    assert!(
        effective_path.contains(cargo_bin_str.as_ref())
            || effective_path.contains(&cargo_bin_str.replace('/', "\\")),
        "effective PATH should include %USERPROFILE%\\.cargo\\bin when present; got: {effective_path}"
    );

    let isolated_path = get_isolated_env()
        .into_iter()
        .find_map(|(key, value)| (key.eq_ignore_ascii_case("PATH")).then_some(value))
        .expect("isolated env should always include PATH");
    assert!(
        isolated_path.contains(cargo_bin_str.as_ref())
            || isolated_path.contains(&cargo_bin_str.replace('/', "\\")),
        "isolated PATH must preserve discovered Cargo bin dir"
    );
}

#[cfg(windows)]
#[test]
fn test_collect_windows_user_tool_dirs_includes_cargo_and_cli_managers() {
    use std::path::PathBuf;
    use tauri_mcp_agent_lib::utils::windows_path_discovery::collect_windows_user_tool_dirs;

    let root = std::env::temp_dir().join("libragent_user_tool_dirs_itest");
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
