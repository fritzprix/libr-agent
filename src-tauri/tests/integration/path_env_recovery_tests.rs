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
