use super::super::contracts::{
    DiscoveredSkillRoot, HookPreviewExecutionStatus, PreparedSkillImport, SKILL_FILE_NAME,
};
use super::super::plugin_manifest;
use super::super::scanning::{
    parse_skill_metadata, plugin_owner_marker_of_dir, skill_storage_directory_name,
};
use super::archive::commit_from_archive_root;
use super::plugin_install::{
    build_hook_preview_entries, build_mcp_preview_entries, install_plugin_source, plugin_source_id,
    try_load_plugin_hooks,
};
use super::user_install::{
    install_prepared_skills_to_directory, remove_skill_names_from_user_skills,
    write_skill_owner_marker,
};
use std::fs;
use std::path::Path;

pub(super) fn write_valid_skill(dir: &Path, name: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: test\n---\n"),
    )
    .unwrap();
}

// #2927 D2: a plugin install marks every skill directory it writes, the
// owning plugin (an update) may replace its own, and neither another
// plugin nor a user import may overwrite a marked skill.
#[test]
fn plugin_install_marks_skills_and_only_the_owner_may_replace() {
    let temp = tempfile::tempdir().unwrap();
    let staging = temp.path().join("staging");
    let staged = staging.join("alpha");
    write_valid_skill(&staged, "alpha");
    let prepared = PreparedSkillImport {
        _temp_dir: temp,
        scan_root: staging.clone(),
        source_key: "https://github.com/acme/plugin".to_string(),
        discovered_skills: vec![DiscoveredSkillRoot {
            metadata: parse_skill_metadata(&staged.join(SKILL_FILE_NAME)).unwrap(),
            root_dir: staged,
        }],
    };
    let target = tempfile::tempdir().unwrap();

    install_prepared_skills_to_directory(
        &prepared,
        target.path().to_path_buf(),
        false,
        Some("plugin-acme"),
    )
    .unwrap();
    // Skills install under their storage name (`<slug>-<digest>`), not
    // the bare skill name.
    let installed = target
        .path()
        .join(skill_storage_directory_name("alpha").unwrap());
    assert_eq!(
        plugin_owner_marker_of_dir(&installed).as_deref(),
        Some("plugin-acme"),
        "every skill the plugin installs carries its owner marker"
    );

    install_prepared_skills_to_directory(
        &prepared,
        target.path().to_path_buf(),
        true,
        Some("plugin-acme"),
    )
    .unwrap();
    assert_eq!(
        plugin_owner_marker_of_dir(&installed).as_deref(),
        Some("plugin-acme"),
        "an update replaces the plugin's own skill and re-marks it"
    );

    for intruder in [Some("plugin-other"), None] {
        let error = install_prepared_skills_to_directory(
            &prepared,
            target.path().to_path_buf(),
            true,
            intruder,
        )
        .unwrap_err();
        assert!(error.contains("owned by plugin plugin-acme"), "{error}");
    }
    assert_eq!(
        plugin_owner_marker_of_dir(&installed).as_deref(),
        Some("plugin-acme"),
        "rejected installs must leave the owned skill intact"
    );
}

// #2927 D2: uninstall removes the plugin's own skills (and pre-#2927
// markerless ones) but never a skill marked for another plugin.
#[test]
fn uninstall_removes_own_skills_but_not_other_owners() {
    let user = tempfile::tempdir().unwrap();

    let mine = user.path().join("mine");
    write_valid_skill(&mine, "mine");
    write_skill_owner_marker(&mine, "plugin-mine").unwrap();

    let theirs = user.path().join("theirs");
    write_valid_skill(&theirs, "theirs");
    write_skill_owner_marker(&theirs, "plugin-theirs").unwrap();

    let legacy = user.path().join("legacy");
    write_valid_skill(&legacy, "legacy");

    remove_skill_names_from_user_skills(
        user.path(),
        &[
            "mine".to_string(),
            "theirs".to_string(),
            "legacy".to_string(),
        ],
        "plugin-mine",
    )
    .unwrap();

    assert!(!mine.exists(), "the plugin's own skills go with it");
    assert!(
        theirs.exists(),
        "a skill another plugin owns must survive this uninstall"
    );
    assert!(
        !legacy.exists(),
        "a markerless skill is a pre-#2927 install and goes by name"
    );
}

// #2927: GitHub serves an API zipball as `<owner>-<repo>-<sha>`, so the
// installed commit is readable from the archive root without a git call.
#[test]
fn installed_commit_reads_zipball_root_name() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let dir = tempfile::tempdir().unwrap();
    let zipball = dir.path().join(format!("acme-myrepo-{sha}"));
    fs::create_dir_all(zipball.join("plugin")).unwrap();

    assert_eq!(
        commit_from_archive_root(&zipball).as_deref(),
        Some(sha),
        "the archive root is usually the plugin root itself"
    );
    assert_eq!(
        commit_from_archive_root(&zipball.join("plugin")).as_deref(),
        Some(sha),
        "a nested plugin root still sees the archive root"
    );
    // Branch archives carry no commit, and neither does a plain checkout.
    let branch = dir.path().join("myrepo-main");
    fs::create_dir_all(&branch).unwrap();
    assert_eq!(commit_from_archive_root(&branch), None);
    assert_eq!(commit_from_archive_root(dir.path()), None);
}

#[test]
fn skill_bundle_when_no_hooks_present() {
    let dir = tempfile::tempdir().unwrap();
    write_valid_skill(dir.path(), "alpha");
    let hooks = try_load_plugin_hooks(dir.path(), None).unwrap();
    assert!(hooks.is_none());
}

// #2924 (D3): install persists the declared MCP servers, and the
// install-time approval flag covers them alongside hooks.
#[test]
fn install_persists_mcp_snapshot_and_approval() {
    let dir = tempfile::tempdir().unwrap();
    write_valid_skill(dir.path(), "alpha");
    fs::write(
        dir.path().join(".mcp.json"),
        r#"{"mcpServers":{"fetch":{"command":"uvx","args":["srv"]}}}"#,
    )
    .unwrap();

    let plugins = tempfile::tempdir().unwrap();
    let source_key = "https://example.com/mcp-plugin";
    install_plugin_source(
        dir.path(),
        source_key,
        &["alpha".to_string()],
        true,
        plugins.path(),
    )
    .unwrap();
    let record = crate::services::hooks::registry::get_source_in(
        plugins.path(),
        &plugin_source_id(source_key),
    )
    .unwrap();
    assert_eq!(record.manifest.mcp_servers.len(), 1);
    assert_eq!(record.manifest.mcp_servers[0].name, "fetch");
    assert_eq!(record.manifest.mcp_servers[0].entry["command"], "uvx");
    assert!(record.manifest.mcp_approved);

    // Without approval the snapshot is still recorded (so the install is
    // inspectable) but the servers are never started.
    let plugins2 = tempfile::tempdir().unwrap();
    install_plugin_source(
        dir.path(),
        source_key,
        &["alpha".to_string()],
        false,
        plugins2.path(),
    )
    .unwrap();
    let record = crate::services::hooks::registry::get_source_in(
        plugins2.path(),
        &plugin_source_id(source_key),
    )
    .unwrap();
    assert_eq!(record.manifest.mcp_servers.len(), 1);
    assert!(!record.manifest.mcp_approved);
}

// #2924 (gatekeeper G1.3): the install preview shows the full command
// line and declared env/header NAMES — never their values.
#[test]
fn mcp_preview_shows_args_and_names_not_values() {
    use crate::services::hooks::registry::PluginMcpServerSnapshot;
    let servers = vec![
        PluginMcpServerSnapshot {
            name: "local".to_string(),
            entry: serde_json::json!({
                "command": "uvx",
                "args": ["mcp-fetch", "--max", "10", "--path", "/tmp/my dir"],
                "env": { "Z_TOKEN": "super-secret", "A_KEY": "another-secret" }
            }),
        },
        PluginMcpServerSnapshot {
            name: "remote".to_string(),
            entry: serde_json::json!({
                "url": "https://x.example/mcp",
                "headers": { "Authorization": "Bearer tok" }
            }),
        },
    ];
    let entries = build_mcp_preview_entries(&servers);
    assert_eq!(entries.len(), 2);

    let local = &entries[0];
    assert_eq!(local.transport, "stdio");
    assert_eq!(local.command.as_deref(), Some("uvx"));
    assert_eq!(
        local.args.as_deref(),
        Some("mcp-fetch --max 10 --path \"/tmp/my dir\""),
        "arguments with whitespace are quoted (S2.2)"
    );
    assert_eq!(
        local.env_names.as_deref(),
        Some(&["A_KEY".to_string(), "Z_TOKEN".to_string()][..]),
        "names only, sorted"
    );
    assert!(local.url.is_none());

    let remote = &entries[1];
    assert_eq!(remote.transport, "http");
    assert_eq!(remote.url.as_deref(), Some("https://x.example/mcp"));
    assert_eq!(
        remote.header_names.as_deref(),
        Some(&["Authorization".to_string()][..])
    );
    assert!(remote.args.is_none());

    // The preview struct itself never carries the declared values.
    let serialized = serde_json::to_string(&entries).unwrap();
    assert!(!serialized.contains("super-secret"), "{serialized}");
    assert!(!serialized.contains("another-secret"), "{serialized}");
    assert!(!serialized.contains("Bearer tok"), "{serialized}");
}

#[test]
fn plugin_preview_when_hooks_valid() {
    let dir = tempfile::tempdir().unwrap();
    write_valid_skill(dir.path(), "alpha");
    let hooks_dir = dir.path().join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(
        hooks_dir.join("hooks.json"),
        r#"{"hooks":{"SessionStart":[{"command":"echo hi"}]}}"#,
    )
    .unwrap();
    let cfg = try_load_plugin_hooks(dir.path(), None)
        .unwrap()
        .expect("hooks");
    let entries = build_hook_preview_entries(&cfg);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].execution_status,
        HookPreviewExecutionStatus::Executed
    );
}

/// #3009: `http` handlers install and preview as executed (their POST
/// target is shown); other handler types preview as not supported
/// instead of failing the load.
#[test]
fn plugin_preview_shows_http_executed_and_other_types_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    write_valid_skill(dir.path(), "alpha");
    let hooks_dir = dir.path().join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(
        hooks_dir.join("hooks.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"workspace__runShell","hooks":[
            {"type":"http","url":"http://127.0.0.1:9/hook","headers":{"Authorization":"Bearer $API_KEY"},"allowedEnvVars":["API_KEY"]},
            {"type":"prompt","prompt":"check"}
        ]}]}}"#,
    )
    .unwrap();
    let cfg = try_load_plugin_hooks(dir.path(), None)
        .unwrap()
        .expect("hooks");
    let entries = build_hook_preview_entries(&cfg);
    assert_eq!(entries.len(), 2);
    let http = entries
        .iter()
        .find(|e| e.command.contains("127.0.0.1:9"))
        .expect("http entry shows its url");
    assert_eq!(http.execution_status, HookPreviewExecutionStatus::Executed);
    // #3041 G1.1: the preview must show the declared header names with
    // their raw templates and the env-var names the plugin may read —
    // the allowlist is written by the plugin itself, so the user has to
    // see it before approving.
    assert_eq!(
        http.http_headers
            .as_ref()
            .and_then(|h| h.get("Authorization"))
            .map(String::as_str),
        Some("Bearer $API_KEY"),
        "header name with its raw $VAR template must be visible"
    );
    assert_eq!(
        http.http_allowed_env_vars.as_deref(),
        Some(&["API_KEY".to_string()][..]),
        "allowed env-var names must be visible"
    );
    let prompt = entries
        .iter()
        .find(|e| e.command.contains("prompt"))
        .expect("prompt entry is recorded");
    assert_eq!(prompt.execution_status, HookPreviewExecutionStatus::Stubbed);
}

/// #3005 gatekeeper G1.2: exec-form `args` can carry `-c`/`-e` and
/// arbitrary code while `command` is just `node` or `bash`, so the
/// preview entry must carry `args` through to the approval dialog.
#[test]
fn preview_carries_exec_form_args() {
    let dir = tempfile::tempdir().unwrap();
    write_valid_skill(dir.path(), "alpha");
    let hooks_dir = dir.path().join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(
        hooks_dir.join("hooks.json"),
        r#"{"hooks":{"SessionStart":[{"hooks":[
            {"type":"command","command":"node","args":["-e","process.exit(0)"]}
        ]}]}}"#,
    )
    .unwrap();
    let cfg = try_load_plugin_hooks(dir.path(), None)
        .unwrap()
        .expect("hooks");
    let entries = build_hook_preview_entries(&cfg);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].command, "node");
    assert_eq!(
        entries[0].args.as_deref(),
        Some(["-e".to_string(), "process.exit(0)".to_string()].as_slice()),
        "the approval dialog must show everything the exec-form handler runs"
    );
    let serialized = serde_json::to_string(&entries[0]).unwrap();
    assert!(serialized.contains("process.exit(0)"), "{serialized}");
}

#[test]
fn invalid_hooks_fail_preview_load() {
    let dir = tempfile::tempdir().unwrap();
    write_valid_skill(dir.path(), "alpha");
    let hooks_dir = dir.path().join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(
        hooks_dir.join("hooks.json"),
        r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"x","timeout":0}]}]}}"#,
    )
    .unwrap();
    let err = try_load_plugin_hooks(dir.path(), None).unwrap_err();
    assert!(err.contains("timeout"));
}

pub(super) fn write_manifest(root: &Path, json: &str) {
    let dir = root.join(".claude-plugin");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("plugin.json"), json).unwrap();
}

#[test]
fn manifest_hooks_paths_add_to_the_default_discovery() {
    let dir = tempfile::tempdir().unwrap();
    write_valid_skill(dir.path(), "alpha");
    let hooks_dir = dir.path().join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(
        hooks_dir.join("hooks.json"),
        r#"{"hooks":{"SessionStart":[{"command":"echo default"}]}}"#,
    )
    .unwrap();
    fs::write(
        hooks_dir.join("custom.json"),
        r#"{"hooks":{"SessionStart":[{"command":"echo custom"}]}}"#,
    )
    .unwrap();
    write_manifest(
        dir.path(),
        r#"{ "name": "p", "hooks": "./hooks/custom.json" }"#,
    );
    let manifest = plugin_manifest::load_plugin_manifest(dir.path())
        .unwrap()
        .expect("manifest");
    let cfg = try_load_plugin_hooks(dir.path(), Some(&manifest))
        .unwrap()
        .expect("hooks");
    let entries = build_hook_preview_entries(&cfg);
    assert_eq!(entries.len(), 2, "manifest hooks add to the default");
    let commands: Vec<&str> = entries.iter().map(|e| e.command.as_str()).collect();
    assert_eq!(commands, ["echo custom", "echo default"]);
}

#[test]
fn manifest_hooks_path_duplicating_the_default_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    write_valid_skill(dir.path(), "alpha");
    let hooks_dir = dir.path().join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(
        hooks_dir.join("hooks.json"),
        r#"{"hooks":{"SessionStart":[{"command":"echo default"}]}}"#,
    )
    .unwrap();
    write_manifest(
        dir.path(),
        r#"{ "name": "p", "hooks": "./hooks/hooks.json" }"#,
    );
    let manifest = plugin_manifest::load_plugin_manifest(dir.path())
        .unwrap()
        .expect("manifest");
    let err = try_load_plugin_hooks(dir.path(), Some(&manifest)).unwrap_err();
    assert!(err.contains("duplicates"), "error explains the rule: {err}");
}

#[test]
fn manifest_hooks_path_missing_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    write_valid_skill(dir.path(), "alpha");
    write_manifest(
        dir.path(),
        r#"{ "name": "p", "hooks": "./hooks/missing.json" }"#,
    );
    let manifest = plugin_manifest::load_plugin_manifest(dir.path())
        .unwrap()
        .expect("manifest");
    let err = try_load_plugin_hooks(dir.path(), Some(&manifest)).unwrap_err();
    assert!(err.contains("missing.json"), "error names the path: {err}");
}
