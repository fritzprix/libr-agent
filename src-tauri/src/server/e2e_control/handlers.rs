use std::sync::Arc;

use serde::{Deserialize, Serialize};
use warp::http::StatusCode;
use warp::{Filter, Rejection, Reply};

use crate::agent::AgentSessionManager;
use crate::mcp::builtin::workspace::{
    LIST_PENDING_INTERACTIVE_SHELL_INPUT_INTERNAL, SUBMIT_INTERACTIVE_SHELL_INPUT_INTERNAL,
    CANCEL_INTERACTIVE_SHELL_INPUT_INTERNAL,
};
use crate::server::e2e_control::token::{authorize, e2e_control_enabled};
use crate::server::handlers::helpers::resolve_http_session_ref;
use crate::server::handlers::types::ErrorResponse;
use crate::state::{get_app_handle, get_mcp_service_proxy_manager};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct E2eControlStatus {
    pub enabled: bool,
    pub debug_build: bool,
    pub loopback_only: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveShellPromptSnapshot {
    pub execution_id: String,
    pub prompt: String,
    pub input_type: String,
    pub command_preview: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingApprovalSnapshot {
    pub tool_call_id: String,
    pub tool_name: String,
    pub approval_kind: String,
    pub command_preview: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInteractionsSnapshot {
    pub session_id: String,
    pub interactive_shell_prompts: Vec<InteractiveShellPromptSnapshot>,
    pub pending_approvals: Vec<PendingApprovalSnapshot>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitInteractiveShellBody {
    pub execution_id: String,
    pub action: String,
    #[serde(default)]
    pub input: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitToolApprovalBody {
    pub tool_call_id: String,
    pub approved: bool,
}

fn e2e_disabled_reply() -> warp::reply::WithStatus<warp::reply::Json> {
    warp::reply::with_status(
        warp::reply::json(&ErrorResponse {
            error: "E2E control is not enabled".to_string(),
        }),
        StatusCode::NOT_FOUND,
    )
}

async fn guard_e2e(headers: warp::http::HeaderMap) -> Result<(), Rejection> {
    if !e2e_control_enabled() {
        return Err(warp::reject::not_found());
    }
    authorize(&headers).map_err(|reason| {
        warp::reject::custom(crate::server::access_guard::AccessDenied { reason })
    })
}

pub async fn get_interactions_snapshot(
    manager: &AgentSessionManager,
    session_id: &str,
) -> Result<SessionInteractionsSnapshot, String> {
    let session_id = resolve_http_session_ref(session_id)
        .await
        .map_err(|(_, err)| err)?;

    let mut interactive_shell_prompts = Vec::new();
    let proxy_manager = get_mcp_service_proxy_manager();
    if let Some(proxy) = proxy_manager.get_proxy(&session_id).await {
        let list = proxy
            .call_builtin_internal(
                "workspace",
                LIST_PENDING_INTERACTIVE_SHELL_INPUT_INTERNAL,
                serde_json::json!({}),
            )
            .await?;
        if let Some(items) = list
            .structured_content
            .as_ref()
            .and_then(|v| v.get("items"))
            .and_then(|v| v.as_array())
        {
            for item in items {
                if let (Some(execution_id), Some(prompt), Some(input_type)) = (
                    item.get("executionId").and_then(|v| v.as_str()),
                    item.get("prompt").and_then(|v| v.as_str()),
                    item.get("inputType").and_then(|v| v.as_str()),
                ) {
                    interactive_shell_prompts.push(InteractiveShellPromptSnapshot {
                        execution_id: execution_id.to_string(),
                        prompt: prompt.to_string(),
                        input_type: input_type.to_string(),
                        command_preview: item
                            .get("commandPreview")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        created_at: item
                            .get("createdAt")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    });
                }
            }
        }
    }

    let pending_approvals = {
        let active = manager.active_sessions.read().await;
        let Some(session) = active.get(&session_id) else {
            return Err(format!("Session not found: {}", session_id));
        };
        let approvals = session.pending_approvals.read().await;
        approvals
            .iter()
            .map(|(tool_call_id, data)| PendingApprovalSnapshot {
                tool_call_id: tool_call_id.clone(),
                tool_name: data.tool_name.clone(),
                approval_kind: format!("{:?}", data.approval_kind),
                command_preview: data.input_preview.clone(),
                request_id: data.request_id.clone(),
            })
            .collect()
    };

    Ok(SessionInteractionsSnapshot {
        session_id: crate::utils::session_id::display_session_id(&session_id),
        interactive_shell_prompts,
        pending_approvals,
    })
}

pub async fn submit_interactive_shell(
    session_id: &str,
    body: SubmitInteractiveShellBody,
) -> Result<(), String> {
    let session_id = resolve_http_session_ref(session_id)
        .await
        .map_err(|(_, err)| err)?;

    let proxy_manager = get_mcp_service_proxy_manager();
    proxy_manager
        .ensure_configured_proxy(&session_id, get_app_handle().cloned())
        .await?;

    let proxy = proxy_manager
        .get_proxy(&session_id)
        .await
        .ok_or_else(|| format!("No proxy found for session: {}", session_id))?;

    match body.action.as_str() {
        "submit" => {
            let input = body.input.ok_or_else(|| {
                "Missing input for interactive shell submit action".to_string()
            })?;
            proxy
                .call_builtin_internal(
                    "workspace",
                    SUBMIT_INTERACTIVE_SHELL_INPUT_INTERNAL,
                    serde_json::json!({
                        "execution_id": body.execution_id,
                        "input": input,
                    }),
                )
                .await?;
        }
        "cancel" => {
            proxy
                .call_builtin_internal(
                    "workspace",
                    CANCEL_INTERACTIVE_SHELL_INPUT_INTERNAL,
                    serde_json::json!({
                        "execution_id": body.execution_id,
                    }),
                )
                .await?;
        }
        other => return Err(format!("Unknown interactive shell action: {other}")),
    }
    Ok(())
}

pub async fn submit_tool_approval(
    manager: &AgentSessionManager,
    session_id: &str,
    body: SubmitToolApprovalBody,
) -> Result<(), String> {
    let session_id = resolve_http_session_ref(session_id)
        .await
        .map_err(|(_, err)| err)?;

    manager
        .respond_tool_approval(&session_id, &body.tool_call_id, body.approved)
        .await
}

async fn handle_status(headers: warp::http::HeaderMap) -> Result<impl Reply, Rejection> {
    if guard_e2e(headers).await.is_err() {
        return Ok(e2e_disabled_reply());
    }
    Ok(warp::reply::json(&E2eControlStatus {
        enabled: e2e_control_enabled(),
        debug_build: cfg!(debug_assertions),
        loopback_only: true,
    }))
}

async fn handle_get_interactions(
    session_id: String,
    headers: warp::http::HeaderMap,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    guard_e2e(headers).await?;
    match get_interactions_snapshot(manager.as_ref(), &session_id).await {
        Ok(snapshot) => Ok(warp::reply::with_status(
            warp::reply::json(&snapshot),
            StatusCode::OK,
        )),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error }),
            StatusCode::NOT_FOUND,
        )),
    }
}

async fn handle_submit_interactive(
    session_id: String,
    headers: warp::http::HeaderMap,
    body: SubmitInteractiveShellBody,
) -> Result<impl Reply, Rejection> {
    guard_e2e(headers).await?;
    match submit_interactive_shell(&session_id, body).await {
        Ok(()) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "ok": true })),
            StatusCode::OK,
        )),
        Err(error) => {
            let status = if error.contains("not found") || error.contains("Session not found") {
                StatusCode::NOT_FOUND
            } else if error.contains("different session") {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse { error }),
                status,
            ))
        }
    }
}

async fn handle_submit_approval(
    session_id: String,
    headers: warp::http::HeaderMap,
    manager: Arc<AgentSessionManager>,
    body: SubmitToolApprovalBody,
) -> Result<impl Reply, Rejection> {
    guard_e2e(headers).await?;
    match submit_tool_approval(manager.as_ref(), &session_id, body).await {
        Ok(()) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "ok": true })),
            StatusCode::OK,
        )),
        Err(error) => {
            let status = if error.contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::BAD_REQUEST
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse { error }),
                status,
            ))
        }
    }
}

/// Routes mounted only on loopback (never when `expose` is true without extra guard — we always loopback-guard).
pub fn e2e_control_routes(
    agent_manager: Arc<AgentSessionManager>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = Rejection> + Clone {
    let manager = warp::any().map(move || agent_manager.clone());

    let status = warp::get()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("status"))
        .and(warp::path::end())
        .and(warp::header::headers_cloned())
        .and_then(handle_status);

    let interactions = warp::get()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("sessions"))
        .and(warp::path::param::<String>())
        .and(warp::path("interactions"))
        .and(warp::path::end())
        .and(warp::header::headers_cloned())
        .and(manager.clone())
        .and_then(handle_get_interactions);

    let submit_shell = warp::post()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("sessions"))
        .and(warp::path::param::<String>())
        .and(warp::path("interactive-shell"))
        .and(warp::path::end())
        .and(warp::header::headers_cloned())
        .and(warp::body::json())
        .and_then(handle_submit_interactive);

    let submit_approval = warp::post()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("sessions"))
        .and(warp::path::param::<String>())
        .and(warp::path("tool-approvals"))
        .and(warp::path::end())
        .and(warp::header::headers_cloned())
        .and(manager)
        .and(warp::body::json())
        .and_then(handle_submit_approval);

    status
        .or(interactions)
        .or(submit_shell)
        .or(submit_approval)
}
