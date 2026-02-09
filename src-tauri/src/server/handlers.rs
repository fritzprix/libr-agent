use crate::agent::AgentSessionManager;
use crate::commands::messages_commands::Message;
use crate::mcp::types::MCPContent;
use crate::repositories::message_repository::MessageRepository;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use warp::{http::StatusCode, Rejection, Reply};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub name: Option<String>,
    pub assistant_id: String, // Replaces agent_config
    pub workspace_path: Option<String>,
    pub request: String,
}

#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub id: String,
    pub name: Option<String>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
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

    // 3. Create Session
    let session_id = format!("session-{}", Uuid::new_v4());

    // Use provided name or default to Assistant name
    let session_name = body.name.or(Some(assistant.name.clone()));

    // Register override if path provided
    if let Some(path_str) = body.workspace_path {
        if let Ok(session_manager) = crate::session::get_session_manager() {
            let path = std::path::PathBuf::from(path_str);
            if path.is_absolute() {
                if let Err(e) = session_manager
                    .register_session_override(&session_id, path)
                    .await
                {
                    log::warn!("Failed to register workspace override: {}", e);
                }
            } else {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&ErrorResponse {
                        error: "Workspace path must be absolute".to_string(),
                    }),
                    StatusCode::BAD_REQUEST,
                ));
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
    _manager: Arc<AgentSessionManager>, // Not used directly, but kept for consistency if needed later
) -> Result<impl Reply, Rejection> {
    // We access the repo directly here as per the plan
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

    let assistant_id = session.agent_config.as_ref().and_then(|config_str| {
        crate::agent::AgentConfig::from_json(config_str)
            .ok()
            .and_then(|c| c.id)
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
        source: Some("api".to_string()),
        error: None,
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

pub async fn terminate_session(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    match manager.terminate_session(id).await {
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "success": true })),
            StatusCode::OK,
        )),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error: e }),
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
