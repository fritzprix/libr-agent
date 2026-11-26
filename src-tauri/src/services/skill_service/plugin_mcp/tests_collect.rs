use super::*;
use crate::services::skill_service::plugin_manifest::load_plugin_manifest;
use std::path::Path;

use std::fs;

pub(super) fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn default_file_wrapper_and_bare_map_parse() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".mcp.json",
        r#"{"mcpServers": {"fetch": {"command": "uvx", "args": ["mcp-fetch"]}}}"#,
    );
    let servers = collect_plugin_mcp_servers(dir.path(), None).unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "fetch");
    assert_eq!(servers[0].entry["command"], "uvx");

    let dir2 = tempfile::tempdir().unwrap();
    write(
        dir2.path(),
        ".mcp.json",
        r#"{"bare": {"url": "https://example.com/mcp"}}"#,
    );
    let servers = collect_plugin_mcp_servers(dir2.path(), None).unwrap();
    assert_eq!(servers[0].name, "bare");
    assert_eq!(servers[0].entry["url"], "https://example.com/mcp");
}

#[test]
fn manifest_paths_replace_default_by_name() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".mcp.json",
        r#"{"mcpServers": {
             "keep": {"command": "keep-cmd"},
             "dup": {"command": "old-cmd"}
           }}"#,
    );
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name": "p", "mcpServers": "./extra.json"}"#,
    );
    write(
        dir.path(),
        "extra.json",
        r#"{"mcpServers": {"dup": {"command": "new-cmd"}, "added": {"url": "https://x/mcp"}}}"#,
    );
    let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
    let servers = collect_plugin_mcp_servers(dir.path(), Some(&manifest)).unwrap();
    let names: Vec<&str> = servers.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["keep", "dup", "added"]);
    assert_eq!(servers[1].entry["command"], "new-cmd");
}

#[test]
fn manifest_inline_map_supported() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name": "p", "mcpServers": {"inlined": {"command": "run", "args": ["--stdio"]}}}"#,
    );
    let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
    let servers = collect_plugin_mcp_servers(dir.path(), Some(&manifest)).unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "inlined");
    assert_eq!(servers[0].entry["args"][0], "--stdio");
}

#[test]
fn missing_manifest_path_and_bad_shape_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name": "p", "mcpServers": "./nope.json"}"#,
    );
    let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
    let error = collect_plugin_mcp_servers(dir.path(), Some(&manifest)).unwrap_err();
    assert!(error.contains("nope.json"), "{error}");

    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), ".mcp.json", r#"{"bad": {"noTransport": true}}"#);
    let error = collect_plugin_mcp_servers(dir.path(), None).unwrap_err();
    assert!(error.contains("bad"), "{error}");
    assert!(error.contains("command"), "{error}");
}

// #2924 (gatekeeper S1.1): symlinked MCP declaration files are refused —
// a symlink could read a file from outside the plugin root.
#[cfg(unix)]
#[test]
fn symlinked_declaration_files_are_refused() {
    use std::os::unix::fs::symlink;

    // `.mcp.json` itself is a symlink.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("outside.json"),
        r#"{"srv":{"command":"x"}}"#,
    )
    .unwrap();
    symlink(
        dir.path().join("outside.json"),
        dir.path().join(".mcp.json"),
    )
    .unwrap();
    let error = collect_plugin_mcp_servers(dir.path(), None).unwrap_err();
    assert!(error.contains("symlink"), "{error}");

    // A manifest `mcpServers` path resolves to a symlink.
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"p","mcpServers":"./linked.json"}"#,
    );
    fs::write(
        dir.path().join("outside.json"),
        r#"{"srv":{"command":"x"}}"#,
    )
    .unwrap();
    symlink(
        dir.path().join("outside.json"),
        dir.path().join("linked.json"),
    )
    .unwrap();
    let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
    let error = collect_plugin_mcp_servers(dir.path(), Some(&manifest)).unwrap_err();
    assert!(error.contains("symlink"), "{error}");
}

// #2921 hostile-path rule also guards manifest `mcpServers` paths.
#[test]
fn manifest_mcp_servers_path_traversal_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"p","mcpServers":"./../evil.json"}"#,
    );
    let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
    let error = collect_plugin_mcp_servers(dir.path(), Some(&manifest)).unwrap_err();
    assert!(error.contains("mcpServers"), "{error}");
}

#[test]
fn plugin_root_placeholders_resolve_everywhere() {
    let dir = tempfile::tempdir().unwrap();
    let entry = serde_json::json!({
        "command": "${CLAUDE_PLUGIN_ROOT}/bin/run",
        "args": ["--root", "${LIBRAGENT_PLUGIN_ROOT}/cfg"],
        "env": {"ROOT": "$CURSOR_PLUGIN_ROOT/data"}
    });
    let resolved = resolve_plugin_root(&entry, dir.path());
    let root = dir.path().to_string_lossy();
    assert_eq!(resolved["command"], format!("{root}/bin/run"));
    assert_eq!(resolved["args"][1], format!("{root}/cfg"));
    assert_eq!(resolved["env"]["ROOT"], format!("{root}/data"));
}

#[test]
fn entry_converts_to_stored_config_shape() {
    let entry = serde_json::json!({
        "command": "node",
        "args": ["/server/index.js"],
        "env": {"A": "1"}
    });
    let config = entry_to_mcp_config_json("srv", &entry, "plugin-0123456789abcdef").unwrap();
    assert_eq!(config["transport"]["type"], "stdio");
    assert_eq!(config["transport"]["command"], "node");
    assert_eq!(config["transport"]["env"]["A"], "1");
    assert_eq!(config["pluginSource"], "plugin:plugin-0123456789abcdef");

    let http_entry = serde_json::json!({"url": "https://mcp.example.com", "headers": {"k": "v"}});
    let config = entry_to_mcp_config_json("remote", &http_entry, "plugin-x").unwrap();
    assert_eq!(config["transport"]["type"], "http");
    assert_eq!(config["transport"]["url"], "https://mcp.example.com");
    assert_eq!(config["transport"]["headers"]["k"], "v");

    // The produced JSON parses back as a valid MCPServerConfig.
    let parsed: crate::mcp::types::MCPServerConfig =
        serde_json::from_value(config).expect("valid MCPServerConfig");
    match parsed.transport {
        crate::mcp::types::TransportConfig::Http { url, .. } => {
            assert_eq!(url, "https://mcp.example.com")
        }
        _ => panic!("expected http"),
    }
}
