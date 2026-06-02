use tauri_mcp_agent_lib::mcp::service_proxy_manager::{
    decide_existing_proxy_disposition, ExistingProxyDisposition,
};

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn create_proxy_recreates_lazy_builtin_only_proxy_when_external_servers_are_requested() {
    let disposition = decide_existing_proxy_disposition(
        &strings(&["attachments", "knowledge"]),
        &[],
        &strings(&["attachments", "knowledge"]),
        &strings(&["exa"]),
        false,
    );

    assert_eq!(
        disposition,
        ExistingProxyDisposition::Recreate,
        "builtin-only lazy proxies must be recreated when the requested session config adds external MCP servers"
    );
}

#[test]
fn create_proxy_reuses_existing_proxy_when_config_load_fails_but_builtin_set_matches() {
    let disposition = decide_existing_proxy_disposition(
        &strings(&["attachments", "knowledge"]),
        &strings(&["exa"]),
        &strings(&["knowledge", "attachments"]),
        &[],
        true,
    );

    assert_eq!(
        disposition,
        ExistingProxyDisposition::Reuse,
        "transient MCP config load failures must keep the existing proxy alive when builtin requirements are unchanged"
    );
}

#[test]
fn create_proxy_fails_when_config_load_fails_and_builtin_set_changes() {
    let disposition = decide_existing_proxy_disposition(
        &strings(&["attachments"]),
        &strings(&["exa"]),
        &strings(&["attachments", "planning"]),
        &[],
        true,
    );

    assert_eq!(
        disposition,
        ExistingProxyDisposition::Fail,
        "config load failures must not silently reuse a proxy with the wrong builtin tool set"
    );
}
