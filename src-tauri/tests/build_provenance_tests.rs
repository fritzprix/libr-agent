//! Build provenance is embedded at compile time (#2744).

#[test]
fn build_sha_is_non_empty() {
    assert!(
        !tauri_mcp_agent_lib::build_provenance::BUILD_SHA.is_empty(),
        "BUILD_SHA must be set at compile time"
    );
}

#[test]
fn build_dirty_is_a_bool_fn() {
    let _ = tauri_mcp_agent_lib::build_provenance::build_dirty();
}
