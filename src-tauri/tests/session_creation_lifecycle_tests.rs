use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri_mcp_agent_lib::agent::lifecycle::creation::{
    normalize_lineage_defaults, resolve_model_and_provider, resolve_workspace_override,
};
use tauri_mcp_agent_lib::agent::AgentConfig;
use tauri_mcp_agent_lib::entity::settings;
use tauri_mcp_agent_lib::execution_mode::ExecutionMode;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::error::DbError;
use tauri_mcp_agent_lib::repositories::settings_repository::SettingsRepository;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};
use tempfile::TempDir;

struct MockSettingsRepo {
    values: HashMap<String, String>,
    fail_get: bool,
    get_calls: AtomicUsize,
}

impl MockSettingsRepo {
    fn with_preferred_model(raw_value: &str) -> Self {
        let mut values = HashMap::new();
        values.insert("preferredModel".to_string(), raw_value.to_string());
        Self {
            values,
            fail_get: false,
            get_calls: AtomicUsize::new(0),
        }
    }

    fn empty() -> Self {
        Self {
            values: HashMap::new(),
            fail_get: false,
            get_calls: AtomicUsize::new(0),
        }
    }

    fn failing() -> Self {
        Self {
            values: HashMap::new(),
            fail_get: true,
            get_calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl SettingsRepository for MockSettingsRepo {
    async fn get(&self, key: &str) -> Result<Option<settings::Model>, DbError> {
        self.get_calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_get {
            return Err(DbError::NotFound("simulated failure".to_string()));
        }
        Ok(self.values.get(key).map(|value| settings::Model {
            key: key.to_string(),
            value: value.clone(),
            created_at: 0,
            updated_at: 0,
        }))
    }

    async fn set(&self, _key: &str, _value: Value) -> Result<settings::Model, DbError> {
        unimplemented!()
    }

    async fn set_many(
        &self,
        _settings: HashMap<String, Value>,
    ) -> Result<Vec<settings::Model>, DbError> {
        unimplemented!()
    }

    async fn delete(&self, _key: &str) -> Result<(), DbError> {
        unimplemented!()
    }

    async fn list(&self) -> Result<Vec<settings::Model>, DbError> {
        unimplemented!()
    }
}

fn sample_session_metadata(id: &str, workspace_override: Option<String>) -> SessionMetadata {
    SessionMetadata {
        id: id.to_string(),
        name: Some("test".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-4".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: Some(id.to_string()),
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        is_bookmarked: false,
        created_at: 0,
        updated_at: 0,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        execution_mode: ExecutionMode::Normal,
        workspace_override,
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
        status_reason: None,
        forked_from: None,
    }
}

#[test]
fn normalize_lineage_defaults_sets_root_session_lineage_and_zero_depth() {
    let mut config = AgentConfig::default();
    normalize_lineage_defaults(&mut config, "root-session-1");

    assert_eq!(config.lineage_id.as_deref(), Some("root-session-1"));
    assert_eq!(config.depth, Some(0));
}

#[test]
fn normalize_lineage_defaults_sets_child_session_lineage_to_parent_and_depth_one() {
    let mut config = AgentConfig {
        parent_session_id: Some("parent-session-1".to_string()),
        ..AgentConfig::default()
    };
    normalize_lineage_defaults(&mut config, "child-session-1");

    assert_eq!(config.lineage_id.as_deref(), Some("parent-session-1"));
    assert_eq!(config.depth, Some(1));
}

#[test]
fn normalize_lineage_defaults_preserves_explicit_lineage_and_depth() {
    let mut config = AgentConfig {
        parent_session_id: Some("parent-session-1".to_string()),
        lineage_id: Some("grandparent-lineage".to_string()),
        depth: Some(3),
        ..AgentConfig::default()
    };
    normalize_lineage_defaults(&mut config, "child-session-3");

    assert_eq!(config.lineage_id.as_deref(), Some("grandparent-lineage"));
    assert_eq!(config.depth, Some(3));
}

#[tokio::test]
async fn resolve_model_and_provider_uses_explicit_values_without_querying_settings() {
    let repo = MockSettingsRepo::with_preferred_model(
        r#"{"model":"claude-3-7-sonnet","provider":"anthropic"}"#,
    );

    let (model, provider) = resolve_model_and_provider(
        Some("gemini-2.5-pro".to_string()),
        Some("google".to_string()),
        &repo,
    )
    .await;

    assert_eq!(model, "gemini-2.5-pro");
    assert_eq!(provider, "google");
    assert_eq!(repo.get_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn resolve_model_and_provider_falls_back_to_preferred_model_setting() {
    let repo = MockSettingsRepo::with_preferred_model(
        r#"{"model":"claude-3-7-sonnet","provider":"anthropic"}"#,
    );

    let (model, provider) = resolve_model_and_provider(None, None, &repo).await;
    assert_eq!(model, "claude-3-7-sonnet");
    assert_eq!(provider, "anthropic");
    assert_eq!(repo.get_calls.load(Ordering::SeqCst), 1);

    // Partial explicit input (missing provider) also falls back to preferredModel.
    let (model2, provider2) =
        resolve_model_and_provider(Some("only-model".to_string()), None, &repo).await;
    assert_eq!(model2, "claude-3-7-sonnet");
    assert_eq!(provider2, "anthropic");
}

#[tokio::test]
async fn resolve_model_and_provider_falls_back_to_hardcoded_defaults_on_missing_or_invalid_setting()
{
    let empty_repo = MockSettingsRepo::empty();
    assert_eq!(
        resolve_model_and_provider(None, None, &empty_repo).await,
        ("gpt-4".to_string(), "openai".to_string())
    );

    let invalid_json_repo = MockSettingsRepo::with_preferred_model("not-json");
    assert_eq!(
        resolve_model_and_provider(None, None, &invalid_json_repo).await,
        ("gpt-4".to_string(), "openai".to_string())
    );

    let partial_json_repo = MockSettingsRepo::with_preferred_model(r#"{"model":"o3-mini"}"#);
    assert_eq!(
        resolve_model_and_provider(None, None, &partial_json_repo).await,
        ("o3-mini".to_string(), "openai".to_string())
    );

    let failing_repo = MockSettingsRepo::failing();
    assert_eq!(
        resolve_model_and_provider(None, None, &failing_repo).await,
        ("gpt-4".to_string(), "openai".to_string())
    );
}

#[tokio::test]
async fn resolve_workspace_override_preserves_unavailable_persisted_path() {
    let temp = TempDir::new().expect("create temp dir");
    let missing_path = temp.path().join("does-not-exist");
    let missing_str = missing_path.to_string_lossy().to_string();

    let existing = sample_session_metadata("sess-missing", Some(missing_str.clone()));
    let resolved = resolve_workspace_override("sess-missing", Some(&existing)).await;

    assert_eq!(resolved, Some(missing_str));
}

#[tokio::test]
async fn resolve_workspace_override_preserves_non_directory_persisted_path() {
    let temp = TempDir::new().expect("create temp dir");
    let file_path = temp.path().join("not-a-dir.txt");
    std::fs::write(&file_path, "hello").expect("write temp file");
    let file_str = file_path.to_string_lossy().to_string();

    let existing = sample_session_metadata("sess-file", Some(file_str.clone()));
    let resolved = resolve_workspace_override("sess-file", Some(&existing)).await;

    assert_eq!(resolved, Some(file_str));
}

#[tokio::test]
async fn resolve_workspace_override_returns_existing_directory_and_none_when_unset() {
    let temp = TempDir::new().expect("create temp dir");
    let dir_str = temp.path().to_string_lossy().to_string();

    let existing_with_dir = sample_session_metadata("sess-dir", Some(dir_str.clone()));
    assert_eq!(
        resolve_workspace_override("sess-dir", Some(&existing_with_dir)).await,
        Some(dir_str)
    );

    let existing_without_override = sample_session_metadata("sess-none", None);
    assert_eq!(
        resolve_workspace_override("sess-none", Some(&existing_without_override)).await,
        None
    );

    assert_eq!(
        resolve_workspace_override("sess-brand-new", None).await,
        None
    );
}
