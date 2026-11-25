use super::discovery::discover_agent_files;
use super::ids::{
    is_plugin_agent_id, parse_plugin_agent_id, plugin_agent_id, reject_plugin_agent_mutation,
    PLUGIN_SOURCE_KEY,
};
use super::mapping::{map_plugin_tools, plugin_model_to_complexity};
use super::preview::{agent_config_json, plugin_agent_previews, synth_assistant_model};
use super::session::{
    declared_plugin_agent_model, infos_from_records, resolve_session_plugin_assistant_from_records,
};
use super::sync::{remove_global_plugin_agents, sync_global_plugin_agents};
use crate::services::hooks::registry::PluginSourceRecord;
use crate::services::skill_service::plugin_manifest::PLUGIN_MANIFEST_FILE;
use std::fs;
use std::path::Path;

fn write_agent(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, content).unwrap();
}

const REVIEWER: &str = "---\nname: reviewer\ndescription: Reviews code\nmodel: sonnet\ntools: Read, Grep, CustomLint\n---\nYou review code carefully.\n";

#[test]
fn discovers_agents_dir_with_frontmatter_and_body() {
    let dir = tempfile::tempdir().unwrap();
    write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
    let agents = discover_agent_files(dir.path(), None);
    assert_eq!(agents.len(), 1);
    let agent = &agents[0];
    assert_eq!(agent.name, "reviewer");
    assert_eq!(agent.description, "Reviews code");
    assert_eq!(agent.model.as_deref(), Some("sonnet"));
    assert_eq!(agent.tools, vec!["Read", "Grep", "CustomLint"]);
    assert_eq!(agent.system_prompt, "You review code carefully.");
}

#[test]
fn falls_back_to_file_stem_and_treats_inherit_as_none() {
    let dir = tempfile::tempdir().unwrap();
    write_agent(
        dir.path(),
        "agents/writer.md",
        "---\nmodel: inherit\n---\nWrite docs.\n",
    );
    let agents = discover_agent_files(dir.path(), None);
    assert_eq!(agents[0].name, "writer");
    assert_eq!(agents[0].model, None);
    assert!(agents[0].tools.is_empty());
}

#[test]
fn manifest_agents_paths_are_resolved() {
    let dir = tempfile::tempdir().unwrap();
    write_agent(
        dir.path(),
        &format!(
            "{}/{PLUGIN_MANIFEST_FILE}",
            super::super::plugin_manifest::PLUGIN_MANIFEST_DIR
        ),
        r#"{ "name": "p", "agents": ["./custom/sub.md", "./extra/"] }"#,
    );
    write_agent(
        dir.path(),
        "custom/sub.md",
        "---\ndescription: s\n---\nbody",
    );
    write_agent(dir.path(), "extra/one.md", "---\ndescription: e\n---\nbody");
    write_agent(dir.path(), "agents/ignored.md", "not loaded");
    let manifest = super::super::plugin_manifest::load_plugin_manifest(dir.path())
        .unwrap()
        .expect("manifest");
    let mut names: Vec<String> = discover_agent_files(dir.path(), Some(&manifest))
        .into_iter()
        .map(|a| a.name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["one", "sub"]);
}

#[test]
fn tool_mapping_maps_known_and_reports_unmapped() {
    let tools: Vec<String> = [
        "Read",
        "Bash",
        "WebSearch",
        "Task",
        "TodoWrite",
        "AskUserQuestion",
        "McpCustom",
    ]
    .iter()
    .map(|t| t.to_string())
    .collect();
    let (mapped, unmapped) = map_plugin_tools(&tools);
    assert_eq!(
        mapped,
        vec!["agent", "browser", "planning", "ui", "workspace"]
    );
    assert_eq!(unmapped, vec!["McpCustom"]);
}

#[test]
fn preview_lists_agents_their_granted_and_unmapped_tools() {
    let dir = tempfile::tempdir().unwrap();
    write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
    let previews = plugin_agent_previews(dir.path(), None);
    assert_eq!(previews.len(), 1);
    assert_eq!(previews[0].name, "reviewer");
    assert_eq!(previews[0].unmapped_tools, vec!["CustomLint"]);
    // G1.2: Read/Grep map to `workspace`; the user sees the grant too.
    assert_eq!(previews[0].granted_services, vec!["workspace"]);
    assert_eq!(previews[0].grants_all_services, false);
    assert_eq!(previews[0].model.as_deref(), Some("sonnet"));
}

#[test]
fn preview_without_tools_declares_all_services_granted() {
    // G1.2: an absent `tools` line grants everything; the preview must
    // say so instead of rendering an empty grant.
    let dir = tempfile::tempdir().unwrap();
    write_agent(
        dir.path(),
        "agents/plain.md",
        "---\ndescription: d\n---\nbody",
    );
    let previews = plugin_agent_previews(dir.path(), None);
    assert_eq!(previews[0].granted_services, Vec::<String>::new());
    assert_eq!(previews[0].grants_all_services, true);
    assert_eq!(previews[0].unmapped_tools, Vec::<String>::new());
}

#[test]
fn preview_of_read_only_tools_lists_workspace_grant() {
    // G1.2 test pair: `tools: Read` grants exactly `workspace`.
    let dir = tempfile::tempdir().unwrap();
    write_agent(
        dir.path(),
        "agents/reader.md",
        "---\ndescription: d\ntools: Read\n---\nbody",
    );
    let previews = plugin_agent_previews(dir.path(), None);
    assert_eq!(previews[0].granted_services, vec!["workspace"]);
    assert_eq!(previews[0].grants_all_services, false);
}

#[test]
fn manifest_paths_traversing_out_of_the_root_are_never_probed() {
    // S1.1: manifest `agents` paths follow the same component-path
    // rule as the other plugin loaders; `../` entries are skipped.
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write_agent(
        outside.path(),
        "outside/evil.md",
        "---\ndescription: e\n---\nsecret",
    );
    write_agent(
        dir.path(),
        &format!(
            "{}/{PLUGIN_MANIFEST_FILE}",
            super::super::plugin_manifest::PLUGIN_MANIFEST_DIR
        ),
        r#"{ "name": "p", "agents": ["./../outside"] }"#,
    );
    let manifest = super::super::plugin_manifest::load_plugin_manifest(dir.path())
        .unwrap()
        .expect("manifest");
    assert!(discover_agent_files(dir.path(), Some(&manifest)).is_empty());
}

#[test]
fn ids_roundtrip_and_reject_other_shapes() {
    let id = plugin_agent_id("plugin-0123456789abcdef", "reviewer");
    assert_eq!(id, "plugin:plugin-0123456789abcdef:reviewer");
    assert_eq!(
        parse_plugin_agent_id(&id),
        Some(("plugin-0123456789abcdef", "reviewer"))
    );
    assert!(is_plugin_agent_id(&id));
    assert!(!is_plugin_agent_id("b15fdf3e-6192-453d"));
    assert_eq!(parse_plugin_agent_id("plugin:only-source"), None);
    assert_eq!(parse_plugin_agent_id("plugin::agent"), None);
}

#[test]
fn mutation_guard_rejects_plugin_ids_and_spoofed_configs() {
    assert!(reject_plugin_agent_mutation(&plugin_agent_id("s", "a"), None).is_err());
    assert!(reject_plugin_agent_mutation("user-assistant", None).is_ok());
    let spoofed = serde_json::json!({ "pluginSource": "plugin:s" });
    assert!(reject_plugin_agent_mutation("user-assistant", Some(&spoofed)).is_err());
}

#[test]
fn synth_config_carries_ownership_prompt_and_aliases() {
    let dir = tempfile::tempdir().unwrap();
    write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
    let agent = discover_agent_files(dir.path(), None).remove(0);
    let config = agent_config_json(&agent, "plugin-aa", "myp", "global");
    assert_eq!(config["systemPrompt"], "You review code carefully.");
    assert_eq!(config[PLUGIN_SOURCE_KEY], "plugin:plugin-aa");
    assert_eq!(config["pluginSourceName"], "myp");
    assert_eq!(config["pluginScope"], "global");
    assert_eq!(config["deletionProtected"], true);
    assert_eq!(config["pluginAgentModel"], "sonnet");
    // Read/Grep map to workspace; CustomLint is never granted.
    assert_eq!(
        config["allowedBuiltInServiceAliases"],
        serde_json::json!(["workspace"])
    );

    let model = synth_assistant_model(&agent, "plugin-aa", "myp", "global");
    assert_eq!(model.id, "plugin:plugin-aa:reviewer");
    assert_eq!(model.name, "reviewer");
}

#[test]
fn no_tools_frontmatter_keeps_default_services() {
    let dir = tempfile::tempdir().unwrap();
    write_agent(
        dir.path(),
        "agents/plain.md",
        "---\ndescription: d\n---\nbody",
    );
    let agent = discover_agent_files(dir.path(), None).remove(0);
    let config = agent_config_json(&agent, "s", "p", "global");
    assert!(config.get("allowedBuiltInServiceAliases").is_none());
}

#[test]
fn model_tiers_map_only_known_names() {
    assert_eq!(plugin_model_to_complexity("Haiku"), Some("low"));
    assert_eq!(plugin_model_to_complexity("sonnet"), Some("normal"));
    assert_eq!(plugin_model_to_complexity("opus"), Some("high"));
    assert_eq!(plugin_model_to_complexity("gpt-5"), None);
}

#[cfg(unix)]
#[test]
fn symlinked_agent_outside_root_is_skipped() {
    // `std::os::unix::fs::symlink` does not compile on Windows, so the
    // whole test is unix-only (same as the plugin_commands symlink
    // discovery test).
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write_agent(
        outside.path(),
        "evil.md",
        "---\ndescription: e\n---\nsecret",
    );
    let link = dir.path().join("agents");
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    assert!(discover_agent_files(dir.path(), None).is_empty());
}

#[test]
fn session_resolution_finds_agent_by_name_and_synthesizes() {
    let dir = tempfile::tempdir().unwrap();
    write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
    let record = record_for(dir.path(), "plugin-aa", "https://github.com/acme/p");
    let model = resolve_session_plugin_assistant_from_records(&[record], "plugin-aa", "Reviewer")
        .expect("agent resolves case-insensitively");
    assert_eq!(model.id, "plugin:plugin-aa:reviewer");
    assert_eq!(model.config.contains("\"pluginScope\":\"session\""), true);

    assert!(resolve_session_plugin_assistant_from_records(&[], "plugin-aa", "reviewer").is_none());
    assert!(resolve_session_plugin_assistant_from_records(
        std::slice::from_ref(&record_for(dir.path(), "plugin-aa", "u")),
        "plugin-bb",
        "reviewer"
    )
    .is_none());
}

#[test]
fn infos_list_session_agents_with_deterministic_ids() {
    let dir = tempfile::tempdir().unwrap();
    write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
    let record = record_for(dir.path(), "plugin-aa", "https://github.com/acme/p");
    let infos = infos_from_records(&[record]);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].id, "plugin:plugin-aa:reviewer");
    assert_eq!(infos[0].name, "reviewer");
    assert_eq!(infos[0].description, "Reviews code");
}

fn record_for(root: &Path, source_id: &str, source_url: &str) -> PluginSourceRecord {
    PluginSourceRecord {
        manifest: crate::services::hooks::registry::PluginSourceManifest {
            source_url: source_url.to_string(),
            source_id: source_id.to_string(),
            skill_names: Vec::new(),
            hooks: None,
            hooks_approved: false,
            approval_hashes: Default::default(),
            created_at: 0,
            plugin: None,
            hook_files: Vec::new(),
            mcp_servers: Vec::new(),
            mcp_approved: false,
            enabled: true,
            installed_commit: None,
        },
        root_dir: root.to_path_buf(),
    }
}

async fn memory_db() -> sea_orm::DatabaseConnection {
    use sea_orm_migration::MigratorTrait;
    crate::lifecycle::database::register_sqlite_vec();
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("memory db");
    crate::migration::Migrator::up(&db, None)
        .await
        .expect("migrations");
    db
}

// G1.3(b): the store-facing lifecycle paths, not just the pure helpers.

#[tokio::test]
async fn sync_converges_on_current_definitions_across_updates() {
    let _guard = crate::state::lock_test_global_state().await;
    crate::state::set_assistant_repository(
        crate::repositories::assistant_repository::SqliteAssistantRepository::new(
            memory_db().await,
        ),
    );
    use crate::repositories::AssistantRepository;
    let repo = crate::state::get_assistant_repository();
    // A user row and another plugin's row must never be touched.
    repo.create_assistant(
        "user-assistant".to_string(),
        "user".to_string(),
        "{}".to_string(),
    )
    .await
    .unwrap();

    let dir = tempfile::tempdir().unwrap();
    write_agent(dir.path(), "agents/reviewer.md", REVIEWER);
    let record = record_for(
        dir.path(),
        "plugin-cc00000000000001",
        "https://github.com/acme/p",
    );

    // Install.
    sync_global_plugin_agents(&record).await.unwrap();
    assert!(repo
        .get_assistant("plugin:plugin-cc00000000000001:reviewer")
        .await
        .unwrap()
        .is_some());

    // Update: the plugin dropped reviewer and added writer — the rows
    // must converge exactly on the current definitions (remove-first).
    fs::remove_file(dir.path().join("agents/reviewer.md")).unwrap();
    write_agent(
        dir.path(),
        "agents/writer.md",
        "---\ndescription: w\n---\nbody",
    );
    sync_global_plugin_agents(&record).await.unwrap();
    assert!(repo
        .get_assistant("plugin:plugin-cc00000000000001:reviewer")
        .await
        .unwrap()
        .is_none());
    assert!(repo
        .get_assistant("plugin:plugin-cc00000000000001:writer")
        .await
        .unwrap()
        .is_some());

    // Uninstall (AC4): this source's rows go, everything else stays.
    remove_global_plugin_agents("plugin-cc00000000000001")
        .await
        .unwrap();
    assert!(repo
        .get_assistant("plugin:plugin-cc00000000000001:writer")
        .await
        .unwrap()
        .is_none());
    assert!(repo
        .get_assistant("user-assistant")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn declared_model_reads_store_row_and_survives_broken_config() {
    let _guard = crate::state::lock_test_global_state().await;
    crate::state::set_assistant_repository(
        crate::repositories::assistant_repository::SqliteAssistantRepository::new(
            memory_db().await,
        ),
    );
    use crate::repositories::AssistantRepository;
    let repo = crate::state::get_assistant_repository();
    repo.create_assistant(
        "plugin:plugin-dd00000000000001:reviewer".to_string(),
        "reviewer".to_string(),
        serde_json::json!({ "pluginAgentModel": "sonnet" }).to_string(),
    )
    .await
    .unwrap();
    repo.create_assistant(
        "plugin:plugin-dd00000000000002:broken".to_string(),
        "broken".to_string(),
        "not json".to_string(),
    )
    .await
    .unwrap();

    assert_eq!(
        declared_plugin_agent_model("plugin:plugin-dd00000000000001:reviewer", None)
            .await
            .as_deref(),
        Some("sonnet")
    );
    // A broken row must not fail the spawn: None means "inherit the
    // parent model" (spawn.rs keeps the caller's complexity tier).
    assert_eq!(
        declared_plugin_agent_model("plugin:plugin-dd00000000000002:broken", None).await,
        None
    );
    // A missing row and a non-plugin id resolve the same way.
    assert_eq!(
        declared_plugin_agent_model("plugin:plugin-dd00000000000003:ghost", None).await,
        None
    );
    assert_eq!(
        declared_plugin_agent_model("user-assistant", None).await,
        None
    );
}

#[test]
fn session_resolution_skips_missing_or_broken_plugin_root() {
    // A store entry whose plugin directory was deleted resolves to
    // "nothing" (spawn/resume then fall back), never a panic.
    let record = record_for(Path::new("Z:/definitely-not-installed"), "plugin-aa", "u");
    assert!(resolve_session_plugin_assistant_from_records(
        &[record.clone()],
        "plugin-aa",
        "reviewer"
    )
    .is_none());
    assert!(infos_from_records(&[record]).is_empty());
}
