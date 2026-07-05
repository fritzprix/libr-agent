use std::collections::HashMap;
use std::sync::Arc;

use crate::agent::runtime_state::{
    SessionRuntimeDockerState, SessionRuntimeInitResult, SessionRuntimePhase,
};
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::workspace_isolation::WorkspaceIsolationMode;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{SessionMetadata, SessionStatus};
use crate::services::message_service::MessageService;
use crate::services::workspace_runtime_manager::{DockerStepReporter, WorkspaceRuntimeManager};
use tauri::AppHandle;
use tokio::sync::RwLock;

pub struct DockerProvisioningDeps {
    pub session_repo: Arc<dyn SessionRepository>,
    pub active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    pub proxy_manager: Arc<MCPServiceProxyManager>,
    pub app_handle: AppHandle,
}

pub fn spawn_docker_provisioning(
    deps: DockerProvisioningDeps,
    session: SessionMetadata,
    delete_session_on_failure: bool,
) {
    if !WorkspaceRuntimeManager::try_mark_provisioning_in_flight(&session.id) {
        return;
    }

    tokio::spawn(async move {
        let session_id = session.id.clone();
        let result = run_docker_provisioning(&deps, &session).await;
        WorkspaceRuntimeManager::clear_provisioning_in_flight(&session_id);

        match result {
            Ok(()) => {
                if let Err(error) = complete_docker_provisioning(&deps, &session_id).await {
                    log::error!(
                        "Docker provisioning post-complete failed for session {}: {}",
                        session_id,
                        error
                    );
                }
            }
            Err(error) => {
                log::error!(
                    "Docker provisioning failed for session {}: {}",
                    session_id,
                    error
                );
                if let Err(mark_error) =
                    fail_docker_provisioning(&deps, &session, &error, delete_session_on_failure)
                        .await
                {
                    log::error!(
                        "Failed to mark Docker provisioning failure for session {}: {}",
                        session_id,
                        mark_error
                    );
                }
            }
        }
    });
}

pub async fn ensure_provisioning_task_for_session(
    deps: &DockerProvisioningDeps,
    session: &SessionMetadata,
) -> Result<(), String> {
    if session.workspace_isolation != WorkspaceIsolationMode::Docker {
        return Ok(());
    }

    if session.status != SessionStatus::Provisioning {
        return Ok(());
    }

    if WorkspaceRuntimeManager::is_provisioning_in_flight(&session.id) {
        return Ok(());
    }

    spawn_docker_provisioning(
        DockerProvisioningDeps {
            session_repo: Arc::clone(&deps.session_repo),
            active_sessions: Arc::clone(&deps.active_sessions),
            proxy_manager: Arc::clone(&deps.proxy_manager),
            app_handle: deps.app_handle.clone(),
        },
        session.clone(),
        false,
    );
    Ok(())
}

pub async fn recover_provisioning_sessions(deps: &DockerProvisioningDeps) -> Result<(), String> {
    let sessions = deps
        .session_repo
        .get_all_sessions()
        .await
        .map_err(|error| format!("Failed to load sessions for Docker recovery: {error}"))?;

    for session in sessions {
        if session.workspace_isolation != WorkspaceIsolationMode::Docker {
            continue;
        }
        if session.status != SessionStatus::Provisioning {
            continue;
        }

        log::info!(
            "Recovering Docker provisioning for session '{}'",
            session.id
        );
        ensure_provisioning_task_for_session(deps, &session).await?;
    }

    Ok(())
}

async fn run_docker_provisioning(
    deps: &DockerProvisioningDeps,
    session: &SessionMetadata,
) -> Result<(), String> {
    let image = session
        .docker_config
        .as_ref()
        .map(|config| config.image.clone())
        .ok_or_else(|| "dockerConfig is required for Docker workspace isolation".to_string())?;

    emit_docker_runtime_step(
        &deps.proxy_manager,
        Some(&deps.app_handle),
        &session.id,
        &image,
        "Preparing Docker workspace",
        false,
        None,
    )
    .await;

    let session_id = session.id.clone();
    let proxy_manager = Arc::clone(&deps.proxy_manager);
    let app_handle = deps.app_handle.clone();
    let image_for_reporter = image.clone();

    let reporter: DockerStepReporter = Arc::new(move |step: &str| {
        let proxy_manager = Arc::clone(&proxy_manager);
        let app_handle = app_handle.clone();
        let session_id = session_id.clone();
        let image = image_for_reporter.clone();
        let step = step.to_string();
        tokio::spawn(async move {
            emit_docker_runtime_step(
                &proxy_manager,
                Some(&app_handle),
                &session_id,
                &image,
                &step,
                false,
                None,
            )
            .await;
        });
    });

    WorkspaceRuntimeManager::provision_runtime_with_steps(session, Some(reporter))
        .await
        .map_err(|error| error.to_string())
}

async fn complete_docker_provisioning(
    deps: &DockerProvisioningDeps,
    session_id: &str,
) -> Result<(), String> {
    let image = {
        let active = deps.active_sessions.read().await;
        active
            .get(session_id)
            .and_then(|session| session.metadata.docker_config.as_ref())
            .map(|config| config.image.clone())
    };

    if let Some(image) = image.as_ref() {
        emit_docker_runtime_step(
            &deps.proxy_manager,
            Some(&deps.app_handle),
            session_id,
            image,
            "Docker workspace ready",
            false,
            None,
        )
        .await;
    }

    crate::agent::lifecycle::update_session_status(
        &deps.session_repo,
        &deps.active_sessions,
        &deps.app_handle,
        session_id,
        SessionStatus::Idle,
    )
    .await?;

    drain_and_start_pending_workflows(deps, session_id).await
}

async fn fail_docker_provisioning(
    deps: &DockerProvisioningDeps,
    session: &SessionMetadata,
    error: &str,
    delete_session_on_failure: bool,
) -> Result<(), String> {
    let image = session
        .docker_config
        .as_ref()
        .map(|config| config.image.clone())
        .unwrap_or_else(|| "unknown".to_string());

    emit_docker_runtime_step(
        &deps.proxy_manager,
        Some(&deps.app_handle),
        &session.id,
        &image,
        "Docker workspace setup failed",
        true,
        Some(error),
    )
    .await;

    if delete_session_on_failure {
        let _ = deps.session_repo.delete_session(&session.id).await;
        let mut active = deps.active_sessions.write().await;
        active.remove(&session.id);
        deps.proxy_manager.destroy_proxy(&session.id).await;
        let _ = WorkspaceRuntimeManager::remove_runtime_for_session(session).await;
        return Ok(());
    }

    crate::agent::lifecycle::update_session_status(
        &deps.session_repo,
        &deps.active_sessions,
        &deps.app_handle,
        &session.id,
        SessionStatus::Error,
    )
    .await?;

    Ok(())
}

async fn emit_docker_runtime_step(
    proxy_manager: &MCPServiceProxyManager,
    app_handle: Option<&AppHandle>,
    session_id: &str,
    image: &str,
    step: &str,
    failed: bool,
    error: Option<&str>,
) {
    let image = image.to_string();
    let step = step.to_string();
    let error = error.map(str::to_string);

    let _ = proxy_manager
        .update_runtime_state(session_id, app_handle, move |state| {
            let is_ready = step == "Docker workspace ready";
            state.phase = if failed {
                SessionRuntimePhase::Failed
            } else if is_ready {
                SessionRuntimePhase::Ready
            } else {
                SessionRuntimePhase::Initializing
            };
            state.initialization.current_step = Some(step.clone());
            state.initialization.result = if failed {
                SessionRuntimeInitResult::Failed
            } else if is_ready {
                SessionRuntimeInitResult::Success
            } else {
                SessionRuntimeInitResult::Pending
            };
            state.initialization.error = error.clone();
            state.initialization.docker = Some(SessionRuntimeDockerState {
                image,
                step: Some(step),
                progress: None,
                error,
            });
            if is_ready {
                state.recompute_summary();
            }
        })
        .await;
}

async fn drain_and_start_pending_workflows(
    deps: &DockerProvisioningDeps,
    session_id: &str,
) -> Result<(), String> {
    let pending_ids: Vec<String> = {
        let active = deps.active_sessions.read().await;
        let Some(session) = active.get(session_id) else {
            return Ok(());
        };
        let mut pending = session.pending_events.write().await;
        pending.drain_messages()
    };

    if pending_ids.is_empty() {
        return Ok(());
    }

    let repo = crate::state::get_message_repository();
    let messages = repo.get_by_ids(pending_ids).await.map_err(|error| {
        format!("Failed to load pending messages after Docker provisioning: {error}")
    })?;

    for (index, message) in messages.into_iter().enumerate() {
        if index == 0 {
            crate::agent::workflow::start_workflow(
                &deps.session_repo,
                &deps.active_sessions,
                &deps.proxy_manager,
                &deps.app_handle,
                session_id.to_string(),
                message,
            )
            .await?;
        } else {
            MessageService::queue_user_message(&deps.active_sessions, session_id, &message).await?;
        }
    }

    Ok(())
}
