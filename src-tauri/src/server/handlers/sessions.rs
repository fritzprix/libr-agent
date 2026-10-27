use crate::agent::AgentSessionManager;
use crate::models::chat::MessageSource;
use crate::utils::session_id::display_session_id;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::helpers::{
    lineage_store, map_create_session_response_for_http, map_session_metadata_for_http,
    resolve_create_session_body_refs, resolve_error_reply, resolve_http_session_ref,
};
use super::types::{ChildSessionsResponse, CreateSessionRequest, ErrorResponse};

/// Limits for sessions requested through a managed session's own endpoint (`origin` is the
/// session that owns the endpoint, fixed by the listener, never taken from the request).
/// Such a request may only create managed Docker sessions with defaults: no attaching to an
/// existing container, no host `workspacePath` to mount, no raised execution mode.
fn check_endpoint_origin_limits(
    origin: Option<&str>,
    body: &CreateSessionRequest,
) -> Result<(), &'static str> {
    use crate::execution_mode::ExecutionMode;
    use crate::models::workspace_isolation::WorkspaceIsolationMode;

    if origin.is_none() {
        return Ok(());
    }
    if body.workspace_isolation != Some(WorkspaceIsolationMode::Docker) {
        return Err(
            "Sessions created from a managed Docker session must use workspaceIsolation \"docker\"",
        );
    }
    // Only `image` may be set (Docker sessions require one). `portBindings` would publish host
    // ports; `env`, `workdir`, `attachContainer` and `manageLifecycle` change what runs and what
    // is kept on the host.
    if body.docker_config.as_ref().is_some_and(|config| {
        config.attach_container.is_some()
            || config.workdir.is_some()
            || config.manage_lifecycle.is_some()
            || !config.env.is_empty()
            || !config.port_bindings.is_empty()
    }) {
        return Err("Sessions created from a managed Docker session cannot set dockerConfig fields other than image");
    }
    if body.workspace_path.is_some() {
        return Err("Sessions created from a managed Docker session cannot set workspacePath");
    }
    if body
        .execution_mode
        .is_some_and(|mode| mode != ExecutionMode::Normal)
    {
        return Err(
            "Sessions created from a managed Docker session must use the normal execution mode",
        );
    }
    Ok(())
}

/// Pins the lineage of a session created through a managed session's endpoint: its parent is
/// always the origin session, so "the origin session and its children" can be decided from the
/// lineage, and it cannot join another session's tree or organization. Call after refs are
/// resolved (a parent may be given as a short id).
fn pin_endpoint_origin_lineage(
    origin: Option<&str>,
    body: &mut CreateSessionRequest,
) -> Result<(), &'static str> {
    use crate::execution_mode::ExecutionMode;

    let Some(origin) = origin else {
        return Ok(());
    };
    if body
        .parent_session_id
        .as_deref()
        .is_some_and(|parent| parent != origin)
    {
        return Err(
            "Sessions created from a managed Docker session must be children of that session",
        );
    }
    if body.org_id.is_some() || body.org_name.is_some() || body.org_root_session_id.is_some() {
        return Err(
            "Sessions created from a managed Docker session cannot set organization fields",
        );
    }
    body.parent_session_id = Some(origin.to_string());
    body.execution_mode = Some(ExecutionMode::Normal);
    Ok(())
}

/// A session created through a managed session's endpoint may tighten `maxDepth` / `maxFanout`
/// but not raise them above what it inherits. No inherited limit means unlimited, so a request is
/// always fine then (decision on #2346: the baseline is `advancedSettings`, where 0 is unlimited).
fn check_requested_limits(
    requested_depth: Option<u32>,
    requested_fanout: Option<u32>,
    inherited_depth: Option<u32>,
    inherited_fanout: Option<u32>,
) -> Result<(), &'static str> {
    use crate::services::agent_service::request_within_limit;

    if !request_within_limit(requested_depth, inherited_depth) {
        return Err("maxDepth above the limit inherited from the origin session");
    }
    if !request_within_limit(requested_fanout, inherited_fanout) {
        return Err("maxFanout above the limit inherited from the origin session");
    }
    Ok(())
}

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
    {
        (StatusCode::BAD_REQUEST, err.to_string())
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }
}

pub async fn create_session(
    manager: Arc<AgentSessionManager>,
    origin: Option<String>,
    body: CreateSessionRequest,
) -> Result<impl Reply, Rejection> {
    if let Err(error) = check_endpoint_origin_limits(origin.as_deref(), &body) {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: error.to_string(),
            }),
            StatusCode::FORBIDDEN,
        ));
    }

    let mut body = match resolve_create_session_body_refs(body).await {
        Ok(body) => body,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };
    if let Err(error) = pin_endpoint_origin_lineage(origin.as_deref(), &mut body) {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: error.to_string(),
            }),
            StatusCode::FORBIDDEN,
        ));
    }
    if let Some(origin) = origin.as_deref() {
        // The new session's parent is the origin session, so its limits are the origin's.
        let (inherited_depth, inherited_fanout) =
            crate::services::agent_service::inherited_limits_for(origin).await;
        if let Err(error) = check_requested_limits(
            body.max_depth,
            body.max_fanout,
            inherited_depth,
            inherited_fanout,
        ) {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: error.to_string(),
                }),
                StatusCode::FORBIDDEN,
            ));
        }
    }

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

pub async fn get_session(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    match manager.get_session(&id).await {
        Ok(Some(session)) => Ok(warp::reply::with_status(
            warp::reply::json(&map_session_metadata_for_http(session)),
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

/// POST /api/sessions/:id/resume
/// Loads a paused session into memory and resumes workflow without adding any new message.
/// Used by awaitAgent crash recovery to avoid injecting garbage user messages.
pub async fn resume_session_workflow(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

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
    use serde_json::json;

    fn request(extra: serde_json::Value) -> CreateSessionRequest {
        let mut value = json!({"assistantId": "a1"});
        value
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn main_port_requests_are_unrestricted() {
        assert!(check_endpoint_origin_limits(None, &request(json!({}))).is_ok());
        assert!(check_endpoint_origin_limits(
            None,
            &request(json!({"workspacePath": "/tmp", "executionMode": "unsafe"}))
        )
        .is_ok());
    }

    #[test]
    fn endpoint_requests_may_only_create_default_managed_sessions() {
        let origin = Some("s1");
        let managed = json!({"workspaceIsolation": "docker"});
        assert!(check_endpoint_origin_limits(origin, &request(managed)).is_ok());
        let with_image =
            json!({"workspaceIsolation": "docker", "dockerConfig": {"image": "node:22"}});
        assert!(check_endpoint_origin_limits(origin, &request(with_image)).is_ok());

        for rejected in [
            json!({}),
            json!({"workspaceIsolation": "host"}),
            json!({"workspaceIsolation": "docker", "dockerConfig": {"attachContainer": "web"}}),
            json!({"workspaceIsolation": "docker", "dockerConfig": {"portBindings": [{"containerPort": 80, "hostPort": 8080}]}}),
            json!({"workspaceIsolation": "docker", "dockerConfig": {"env": {"A": "b"}}}),
            json!({"workspaceIsolation": "docker", "dockerConfig": {"workdir": "/"}}),
            json!({"workspaceIsolation": "docker", "workspacePath": "/"}),
            json!({"workspaceIsolation": "docker", "executionMode": "yolo"}),
        ] {
            assert!(
                check_endpoint_origin_limits(origin, &request(rejected.clone())).is_err(),
                "{rejected}"
            );
        }
    }

    #[test]
    fn endpoint_requests_may_tighten_but_not_raise_the_inherited_limits() {
        // Inherited depth 3 / fanout 5.
        let limits = |depth, fanout| check_requested_limits(depth, fanout, Some(3), Some(5));
        assert!(limits(None, None).is_ok(), "no request inherits");
        assert!(limits(Some(3), Some(5)).is_ok(), "equal is allowed");
        assert!(limits(Some(1), Some(2)).is_ok(), "tightening is allowed");
        assert!(limits(Some(4), None).is_err(), "raising depth");
        assert!(limits(None, Some(6)).is_err(), "raising fanout");
        assert!(limits(Some(4), Some(6)).is_err());

        // No inherited limit (baseline 0 / unlimited): nothing to exceed.
        assert!(check_requested_limits(Some(99), Some(99), None, None).is_ok());
        // Only one of them limited.
        assert!(check_requested_limits(Some(2), Some(99), Some(3), None).is_ok());
        assert!(check_requested_limits(Some(4), Some(99), Some(3), None).is_err());
    }

    #[test]
    fn endpoint_requests_are_pinned_to_the_origin_lineage() {
        let origin = Some("s1");

        let mut body = request(json!({"workspaceIsolation": "docker"}));
        pin_endpoint_origin_lineage(origin, &mut body).unwrap();
        assert_eq!(body.parent_session_id.as_deref(), Some("s1"));
        assert_eq!(
            body.execution_mode,
            Some(crate::execution_mode::ExecutionMode::Normal)
        );

        let mut own_parent = request(json!({"parentSessionId": "s1"}));
        assert!(pin_endpoint_origin_lineage(origin, &mut own_parent).is_ok());

        for rejected in [
            json!({"parentSessionId": "other"}),
            json!({"orgId": "o1"}),
            json!({"orgName": "n"}),
            json!({"orgRootSessionId": "r"}),
        ] {
            let mut body = request(rejected.clone());
            assert!(
                pin_endpoint_origin_lineage(origin, &mut body).is_err(),
                "{rejected}"
            );
        }

        let mut main_port = request(json!({"parentSessionId": "other", "orgId": "o1"}));
        pin_endpoint_origin_lineage(None, &mut main_port).unwrap();
        assert_eq!(main_port.parent_session_id.as_deref(), Some("other"));
        assert!(main_port.execution_mode.is_none());
    }
}
