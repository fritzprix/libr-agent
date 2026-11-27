//! Unit tests for plugin manifest discovery, path rules and option resolution.
//!
//! Split out of `plugin_manifest.rs` to keep every file in this module under
//! 600 lines (#3056).

use super::*;

fn write_manifest(root: &Path, json: &str) {
    let dir = root.join(PLUGIN_MANIFEST_DIR);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(PLUGIN_MANIFEST_FILE), json).unwrap();
}

#[test]
fn missing_manifest_is_none() {
    let dir = tempfile::tempdir().unwrap();
    assert!(load_plugin_manifest(dir.path()).unwrap().is_none());
}

#[test]
fn parses_all_documented_fields() {
    let dir = tempfile::tempdir().unwrap();
    write_manifest(
        dir.path(),
        r#"{
            "name": "superpowers",
            "version": "1.2.3",
            "description": "Skills for agents",
            "author": { "name": "Jesse Vincent", "email": "jesse@example.com" },
            "homepage": "https://example.com/home",
            "repository": "https://github.com/obra/superpowers",
            "license": "MIT",
            "keywords": ["skills", "agents"],
            "skills": "./skills/",
            "commands": ["./cmds/a.md", "./cmds/b.md"],
            "agents": "./agents/",
            "hooks": "./hooks/custom.json",
            "mcpServers": "./mcp.json"
        }"#,
    );
    let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
    assert_eq!(manifest.name, "superpowers");
    assert_eq!(manifest.version.as_deref(), Some("1.2.3"));
    assert_eq!(manifest.description.as_deref(), Some("Skills for agents"));
    assert_eq!(
        manifest.author.as_ref().unwrap().display_name(),
        "Jesse Vincent"
    );
    assert_eq!(
        manifest.homepage.as_deref(),
        Some("https://example.com/home")
    );
    assert_eq!(
        manifest.repository.as_deref(),
        Some("https://github.com/obra/superpowers")
    );
    assert_eq!(manifest.license.as_deref(), Some("MIT"));
    assert_eq!(
        manifest.keywords.as_deref().unwrap(),
        ["skills".to_string(), "agents".to_string()]
    );
    assert_eq!(
        manifest.skills,
        Some(ComponentPaths::Single("./skills/".into()))
    );
    assert_eq!(
        manifest.commands,
        Some(ComponentPaths::Many(vec![
            "./cmds/a.md".into(),
            "./cmds/b.md".into()
        ]))
    );
    assert_eq!(
        hook_config_paths(Some(&manifest)),
        Some(vec!["./hooks/custom.json".to_string()])
    );
    validate_manifest_paths(&manifest).unwrap();
}

#[test]
fn string_author_and_missing_optional_fields() {
    let dir = tempfile::tempdir().unwrap();
    write_manifest(dir.path(), r#"{ "name": "p", "author": "Jane Doe" }"#);
    let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
    assert_eq!(manifest.author.as_ref().unwrap().display_name(), "Jane Doe");
    assert!(manifest.version.is_none());
    assert!(hook_config_paths(Some(&manifest)).is_none());
    assert!(hook_inline_values(Some(&manifest)).is_empty());
    validate_manifest_paths(&manifest).unwrap();
}

/// `hooks` may be an inline object instead of a path (#3005).
#[test]
fn inline_hooks_object_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    write_manifest(
        dir.path(),
        r#"{
          "name": "p",
          "hooks": { "PreToolUse": [ { "matcher": "Bash", "command": "echo hi" } ] }
        }"#,
    );
    let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
    assert_eq!(hook_config_paths(Some(&manifest)), None);
    let inline = hook_inline_values(Some(&manifest));
    assert_eq!(inline.len(), 1);
    assert!(inline[0].get("PreToolUse").is_some());
    validate_manifest_paths(&manifest).unwrap();
}

/// A `hooks` array may mix file paths and inline objects (#3005).
#[test]
fn hooks_array_mixes_paths_and_inline_objects() {
    let dir = tempfile::tempdir().unwrap();
    write_manifest(
        dir.path(),
        r#"{
          "name": "p",
          "hooks": [
            "./hooks/custom.json",
            { "SessionEnd": [ { "command": "echo bye" } ] }
          ]
        }"#,
    );
    let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
    assert_eq!(
        hook_config_paths(Some(&manifest)),
        Some(vec!["./hooks/custom.json".to_string()])
    );
    let inline = hook_inline_values(Some(&manifest));
    assert_eq!(inline.len(), 1);
    assert!(inline[0].get("SessionEnd").is_some());
    validate_manifest_paths(&manifest).unwrap();
}

/// A path outside the plugin root is still rejected when it sits in a mixed
/// `hooks` array (#3005 keeps the #2921 path rule).
#[test]
fn manifest_hooks_path_outside_root_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    write_manifest(
        dir.path(),
        r#"{ "name": "p", "hooks": ["./hooks/a.json", "./../escape.json"] }"#,
    );
    let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
    let error = validate_manifest_paths(&manifest).unwrap_err();
    assert!(error.contains("inside the plugin root"), "{error}");
}

#[test]
fn invalid_json_and_missing_name_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    write_manifest(dir.path(), "{ not json");
    assert!(load_plugin_manifest(dir.path())
        .unwrap_err()
        .contains("parse"));

    let dir = tempfile::tempdir().unwrap();
    write_manifest(dir.path(), r#"{ "version": "1.0" }"#);
    assert!(load_plugin_manifest(dir.path())
        .unwrap_err()
        .contains("name"));
}

#[test]
fn unsafe_namespace_names_are_rejected() {
    for name in ["", "  ", "a:b", "a/b", "a\\b"] {
        let dir = tempfile::tempdir().unwrap();
        let manifest = format!("{{ \"name\": {name:?} }}");
        write_manifest(dir.path(), &manifest);
        assert!(
            load_plugin_manifest(dir.path()).is_err(),
            "name {name:?} should be rejected"
        );
    }
}

#[test]
fn component_path_rules() {
    assert!(validate_component_path("./hooks/hooks.json").is_ok());
    assert!(validate_component_path("./skills").is_ok());
    assert!(validate_component_path("./a/b/c.json").is_ok());
    assert!(validate_component_path("hooks/hooks.json").is_err());
    assert!(validate_component_path("./").is_err());
    assert!(validate_component_path("./../evil").is_err());
    assert!(validate_component_path("./a/../../evil").is_err());
    assert!(validate_component_path("/etc/passwd").is_err());
    assert!(validate_component_path("").is_err());
}

#[test]
fn manifest_path_outside_root_fails_validation() {
    let dir = tempfile::tempdir().unwrap();
    write_manifest(dir.path(), r#"{ "name": "p", "hooks": "../outside.json" }"#);
    let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
    let error = validate_manifest_paths(&manifest).unwrap_err();
    assert!(error.contains("hooks"), "error names the field: {error}");
    assert!(error.contains("./"), "error explains the rule: {error}");
}
