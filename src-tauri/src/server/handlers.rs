use crate::agent::AgentSessionManager;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::session_repository::SessionRepository;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::RwLock as TokioRwLock;
use uuid::Uuid;
use warp::{http::StatusCode, Rejection, Reply};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub name: Option<String>,
    pub assistant_id: String, // Replaces agent_config
    pub workspace_path: Option<String>,
    pub request: String,
    pub parent_session_id: Option<String>,
    pub max_depth: Option<u32>,
    pub max_fanout: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionResponse {
    pub id: String,
    pub name: Option<String>,
    pub status: String,
    pub parent_session_id: Option<String>,
    pub lineage_id: String,
    pub depth: u32,
    pub max_depth: Option<u32>,
    pub max_fanout: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub id: String,
    pub status: String, // "processed" or "queued"
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct GetMessagesQuery {
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLineageMeta {
    pub parent_session_id: Option<String>,
    pub lineage_id: String,
    pub depth: u32,
    pub max_depth: Option<u32>,
    pub max_fanout: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildSessionsResponse {
    pub parent_session_id: String,
    pub count: usize,
    pub children: Vec<String>,
}

static SESSION_LINEAGE: OnceLock<TokioRwLock<HashMap<String, SessionLineageMeta>>> =
    OnceLock::new();

fn lineage_store() -> &'static TokioRwLock<HashMap<String, SessionLineageMeta>> {
    SESSION_LINEAGE.get_or_init(|| TokioRwLock::new(HashMap::new()))
}

/// Returns true if the path points to a restricted system directory that agents
/// should not be allowed to use as a workspace.
fn is_restricted_system_path(path: &std::path::Path) -> bool {
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

pub async fn create_session(
    manager: Arc<AgentSessionManager>,
    body: CreateSessionRequest,
) -> Result<impl Reply, Rejection> {
    use crate::repositories::assistant_repository::AssistantRepository;

    // 1. Fetch Assistant to get config
    let assistant_repo = crate::state::get_assistant_repository();
    let assistant = match assistant_repo.get_assistant(&body.assistant_id).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Assistant not found: {}", body.assistant_id),
                }),
                StatusCode::NOT_FOUND,
            ))
        }
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to fetch assistant: {}", e),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    };

    // 2. Build AgentConfig from Assistant
    // The assistant.config is a JSON string. We need to parse it and ensure ID/Name are set.
    let mut agent_config = match crate::agent::AgentConfig::from_json(&assistant.config) {
        Ok(c) => c,
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Invalid assistant configuration: {}", e),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    };

    // Ensure ID and Name match the assistant (critical for tracking)
    agent_config.id = Some(assistant.id.clone());
    agent_config.name = assistant.name.clone();
    let assistant_id = agent_config.id.clone();

    // 3. Resolve lineage metadata (parent/reference contract)
    let parent_session_id = body.parent_session_id.clone();
    let requested_max_depth = body.max_depth;
    let requested_max_fanout = body.max_fanout;

    if let Some(ref parent_id) = parent_session_id {
        match manager.get_session(parent_id).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&ErrorResponse {
                        error: format!("Parent session not found: {}", parent_id),
                    }),
                    StatusCode::BAD_REQUEST,
                ))
            }
            Err(e) => {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&ErrorResponse {
                        error: format!("Failed to validate parent session: {}", e),
                    }),
                    StatusCode::INTERNAL_SERVER_ERROR,
                ))
            }
        }
    }

    // 4. Create Session
    let session_id = format!("session-{}", Uuid::new_v4());

    // Use provided name, or generate a descriptive default from the request preview
    let session_name = body.name.or_else(|| {
        let short_id = &session_id[session_id.len().saturating_sub(6)..];
        let preview: String = body.request.chars().take(40).collect();
        let trimmed = preview.trim();
        if trimmed.is_empty() {
            Some(format!("{} #{}", assistant.name, short_id))
        } else {
            Some(format!("{}: {} #{}", assistant.name, trimmed, short_id))
        }
    });

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
                .unwrap_or_else(|_| {
                    store
                        .values()
                        .filter(|meta| {
                            meta.parent_session_id.as_deref() == Some(parent_id.as_str())
                        })
                        .count()
                });

            if let Some(limit) = effective_max_depth {
                if next_depth > limit {
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&ErrorResponse {
                            error: format!(
                                "Depth limit exceeded: next depth {} is greater than maxDepth {}",
                                next_depth, limit
                            ),
                        }),
                        StatusCode::BAD_REQUEST,
                    ));
                }
            }

            if let Some(limit) = effective_max_fanout {
                if child_count >= limit as usize {
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&ErrorResponse {
                            error: format!(
                                "Fanout limit exceeded: parent already has {} children, maxFanout is {}",
                                child_count, limit
                            ),
                        }),
                        StatusCode::BAD_REQUEST,
                    ));
                }
            }

            SessionLineageMeta {
                parent_session_id: Some(parent_id),
                lineage_id: parent_meta.lineage_id.clone(),
                depth: next_depth,
                max_depth: effective_max_depth,
                max_fanout: effective_max_fanout,
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
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&ErrorResponse {
                            error: format!(
                                "Depth limit exceeded: next depth {} is greater than maxDepth {}",
                                next_depth, limit
                            ),
                        }),
                        StatusCode::BAD_REQUEST,
                    ));
                }
            }

            if let Some(limit) = effective_max_fanout {
                if child_count >= limit as usize {
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&ErrorResponse {
                            error: format!(
                                "Fanout limit exceeded: parent already has {} children, maxFanout is {}",
                                child_count, limit
                            ),
                        }),
                        StatusCode::BAD_REQUEST,
                    ));
                }
            }

            SessionLineageMeta {
                parent_session_id: Some(parent_id),
                lineage_id: parent_lineage_id,
                depth: next_depth,
                max_depth: effective_max_depth,
                max_fanout: effective_max_fanout,
            }
        }
    } else {
        SessionLineageMeta {
            parent_session_id: None,
            lineage_id: session_id.clone(),
            depth: 0,
            max_depth: requested_max_depth,
            max_fanout: requested_max_fanout,
        }
    };

    // Persist lineage contract into session agent_config so frontend/session list can render hierarchy.
    agent_config.parent_session_id = lineage_meta.parent_session_id.clone();
    agent_config.lineage_id = Some(lineage_meta.lineage_id.clone());
    agent_config.depth = Some(lineage_meta.depth);
    agent_config.max_depth = lineage_meta.max_depth;
    agent_config.max_fanout = lineage_meta.max_fanout;

    // Register override if path provided
    if let Some(path_str) = body.workspace_path {
        let path = std::path::PathBuf::from(&path_str);
        if !path.is_absolute() {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: "Workspace path must be absolute".to_string(),
                }),
                StatusCode::BAD_REQUEST,
            ));
        }
        if is_restricted_system_path(&path) {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!(
                        "Workspace path '{}' is a restricted system directory and cannot be used as an agent workspace",
                        path_str
                    ),
                }),
                StatusCode::BAD_REQUEST,
            ));
        }
        if let Ok(session_manager) = crate::session::get_session_manager() {
            if let Err(e) = session_manager
                .register_session_override(&session_id, path)
                .await
            {
                log::warn!("Failed to register workspace override: {}", e);
            }
        }
    }

    match manager
        .create_session(
            session_id.clone(),
            session_name,
            None, // model resolved internally
            None, // provider resolved internally
            agent_config,
        )
        .await
    {
        Ok(mut session) => {
            lineage_store()
                .write()
                .await
                .insert(session_id.clone(), lineage_meta.clone());

            // Check for initial message to trigger workflow
            let content = body.request;
            let message_id = Uuid::new_v4().to_string();
            let now = chrono::Utc::now().timestamp_millis();

            let message = Message {
                id: message_id.clone(),
                session_id: session_id.clone(),
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
                assistant_id,
                attachments: None,
                tool_use: None,
                created_at: now,
                updated_at: now,
                source: Some("api".to_string()),
                error: None,
                metadata: None,
            };

            log::info!(
                "Initial request provided. Starting workflow for session {}.",
                session_id
            );

            if let Err(e) = manager.start_workflow(session_id.clone(), message).await {
                log::error!("Failed to start initial workflow: {}", e);
            } else {
                session.status = crate::repositories::SessionStatus::Busy;
            }

            let response = CreateSessionResponse {
                id: session.id,
                name: session.name,
                status: format!("{:?}", session.status),
                parent_session_id: lineage_meta.parent_session_id,
                lineage_id: lineage_meta.lineage_id,
                depth: lineage_meta.depth,
                max_depth: lineage_meta.max_depth,
                max_fanout: lineage_meta.max_fanout,
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::CREATED,
            ))
        }
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error: e }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub async fn get_session(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    match manager.get_session(&id).await {
        Ok(Some(session)) => Ok(warp::reply::with_status(
            warp::reply::json(&session),
            StatusCode::OK,
        )),
        Ok(None) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: "Session not found".to_string(),
            }),
            StatusCode::NOT_FOUND,
        )),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error: e }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub async fn get_messages(
    id: String,
    query: GetMessagesQuery,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    // Validate session existence before fetching messages.
    // Without this check the message repo silently returns an empty list
    // for any unknown session ID, masking bugs.
    match manager.get_session(&id).await {
        Ok(None) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Session not found: {}", id),
                }),
                StatusCode::NOT_FOUND,
            ))
        }
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to validate session: {}", e),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
        Ok(Some(_)) => {} // Session exists, proceed
    }

    let repo = crate::state::get_message_repository();
    let limit = query.limit.unwrap_or(50);

    match repo.get_messages_by_session(&id, limit).await {
        Ok(messages) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "messages": messages })),
            StatusCode::OK,
        )),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to fetch messages: {}", e),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub async fn send_message(
    id: String,
    manager: Arc<AgentSessionManager>,
    body: SendMessageRequest,
) -> Result<impl Reply, Rejection> {
    // 1. Check session existence and status
    let session_opt = match manager.get_session(&id).await {
        Ok(s) => s,
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse { error: e }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    };

    if session_opt.is_none() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: "Session not found".to_string(),
            }),
            StatusCode::NOT_FOUND,
        ));
    }

    let session = session_opt.unwrap();
    let is_busy = matches!(session.status, crate::repositories::SessionStatus::Busy);

    // NOTE: Session.agent_config may contain partial/legacy JSON in some code paths (e.g., tests).
    // Avoid strict AgentConfig deserialization here; extract the assistant ID from common fields.
    let assistant_id = session.agent_config.as_ref().and_then(|config_str| {
        let config: serde_json::Value = match serde_json::from_str(config_str) {
            Ok(v) => v,
            Err(e) => {
                log::warn!(
                    "Invalid session.agent_config JSON for session {} (assistant_id will be None): {}",
                    id,
                    e
                );
                return None;
            }
        };

        let assistant_id_value = config
            .get("assistant_id")
            .or_else(|| config.get("assistantId"))
            .or_else(|| config.get("id"));

        match assistant_id_value {
            Some(v) => match v.as_str() {
                Some(s) => Some(s.to_string()),
                None => {
                    log::warn!(
                        "session.agent_config assistant id field is not a string for session {} (assistant_id will be None)",
                        id
                    );
                    None
                }
            },
            None => {
                log::warn!(
                    "No assistant id field found in session.agent_config for session {} (expected one of: assistant_id, assistantId, id)",
                    id
                );
                None
            }
        }
    });

    // 2. Create Message object
    let message_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let message = Message {
        id: message_id.clone(),
        session_id: id.clone(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: body.content,
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: None,
        thinking: None,
        thinking_signature: None,
        assistant_id,
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: body.source.or_else(|| Some("api".to_string())),
        error: None,
        metadata: None,
    };

    // 3. Handle based on status
    if is_busy {
        // Queue the message
        log::info!("Session {} is busy. Queuing message {}.", id, message_id);
        if let Err(e) = manager
            .inject_messages(id.clone(), vec![message], false)
            .await
        {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to queue message: {}", e),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }

        Ok(warp::reply::with_status(
            warp::reply::json(&SendMessageResponse {
                id: message_id,
                status: "queued".to_string(),
            }),
            StatusCode::OK, // 200 OK, but status indicates queued
        ))
    } else {
        // Trigger workflow
        log::info!(
            "Session {} is idle. Starting workflow with message {}.",
            id,
            message_id
        );
        if let Err(e) = manager.start_workflow(id.clone(), message).await {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to start workflow: {}", e),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }

        Ok(warp::reply::with_status(
            warp::reply::json(&SendMessageResponse {
                id: message_id,
                status: "processed".to_string(),
            }),
            StatusCode::OK,
        ))
    }
}

/// POST /api/sessions/:id/resume
/// Loads a paused session into memory and resumes workflow without adding any new message.
/// Used by awaitAgent crash recovery to avoid injecting garbage user messages.
pub async fn resume_session_workflow(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    // Step 1: Load the session into active_sessions and recreate the MCP proxy
    if let Err(e) = manager.resume_session(&id).await {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to resume session: {}", e),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        ));
    }

    // Step 2: Resume the workflow from existing messages (no new message injection)
    if let Err(e) = manager.resume_workflow(id.clone()).await {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to resume workflow: {}", e),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "resumed" })),
        StatusCode::OK,
    ))
}

pub async fn terminate_session(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    match manager.terminate_session(id.clone()).await {
        Ok(_) => {
            lineage_store().write().await.remove(&id);
            Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({ "success": true })),
                StatusCode::OK,
            ))
        }
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error: e }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub async fn get_child_sessions(id: String) -> Result<impl Reply, Rejection> {
    let session_repo = crate::state::get_session_repository();

    let children = match session_repo.get_child_session_ids(&id).await {
        Ok(ids) => ids,
        Err(_) => {
            let store = lineage_store().read().await;
            store
                .iter()
                .filter_map(|(session_id, meta)| {
                    if meta.parent_session_id.as_deref() == Some(id.as_str()) {
                        Some(session_id.clone())
                    } else {
                        None
                    }
                })
                .collect()
        }
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&ChildSessionsResponse {
            parent_session_id: id,
            count: children.len(),
            children,
        }),
        StatusCode::OK,
    ))
}

pub async fn get_assistant(assistant_id: String) -> Result<impl Reply, Rejection> {
    use crate::repositories::assistant_repository::AssistantRepository;

    let repo = crate::state::get_assistant_repository();
    match repo.get_assistant(&assistant_id).await {
        Ok(Some(assistant)) => Ok(warp::reply::with_status(
            warp::reply::json(&assistant),
            StatusCode::OK,
        )),
        Ok(None) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Assistant not found: {}", assistant_id),
            }),
            StatusCode::NOT_FOUND,
        )),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to fetch assistant: {}", e),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub async fn get_assistants() -> Result<impl Reply, Rejection> {
    use crate::repositories::assistant_repository::AssistantRepository;

    let repo = crate::state::get_assistant_repository();
    match repo.list_assistants().await {
        Ok(assistants) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "assistants": assistants })),
            StatusCode::OK,
        )),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to fetch assistants: {}", e),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub async fn health() -> Result<impl Reply, Rejection> {
    Ok(warp::reply::with_status(
        warp::reply::json(&HealthResponse {
            status: "ok".to_string(),
            service: "libr-agent-session-api".to_string(),
        }),
        StatusCode::OK,
    ))
}
