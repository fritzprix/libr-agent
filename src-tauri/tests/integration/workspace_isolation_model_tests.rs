use std::str::FromStr;

use tauri_mcp_agent_lib::models::workspace_isolation::{
    validate_env_key, validate_env_value, DockerWorkspaceConfig, WorkspaceIsolationMode,
};
use tauri_mcp_agent_lib::session_isolation::PathMappingLayer;

#[test]
fn workspace_isolation_mode_round_trips_lowercase_values() {
    assert_eq!(
        WorkspaceIsolationMode::from_str("host").expect("host should parse"),
        WorkspaceIsolationMode::Host
    );
    assert_eq!(
        WorkspaceIsolationMode::from_str("docker").expect("docker should parse"),
        WorkspaceIsolationMode::Docker
    );
    assert_eq!(WorkspaceIsolationMode::Docker.to_string(), "docker");
    assert!(WorkspaceIsolationMode::from_str("Docker").is_err());
}

#[test]
fn docker_workspace_config_defaults_missing_env_to_empty_map() {
    let config: DockerWorkspaceConfig = serde_json::from_value(serde_json::json!({
        "image": "ubuntu:24.04"
    }))
    .expect("config should deserialize");

    assert_eq!(config.image, "ubuntu:24.04");
    assert!(config.env.is_empty());
}

#[test]
fn docker_workspace_config_rejects_empty_image() {
    let config = DockerWorkspaceConfig {
        image: "   ".to_string(),
        env: Default::default(),
    };

    assert!(config.validate().is_err());
}

#[test]
fn docker_env_key_validation_accepts_shell_identifier_names() {
    for key in ["PATH", "_TOKEN", "LIBRAGENT_123"] {
        validate_env_key(key).expect("valid env key should pass");
    }
}

#[test]
fn docker_env_key_validation_rejects_flag_like_or_invalid_names() {
    for key in ["", "1TOKEN", "--rm", "BAD-NAME", "BAD.NAME"] {
        assert!(
            validate_env_key(key).is_err(),
            "invalid env key should fail: {key}"
        );
    }
}

#[test]
fn docker_env_value_validation_rejects_nul_bytes() {
    validate_env_value("TOKEN", "abc").expect("normal values should pass");
    assert!(validate_env_value("TOKEN", "abc\0def").is_err());
}

#[test]
fn path_mapping_maps_workspace_container_paths_to_host_workspace() {
    let mapper = PathMappingLayer::new("/tmp/libragent-host-workspace".into());

    assert_eq!(
        mapper.container_to_host("/workspace").as_deref(),
        Some(std::path::Path::new("/tmp/libragent-host-workspace"))
    );
    assert_eq!(
        mapper
            .container_to_host("/workspace/src/main.rs")
            .as_deref(),
        Some(std::path::Path::new(
            "/tmp/libragent-host-workspace/src/main.rs"
        ))
    );
}

#[test]
fn path_mapping_blocks_container_paths_outside_workspace() {
    let mapper = PathMappingLayer::new("/tmp/libragent-host-workspace".into());

    assert!(mapper.container_to_host("/tmp/file.txt").is_none());
    assert!(mapper
        .container_to_host("/workspace/../tmp/file.txt")
        .is_none());
}
