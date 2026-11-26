use super::super::contracts::{
    DiscoveredSkillRoot, PreparedSkillImport, SkillMetadata, SKILL_FILE_NAME,
};
use super::super::scanning::parse_skill_metadata;
use super::plugin_install::{install_plugin_source, namescope_plugin_skills, plugin_source_id};
use super::preview::build_import_preview;
use super::tests_hook_preview::{write_manifest, write_valid_skill};
use std::collections::HashSet;
use std::fs;

fn staged_skill(dir: &std::path::Path, name: &str) -> DiscoveredSkillRoot {
    let root_dir = dir.join(name);
    write_valid_skill(&root_dir, name);
    DiscoveredSkillRoot {
        metadata: SkillMetadata {
            name: name.to_string(),
            description: "test".to_string(),
            path: root_dir.join(SKILL_FILE_NAME).to_string_lossy().to_string(),
            alias_path: None,
            source: None,
            origin: None,
            plugin_owner: None,
        },
        root_dir,
    }
}

#[test]
fn namescope_plugin_skills_renames_only_collisions() {
    let temp = tempfile::tempdir().unwrap();
    let staging = temp.path().join("staging");
    fs::create_dir_all(&staging).unwrap();

    let mut prepared = PreparedSkillImport {
        _temp_dir: temp,
        scan_root: staging.clone(),
        source_key: "test://plugin".to_string(),
        discovered_skills: vec![
            staged_skill(&staging, "alpha"),
            staged_skill(&staging, "beta"),
        ],
    };
    let existing: HashSet<String> = ["ALPHA".to_string().to_lowercase()].into();

    namescope_plugin_skills(&mut prepared, "superpowers", &existing).unwrap();

    assert_eq!(
        prepared.discovered_skills[0].metadata.name,
        "superpowers:alpha"
    );
    assert_eq!(prepared.discovered_skills[1].metadata.name, "beta");
    // The rename must be on disk so the install carries it.
    let rewritten =
        parse_skill_metadata(&prepared.discovered_skills[0].root_dir.join(SKILL_FILE_NAME))
            .unwrap();
    assert_eq!(rewritten.name, "superpowers:alpha");
    assert_eq!(rewritten.description, "test");
}

#[test]
fn namescope_rewrite_preserves_crlf_and_ignores_nested_name() {
    let temp = tempfile::tempdir().unwrap();
    let staging = temp.path().join("staging");
    fs::create_dir_all(&staging).unwrap();
    let root_dir = staging.join("alpha");
    fs::create_dir_all(&root_dir).unwrap();
    let skill_file = root_dir.join(SKILL_FILE_NAME);
    fs::write(
        &skill_file,
        "---\r\nname: alpha\r\ndescription: test\r\nmetadata:\r\n  name: nested\r\n---\r\nbody\r\n",
    )
    .unwrap();

    let mut prepared = PreparedSkillImport {
        _temp_dir: temp,
        scan_root: staging.clone(),
        source_key: "test://plugin".to_string(),
        discovered_skills: vec![DiscoveredSkillRoot {
            metadata: SkillMetadata {
                name: "alpha".to_string(),
                description: "test".to_string(),
                path: skill_file.to_string_lossy().to_string(),
                alias_path: None,
                source: None,
                origin: None,
                plugin_owner: None,
            },
            root_dir,
        }],
    };
    let existing: HashSet<String> = ["alpha".to_string()].into();

    namescope_plugin_skills(&mut prepared, "p", &existing).unwrap();

    let raw = fs::read_to_string(&skill_file).unwrap();
    assert!(
        raw.contains("name: \"p:alpha\"\r\n"),
        "top-level name rewritten with CRLF kept: {raw:?}"
    );
    assert!(
        raw.contains("  name: nested"),
        "nested name must not be rewritten: {raw:?}"
    );
    assert!(raw.ends_with("body\r\n"), "trailing CRLF kept: {raw:?}");
    let rewritten = parse_skill_metadata(&skill_file).unwrap();
    assert_eq!(rewritten.name, "p:alpha");
}

fn staged_prepared(temp: tempfile::TempDir, skill_names: &[&str]) -> PreparedSkillImport {
    let scan_root = temp.path().join("staging");
    fs::create_dir_all(&scan_root).unwrap();
    let discovered_skills = skill_names
        .iter()
        .map(|name| staged_skill(&scan_root, name))
        .collect();
    PreparedSkillImport {
        _temp_dir: temp,
        scan_root,
        source_key: "test://plugin".to_string(),
        discovered_skills,
    }
}

// Gatekeeper G1.2: preview-level coverage through the real
// build_import_preview with injected directories.

#[tokio::test]
async fn preview_reinstall_same_source_keeps_overwrite_conflict() {
    let mut prepared = staged_prepared(tempfile::tempdir().unwrap(), &["alpha"]);
    write_manifest(&prepared.scan_root, r#"{ "name": "p" }"#);
    let target = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    // The same plugin's own skill is already installed in the target dir.
    write_valid_skill(&target.path().join("alpha"), "alpha");

    let preview = build_import_preview(
        &mut prepared,
        target.path(),
        "global",
        "user",
        system.path(),
        &["alpha".to_string()],
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        preview.discovered_skills[0].name, "alpha",
        "own previous install must not trigger the plugin: rename"
    );
    assert_eq!(preview.conflicts.len(), 1);
    assert_eq!(preview.conflicts[0].name, "alpha");
}

#[tokio::test]
async fn preview_bundled_collision_renames_without_conflict() {
    let mut prepared = staged_prepared(tempfile::tempdir().unwrap(), &["alpha"]);
    write_manifest(&prepared.scan_root, r#"{ "name": "p" }"#);
    let target = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    write_valid_skill(&system.path().join("alpha"), "alpha");

    let preview = build_import_preview(
        &mut prepared,
        target.path(),
        "global",
        "user",
        system.path(),
        &[],
        None,
    )
    .await
    .unwrap();

    assert_eq!(preview.discovered_skills[0].name, "p:alpha");
    assert!(
        preview.conflicts.is_empty(),
        "rename resolves the bundled collision: {:?}",
        preview.conflicts
    );
}

#[tokio::test]
async fn preview_reports_plugin_manifest_summary() {
    let mut prepared = staged_prepared(tempfile::tempdir().unwrap(), &["alpha"]);
    write_manifest(
        &prepared.scan_root,
        r#"{ "name": "p", "version": "1.2.3", "description": "A plugin" }"#,
    );
    let target = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();

    let preview = build_import_preview(
        &mut prepared,
        target.path(),
        "global",
        "user",
        system.path(),
        &[],
        None,
    )
    .await
    .unwrap();

    let summary = preview.plugin_manifest.expect("manifest summary");
    assert_eq!(summary.name, "p");
    assert_eq!(summary.version.as_deref(), Some("1.2.3"));
    assert_eq!(summary.description.as_deref(), Some("A plugin"));
}

#[tokio::test]
async fn preview_fails_on_manifest_path_outside_root() {
    let mut prepared = staged_prepared(tempfile::tempdir().unwrap(), &["alpha"]);
    write_manifest(
        &prepared.scan_root,
        r#"{ "name": "p", "skills": "../evil" }"#,
    );
    let target = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();

    let err = build_import_preview(
        &mut prepared,
        target.path(),
        "global",
        "user",
        system.path(),
        &[],
        None,
    )
    .await
    .unwrap_err();
    assert!(err.contains("skills"), "error names the field: {err}");
}

/// Gatekeeper G2.2: a real install of a plugin whose hooks are declared
/// only in `.claude-plugin/plugin.json` must persist those files in the
/// source manifest, so the record-aware loader (dispatch and overview)
/// sees exactly the config the user approved.
#[test]
fn install_persists_manifest_only_hook_files() {
    use crate::services::hooks::registry::{get_source_in, hook_config_for_record};
    use crate::services::hooks::schema::HookEvent;

    let temp = tempfile::tempdir().unwrap();
    let scan_root = temp.path().join("plugin");
    let plugins_dir = temp.path().join("store");
    fs::create_dir_all(scan_root.join("custom-hooks")).unwrap();
    fs::write(
        scan_root.join("custom-hooks").join("extra.json"),
        r#"{"hooks":{"SessionStart":[{"command":"echo manifest only"}]}}"#,
    )
    .unwrap();
    write_manifest(
        &scan_root,
        r#"{ "name": "p", "hooks": "./custom-hooks/extra.json" }"#,
    );

    install_plugin_source(
        &scan_root,
        "test://manifest-only-hooks",
        &[],
        true,
        &plugins_dir,
    )
    .unwrap();

    let record = get_source_in(
        &plugins_dir,
        &plugin_source_id("test://manifest-only-hooks"),
    )
    .unwrap();
    assert_eq!(
        record.manifest.hook_files,
        vec!["custom-hooks/extra.json".to_string()],
        "install persists the manifest hook file relative to the root"
    );
    assert!(record.manifest.hooks_approved);
    assert!(
        !record.manifest.approval_hashes.is_empty(),
        "manifest-only handlers get approval hashes"
    );

    let cfg = hook_config_for_record(&record).unwrap();
    assert!(cfg.events.contains_key(&HookEvent::SessionStart));
    assert_eq!(
        super::super::plugin_sources::hook_events_for_record(&record),
        vec!["SessionStart".to_string()],
        "overview lists manifest-declared events"
    );
}

// #2927 gatekeeper G1.2: a fresh install starts in the manifest's
// `defaultEnabled` state, and an update keeps the state the user chose
// instead of falling back to the manifest default.
#[test]
fn install_honours_default_enabled_and_keeps_the_user_choice() {
    use crate::services::hooks::registry::{
        get_source_in, list_approved_sources_in, set_source_enabled_in,
    };

    let temp = tempfile::tempdir().unwrap();
    let scan_root = temp.path().join("plugin");
    let plugins_dir = temp.path().join("store");
    fs::create_dir_all(&scan_root).unwrap();
    write_valid_skill(&scan_root, "alpha");
    write_manifest(
        &scan_root,
        r#"{ "name": "opt-in", "defaultEnabled": false }"#,
    );
    let source_id = plugin_source_id("test://opt-in");
    let skills = ["alpha".to_string()];

    // Fresh install: the manifest asks to start disabled, and the loaders
    // skip it at once.
    install_plugin_source(&scan_root, "test://opt-in", &skills, false, &plugins_dir).unwrap();
    assert!(
        !get_source_in(&plugins_dir, &source_id)
            .unwrap()
            .manifest
            .enabled,
        "defaultEnabled: false must install disabled"
    );
    assert!(list_approved_sources_in(&plugins_dir).is_empty());

    // The user turns it on; a later update of the same source keeps that.
    set_source_enabled_in(&plugins_dir, &source_id, true).unwrap();
    install_plugin_source(&scan_root, "test://opt-in", &skills, false, &plugins_dir).unwrap();
    assert!(
        get_source_in(&plugins_dir, &source_id)
            .unwrap()
            .manifest
            .enabled,
        "an update must keep the state the user chose"
    );

    // The same holds for a plugin the user turned off.
    set_source_enabled_in(&plugins_dir, &source_id, false).unwrap();
    install_plugin_source(&scan_root, "test://opt-in", &skills, false, &plugins_dir).unwrap();
    assert!(
        !get_source_in(&plugins_dir, &source_id)
            .unwrap()
            .manifest
            .enabled,
        "an update must not re-enable a plugin the user disabled"
    );
}

// #2927: `defaultEnabled: true` and an absent field both install enabled,
// so every plugin written before this spec keeps working.
#[test]
fn absent_default_enabled_installs_enabled() {
    use crate::services::hooks::registry::get_source_in;

    let temp = tempfile::tempdir().unwrap();
    let scan_root = temp.path().join("plugin");
    let plugins_dir = temp.path().join("store");
    fs::create_dir_all(&scan_root).unwrap();
    write_valid_skill(&scan_root, "alpha");
    write_manifest(&scan_root, r#"{ "name": "plain" }"#);

    install_plugin_source(&scan_root, "test://plain", &[], false, &plugins_dir).unwrap();
    assert!(
        get_source_in(&plugins_dir, &plugin_source_id("test://plain"))
            .unwrap()
            .manifest
            .enabled
    );
}
