use crate::agent::context::registry::ContextRegistry;
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::advanced_settings::AdvancedSettings;
use crate::models::workspace_isolation::{DockerWorkspaceConfig, WorkspaceIsolationMode};
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::{SessionMetadata, SessionStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Parameters for session creation: the fields the testable core needs, plus
/// the production handles used to build the Tauri runtime.
pub(crate) struct CreateSessionParams {
    pub core: CreateSessionCoreParams,
    pub proxy_manager: Arc<MCPServiceProxyManager>,
    pub app_handle: AppHandle,
}

/// Injectable runtime effects for session creation so production uses Tauri
/// `AppHandle` while tests can drive the full creation path without one.
#[async_trait::async_trait]
pub(crate) trait SessionCreationRuntime: Send + Sync {
    fn spawn_docker_provisioning(&self, session: SessionMetadata, delete_session_on_failure: bool);
    async fn spawn_create_proxy(
        &self,
        session_id: String,
        tool_ids: Vec<String>,
        mcp_server_ids: Vec<String>,
    );
}

struct TauriSessionCreationRuntime {
    session_repo: Arc<dyn SessionRepository>,
    active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: Arc<MCPServiceProxyManager>,
    app_handle: AppHandle,
}

#[async_trait::async_trait]
impl SessionCreationRuntime for TauriSessionCreationRuntime {
    fn spawn_docker_provisioning(&self, session: SessionMetadata, delete_session_on_failure: bool) {
        crate::services::docker_provisioning::spawn_docker_provisioning(
            crate::services::docker_provisioning::DockerProvisioningDeps::tauri(
                Arc::clone(&self.session_repo),
                Arc::clone(&self.active_sessions),
                Arc::clone(&self.proxy_manager),
                self.app_handle.clone(),
            ),
            session,
            delete_session_on_failure,
        );
    }

    async fn spawn_create_proxy(
        &self,
        session_id: String,
        tool_ids: Vec<String>,
        mcp_server_ids: Vec<String>,
    ) {
        crate::agent::lifecycle::spawn_create_proxy_for_session(
            Arc::clone(&self.proxy_manager),
            self.app_handle.clone(),
            session_id,
            tool_ids,
            mcp_server_ids,
        )
        .await;
    }
}

pub(crate) struct CreateSessionCoreParams {
    pub session_repo: Arc<dyn SessionRepository>,
    pub active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    pub context_registry: Arc<ContextRegistry>,
    pub session_id: String,
    pub name: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub agent_config: crate::agent::AgentConfig,
    pub workspace_isolation: WorkspaceIsolationMode,
    pub docker_config: Option<DockerWorkspaceConfig>,
}

/// Normalize lineage defaults so root sessions are always groupable in UI.
pub fn normalize_lineage_defaults(agent_config: &mut crate::agent::AgentConfig, session_id: &str) {
    if agent_config.lineage_id.is_none() {
        let normalized_lineage_id = if let Some(parent_id) = &agent_config.parent_session_id {
            parent_id.clone()
        } else {
            session_id.to_string()
        };
        agent_config.lineage_id = Some(normalized_lineage_id);
    }

    if agent_config.depth.is_none() {
        agent_config.depth = Some(if agent_config.parent_session_id.is_some() {
            1
        } else {
            0
        });
    }
}

/// Resolve mandatory model/provider, falling back to `preferredModel` in global settings
/// and finally `("gpt-4", "openai")`.
pub async fn resolve_model_and_provider(
    model: Option<String>,
    provider: Option<String>,
    settings_repo: &dyn SettingsRepository,
) -> (String, String) {
    if let (Some(m), Some(p)) = (model, provider) {
        return (m, p);
    }

    match settings_repo.get("preferredModel").await {
        Ok(Some(setting)) => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&setting.value) {
                let m = val["model"].as_str().unwrap_or("gpt-4").to_string();
                let p = val["provider"].as_str().unwrap_or("openai").to_string();
                (m, p)
            } else {
                ("gpt-4".to_string(), "openai".to_string())
            }
        }
        _ => ("gpt-4".to_string(), "openai".to_string()),
    }
}

/// Resolve workspace override from an existing persisted session or the workspace pool.
///
/// Preserves unavailable persisted overrides instead of clearing them on transient path loss.
pub async fn resolve_workspace_override(
    session_id: &str,
    existing_session: Option<&SessionMetadata>,
) -> Option<String> {
    if let Some(existing_session) = existing_session {
        if let Some(db_override) = existing_session.workspace_override.clone() {
            let path = std::path::PathBuf::from(&db_override);
            if path.is_dir() {
                // Pre-register in pool so WorkspaceService and tools see the correct directory
                if let Ok(session_manager) = crate::session::get_session_manager() {
                    if let Err(e) = session_manager
                        .register_session_override(session_id, path)
                        .await
                    {
                        log::warn!(
                            "Failed to pre-register workspace override for session {}: {}",
                            session_id,
                            e
                        );
                    }
                }
            } else {
                // Keep the DB preference — do not clear on transient path unavailability.
                log::warn!(
                    "Persisted workspace override '{}' for session {} is unavailable or not a directory; \
                     keeping the preference and falling back to the default workspace until it returns.",
                    db_override,
                    session_id
                );
            }
            Some(db_override)
        } else {
            None
        }
    } else {
        // No existing DB record — resolve from pool (handles the case where override was
        // registered before the session was first persisted)
        crate::services::WorkspaceService::get_override(session_id)
            .await
            .unwrap_or(None)
    }
}

/// Create or update a session in the database
pub(crate) async fn create_session(params: CreateSessionParams) -> Result<SessionMetadata, String> {
    let CreateSessionParams {
        core,
        proxy_manager,
        app_handle,
    } = params;
    let runtime: Arc<dyn SessionCreationRuntime> = Arc::new(TauriSessionCreationRuntime {
        session_repo: Arc::clone(&core.session_repo),
        active_sessions: Arc::clone(&core.active_sessions),
        proxy_manager,
        app_handle,
    });
    let hook_dispatcher: Arc<dyn crate::services::hooks::execute::HookDispatcher> =
        Arc::new(crate::services::hooks::execute::DefaultHookDispatcher);
    create_session_with_runtime(core, runtime, hook_dispatcher).await
}

pub(crate) async fn create_session_with_runtime(
    params: CreateSessionCoreParams,
    runtime: Arc<dyn SessionCreationRuntime>,
    hook_dispatcher: Arc<dyn crate::services::hooks::execute::HookDispatcher>,
) -> Result<SessionMetadata, String> {
    let CreateSessionCoreParams {
        session_repo,
        active_sessions,
        context_registry,
        session_id,
        name,
        model,
        provider,
        mut agent_config,
        workspace_isolation,
        docker_config,
    } = params;

    let now = chrono::Utc::now().timestamp_millis();

    // Validate agent config
    agent_config.validate()?;

    normalize_lineage_defaults(&mut agent_config, &session_id);

    // Resolve mandatory model/provider
    let settings_repo = crate::state::get_settings_repository();
    let (resolved_model, resolved_provider) =
        resolve_model_and_provider(model, provider, settings_repo).await;

    // Resolve workspace override: check DB first so the pool is correct before any lazy loading.
    // For existing sessions, the override may already be in the DB (set via set_override which
    // now persists to DB). For new sessions, DB returns None and WorkspaceService returns None.
    let existing_session = session_repo.get_session(&session_id).await.ok().flatten();
    let workspace_override =
        resolve_workspace_override(&session_id, existing_session.as_ref()).await;

    let docker_host_workspace_path = if workspace_isolation == WorkspaceIsolationMode::Docker {
        let host_workspace = if let Some(override_path) = workspace_override.as_ref() {
            std::path::PathBuf::from(override_path)
        } else {
            crate::session::get_session_manager()
                .map_err(|e| format!("Failed to get session manager for Docker workspace: {e}"))?
                .get_session_workspace_dir_by_id(&session_id)
        };

        let is_attach = docker_config
            .as_ref()
            .is_some_and(DockerWorkspaceConfig::is_attach);
        if existing_session.is_none() && !is_attach {
            crate::services::workspace_runtime_manager::refuse_home_workspace(
                &host_workspace,
                dirs::home_dir().as_deref(),
            )
            .map_err(|e| e.to_string())?;
        }

        Some(host_workspace.to_string_lossy().to_string())
    } else {
        None
    };

    let docker_container_name = if workspace_isolation == WorkspaceIsolationMode::Docker {
        if let Some(attach) = docker_config
            .as_ref()
            .and_then(DockerWorkspaceConfig::attach_container_name)
        {
            Some(attach.to_string())
        } else {
            Some(format!("libragent-session-{session_id}"))
        }
    } else {
        None
    };

    // Root sessions created without explicit limits get the configured defaults, so
    // the limits also apply to requests that did not come through the UI.
    let (default_max_depth, default_max_fanout) = if existing_session.is_none()
        && agent_config.parent_session_id.is_none()
    {
        let defaults = AdvancedSettings::load_global().await;
        (
            (defaults.default_session_max_depth > 0).then_some(defaults.default_session_max_depth),
            (defaults.default_session_max_fanout > 0)
                .then_some(defaults.default_session_max_fanout),
        )
    } else {
        (None, None)
    };

    let session = SessionMetadata {
        id: session_id.clone(),
        name,
        status: if workspace_isolation == WorkspaceIsolationMode::Docker {
            SessionStatus::Provisioning
        } else {
            SessionStatus::Idle
        },
        model: resolved_model,
        provider: resolved_provider,
        assistant_id: agent_config.id.clone(),
        parent_session_id: agent_config.parent_session_id.clone(),
        lineage_id: agent_config.lineage_id.clone(),
        depth: agent_config.depth,
        max_depth: agent_config.max_depth.or(default_max_depth),
        max_fanout: agent_config.max_fanout.or(default_max_fanout),
        org_id: agent_config.org_id.clone(),
        org_name: agent_config.org_name.clone(),
        org_root_session_id: agent_config.org_root_session_id.clone(),
        is_bookmarked: false,
        created_at: now,
        updated_at: now,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        execution_mode: crate::execution_mode::ExecutionMode::Normal,
        workspace_override,
        workspace_isolation,
        docker_config,
        docker_container_name,
        docker_host_workspace_path,
        status_reason: None,
        forked_from: None,
    };

    // Persist to database using injected repository
    session_repo
        .upsert_session(&session)
        .await
        .map_err(|e| format!("Failed to create session: {}", e))?;

    let spawn_session_start_hooks = session.status != SessionStatus::Provisioning;

    if session.workspace_isolation == WorkspaceIsolationMode::Docker {
        runtime.spawn_docker_provisioning(session.clone(), false);
    }

    // SSOT: MCP activation is session → assistant → mcpServerIds (not create-request payload).
    let (tool_ids, mcp_server_ids) = crate::agent::resolve_session_mcp_bindings(&session).await?;

    // Load compact context if exists (SP17)
    let compact_context_record =
        crate::agent::lifecycle::load_compact_context_record(&session_id, "session creation")
            .await?;

    // Activate session in memory first so create can return without waiting on MCP.
    let mut active = active_sessions.write().await;
    if let Some(existing_session) = active.get_mut(&session_id) {
        log::info!(
            "Session {} already active during creation/update, updating metadata only",
            session_id
        );
        existing_session.metadata = session.clone();
        // Update compact context if it was loaded
        crate::agent::lifecycle::set_compact_context_if_loaded(
            existing_session,
            compact_context_record,
        )
        .await;
    } else {
        log::info!("Initializing new active state for session: {}", session_id);
        active.insert(
            session_id.clone(),
            AgentSession::new(session.clone(), context_registry, compact_context_record),
        );
    }
    drop(active);

    // MCP proxy creation must not block session create — same soft-wait model as resume/open.
    runtime
        .spawn_create_proxy(session_id.clone(), tool_ids, mcp_server_ids)
        .await;

    log::info!("Created agent session: {}", session_id);

    if spawn_session_start_hooks {
        // #2812 G3.1: the marker is registered here, synchronously, before
        // the spawn — the first prompt of this session waits on it.
        hook_dispatcher.spawn_session_start(
            &session_id,
            crate::services::hooks::SessionStartSource::Startup,
        );

        if let Some(parent_id) = session.parent_session_id.as_deref() {
            // #3007 / #3071 G1.3: shared fire-and-forget SubagentStart helper
            // (Docker children call the same helper after provisioning).
            crate::agent::workflow::finish::spawn_subagent_start_with_dispatcher(
                &session_id,
                parent_id,
                session.assistant_id.as_deref(),
                session.name.as_deref(),
                Arc::clone(&hook_dispatcher),
            );
        }
    }

    // Emit resource updated event for frontend cache revalidation
    crate::agent::tauri_events::emit_resource_updated(
        "session",
        "create",
        Some(session_id.clone()),
    );

    Ok(session)
}

#[cfg(test)]
pub(crate) mod test_support {
    use crate::agent::events::{AgentEvent, AgentEventDispatcher};
    use crate::agent::llm::types::{CompactStateEvent, CompletionCancelRequest};
    use crate::mcp::MCPServiceProxyManager;
    use crate::repositories::{
        SqliteAssistantRepository, SqliteCompactContextRepository, SqliteMCPServerRepository,
        SqlitePlanningRepository, SqliteSettingsRepository,
    };
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;
    use std::sync::Arc;

    pub(crate) struct IsolatedTestEnv {
        pub db: Arc<sea_orm::DatabaseConnection>,
        pub proxy_manager: Arc<MCPServiceProxyManager>,
        pub _temp_dir: tempfile::TempDir,
    }

    pub(crate) async fn init_isolated_test_env() -> IsolatedTestEnv {
        crate::lifecycle::database::register_sqlite_vec();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("run migrations");
        crate::state::set_settings_repository(SqliteSettingsRepository::new(db.clone()));
        crate::state::set_compact_context_repository(SqliteCompactContextRepository::new(
            db.clone(),
        ));
        crate::state::set_assistant_repository(SqliteAssistantRepository::new(db.clone()));
        crate::state::set_mcp_server_repository(SqliteMCPServerRepository::new(db.clone()));
        crate::state::set_planning_repository(SqlitePlanningRepository::new(db.clone()));

        let temp_dir = tempfile::tempdir().expect("create isolated test temp dir");
        let session_manager = Arc::new(
            crate::session::SessionManager::new_with_base_dir(temp_dir.path().to_path_buf())
                .expect("isolated SessionManager"),
        );
        let db = Arc::new(db);
        let proxy_manager = Arc::new(MCPServiceProxyManager::new(
            Arc::clone(&db),
            session_manager,
        ));
        IsolatedTestEnv {
            db,
            proxy_manager,
            _temp_dir: temp_dir,
        }
    }

    #[derive(Default)]
    pub(crate) struct RecordingEventDispatcher {
        pub agent_events: std::sync::Mutex<Vec<AgentEvent>>,
        pub compact_events: std::sync::Mutex<Vec<CompactStateEvent>>,
    }

    impl AgentEventDispatcher for RecordingEventDispatcher {
        fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
            self.agent_events
                .lock()
                .expect("agent_events lock")
                .push(event);
            Ok(())
        }

        fn emit_compact_state(&self, event: CompactStateEvent) -> Result<(), String> {
            self.compact_events
                .lock()
                .expect("compact_events lock")
                .push(event);
            Ok(())
        }

        fn emit_completion_cancel(&self, _request: CompletionCancelRequest) -> Result<(), String> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::init_isolated_test_env;
    use super::*;
    use crate::repositories::InMemorySessionRepository;
    use crate::services::hooks::execute::RecordingHookDispatcher;
    use crate::services::hooks::HookEvent;
    use std::time::Duration;

    #[derive(Default)]
    struct RecordingCreationRuntime {
        proxy_calls: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl SessionCreationRuntime for RecordingCreationRuntime {
        fn spawn_docker_provisioning(
            &self,
            _session: SessionMetadata,
            _delete_session_on_failure: bool,
        ) {
        }

        async fn spawn_create_proxy(
            &self,
            session_id: String,
            _tool_ids: Vec<String>,
            _mcp_server_ids: Vec<String>,
        ) {
            self.proxy_calls
                .lock()
                .expect("proxy_calls lock")
                .push(session_id);
        }
    }

    #[tokio::test]
    async fn create_session_dispatches_subagent_start_for_child_session() {
        let _lock = crate::state::lock_test_global_state().await;
        let _env = init_isolated_test_env().await;
        let session_repo: Arc<dyn SessionRepository> = Arc::new(InMemorySessionRepository::new());
        let active_sessions = Arc::new(RwLock::new(HashMap::new()));
        let context_registry = Arc::new(ContextRegistry::new());
        let child_id = format!("child-create-{}", uuid::Uuid::new_v4());

        let runtime = Arc::new(RecordingCreationRuntime::default());
        let hooks = Arc::new(RecordingHookDispatcher::new());

        let mut agent_config = crate::agent::AgentConfig::default();
        agent_config.parent_session_id = Some("parent-session".to_string());

        let created = create_session_with_runtime(
            CreateSessionCoreParams {
                session_repo,
                active_sessions,
                context_registry,
                session_id: child_id.clone(),
                name: Some("worker-child".to_string()),
                model: Some("gpt-4".to_string()),
                provider: Some("openai".to_string()),
                agent_config,
                workspace_isolation: WorkspaceIsolationMode::default(),
                docker_config: None,
            },
            runtime.clone(),
            hooks.clone(),
        )
        .await
        .expect("create_session should succeed without AppHandle");

        assert_eq!(created.id, child_id);
        assert_eq!(
            runtime.proxy_calls.lock().expect("proxy_calls").as_slice(),
            &[child_id.clone()]
        );
        let observed = hooks
            .wait_for(&child_id, HookEvent::SubagentStart, Duration::from_secs(2))
            .await
            .expect("SubagentStart must be dispatched for newly created child session");
        assert_eq!(observed.event, HookEvent::SubagentStart);
        assert_eq!(observed.session_id, child_id);
        assert_eq!(observed.matcher_value, "worker-child");
    }
}
