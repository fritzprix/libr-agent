use crate::agent::types::{CreateSessionRequest, CreateSessionResponse, SessionLineageMeta};
use crate::agent::AgentSessionManager;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::repositories::SessionMetadata;
use crate::session::get_session_manager;
use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;
use tokio::sync::RwLock as TokioRwLock;
use uuid::Uuid;

pub static SESSION_LINEAGE: OnceLock<TokioRwLock<HashMap<String, SessionLineageMeta>>> =
    OnceLock::new();

pub fn lineage_store() -> &'static TokioRwLock<HashMap<String, SessionLineageMeta>> {
    SESSION_LINEAGE.get_or_init(|| TokioRwLock::new(HashMap::new()))
}

/// Returns true if the path points to a restricted system directory that agents
/// should not be allowed to use as a workspace.
pub fn is_restricted_system_path(path: &std::path::Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let restricted_prefixes = [
            "c:\\windows",
            "c:\\program files",
            "c:\\program files (x86)",
            "c:\\programdata",
            "c:\\system volume information",
        ];

        let path_components: Vec<_> = path.components().collect();

        for prefix in restricted_prefixes.iter() {
            let prefix_components: Vec<_> = std::path::Path::new(prefix).components().collect();

            if path_components.len() < prefix_components.len() {
                continue;
            }

            let mut matches = true;
            for (p_comp, pref_comp) in path_components.iter().zip(prefix_components.iter()) {
                use std::path::{Component, Prefix};

                let p_disk = match p_comp {
                    Component::Prefix(p) => match p.kind() {
                        Prefix::Disk(d) | Prefix::VerbatimDisk(d) => Some(d.to_ascii_lowercase()),
                        _ => None,
                    },
                    _ => None,
                };

                let pref_disk = match pref_comp {
                    Component::Prefix(p) => match p.kind() {
                        Prefix::Disk(d) | Prefix::VerbatimDisk(d) => Some(d.to_ascii_lowercase()),
                        _ => None,
                    },
                    _ => None,
                };

                if let (Some(d1), Some(d2)) = (p_disk, pref_disk) {
                    if d1 != d2 {
                        matches = false;
                        break;
                    }
                } else {
                    let p_str = p_comp.as_os_str().to_string_lossy().to_lowercase();
                    let pref_str = pref_comp.as_os_str().to_string_lossy().to_lowercase();
                    if p_str != pref_str {
                        matches = false;
                        break;
                    }
                }
            }

            if matches {
                return true;
            }
        }

        false
    }

    #[cfg(not(target_os = "windows"))]
    {
        // macOS and Linux file systems are often case-insensitive by default or case-preserving.
        // Lowercase the path to ensure safe, case-insensitive component matching across Unix OSes.
        let path_lower = std::path::PathBuf::from(path.to_string_lossy().to_lowercase());

        let restricted_prefixes = [
            "/etc",
            "/sys",
            "/proc",
            "/dev",
            "/run",
            "/boot",
            "/bin",
            "/sbin",
            "/lib",
            "/lib64",
            "/usr/bin",
            "/usr/sbin",
            "/usr/lib",
            "/system",  // macOS
            "/library", // macOS
        ];

        for prefix in restricted_prefixes.iter() {
            if path_lower.starts_with(prefix) {
                return true;
            }
        }

        false
    }
}

pub struct AgentService;

pub fn normalize_explicit_org(
    org_id: Option<String>,
    org_name: Option<String>,
    org_root_session_id: Option<String>,
) -> Result<Option<(String, String, String)>, String> {
    match (org_id, org_name, org_root_session_id) {
        (None, None, None) => Ok(None),
        (Some(org_id), Some(org_name), Some(org_root_session_id)) => {
            let org_id = org_id.trim().to_string();
            let org_name = org_name.trim().to_string();
            let org_root_session_id = org_root_session_id.trim().to_string();

            if org_id.is_empty() || org_name.is_empty() || org_root_session_id.is_empty() {
                return Err(
                    "Explicit org metadata must include non-empty orgId, orgName, and orgRootSessionId together"
                        .to_string(),
                );
            }

            Ok(Some((org_id, org_name, org_root_session_id)))
        }
        _ => Err(
            "Explicit org metadata must include orgId, orgName, and orgRootSessionId together"
                .to_string(),
        ),
    }
}

pub fn resolve_child_session_model_provider(
    requested_model: Option<String>,
    requested_provider: Option<String>,
    parent_session: Option<&SessionMetadata>,
) -> Result<(Option<String>, Option<String>), String> {
    let inherited_model = parent_session.map(|session| session.model.clone());
    let inherited_provider = parent_session.map(|session| session.provider.clone());

    if parent_session.is_none() && (requested_model.is_some() != requested_provider.is_some()) {
        return Err(
            "Child session model/provider override must include both model and provider when no parent session is available for inheritance"
                .to_string(),
        );
    }

    Ok((
        requested_model.or(inherited_model),
        requested_provider.or(inherited_provider),
    ))
}

#[derive(Debug, Clone)]
pub struct SendSessionMessageResponse {
    pub message_id: String,
    pub status: String,
}

impl AgentService {
    /// Create a new session with assistant ID and initial request (Spawn Agent logic)
    pub async fn spawn_agent(
        manager: &AgentSessionManager,
        body: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, String> {
        Self::spawn_agent_with_source(manager, body, Some("agent_tool".to_string())).await
    }

    /// Create a new session with assistant ID and initial request, tagging the initial
    /// message with the provided source for transport-specific observability.
    pub async fn spawn_agent_with_source(
        manager: &AgentSessionManager,
        body: CreateSessionRequest,
        message_source: Option<String>,
    ) -> Result<CreateSessionResponse, String> {
        use crate::repositories::assistant_repository::AssistantRepository;
        use crate::repositories::session_repository::SessionRepository;

        // 1. Fetch Assistant to get config
        let assistant_repo = crate::state::get_assistant_repository();
        let assistant = assistant_repo
            .get_assistant(&body.assistant_id)
            .await
            .map_err(|e| format!("Failed to fetch assistant: {}", e))?
            .ok_or_else(|| format!("Assistant not found: {}", body.assistant_id))?;

        // 2. Build AgentConfig from Assistant
        let mut agent_config = crate::agent::AgentConfig::from_json(&assistant.config)
            .map_err(|e| format!("Invalid assistant configuration: {}", e))?;

        agent_config.id = Some(assistant.id.clone());
        agent_config.name = assistant.name.clone();
        let assistant_id = agent_config.id.clone();

        // 3. Resolve lineage metadata
        let parent_session_id = body.parent_session_id.clone();
        let mut parent_session = None;
        let requested_max_depth = body.max_depth;
        let requested_max_fanout = body.max_fanout;

        if let Some(ref parent_id) = parent_session_id {
            parent_session = manager.get_session(parent_id).await?;
            if parent_session.is_none() {
                return Err(format!("Parent session not found: {}", parent_id));
            }
        }

        let session_id = format!("session-{}", Uuid::new_v4());
        let session_name = body.name.clone().or_else(|| {
            let short_id = &session_id[session_id.len().saturating_sub(6)..];
            let preview: String = body.request.chars().take(40).collect();
            let trimmed = preview.trim();
            if trimmed.is_empty() {
                Some(format!("{} #{}", assistant.name, short_id))
            } else {
                Some(format!("{}: {} #{}", assistant.name, trimmed, short_id))
            }
        });

        let explicit_org = normalize_explicit_org(
            body.org_id.clone(),
            body.org_name.clone(),
            body.org_root_session_id.clone(),
        )?;

        let (resolved_model, resolved_provider) = resolve_child_session_model_provider(
            body.model.clone(),
            body.provider.clone(),
            parent_session.as_ref(),
        )?;

        let lineage_meta = if let Some(parent_id) = parent_session_id.clone() {
            let store = lineage_store().read().await;
            if let Some(parent_meta) = store.get(&parent_id) {
                let effective_max_depth = requested_max_depth.or(parent_meta.max_depth);
                let effective_max_fanout = requested_max_fanout.or(parent_meta.max_fanout);
                let next_depth = parent_meta.depth.saturating_add(1);

                let session_repo = crate::state::get_session_repository();
                let child_count = session_repo
                    .get_child_session_ids(&parent_id)
                    .await
                    .map(|children| children.len())
                    .unwrap_or(0);

                if let Some(limit) = effective_max_depth {
                    if next_depth > limit {
                        return Err(format!(
                            "Depth limit exceeded: next depth {} > maxDepth {}",
                            next_depth, limit
                        ));
                    }
                }

                if let Some(limit) = effective_max_fanout {
                    if child_count >= limit as usize {
                        return Err(format!(
                            "Fanout limit exceeded: parent has {} children, maxFanout is {}",
                            child_count, limit
                        ));
                    }
                }

                SessionLineageMeta {
                    parent_session_id: Some(parent_id),
                    lineage_id: parent_meta.lineage_id.clone(),
                    depth: next_depth,
                    max_depth: effective_max_depth,
                    max_fanout: effective_max_fanout,
                    org_id: explicit_org.as_ref().map(|(org_id, _, _)| org_id.clone()),
                    org_name: explicit_org
                        .as_ref()
                        .map(|(_, org_name, _)| org_name.clone()),
                    org_root_session_id: explicit_org
                        .as_ref()
                        .map(|(_, _, org_root_session_id)| org_root_session_id.clone()),
                }
            } else {
                drop(store);
                let session_repo = crate::state::get_session_repository();
                let parent_meta = session_repo.get_session(&parent_id).await.ok().flatten();

                let parent_depth = parent_meta.as_ref().and_then(|m| m.depth).unwrap_or(0);
                let parent_lineage_id = parent_meta
                    .as_ref()
                    .and_then(|m| m.lineage_id.clone())
                    .unwrap_or_else(|| parent_id.clone());
                let inherited_max_depth = parent_meta.as_ref().and_then(|m| m.max_depth);
                let inherited_max_fanout = parent_meta.as_ref().and_then(|m| m.max_fanout);

                let effective_max_depth = requested_max_depth.or(inherited_max_depth);
                let effective_max_fanout = requested_max_fanout.or(inherited_max_fanout);
                let next_depth = parent_depth.saturating_add(1);

                let child_count = session_repo
                    .get_child_session_ids(&parent_id)
                    .await
                    .map(|children| children.len())
                    .unwrap_or(0);

                if let Some(limit) = effective_max_depth {
                    if next_depth > limit {
                        return Err(format!(
                            "Depth limit exceeded: next depth {} > maxDepth {}",
                            next_depth, limit
                        ));
                    }
                }

                if let Some(limit) = effective_max_fanout {
                    if child_count >= limit as usize {
                        return Err(format!(
                            "Fanout limit exceeded: parent has {} children, maxFanout is {}",
                            child_count, limit
                        ));
                    }
                }

                SessionLineageMeta {
                    parent_session_id: Some(parent_id),
                    lineage_id: parent_lineage_id,
                    depth: next_depth,
                    max_depth: effective_max_depth,
                    max_fanout: effective_max_fanout,
                    org_id: explicit_org.as_ref().map(|(org_id, _, _)| org_id.clone()),
                    org_name: explicit_org
                        .as_ref()
                        .map(|(_, org_name, _)| org_name.clone()),
                    org_root_session_id: explicit_org
                        .as_ref()
                        .map(|(_, _, org_root_session_id)| org_root_session_id.clone()),
                }
            }
        } else {
            SessionLineageMeta {
                parent_session_id: None,
                lineage_id: session_id.clone(),
                depth: 0,
                max_depth: requested_max_depth,
                max_fanout: requested_max_fanout,
                org_id: explicit_org.as_ref().map(|(org_id, _, _)| org_id.clone()),
                org_name: explicit_org
                    .as_ref()
                    .map(|(_, org_name, _)| org_name.clone()),
                org_root_session_id: explicit_org
                    .as_ref()
                    .map(|(_, _, org_root_session_id)| org_root_session_id.clone()),
            }
        };

        agent_config.parent_session_id = lineage_meta.parent_session_id.clone();
        agent_config.lineage_id = Some(lineage_meta.lineage_id.clone());
        agent_config.depth = Some(lineage_meta.depth);
        agent_config.max_depth = lineage_meta.max_depth;
        agent_config.max_fanout = lineage_meta.max_fanout;
        agent_config.org_id = lineage_meta.org_id.clone();
        agent_config.org_name = lineage_meta.org_name.clone();
        agent_config.org_root_session_id = lineage_meta.org_root_session_id.clone();

        if let Some(path_str) = body.workspace_path {
            Self::validate_and_register_workspace_override(&path_str, &session_id).await?;
        }

        let session = manager
            .create_session(
                session_id.clone(),
                session_name,
                resolved_model,
                resolved_provider,
                agent_config,
            )
            .await?;

        lineage_store()
            .write()
            .await
            .insert(session_id.clone(), lineage_meta.clone());

        if let Some(parent_id) = lineage_meta.parent_session_id.as_deref() {
            if manager.get_yolo_mode(parent_id).await {
                let _ = manager.set_yolo_mode(&session_id, true).await;
            }
        }

        let message = Message {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.clone(),
            role: "user".to_string(),
            content: vec![MCPContent::Text {
                text: body.request,
                is_error: None,
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id,
            usage: None,
            attachments: None,
            tool_use: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
            source: message_source,
            error: None,
            metadata: None,
        };

        if let Err(e) = manager.start_workflow(session_id.clone(), message).await {
            log::error!("Failed to start initial workflow for spawned agent: {}", e);

            lineage_store().write().await.remove(&session_id);

            if let Err(cleanup_err) = manager.delete_session(session_id.clone()).await {
                log::error!(
                    "Failed to clean up spawned session {} after workflow start error: {}",
                    session_id,
                    cleanup_err
                );
            }

            return Err(format!("Failed to start initial workflow: {}", e));
        }

        Ok(CreateSessionResponse {
            id: session.id,
            name: session.name,
            status: format!("{:?}", crate::repositories::SessionStatus::Busy),
            parent_session_id: lineage_meta.parent_session_id,
            lineage_id: lineage_meta.lineage_id,
            depth: lineage_meta.depth,
            max_depth: lineage_meta.max_depth,
            max_fanout: lineage_meta.max_fanout,
            org_id: lineage_meta.org_id,
            org_name: lineage_meta.org_name,
            org_root_session_id: lineage_meta.org_root_session_id,
        })
    }

    /// Validates a workspace override path and registers it for the given session.
    ///
    /// The path must be absolute, must exist, and must be a directory.
    pub async fn validate_and_register_workspace_override(
        path_str: &str,
        session_id: &str,
    ) -> Result<(), String> {
        let Ok(session_manager) = get_session_manager() else {
            log::warn!("Failed to get session manager for workspace override");
            return Ok(());
        };
        let path = std::path::PathBuf::from(path_str);
        if !path.is_absolute() {
            return Err("Workspace path must be absolute".to_string());
        }

        // Security check: prevent using restricted system directories
        if is_restricted_system_path(&path) {
            return Err(format!(
                "Workspace path '{}' is a restricted system directory and cannot be used as an agent workspace",
                path_str
            ));
        }

        match tokio::fs::metadata(&path).await {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    return Err("Workspace path must be a directory".to_string());
                }
            }
            Err(err) => {
                return Err(format!("Workspace path is not accessible: {}", err));
            }
        }
        session_manager
            .register_session_override(session_id, path)
            .await
    }

    fn extract_assistant_id_from_config(
        session_id: &str,
        agent_config: Option<&String>,
    ) -> Option<String> {
        let config_str = agent_config?;
        let config: serde_json::Value = match serde_json::from_str(config_str) {
            Ok(value) => value,
            Err(error) => {
                log::warn!(
                    "Invalid session.agent_config JSON for session {} (assistant_id will be None): {}",
                    session_id,
                    error
                );
                return None;
            }
        };

        let assistant_id_value = config
            .get("assistant_id")
            .or_else(|| config.get("assistantId"))
            .or_else(|| config.get("id"));

        match assistant_id_value {
            Some(value) => match value.as_str() {
                Some(assistant_id) => Some(assistant_id.to_string()),
                None => {
                    log::warn!(
                        "session.agent_config assistant id field is not a string for session {} (assistant_id will be None)",
                        session_id
                    );
                    None
                }
            },
            None => {
                log::warn!(
                    "No assistant id field found in session.agent_config for session {} (expected one of: assistant_id, assistantId, id)",
                    session_id
                );
                None
            }
        }
    }

    pub async fn send_message_to_session(
        manager: &AgentSessionManager,
        session_id: &str,
        content: String,
        source: Option<String>,
    ) -> Result<SendSessionMessageResponse, String> {
        let persisted_session = manager
            .get_session(session_id)
            .await?
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        let is_active = {
            let active_sessions = manager.active_sessions_arc();
            let active = active_sessions.read().await;
            active.contains_key(session_id)
        };

        let session = if !is_active {
            log::info!(
                "Auto-resuming inactive session before send_message_to_session: {}",
                session_id
            );
            let resumed_session = manager.resume_session(session_id).await?;
            manager.init_session_with_messages(session_id).await?;
            resumed_session
        } else {
            persisted_session
        };

        let message_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        let message = Message {
            id: message_id.clone(),
            session_id: session_id.to_string(),
            role: "user".to_string(),
            content: vec![MCPContent::Text {
                text: content,
                is_error: None,
            }],
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: Self::extract_assistant_id_from_config(
                session_id,
                session.agent_config.as_ref(),
            ),
            usage: None,
            attachments: None,
            tool_use: None,
            created_at: now,
            updated_at: now,
            source,
            error: None,
            metadata: None,
        };

        let triggered = manager
            .inject_messages(session_id.to_string(), vec![message])
            .await?;
        let status = if triggered { "processed" } else { "queued" };

        Ok(SendSessionMessageResponse {
            message_id,
            status: status.to_string(),
        })
    }

    /// Create a new agent session
    pub async fn create_session(
        manager: &AgentSessionManager,
        request: crate::commands::agent_commands::CreateAgentSessionRequest,
    ) -> Result<crate::repositories::SessionMetadata, String> {
        use crate::repositories::in_memory_session_repository::InMemorySessionRepository;
        use crate::repositories::SessionRepository;
        use std::sync::Arc;

        // Handle workspace override if path is provided
        if let Some(path_str) = &request.workspace_path {
            Self::validate_and_register_workspace_override(path_str, &request.session_id).await?;
        }
        let session_repo: Arc<dyn SessionRepository> = if request.is_ephemeral {
            log::info!(
                "Creating ephemeral session (in-memory only): {}",
                request.session_id
            );
            Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>
        } else {
            log::info!(
                "Creating persistent session (DB-backed): {}",
                request.session_id
            );
            Arc::new(crate::state::get_session_repository().clone())
        };

        manager
            .create_session_with_repo(
                session_repo,
                request.session_id,
                request.name,
                request.model,
                request.provider,
                request.agent_config,
            )
            .await
    }

    /// Create a new session and IMMEDIATELY start the workflow with an initial message
    /// This is used for "Draft Mode" where the session is created only when the first message is sent.
    pub async fn create_session_with_initial_message(
        manager: &AgentSessionManager,
        request: crate::commands::agent_commands::CreateAgentSessionWithMessageRequest,
    ) -> Result<crate::commands::agent_commands::AgentResponse, String> {
        // Handle workspace override if path is provided
        if let Some(path_str) = &request.workspace_path {
            Self::validate_and_register_workspace_override(path_str, &request.session_id).await?;
        }

        // 1. Create the session first (persistent by default)
        // We use the default persistent repository here
        let session_repo = std::sync::Arc::new(crate::state::get_session_repository().clone());

        manager
            .create_session_with_repo(
                session_repo,
                request.session_id.clone(),
                request.name,
                request.model,
                request.provider,
                request.agent_config,
            )
            .await?;

        // 2. Start the workflow with the initial message
        manager
            .start_workflow(request.session_id.clone(), request.message)
            .await
            .map(|_| crate::commands::agent_commands::AgentResponse {
                success: true,
                message: "Session created and workflow started".to_string(),
                data: None,
            })
    }

    /// Call a builtin tool directly via proxy_manager (for testing and direct execution)
    /// Returns the unwrapped MCPResult (not the full MCPResponse wrapper)
    pub async fn call_builtin_tool(
        session_id: String,
        tool_name: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use crate::mcp::types::MCPResponseResult;
        use crate::state::get_mcp_service_proxy_manager;

        let proxy_manager = get_mcp_service_proxy_manager();

        let response = proxy_manager
            .call_tool(&session_id, &tool_name, args)
            .await?;

        // Handle errors from tool execution
        if let Some(error) = response.error {
            return Err(format!("Tool execution error: {}", error.message));
        }

        // Extract result from MCPResponse
        let result = response
            .result
            .ok_or_else(|| "Tool execution returned no result or error".to_string())?;

        // Unwrap MCPResult from MCPResponseResult::ToolCall variant
        // This matches the TypeScript expectation of receiving MCPResult directly
        match result {
            MCPResponseResult::ToolCall(mcp_result) => {
                // Serialize MCPResult (with camelCase field names matching TypeScript interface)
                serde_json::to_value(mcp_result)
                    .map_err(|e| format!("Failed to serialize MCPResult: {}", e))
            }
            _ => Err(format!(
                "Unexpected response type for builtin tool '{}': expected ToolCall variant",
                tool_name
            )),
        }
    }

    /// Save an attachment to the session-scoped attachment store through an internal UI-only API.
    ///
    /// Routes through the session-bound `MCPServiceProxy` so that the same
    /// `AttachmentsServer` instance used by the agent is updated — keeping
    /// `recent_uploads` tracking and the BM25 search index in sync.
    pub async fn add_attachment(
        session_id: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use crate::state::get_mcp_service_proxy_manager;

        let proxy_manager = get_mcp_service_proxy_manager();

        // Attempt to route through the live session proxy (normal case).
        if let Some(proxy) = proxy_manager.get_proxy(&session_id).await {
            let result = proxy
                .call_builtin_internal("attachments", "add", args)
                .await?;
            return serde_json::to_value(result)
                .map_err(|e| format!("Failed to serialize MCPResult: {}", e));
        }

        // Fallback: session proxy not yet created (e.g. file attached before first
        // message sends). Create a temporary AttachmentsServer backed by the
        // global DB connection so the data is persisted correctly.
        log::debug!(
            "No proxy for session '{}'; falling back to direct AttachmentsServer for add_attachment",
            session_id
        );
        use crate::mcp::builtin::attachments::AttachmentsServer;
        use crate::state::get_database_connection;
        use std::sync::Arc;

        let session_manager =
            get_session_manager().map_err(|e| format!("SessionManager not initialized: {}", e))?;
        let db = get_database_connection();
        let server = AttachmentsServer::new_with_db(
            session_id.clone(),
            Arc::new(session_manager.clone()),
            db.clone(),
        )
        .await?;
        let result = server.add_attachment_internal(args, &session_id).await?;
        serde_json::to_value(result).map_err(|e| format!("Failed to serialize MCPResult: {}", e))
    }

    /// Delete an attachment from the session-scoped attachment store through an internal UI-only API.
    ///
    /// Routes through the session-bound `MCPServiceProxy` so that the same
    /// `AttachmentsServer` instance used by the agent is updated — keeping all
    /// in-memory state consistent.
    pub async fn delete_attachment(
        session_id: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use crate::state::get_mcp_service_proxy_manager;

        let proxy_manager = get_mcp_service_proxy_manager();

        if let Some(proxy) = proxy_manager.get_proxy(&session_id).await {
            let result = proxy
                .call_builtin_internal("attachments", "delete", args)
                .await?;
            return serde_json::to_value(result)
                .map_err(|e| format!("Failed to serialize MCPResult: {}", e));
        }

        log::debug!(
            "No proxy for session '{}'; falling back to direct AttachmentsServer for delete_attachment",
            session_id
        );
        use crate::mcp::builtin::attachments::AttachmentsServer;
        use crate::state::get_database_connection;
        use std::sync::Arc;

        let session_manager =
            get_session_manager().map_err(|e| format!("SessionManager not initialized: {}", e))?;
        let db = get_database_connection();
        let server = AttachmentsServer::new_with_db(
            session_id.clone(),
            Arc::new(session_manager.clone()),
            db.clone(),
        )
        .await?;
        let result = server.delete_attachment_internal(args, &session_id).await?;
        serde_json::to_value(result).map_err(|e| format!("Failed to serialize MCPResult: {}", e))
    }

    /// Get service contexts for a session
    pub async fn get_service_contexts(
        session_id: String,
    ) -> Result<std::collections::HashMap<String, crate::mcp::types::ServiceContext>, String> {
        use crate::state::get_mcp_service_proxy_manager;

        let proxy_manager = get_mcp_service_proxy_manager();

        let proxy = proxy_manager
            .get_proxy(&session_id)
            .await
            .ok_or_else(|| format!("No proxy found for session: {}", session_id))?;

        Ok(proxy.get_service_contexts(None).await)
    }

    /// Clear all agent sessions (used for "Clear All Sessions" feature)
    pub async fn clear_all_sessions(manager: &AgentSessionManager) -> Result<usize, String> {
        // 1. Get all sessions
        let sessions = manager.get_all_sessions().await?;
        let count = sessions.len();

        // 2. Delete each session
        for session in sessions {
            if let Err(e) = manager.delete_session(session.id.clone()).await {
                log::error!(
                    "Failed to delete session {} during clear all: {}",
                    session.id,
                    e
                );
            }
        }

        // 3. Cleanup dangled workspaces (FS only)
        // Clean up dangling workspace directories (exist on disk but no longer have DB sessions)
        if let Ok(session_manager) = crate::session::get_session_manager() {
            if let Ok(fs_sessions) = session_manager.list_sessions() {
                let mut dangled_count = 0;
                for session_id in fs_sessions {
                    // Skip 'default' workspace to preserve the fallback environment
                    if session_id != "default" {
                        // Lazy load workspace into pool, then attempt removal
                        let _ = session_manager.get_session_workspace_dir_by_id(&session_id);
                        if let Err(e) = session_manager.remove_session(&session_id).await {
                            log::debug!(
                                "Attempted to remove potential dangled session {}: {}",
                                session_id,
                                e
                            );
                        } else {
                            dangled_count += 1;
                        }
                    }
                }
                if dangled_count > 0 {
                    log::info!(
                        "Cleaned up {} dangled/residual workspace directories",
                        dangled_count
                    );
                }
            }
        }

        Ok(count)
    }

    /// Factory reset the agent system (used for "Reset All Data & Settings" feature)
    /// Deletes all sessions, assistants, playbooks, mcp servers, and logs.
    pub async fn factory_reset(manager: &AgentSessionManager) -> Result<(), String> {
        use crate::repositories::mcp_server_repository::MCPServerRepository;
        use crate::repositories::AssistantRepository;
        use crate::repositories::PlaybookRepository;
        use crate::state::get_mcp_server_repository;

        // 1. Clear all sessions first
        Self::clear_all_sessions(manager).await?;

        // 2. Delete all Playbooks (must happen before assistants due to foreign key)
        let playbook_repo = crate::state::get_playbook_repository();
        let all_playbooks = playbook_repo
            .list_playbooks(
                None,
                crate::repositories::PaginationParams {
                    page: 1,
                    page_size: 100000,
                },
            )
            .await
            .map_err(|e| format!("Failed to list playbooks: {}", e))?;

        for playbook in all_playbooks.items {
            playbook_repo
                .delete_playbook(&playbook.id, &playbook.assistant_id)
                .await
                .map_err(|e| format!("Failed to delete playbook {}: {}", playbook.id, e))?;
        }

        // 3. Delete all Assistants
        let assistant_repo = crate::state::get_assistant_repository();
        let all_assistants = assistant_repo
            .list_assistants()
            .await
            .map_err(|e| format!("Failed to list assistants: {}", e))?;

        for assistant in all_assistants {
            assistant_repo
                .delete_assistant(&assistant.id)
                .await
                .map_err(|e| format!("Failed to delete assistant {}: {}", assistant.id, e))?;
        }

        // 4. Delete all MCP Servers
        let mcp_repo = get_mcp_server_repository();
        let servers = mcp_repo
            .list()
            .await
            .map_err(|e| format!("Failed to list MCP servers: {}", e))?;
        for server in servers {
            mcp_repo
                .delete(&server.name)
                .await
                .map_err(|e| format!("Failed to delete MCP server {}: {}", server.name, e))?;
        }

        // 5. Restore default assistants so the app is not empty
        if let Err(e) = crate::services::assistant_init::ensure_default_assistants().await {
            return Err(format!(
                "Factory reset failed to restore default assistants: {}",
                e
            ));
        }

        // 6. Clear application logs
        // We do this last to preserve logging of the reset process as much as possible
        if let Ok(session_mgr) = get_session_manager() {
            let log_dir = session_mgr.get_logs_dir();
            if log_dir.exists() {
                if let Ok(entries) = fs::read_dir(&log_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                                if filename.ends_with(".log") || filename.ends_with(".log.bak") {
                                    if let Err(e) = fs::remove_file(&path) {
                                        log::warn!("Failed to delete log file {:?}: {}", path, e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
