use super::container_check::{check_in_container, verification_plan, VerificationPlan};
use super::secrets::{clean_secret, plan_secrets, store_secrets};
use super::server_row::write_server;
use super::*;
use crate::mcp::session_isolation::exec_target::test_docker_session;
use crate::models::workspace_isolation::DockerWorkspaceConfig;
use crate::repositories::mcp_server_repository::SqliteMCPServerRepository;
use sea_orm::Database;
use sea_orm_migration::MigratorTrait;

async fn repo() -> SqliteMCPServerRepository {
    crate::lifecycle::database::register_sqlite_vec();
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("memory db");
    crate::migration::Migrator::up(&db, None)
        .await
        .expect("migrations");
    SqliteMCPServerRepository::new(db)
}

fn stdio_config() -> MCPServerConfig {
    serde_json::from_value(serde_json::json!({
        "name": "srv",
        "transport": { "type": "stdio", "command": "npx", "args": ["-y", "srv"] }
    }))
    .expect("config")
}

fn docker_target() -> StdioExecTarget {
    StdioExecTarget::from_session(test_docker_session(DockerWorkspaceConfig {
        image: Some("python:3.12-slim".to_string()),
        attach_container: None,
        workdir: None,
        manage_lifecycle: None,
        env: Default::default(),
        port_bindings: Vec::new(),
    }))
}

#[test]
fn a_pasted_secret_is_trimmed_and_control_characters_are_refused() {
    assert_eq!(
        clean_secret("  ghp_abc\r\n", "GITHUB_PAT").unwrap(),
        "ghp_abc"
    );
    assert!(clean_secret(" \n", "GITHUB_PAT").is_err());
    assert!(clean_secret("ghp_\nabc", "GITHUB_PAT").is_err());
}

#[test]
fn docker_session_register_tests_in_container_not_background_probe() {
    let config = stdio_config();
    assert_eq!(
        verification_plan(true, &docker_target(), &config),
        VerificationPlan::SessionContainer
    );
    assert_eq!(
        verification_plan(true, &StdioExecTarget::Host, &config),
        VerificationPlan::BackgroundProbe
    );
    assert_eq!(
        verification_plan(false, &docker_target(), &config),
        VerificationPlan::None
    );
}

#[test]
fn docker_session_http_server_keeps_background_probe() {
    let config: MCPServerConfig = serde_json::from_value(serde_json::json!({
        "name": "web",
        "transport": { "type": "http", "url": "http://localhost:1/mcp" }
    }))
    .expect("config");
    assert_eq!(
        verification_plan(true, &docker_target(), &config),
        VerificationPlan::BackgroundProbe
    );
}

#[tokio::test]
async fn unverified_save_never_writes_pending() {
    let repo = repo().await;
    let created = McpServerService::create_server_config(
        &repo,
        "srv".to_string(),
        serde_json::to_value(stdio_config()).unwrap(),
        SaveVerification::Unverified,
    )
    .await
    .expect("create");
    assert_eq!(created.verification_status, None);
}

#[tokio::test]
async fn unverified_update_drops_the_old_cached_tools() {
    let repo = repo().await;
    let created = McpServerService::create_server_config(
        &repo,
        "srv".to_string(),
        serde_json::to_value(stdio_config()).unwrap(),
        SaveVerification::Pending,
    )
    .await
    .expect("create");
    repo.update_cached_tools(&created.id, 3, "[]".to_string())
        .await
        .expect("cache");

    let new_config = serde_json::json!({
        "name": "srv",
        "transport": { "type": "stdio", "command": "uvx", "args": ["srv"] }
    });
    let updated = McpServerService::update_server_config(
        &repo,
        created.id.clone(),
        None,
        Some(new_config),
        SaveVerification::Unverified,
    )
    .await
    .expect("update");

    assert_eq!(updated.verification_status, None);
    assert_eq!(updated.cached_tools, None);
    assert_eq!(updated.tool_count, None);
}

#[tokio::test]
async fn failed_container_check_leaves_no_global_result() {
    let repo = repo().await;
    let created = McpServerService::create_server_config(
        &repo,
        "srv".to_string(),
        serde_json::to_value(stdio_config()).unwrap(),
        SaveVerification::Unverified,
    )
    .await
    .expect("create");

    let check = check_in_container(&repo, &created.id, "srv", async {
        Err("The command 'npx' is not in the session container.".to_string())
    })
    .await
    .expect("check");

    assert!(matches!(check, ContainerCheck::Failed(ref e) if e.contains("'npx'")));
    let stored = repo.get(&created.id).await.unwrap().expect("record");
    assert_eq!(stored.verification_status, None);
    assert_eq!(stored.last_verification_error, None);
    assert_eq!(stored.cached_tools, None);
    assert_eq!(stored.tool_count, None);
}

fn exa_config(url: &str) -> MCPServerConfig {
    serde_json::from_value(serde_json::json!({
        "name": "exa",
        "transport": { "type": "http", "url": url }
    }))
    .expect("config")
}

const EXA_URL: &str = "https://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}";

/// Save like `save_server_config` does after the prompt: template row, then values.
async fn saved_exa(repo: &SqliteMCPServerRepository) -> crate::entity::mcp_server::Model {
    let created = McpServerService::create_server_config(
        repo,
        "exa".to_string(),
        serde_json::to_value(exa_config(EXA_URL)).unwrap(),
        SaveVerification::Pending,
    )
    .await
    .expect("create");
    let entered = vec![("EXA_API_KEY".to_string(), "exa-secret-value".to_string())];
    store_secrets(repo, secret_store(), &created.id, &entered, &[])
        .await
        .expect("store");
    repo.get(&created.id).await.unwrap().expect("record")
}

#[tokio::test]
async fn the_row_and_the_list_hold_the_template_and_names_not_the_value() {
    let repo = repo().await;
    let model = saved_exa(&repo).await;

    assert!(model.config.contains("{{secret:EXA_API_KEY}}"));
    assert_eq!(model.secret_names.as_deref(), Some(r#"["EXA_API_KEY"]"#));
    let listed = McpServerService::list_server_configs(&repo).await.unwrap();
    let row = format!("{:?}", listed);
    assert!(!row.contains("exa-secret-value"));
    assert_eq!(
        secret_store()
            .get(&model.id, "EXA_API_KEY")
            .await
            .unwrap()
            .as_deref(),
        Some("exa-secret-value")
    );
}

#[tokio::test]
async fn update_reuses_a_value_only_for_the_same_target_without_reentry() {
    let repo = repo().await;
    let model = saved_exa(&repo).await;
    let none = BTreeSet::new();

    let same_host = exa_config("https://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}&x=1");
    let (ask, reuse) = plan_secrets(secret_store(), &same_host, Some(&model), &none)
        .await
        .unwrap();
    assert!(ask.is_empty());
    assert_eq!(reuse, vec!["EXA_API_KEY".to_string()]);

    let reenter = BTreeSet::from(["EXA_API_KEY".to_string()]);
    let (ask, reuse) = plan_secrets(secret_store(), &same_host, Some(&model), &reenter)
        .await
        .unwrap();
    assert_eq!(ask, vec!["EXA_API_KEY".to_string()]);
    assert!(reuse.is_empty());

    for moved in [
        "http://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}",
        "https://evil.example/mcp?exaApiKey={{secret:EXA_API_KEY}}",
        "https://mcp.exa.ai:8443/mcp?exaApiKey={{secret:EXA_API_KEY}}",
    ] {
        let (ask, reuse) = plan_secrets(secret_store(), &exa_config(moved), Some(&model), &none)
            .await
            .unwrap();
        assert_eq!(ask, vec!["EXA_API_KEY".to_string()], "{moved}");
        assert!(reuse.is_empty(), "{moved}");
    }

    let (ask, _) = plan_secrets(secret_store(), &exa_config(EXA_URL), None, &none)
        .await
        .unwrap();
    assert_eq!(ask, vec!["EXA_API_KEY".to_string()]);
}

#[tokio::test]
async fn the_preview_lookup_names_the_secrets_an_update_reuses() {
    let repo = repo().await;
    saved_exa(&repo).await;
    let none = BTreeSet::new();

    let reused = reused_secret_names(&repo, secret_store(), &exa_config(EXA_URL), &none).await;
    assert_eq!(reused, vec!["EXA_API_KEY".to_string()]);

    let moved = exa_config("https://evil.example/mcp?exaApiKey={{secret:EXA_API_KEY}}");
    assert!(reused_secret_names(&repo, secret_store(), &moved, &none)
        .await
        .is_empty());
}

#[tokio::test]
async fn a_ui_edit_to_a_plain_value_removes_the_stored_secret() {
    let repo = repo().await;
    let model = saved_exa(&repo).await;

    let plain = serde_json::to_value(exa_config("https://mcp.exa.ai/mcp?exaApiKey=plain")).unwrap();
    McpServerService::update_server_config(
        &repo,
        model.id.clone(),
        None,
        Some(plain),
        SaveVerification::Pending,
    )
    .await
    .expect("update");

    let updated = repo.get(&model.id).await.unwrap().expect("record");
    assert_eq!(updated.secret_names, None);
    assert_eq!(
        secret_store().get(&model.id, "EXA_API_KEY").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn delete_removes_the_stored_secrets() {
    let repo = repo().await;
    let model = saved_exa(&repo).await;

    McpServerService::delete_server_config(&repo, &model.id)
        .await
        .expect("delete");

    assert_eq!(
        secret_store().get(&model.id, "EXA_API_KEY").await.unwrap(),
        None
    );
}

struct SetFailsStore;
#[async_trait::async_trait]
impl SecretStore for SetFailsStore {
    async fn get(&self, _: &str, _: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    async fn set(&self, _: &str, _: &str, _: &str) -> Result<Option<String>, String> {
        Err("keychain is locked".to_string())
    }
    async fn delete(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
}

#[tokio::test]
async fn a_failed_keychain_write_on_update_leaves_the_row_as_it_was() {
    let repo = repo().await;
    let model = saved_exa(&repo).await;
    let entered = vec![("OTHER_KEY".to_string(), "new-value".to_string())];
    let new_config = exa_config("https://mcp.exa.ai/v2?otherKey={{secret:OTHER_KEY}}");

    let error = write_server(
        &repo,
        &SetFailsStore,
        Some(model.clone()),
        &new_config,
        SaveVerification::Pending,
        &entered,
        &[],
    )
    .await
    .err()
    .expect("write fails");

    assert!(error.contains("keychain is locked"), "{error}");
    let stored = repo.get(&model.id).await.unwrap().expect("record");
    assert_eq!(stored.config, model.config);
    assert_eq!(stored.secret_names, model.secret_names);
    assert_eq!(
        secret_store()
            .get(&model.id, "EXA_API_KEY")
            .await
            .unwrap()
            .as_deref(),
        Some("exa-secret-value")
    );
}

#[tokio::test]
async fn a_failed_keychain_write_on_create_removes_the_new_row() {
    let repo = repo().await;
    let entered = vec![("EXA_API_KEY".to_string(), "value".to_string())];

    let result = write_server(
        &repo,
        &SetFailsStore,
        None,
        &exa_config(EXA_URL),
        SaveVerification::Pending,
        &entered,
        &[],
    )
    .await;

    assert!(result.is_err());
    assert!(repo.get_by_name("exa").await.unwrap().is_none());
}

fn value_at<'a>(config: &'a serde_json::Value, pointer: &str) -> &'a str {
    config
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
}

#[tokio::test]
async fn the_github_bearer_preset_saves_the_value_and_starts_with_it() {
    let repo = repo().await;
    let config: MCPServerConfig = serde_json::from_value(serde_json::json!({
        "name": "github",
        "transport": {
            "type": "http",
            "url": "https://api.githubcopilot.com/mcp/",
            "headers": { "Authorization": "Bearer {{secret:GITHUB_PAT}}" }
        }
    }))
    .expect("config");
    let entered = vec![("GITHUB_PAT".to_string(), "ghp_value".to_string())];

    let written = write_server(
        &repo,
        secret_store(),
        None,
        &config,
        SaveVerification::Pending,
        &entered,
        &[],
    )
    .await
    .expect("write");

    let row = repo.get(&written.id).await.unwrap().expect("record");
    assert!(row.config.contains("Bearer {{secret:GITHUB_PAT}}"));
    assert!(!row.config.contains("ghp_value"));
    let filled = crate::mcp::secret_templates::fill_secrets(secret_store(), &written.id, &config)
        .await
        .expect("filled");
    let started = serde_json::to_value(&filled.config).unwrap();
    assert_eq!(
        value_at(&started, "/transport/headers/Authorization"),
        "Bearer ghp_value"
    );
}

#[tokio::test]
async fn without_a_keychain_a_save_starts_and_updates_through_the_encrypted_column() {
    use crate::mcp::secret_fallback::{
        FileSecretStore, LayeredSecretStore, FALLBACK_WARNING, KEY_FILE_NAME,
    };
    use crate::mcp::secret_templates::{fill_secrets, MemorySecretStore};

    let repo = repo().await;
    let dir = tempfile::tempdir().unwrap();
    let keychain = MemorySecretStore::unavailable();
    let store = LayeredSecretStore::new(
        &keychain,
        FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
    );
    let entered = vec![("EXA_API_KEY".to_string(), "exa-secret-value".to_string())];
    let write = |config: MCPServerConfig| {
        let (store, repo, entered) = (&store, &repo, &entered);
        async move {
            write_server(
                repo,
                store,
                None,
                &config,
                SaveVerification::Pending,
                entered,
                &[],
            )
            .await
            .expect("write")
        }
    };

    let first = write(exa_config(EXA_URL)).await;
    let mut other = exa_config(EXA_URL);
    other.name = Some("exa2".to_string());
    let second = write(other).await;

    assert_eq!(first.storage_warning.as_deref(), Some(FALLBACK_WARNING));
    assert_eq!(second.storage_warning, None, "the warning shows once");
    let row = repo.get(&first.id).await.unwrap().expect("record");
    let sealed = row.encrypted_secrets.clone().expect("encrypted_secrets");
    assert!(!sealed.contains("exa-secret-value"));
    assert!(!row.config.contains("exa-secret-value"));
    assert_eq!(keychain.get(&first.id, "EXA_API_KEY").await.unwrap(), None);

    let filled = fill_secrets(&store, &first.id, &exa_config(EXA_URL))
        .await
        .expect("start decrypts");
    let started = serde_json::to_value(&filled.config).unwrap();
    assert_eq!(
        value_at(&started, "/transport/url"),
        "https://mcp.exa.ai/mcp?exaApiKey=exa-secret-value"
    );

    let plain = serde_json::to_value(exa_config("https://mcp.exa.ai/mcp?exaApiKey=plain")).unwrap();
    McpServerService::update_server_config_with_store(
        &repo,
        &store,
        first.id.clone(),
        None,
        Some(plain),
        SaveVerification::Pending,
    )
    .await
    .expect("update");
    let updated = repo.get(&first.id).await.unwrap().expect("record");
    assert_eq!(updated.encrypted_secrets, None);
    assert_eq!(updated.secret_names, None);

    McpServerService::delete_server_config_with_store(&repo, &store, &second.id)
        .await
        .expect("delete");
    assert!(repo.get(&second.id).await.unwrap().is_none());
}

/// #2588: a secret removed by an update or by deleting the server during a keychain
/// outage is removed from the keychain once it works again, with no read of that name.
#[tokio::test]
async fn secrets_removed_during_a_keychain_outage_leave_the_keychain_once_it_returns() {
    use crate::mcp::secret_fallback::{FileSecretStore, LayeredSecretStore, KEY_FILE_NAME};
    use crate::mcp::secret_templates::MemorySecretStore;

    let repo = repo().await;
    let dir = tempfile::tempdir().unwrap();
    let keychain = MemorySecretStore::default();
    let store = LayeredSecretStore::new(
        &keychain,
        FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
    );
    let entered = vec![("EXA_API_KEY".to_string(), "exa-secret-value".to_string())];
    let updated = write_server(
        &repo,
        &store,
        None,
        &exa_config(EXA_URL),
        SaveVerification::Pending,
        &entered,
        &[],
    )
    .await
    .expect("write");
    let mut other = exa_config(EXA_URL);
    other.name = Some("exa2".to_string());
    let deleted = write_server(
        &repo,
        &store,
        None,
        &other,
        SaveVerification::Pending,
        &entered,
        &[],
    )
    .await
    .expect("write");

    keychain.set_available(false);
    let plain = serde_json::to_value(exa_config("https://mcp.exa.ai/mcp?exaApiKey=plain")).unwrap();
    McpServerService::update_server_config_with_store(
        &repo,
        &store,
        updated.id.clone(),
        None,
        Some(plain),
        SaveVerification::Pending,
    )
    .await
    .expect("update");
    McpServerService::delete_server_config_with_store(&repo, &store, &deleted.id)
        .await
        .expect("delete");
    assert!(keychain
        .get(&updated.id, "EXA_API_KEY")
        .await
        .unwrap()
        .is_some());

    keychain.set_available(true);
    store.get("unrelated", "OTHER_KEY").await.unwrap();
    assert_eq!(
        keychain.get(&updated.id, "EXA_API_KEY").await.unwrap(),
        None
    );
    assert_eq!(
        keychain.get(&deleted.id, "EXA_API_KEY").await.unwrap(),
        None
    );
    assert_eq!(
        repo.get(&updated.id)
            .await
            .unwrap()
            .unwrap()
            .encrypted_secrets,
        None
    );
}

/// A key file that is gone leaves every name of the row unreadable, so all of them must
/// be entered again, and doing that must recover the row.
#[tokio::test]
async fn a_lost_key_file_needs_every_name_entered_again() {
    use crate::mcp::secret_fallback::{FileSecretStore, KEY_FILE_NAME};

    let two_secrets = "https://mcp.exa.ai/mcp?a={{secret:EXA_API_KEY}}&b={{secret:OTHER_KEY}}";
    let config = exa_config(two_secrets);
    let repo = repo().await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join(KEY_FILE_NAME);
    let store = FileSecretStore::new(&repo, key_path.clone());
    let created = McpServerService::create_server_config(
        &repo,
        "exa".to_string(),
        serde_json::to_value(&config).unwrap(),
        SaveVerification::Pending,
    )
    .await
    .expect("create");
    let entered = vec![
        ("EXA_API_KEY".to_string(), "one".to_string()),
        ("OTHER_KEY".to_string(), "two".to_string()),
    ];
    store_secrets(&repo, &store, &created.id, &entered, &[])
        .await
        .expect("store");
    std::fs::remove_file(&key_path).unwrap();
    let model = repo.get(&created.id).await.unwrap().expect("record");

    let only_one = BTreeSet::from(["EXA_API_KEY".to_string()]);
    let error = plan_secrets(&store, &config, Some(&model), &only_one)
        .await
        .expect_err("the other name cannot be read");
    assert!(error.contains("EXA_API_KEY"), "{error}");
    assert!(error.contains("OTHER_KEY"), "{error}");
    assert!(error.contains("The MCP secret key file"), "{error}");

    let both = BTreeSet::from(["EXA_API_KEY".to_string(), "OTHER_KEY".to_string()]);
    let (ask, reuse) = plan_secrets(&store, &config, Some(&model), &both)
        .await
        .expect("both names are entered again");
    assert_eq!(
        ask,
        vec!["EXA_API_KEY".to_string(), "OTHER_KEY".to_string()]
    );
    assert!(reuse.is_empty());

    let entered_again = vec![
        ("EXA_API_KEY".to_string(), "one-new".to_string()),
        ("OTHER_KEY".to_string(), "two-new".to_string()),
    ];
    write_server(
        &repo,
        &store,
        Some(model),
        &config,
        SaveVerification::Pending,
        &entered_again,
        &[],
    )
    .await
    .expect("write");
    assert_eq!(
        store
            .get(&created.id, "EXA_API_KEY")
            .await
            .unwrap()
            .as_deref(),
        Some("one-new")
    );
    assert_eq!(
        store
            .get(&created.id, "OTHER_KEY")
            .await
            .unwrap()
            .as_deref(),
        Some("two-new")
    );
}

#[test]
fn the_result_line_lists_names_only() {
    let saved = SavedServer {
        id: "id".to_string(),
        name: "exa".to_string(),
        container_check: None,
        entered_secrets: vec!["EXA_API_KEY".to_string()],
        reused_secrets: vec!["OTHER".to_string()],
        storage_warning: None,
    };
    assert_eq!(
        saved.secrets_line(),
        "\n• Secrets: entered EXA_API_KEY; reused OTHER"
    );

    let fallback = SavedServer {
        storage_warning: Some("keychain is not available".to_string()),
        ..saved
    };
    assert!(fallback
        .secrets_line()
        .ends_with("\n• Warning: keychain is not available"));
}
