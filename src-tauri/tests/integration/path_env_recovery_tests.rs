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
