//! SubagentStop matcher tests (#3003 G1.5 / G2.3), split from `tests.rs`
//! so no file under `agent/workflow/` exceeds 600 lines (#3020 / #3064 G1.1).

use super::stop_hook::{subagent_matcher_value, subagent_matcher_value_with_names};

#[test]
fn subagent_matcher_value_uses_agent_type_for_children_only() {
    // Ids exactly as the app builds them: plugin source ids are
    // `plugin-<16 hex>` (skill_service::importing::plugin_source_id), so
    // a plugin agent row is `plugin:plugin-<16 hex>:<agent>` (#3003 G2.3).
    assert_eq!(
        subagent_matcher_value_with_names(None, Some("Explore"), Some("child"), None, None),
        ""
    );
    // Plugin agents resolve to the plugin manifest name for the scope.
    assert_eq!(
        subagent_matcher_value_with_names(
            Some("parent-1"),
            Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
            Some("child-session-name"),
            Some("my-plugin"),
            None,
        ),
        "my-plugin:reviewer"
    );
    // A plugin whose manifest name cannot be read falls back to the
    // source id (still better than the session name).
    assert_eq!(
        subagent_matcher_value_with_names(
            Some("parent-1"),
            Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
            Some("child-session-name"),
            None,
            None,
        ),
        "plugin-1a2b3c4d5e6f7a8b:reviewer"
    );
    // User agents resolve to the assistant configuration's name.
    assert_eq!(
        subagent_matcher_value_with_names(
            Some("parent-1"),
            Some("b15fdf3e-6192-453d-9dc2-7b8ae6dab321"),
            Some("child-session-name"),
            None,
            Some("reviewer"),
        ),
        "reviewer"
    );
    // Without an assistant row, the session name remains the fallback.
    assert_eq!(
        subagent_matcher_value_with_names(
            Some("parent-1"),
            Some("b15fdf3e-6192-453d-9dc2-7b8ae6dab321"),
            Some("reviewer"),
            None,
            None,
        ),
        "reviewer"
    );
    assert_eq!(
        subagent_matcher_value_with_names(Some("parent-1"), None, Some("reviewer"), None, None),
        "reviewer"
    );
    assert_eq!(
        subagent_matcher_value_with_names(Some("parent-1"), Some("  "), None, None, None),
        ""
    );
}

/// Drives the production async wrapper (not just the pure core) with an
/// app-built plugin id. The plugin is not installed in the unit-test
/// environment, so the namespace lookup misses and the scope falls back
/// to the source id — proving the wrapper resolves through the
/// session+global store path without panicking (#3003 G2.3).
#[tokio::test]
async fn subagent_matcher_value_resolves_plugin_scope_async() {
    assert_eq!(
        subagent_matcher_value(
            Some("parent-1"),
            Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
            Some("child-session-name"),
        )
        .await,
        "plugin-1a2b3c4d5e6f7a8b:reviewer"
    );
    assert_eq!(
        subagent_matcher_value(
            None,
            Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
            Some("child")
        )
        .await,
        ""
    );
    assert_eq!(
        subagent_matcher_value(Some("parent-1"), None, Some("reviewer")).await,
        "reviewer"
    );
}

/// #3003 G2.3: session-scoped plugin agents live in the **parent**
/// session's plugin store, so `subagent_matcher_value` must resolve the
/// plugin namespace from the parent session's store and return
/// `<manifest-name>:<agent>`.
#[tokio::test]
async fn subagent_matcher_value_resolves_parent_session_plugin_namespace_async() {
    use crate::services::hooks::registry::{
        get_session_plugins_directory, save_source_in, upsert_session_store_entry, PluginMetadata,
        PluginSourceManifest, SessionPluginStoreEntry,
    };
    let parent_id = format!("test-parent-{}", uuid::Uuid::new_v4());
    let source_id = "plugin-1a2b3c4d5e6f7a8b";
    let plugins_dir = get_session_plugins_directory(&parent_id).expect("session plugins dir");
    std::fs::create_dir_all(plugins_dir.join(source_id)).expect("create plugin dir");

    let manifest = PluginSourceManifest {
        source_url: "https://github.com/example/my-plugin".to_string(),
        source_id: source_id.to_string(),
        skill_names: Vec::new(),
        hooks: None,
        hooks_approved: false,
        approval_hashes: std::collections::BTreeMap::new(),
        created_at: 0,
        plugin: Some(PluginMetadata {
            name: "my-plugin".to_string(),
            version: None,
            description: None,
            author: None,
            homepage: None,
            repository: None,
            license: None,
            keywords: Vec::new(),
        }),
        hook_files: Vec::new(),
        inline_hooks: Vec::new(),
        mcp_servers: Vec::new(),
        mcp_approved: false,
        enabled: true,
        installed_commit: None,
    };
    save_source_in(&plugins_dir, manifest).expect("save plugin manifest");
    upsert_session_store_entry(
        &plugins_dir,
        SessionPluginStoreEntry {
            source_id: source_id.to_string(),
            source_url: "https://github.com/example/my-plugin".to_string(),
            installed_at: 0,
        },
    )
    .expect("upsert session store entry");

    let resolved = subagent_matcher_value(
        Some(&parent_id),
        Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
        Some("child-session-name"),
    )
    .await;

    // Clean up the test parent's default workspace directory.
    if let Ok(manager) = crate::session::get_session_manager() {
        let _ = std::fs::remove_dir_all(
            manager.get_session_default_workspace_dir_unverified(&parent_id),
        );
    }

    assert_eq!(resolved, "my-plugin:reviewer");
}

/// #3003 G2.3: a user agent's matcher value is the **assistant
/// configuration's name** read from the repository — a real lookup, not
/// an injected string.
#[tokio::test]
async fn subagent_matcher_value_resolves_user_assistant_name_async() {
    use sea_orm_migration::MigratorTrait;
    let _guard = crate::state::lock_test_global_state().await;
    crate::lifecycle::database::register_sqlite_vec();
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("memory db");
    crate::migration::Migrator::up(&db, None)
        .await
        .expect("migrations");
    crate::state::set_assistant_repository(
        crate::repositories::assistant_repository::SqliteAssistantRepository::new(db),
    );
    use crate::repositories::AssistantRepository;
    crate::state::get_assistant_repository()
        .create_assistant(
            "b15fdf3e-6192-453d-9dc2-7b8ae6dab321".to_string(),
            "Code Reviewer".to_string(),
            "{}".to_string(),
        )
        .await
        .unwrap();

    assert_eq!(
        subagent_matcher_value(
            Some("parent-1"),
            Some("b15fdf3e-6192-453d-9dc2-7b8ae6dab321"),
            Some("child-session-name"),
        )
        .await,
        "Code Reviewer"
    );
    // Unknown assistant id → session name fallback (with a warning).
    assert_eq!(
        subagent_matcher_value(
            Some("parent-1"),
            Some("00000000-0000-0000-0000-000000000000"),
            Some("child-session-name"),
        )
        .await,
        "child-session-name"
    );
}
