use crate::agent::AgentSessionManager;
use crate::models::chat::MessageSource;
use crate::repositories::SessionRepository;
use crate::utils::session_id::display_session_id;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::helpers::{
    lineage_store, map_create_session_response_for_http, map_session_metadata_for_http, open_gate,
    resolve_create_session_body_refs, resolve_error_reply, resolve_http_session_ref,
    resolve_session_ref_in,
};
use super::types::{
    ChildSessionsResponse, CreateSessionRequest, ErrorResponse, ListSessionsQuery,
    ListSessionsResponse, PatchSessionRequest, PatchSessionResponse, SessionListItem,
    SessionStatusResponse,
};
use crate::agent::session_manager::approvals::list_pending_approvals_in;
use crate::agent::session_manager::execution_mode::set_execution_mode_in;
use crate::agent::state::AgentSession;
use crate::execution_mode::ExecutionMode;
use crate::repositories::session_repository::SessionListCursor;
use crate::server::session_ownership;
use std::collections::HashMap;
use tokio::sync::RwLock;

fn classify_spawn_error(err: &str) -> (StatusCode, String) {
    if err.contains("Assistant not found") {
        (StatusCode::NOT_FOUND, err.to_string())
    } else if err.contains("Failed to fetch assistant") {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "An internal database error occurred while fetching the assistant details.".to_string(),
        )
    } else if err.contains("limit exceeded")
        || err.contains("must be absolute")
        || err.contains("restricted system directory")
        || err.contains("must be a directory")
        || err.contains("not accessible")
        || err.contains("Invalid assistant configuration")
        || err.contains("Invalid Docker workspace config")
    {
        (StatusCode::BAD_REQUEST, err.to_string())
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }
}

pub async fn create_session(
    manager: Arc<AgentSessionManager>,
    body: CreateSessionRequest,
    attachment: Option<String>,
) -> Result<impl Reply, Rejection> {
    // Validate the caller's attachment before spawning, so a bad header never
    // leaves a session behind. HTTP callers never claim (#2854).
    if let Err((status, error)) = session_ownership::reject_attachment_header(attachment.as_deref())
    {
        return Ok(resolve_error_reply(status, error));
    }
    let body = match resolve_create_session_body_refs(body).await {
        Ok(body) => body,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    match crate::services::AgentService::spawn_agent_with_source(
        &manager,
        body,
        Some(MessageSource::Api),
    )
    .await
    {
        Ok(response) => Ok(warp::reply::with_status(
            warp::reply::json(&map_create_session_response_for_http(response)),
            StatusCode::CREATED,
        )),
        Err(e) => {
            let (status, sanitized_err) = classify_spawn_error(&e);
            Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: sanitized_err,
                }),
                status,
            ))
        }
    }
}

/// `GET /api/sessions/:id` — a metadata read. It never claims ownership and
/// never refuses because of ownership (#2759); claiming happens only on the
/// open paths (create, `POST .../open`, resume, send).
pub async fn get_session(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    Ok(get_session_reply(
        &id,
        Some(manager.active_sessions().as_ref()),
        &manager.repositories().session,
    )
    .await)
}

async fn get_session_reply(
    id: &str,
    active_sessions: Option<&RwLock<HashMap<String, AgentSession>>>,
    session_repo: &Arc<dyn SessionRepository>,
) -> warp::reply::WithStatus<warp::reply::Json> {
    let id = match resolve_session_ref_in(session_repo.as_ref(), id).await {
        Ok(id) => id,
        Err((status, error)) => return resolve_error_reply(status, error),
    };

    match session_status_snapshot(&id, active_sessions, session_repo).await {
        Ok(Some(snapshot)) => {
            warp::reply::with_status(warp::reply::json(&snapshot), StatusCode::OK)
        }
        Ok(None) => warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: "Session not found".to_string(),
            }),
            StatusCode::NOT_FOUND,
        ),
        Err(e) => warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error: e }),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

/// `POST /api/sessions/:id/open` — claim the session for the calling
/// frontend. `200` when the caller now owns it (or sent no attachment, an
/// external caller outside the ownership model); `409` with "is open in
/// another process" when a different live process owns it.
pub async fn open_session(
    id: String,
    manager: Arc<AgentSessionManager>,
    attachment: Option<String>,
) -> Result<impl Reply, Rejection> {
    Ok(open_session_reply(&id, attachment.as_deref(), &manager.repositories().session).await)
}

async fn open_session_reply(
    id: &str,
    attachment: Option<&str>,
    session_repo: &Arc<dyn SessionRepository>,
) -> warp::reply::WithStatus<warp::reply::Json> {
    if let Err(refusal) = open_gate(id, attachment, session_repo.as_ref()).await {
        return refusal;
    }
    warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "open" })),
        StatusCode::OK,
    )
}

/// The `GET /api/sessions/:id` body for an exact session id, or `None` when
/// the session does not exist. Also the `status` payload of the event stream.
pub(super) async fn session_status_snapshot(
    id: &str,
    active_sessions: Option<&RwLock<HashMap<String, AgentSession>>>,
    session_repo: &Arc<dyn SessionRepository>,
) -> Result<Option<SessionStatusResponse>, String> {
    let Some(session) = crate::agent::lifecycle::get_session(session_repo, id).await? else {
        return Ok(None);
    };
    let pending_approvals = match active_sessions {
        Some(active) => list_pending_approvals_in(active, id).await,
        None => Vec::new(),
    };
    let pending_user_prompt = match active_sessions {
        Some(active) => {
            let active = active.read().await;
            match active.get(id) {
                Some(agent_session) => {
                    crate::agent::state::pending_user_prompt_summary(
                        &agent_session.pending_user_prompt,
                    )
                    .await
                }
                None => None,
            }
        }
        None => None,
    };
    Ok(Some(SessionStatusResponse {
        session: map_session_metadata_for_http(session),
        pending_approvals,
        pending_user_prompt,
    }))
}

const DEFAULT_SESSIONS_LIMIT: u64 = 50;
const MAX_SESSIONS_LIMIT: u64 = 200;

/// `GET /api/sessions` — sessions ordered by most recent activity.
///
/// Pass the `nextCursor` of one page as `cursorUpdatedAt` + `cursorId` to get
/// the next one.
pub async fn list_sessions(
    query: ListSessionsQuery,
    manager: Arc<AgentSessionManager>,
    attachment: Option<String>,
) -> Result<impl Reply, Rejection> {
    Ok(list_sessions_reply(
        query,
        &manager.repositories().session,
        attachment.as_deref(),
    )
    .await)
}

async fn list_sessions_reply(
    query: ListSessionsQuery,
    session_repo: &Arc<dyn SessionRepository>,
    attachment: Option<&str>,
) -> warp::reply::WithStatus<warp::reply::Json> {
    // Any attachment header is refused (#2854); no header leaves list open.
    if let Err((status, error)) = session_ownership::reject_attachment_header(attachment) {
        return resolve_error_reply(status, error);
    }
    let limit = query.limit.unwrap_or(DEFAULT_SESSIONS_LIMIT);
    if !(1..=MAX_SESSIONS_LIMIT).contains(&limit) {
        return resolve_error_reply(
            StatusCode::BAD_REQUEST,
            format!("Invalid limit {limit}: must be between 1 and {MAX_SESSIONS_LIMIT}"),
        );
    }
    let cursor = match (query.cursor_updated_at, query.cursor_id) {
        (Some(updated_at), Some(id)) => Some(SessionListCursor { updated_at, id }),
        (None, None) => None,
        _ => {
            return resolve_error_reply(
                StatusCode::BAD_REQUEST,
                "cursorUpdatedAt and cursorId must be given together".to_string(),
            )
        }
    };

    match session_repo
        .list_sessions(
            cursor,
            limit,
            query.search.as_deref(),
            query.bookmarked.unwrap_or(false),
            query.status.as_deref(),
        )
        .await
    {
        Ok(page) => warp::reply::with_status(
            warp::reply::json(&ListSessionsResponse {
                sessions: page
                    .items
                    .into_iter()
                    .map(|meta| {
                        // Ownership is keyed by the storage id, so check it
                        // before the display-id rewrite (#2759).
                        let open_elsewhere =
                            session_ownership::is_open_elsewhere(&meta.id, attachment);
                        SessionListItem {
                            session: map_session_metadata_for_http(meta),
                            open_elsewhere,
                        }
                    })
                    .collect(),
                next_cursor: page.next_cursor,
            }),
            StatusCode::OK,
        ),
        Err(e) => resolve_error_reply(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list sessions: {e}"),
        ),
    }
}

pub async fn patch_session(
    id: String,
    manager: Arc<AgentSessionManager>,
    body: PatchSessionRequest,
) -> Result<impl Reply, Rejection> {
    Ok(patch_session_reply(
        &id,
        body,
        manager.active_sessions().as_ref(),
        &manager.repositories().session,
        Some(manager.app_handle()),
    )
    .await)
}

async fn patch_session_reply(
    id: &str,
    body: PatchSessionRequest,
    active_sessions: &RwLock<HashMap<String, AgentSession>>,
    session_repo: &Arc<dyn SessionRepository>,
    app_handle: Option<&tauri::AppHandle>,
) -> warp::reply::WithStatus<warp::reply::Json> {
    let id = match resolve_session_ref_in(session_repo.as_ref(), id).await {
        Ok(id) => id,
        Err((status, error)) => return resolve_error_reply(status, error),
    };

    if body.execution_mode.is_none() && body.name.is_none() {
        return resolve_error_reply(
            StatusCode::BAD_REQUEST,
            "patch requires executionMode and/or name".to_string(),
        );
    }

    // Validate every field before mutating so a bad mode cannot leave a
    // half-applied rename (#2763 G1.2).
    let validated_mode = match body.execution_mode.as_ref() {
        Some(raw_mode) => match raw_mode.parse::<ExecutionMode>() {
            Ok(mode) => Some(mode),
            Err(error) => return resolve_error_reply(StatusCode::BAD_REQUEST, error),
        },
        None => None,
    };
    let validated_name = match body.name.as_ref() {
        Some(raw_name) => {
            let name = raw_name.trim();
            if name.is_empty() {
                return resolve_error_reply(
                    StatusCode::BAD_REQUEST,
                    "name must not be empty".to_string(),
                );
            }
            Some(name.to_string())
        }
        None => None,
    };

    let mut response = PatchSessionResponse {
        id: display_session_id(&id),
        execution_mode: None,
        name: None,
        resolved_approval_ids: Vec::new(),
    };

    if let Some(mode) = validated_mode {
        match set_execution_mode_in(active_sessions, session_repo, app_handle, &id, mode).await {
            Ok(resolved_approval_ids) => {
                response.execution_mode = Some(mode);
                response.resolved_approval_ids = resolved_approval_ids;
            }
            Err(error) => return resolve_error_reply(StatusCode::INTERNAL_SERVER_ERROR, error),
        }
    }

    if let Some(name) = validated_name {
        if let Err(error) = session_repo.update_name(&id, name.clone()).await {
            return resolve_error_reply(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
        }
        response.name = Some(name);
    }

    warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)
}

/// POST /api/sessions/:id/resume
/// Loads a paused session into memory and resumes workflow without adding any new message.
/// Used by awaitAgent crash recovery to avoid injecting garbage user messages.
pub async fn resume_session_workflow(
    id: String,
    manager: Arc<AgentSessionManager>,
    attachment: Option<String>,
) -> Result<impl Reply, Rejection> {
    // Resuming is opening: refused while another live process owns it (#2759).
    let id = match open_gate(
        &id,
        attachment.as_deref(),
        manager.repositories().session.as_ref(),
    )
    .await
    {
        Ok(id) => id,
        Err(refusal) => return Ok(refusal),
    };

    // Reload the session (runtime + MCP proxy), then resume from existing messages.
    if let Err(error) = manager.resume_session_and_workflow(&id).await {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error }),
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
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    match manager.terminate_session(id.clone()).await {
        Ok(_) => {
            crate::services::agent_service::remove_lineage(&id).await;
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

/// DELETE /api/sessions/:id — remove the session (and cascade children) entirely.
pub async fn delete_session(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    match manager.delete_session(id.clone()).await {
        Ok(deleted_ids) => {
            for deleted_id in &deleted_ids {
                crate::services::agent_service::remove_lineage(deleted_id).await;
            }
            let deleted_display: Vec<String> = deleted_ids
                .iter()
                .map(|deleted_id| display_session_id(deleted_id))
                .collect();
            Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({
                    "success": true,
                    "deletedIds": deleted_display,
                })),
                StatusCode::OK,
            ))
        }
        Err(e) => {
            let status = if e.contains("not found") || e.contains("Not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse { error: e }),
                status,
            ))
        }
    }
}

pub async fn get_child_sessions(id: String) -> Result<impl Reply, Rejection> {
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    let session_repo = crate::state::get_session_repository();
    use crate::repositories::session_repository::SessionRepository;

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

    let children_display: Vec<String> = children
        .iter()
        .map(|child_id| display_session_id(child_id))
        .collect();

    Ok(warp::reply::with_status(
        warp::reply::json(&ChildSessionsResponse {
            parent_session_id: display_session_id(&id),
            count: children_display.len(),
            children: children_display,
        }),
        StatusCode::OK,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::{SessionMetadata, SessionStatus, SqliteSessionRepository};
    use migration::{Migrator, MigratorTrait};

    async fn sqlite_session_repo() -> Arc<dyn SessionRepository> {
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        Migrator::up(&db, None)
            .await
            .expect("migrations should run");
        Arc::new(SqliteSessionRepository::new(db))
    }

    #[test]
    fn a_refused_docker_workspace_is_a_bad_request() {
        let (status, error) = classify_spawn_error(
            "Invalid Docker workspace config: The workspace '/home/user' is your home directory or contains it.",
        );
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.contains("home directory"));
    }

    async fn get_session_json(
        id: &str,
        repo: &Arc<dyn SessionRepository>,
    ) -> (StatusCode, serde_json::Value) {
        let response = get_session_reply(id, None, repo).await.into_response();
        let status = response.status();
        let body = warp::hyper::body::to_bytes(response.into_body())
            .await
            .expect("reply body should be readable");
        let json = serde_json::from_slice(&body).expect("reply body should be JSON");
        (status, json)
    }

    async fn reply_to_json(
        reply: warp::reply::WithStatus<warp::reply::Json>,
    ) -> (StatusCode, serde_json::Value) {
        let response = reply.into_response();
        let status = response.status();
        let body = warp::hyper::body::to_bytes(response.into_body())
            .await
            .expect("reply body should be readable");
        let json = serde_json::from_slice(&body).expect("reply body should be JSON");
        (status, json)
    }

    /// #2700: a release build can list sessions, page by page.
    #[tokio::test]
    async fn list_sessions_pages_through_every_session_once() {
        let repo = sqlite_session_repo().await;
        for (index, id) in ["list-a", "list-b", "list-c"].into_iter().enumerate() {
            let mut session = SessionMetadata::test_fixture(id);
            session.updated_at = 1_000 + index as i64;
            repo.upsert_session(&session)
                .await
                .expect("session should be stored");
        }

        let (status, first) = reply_to_json(
            list_sessions_reply(
                ListSessionsQuery {
                    limit: Some(2),
                    ..Default::default()
                },
                &repo,
                None,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let first_ids: Vec<&str> = first["sessions"]
            .as_array()
            .expect("sessions array")
            .iter()
            .map(|session| session["id"].as_str().expect("id"))
            .collect();
        assert_eq!(first_ids.len(), 2, "{first}");
        let cursor = &first["nextCursor"];
        assert!(
            cursor.is_object(),
            "a full page must carry a cursor: {first}"
        );

        let (_, second) = reply_to_json(
            list_sessions_reply(
                ListSessionsQuery {
                    limit: Some(2),
                    cursor_updated_at: cursor["updatedAt"].as_i64(),
                    cursor_id: cursor["id"].as_str().map(str::to_string),
                    ..Default::default()
                },
                &repo,
                None,
            )
            .await,
        )
        .await;
        let second_ids: Vec<&str> = second["sessions"]
            .as_array()
            .expect("sessions array")
            .iter()
            .map(|session| session["id"].as_str().expect("id"))
            .collect();

        let mut all: Vec<&str> = first_ids.iter().chain(second_ids.iter()).copied().collect();
        all.sort_unstable();
        assert_eq!(all, vec!["list-a", "list-b", "list-c"]);
        assert_eq!(second["nextCursor"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn list_sessions_rejects_a_bad_limit_and_half_a_cursor() {
        let repo = sqlite_session_repo().await;

        let (status, _) = reply_to_json(
            list_sessions_reply(
                ListSessionsQuery {
                    limit: Some(0),
                    ..Default::default()
                },
                &repo,
                None,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, json) = reply_to_json(
            list_sessions_reply(
                ListSessionsQuery {
                    cursor_id: Some("list-a".to_string()),
                    ..Default::default()
                },
                &repo,
                None,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(json["error"].as_str().unwrap().contains("together"));
    }

    #[tokio::test]
    async fn get_session_returns_the_failure_reason_until_the_next_status_change() {
        let repo = sqlite_session_repo().await;
        let id = "status-reason-http-session";
        repo.upsert_session(&SessionMetadata::test_fixture(id))
            .await
            .expect("session should be stored");

        let (status, json) = get_session_json(id, &repo).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["statusReason"], serde_json::Value::Null);

        let reason = "Docker workspace setup failed: image 'missing:latest' was not found";
        repo.update_status(id, SessionStatus::Error, Some(reason))
            .await
            .expect("failure should be stored");
        let (status, json) = get_session_json(id, &repo).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "error");
        assert_eq!(json["statusReason"], reason);

        repo.update_status(id, SessionStatus::Idle, None)
            .await
            .expect("status change should be stored");
        let (_, json) = get_session_json(id, &repo).await;
        assert_eq!(json["status"], "idle");
        assert_eq!(json["statusReason"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn a_full_upsert_with_a_new_status_clears_the_failure_reason() {
        let repo = sqlite_session_repo().await;
        let id = "status-reason-upsert-session";
        let mut session = SessionMetadata::test_fixture(id);
        repo.upsert_session(&session)
            .await
            .expect("session should be stored");
        repo.update_status(id, SessionStatus::Error, Some("setup failed"))
            .await
            .expect("failure should be stored");

        session.status = SessionStatus::Idle;
        session.status_reason = None;
        repo.upsert_session(&session)
            .await
            .expect("upsert should be stored");

        let (_, json) = get_session_json(id, &repo).await;
        assert_eq!(json["status"], "idle");
        assert_eq!(json["statusReason"], serde_json::Value::Null);

        session.status = SessionStatus::Error;
        session.status_reason = Some("restart failed".to_string());
        repo.upsert_session(&session)
            .await
            .expect("upsert should be stored");
        let (_, json) = get_session_json(id, &repo).await;
        assert_eq!(json["statusReason"], "restart failed");
    }

    #[tokio::test]
    async fn get_session_shows_pending_approval_details_while_status_stays_busy() {
        use crate::agent::context::registry::ContextRegistry;
        use crate::agent::state::{PendingApprovalData, PendingApprovalKind};
        use crate::repositories::session_repository::SessionAttentionReason;
        use tokio::sync::oneshot;

        let repo = sqlite_session_repo().await;
        let id = "a1b2c3d4e5";
        let mut metadata = SessionMetadata::test_fixture(id);
        metadata.status = SessionStatus::Busy;
        metadata.last_attention_reason = Some(SessionAttentionReason::PendingApproval);
        repo.upsert_session(&metadata)
            .await
            .expect("session should be stored");

        let agent_session = AgentSession::new(metadata, Arc::new(ContextRegistry::new()), None);
        let (tx, _rx) = oneshot::channel();
        agent_session.pending_approvals.write().await.insert(
            "call-hard-1".to_string(),
            PendingApprovalData {
                sender: tx,
                tool_name: "workspace__runShell".to_string(),
                arguments: r#"{"command":"echo $(whoami)"}"#.to_string(),
                approval_kind: PendingApprovalKind::Hard,
                request_id: None,
                description: Some("Command substitution requires approval".to_string()),
                input_preview: Some("echo $(whoami)".to_string()),
            },
        );
        let active_sessions = RwLock::new(HashMap::from([(id.to_string(), agent_session)]));

        let (status, json) =
            reply_to_json(get_session_reply(id, Some(&active_sessions), &repo).await).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "busy");
        assert_eq!(json["lastAttentionReason"], "pendingApproval");
        let pending = json["pendingApprovals"]
            .as_array()
            .expect("pendingApprovals array");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0]["toolCallId"], "call-hard-1");
        assert_eq!(pending[0]["toolName"], "workspace__runShell");
        assert_eq!(
            pending[0]["reason"],
            "Command substitution requires approval"
        );
        assert_eq!(pending[0]["commandPreview"], "echo $(whoami)");
        assert_eq!(pending[0]["approvalKind"], "hard");
    }

    #[tokio::test]
    async fn patch_session_raises_to_unsafe_releasing_hard_approval_and_allows_lowering_mode() {
        use crate::agent::context::registry::ContextRegistry;
        use crate::agent::state::{PendingApprovalData, PendingApprovalKind};
        use tokio::sync::oneshot;

        let repo = sqlite_session_repo().await;
        let id = "f6e5d4c3b2";
        let mut metadata = SessionMetadata::test_fixture(id);
        metadata.status = SessionStatus::Busy;
        metadata.execution_mode = ExecutionMode::Normal;
        repo.upsert_session(&metadata)
            .await
            .expect("session should be stored");

        let agent_session = AgentSession::new(metadata, Arc::new(ContextRegistry::new()), None);
        let (tx, mut rx) = oneshot::channel();
        agent_session.pending_approvals.write().await.insert(
            "call-hard-1".to_string(),
            PendingApprovalData {
                sender: tx,
                tool_name: "workspace__runShell".to_string(),
                arguments: r#"{"command":"echo $(whoami)"}"#.to_string(),
                approval_kind: PendingApprovalKind::Hard,
                request_id: None,
                description: Some("Command substitution requires approval".to_string()),
                input_preview: Some("echo $(whoami)".to_string()),
            },
        );
        let active_sessions = RwLock::new(HashMap::from([(id.to_string(), agent_session)]));

        // Raising to yolo keeps hard approval pending.
        let (status, yolo_json) = reply_to_json(
            patch_session_reply(
                id,
                PatchSessionRequest {
                    execution_mode: Some("yolo".to_string()),
                    name: None,
                },
                &active_sessions,
                &repo,
                None,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(yolo_json["executionMode"], "yolo");
        assert_eq!(yolo_json["resolvedApprovalIds"], serde_json::json!([]));
        assert!(rx.try_recv().is_err());

        // Raising to unsafe releases the pending hard approval.
        let (status, unsafe_json) = reply_to_json(
            patch_session_reply(
                id,
                PatchSessionRequest {
                    execution_mode: Some("unsafe".to_string()),
                    name: None,
                },
                &active_sessions,
                &repo,
                None,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(unsafe_json["executionMode"], "unsafe");
        assert_eq!(
            unsafe_json["resolvedApprovalIds"],
            serde_json::json!(["call-hard-1"])
        );
        assert_eq!(rx.await, Ok(true));
        assert_eq!(
            repo.get_session(id).await.unwrap().unwrap().execution_mode,
            ExecutionMode::Unsafe
        );

        // Lowering mode to normal is allowed and persisted.
        let (status, normal_json) = reply_to_json(
            patch_session_reply(
                id,
                PatchSessionRequest {
                    execution_mode: Some("normal".to_string()),
                    name: None,
                },
                &active_sessions,
                &repo,
                None,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(normal_json["executionMode"], "normal");
        assert_eq!(
            repo.get_session(id).await.unwrap().unwrap().execution_mode,
            ExecutionMode::Normal
        );
    }

    #[tokio::test]
    async fn patch_session_renames_without_changing_execution_mode() {
        let repo = sqlite_session_repo().await;
        let id = "rename01abc";
        let mut metadata = SessionMetadata::test_fixture(id);
        metadata.name = Some("Old Name".to_string());
        metadata.execution_mode = ExecutionMode::Normal;
        repo.upsert_session(&metadata).await.expect("store");
        let active_sessions = RwLock::new(HashMap::new());

        let (status, json) = reply_to_json(
            patch_session_reply(
                id,
                PatchSessionRequest {
                    execution_mode: None,
                    name: Some("New Name".to_string()),
                },
                &active_sessions,
                &repo,
                None,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["name"], "New Name");
        assert!(json.get("executionMode").is_none() || json["executionMode"].is_null());
        assert_eq!(
            repo.get_session(id).await.unwrap().unwrap().name.as_deref(),
            Some("New Name")
        );
    }

    #[tokio::test]
    async fn patch_session_rejects_empty_name_and_empty_body() {
        let repo = sqlite_session_repo().await;
        let id = "rename02def";
        let mut metadata = SessionMetadata::test_fixture(id);
        metadata.name = Some("Keep".to_string());
        repo.upsert_session(&metadata).await.expect("store");
        let active_sessions = RwLock::new(HashMap::new());

        let (status, _) = reply_to_json(
            patch_session_reply(
                id,
                PatchSessionRequest {
                    execution_mode: None,
                    name: Some("   ".to_string()),
                },
                &active_sessions,
                &repo,
                None,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            repo.get_session(id).await.unwrap().unwrap().name.as_deref(),
            Some("Keep")
        );

        let (status, _) = reply_to_json(
            patch_session_reply(
                id,
                PatchSessionRequest {
                    execution_mode: None,
                    name: None,
                },
                &active_sessions,
                &repo,
                None,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn patch_session_invalid_mode_does_not_apply_a_pending_rename() {
        let repo = sqlite_session_repo().await;
        let id = "rename03ghi";
        let mut metadata = SessionMetadata::test_fixture(id);
        metadata.name = Some("Original".to_string());
        metadata.execution_mode = ExecutionMode::Normal;
        repo.upsert_session(&metadata).await.expect("store");
        let active_sessions = RwLock::new(HashMap::new());

        let (status, _) = reply_to_json(
            patch_session_reply(
                id,
                PatchSessionRequest {
                    execution_mode: Some("not-a-mode".to_string()),
                    name: Some("Should Not Stick".to_string()),
                },
                &active_sessions,
                &repo,
                None,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let stored = repo.get_session(id).await.unwrap().unwrap();
        assert_eq!(stored.name.as_deref(), Some("Original"));
        assert_eq!(stored.execution_mode, ExecutionMode::Normal);
    }

    // ---- Session ownership over the Session API (#2759) ----

    use crate::server::routes::{frontend_attachment_header, FRONTEND_ATTACHMENT_HEADER};
    use warp::Filter;

    fn unique_session_id() -> String {
        format!("own-{}", uuid::Uuid::new_v4())
    }

    /// The ownership-relevant Session API routes, built from the real
    /// header extractor, the real `open_gate`, and the real reply functions.
    /// Resume and send run exactly the gate their handlers run first; the
    /// session work behind the gate needs a full `AgentSessionManager`, so
    /// the stub answers 200 where the handler would proceed.
    fn ownership_routes(
        repo: Arc<dyn SessionRepository>,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
        let with_repo = warp::any().map(move || repo.clone());

        let get = warp::get()
            .and(warp::path!("api" / "sessions" / String))
            .and(with_repo.clone())
            .and_then(|id: String, repo: Arc<dyn SessionRepository>| async move {
                Ok::<_, Rejection>(get_session_reply(&id, None, &repo).await)
            });
        let list = warp::get()
            .and(warp::path!("api" / "sessions"))
            .and(with_repo.clone())
            .and(frontend_attachment_header())
            .and_then(
                |repo: Arc<dyn SessionRepository>, attachment: Option<String>| async move {
                    Ok::<_, Rejection>(
                        list_sessions_reply(
                            ListSessionsQuery::default(),
                            &repo,
                            attachment.as_deref(),
                        )
                        .await,
                    )
                },
            );
        let open = warp::post()
            .and(warp::path!("api" / "sessions" / String / "open"))
            .and(with_repo.clone())
            .and(frontend_attachment_header())
            .and_then(
                |id: String, repo: Arc<dyn SessionRepository>, attachment: Option<String>| async move {
                    Ok::<_, Rejection>(open_session_reply(&id, attachment.as_deref(), &repo).await)
                },
            );
        let gated = |suffix: &'static str, ok: &'static str| {
            warp::post()
                .and(warp::path("api"))
                .and(warp::path("sessions"))
                .and(warp::path::param::<String>())
                .and(warp::path(suffix))
                .and(warp::path::end())
                .and(with_repo.clone())
                .and(frontend_attachment_header())
                .and_then(
                    move |id: String,
                          repo: Arc<dyn SessionRepository>,
                          attachment: Option<String>| async move {
                        Ok::<_, Rejection>(
                            match open_gate(&id, attachment.as_deref(), repo.as_ref()).await {
                                Ok(_) => warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({ "status": ok })),
                                    StatusCode::OK,
                                ),
                                Err(refusal) => refusal,
                            },
                        )
                    },
                )
        };
        let resume = gated("resume", "resumed");
        let send = gated("messages", "sent");

        get.or(list).or(open).or(resume).or(send)
    }

    async fn call(
        api: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
        method: &str,
        path: &str,
        attachment: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let mut request = warp::test::request().method(method).path(path);
        if let Some(id) = attachment {
            request = request.header(FRONTEND_ATTACHMENT_HEADER, id);
        }
        let response = request.reply(api).await;
        let json = serde_json::from_slice(response.body()).expect("reply body should be JSON");
        (response.status(), json)
    }

    async fn stored_session(repo: &Arc<dyn SessionRepository>) -> String {
        let id = unique_session_id();
        repo.upsert_session(&SessionMetadata::test_fixture(&id))
            .await
            .expect("session should be stored");
        id
    }

    #[tokio::test]
    async fn a_metadata_read_never_claims_and_is_never_refused() {
        let repo = sqlite_session_repo().await;
        let api = ownership_routes(repo.clone());
        let id = stored_session(&repo).await;

        let (status, json) = call(&api, "GET", &format!("/api/sessions/{id}"), None).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(session_ownership::owner(&id), None);
    }

    #[tokio::test]
    async fn any_attachment_header_is_rejected_on_the_http_path() {
        let repo = sqlite_session_repo().await;
        let api = ownership_routes(repo.clone());
        let id = stored_session(&repo).await;

        for header in [session_ownership::HOST_ATTACHMENT_ID, "never-attached"] {
            for path in [
                format!("/api/sessions/{id}/open"),
                format!("/api/sessions/{id}/resume"),
                format!("/api/sessions/{id}/messages"),
            ] {
                let (status, json) = call(&api, "POST", &path, Some(header)).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{path}/{header}: {json}");
                assert!(
                    json["error"]
                        .as_str()
                        .unwrap()
                        .contains("attachment registry removed"),
                    "{json}"
                );
            }
            let (status, json) = call(&api, "GET", "/api/sessions", Some(header)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{header}: {json}");
        }
        assert_eq!(session_ownership::owner(&id), None, "nothing was claimed");
    }

    #[tokio::test]
    async fn opening_a_missing_session_is_404_and_claims_nothing() {
        let repo = sqlite_session_repo().await;
        let api = ownership_routes(repo);
        let missing = unique_session_id();

        let (status, _) = call(&api, "POST", &format!("/api/sessions/{missing}/open"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(session_ownership::owner(&missing), None);
    }
}
