use super::lineage::{
    acquire_parent_spawn_guard, lineage_store, remove_lineage, resolve_spawn_lineage,
};
use super::{normalize_explicit_org, resolve_child_session_model_provider, AgentService};
use crate::agent::types::{CreateSessionRequest, CreateSessionResponse, SessionLineageMeta};
use crate::agent::{AgentConfig, AgentSessionManager};
use crate::entity::assistant::Model as AssistantModel;
use crate::models::chat::{Message, MessageSource};

impl AgentService {
    /// Create a new session with assistant ID and initial request (Spawn Agent logic)
    pub async fn spawn_agent(
        manager: &AgentSessionManager,
        body: CreateSessionRequest,
    ) -> Result<CreateSessionResponse, String> {
        Self::spawn_agent_with_source(manager, body, Some(MessageSource::AgentTool)).await
    }

    /// Create a new session with assistant ID and initial request, tagging the initial
    /// message with the provided source for transport-specific observability.
    pub async fn spawn_agent_with_source(
        manager: &AgentSessionManager,
        body: CreateSessionRequest,
        message_source: Option<MessageSource>,
    ) -> Result<CreateSessionResponse, String> {
        let assistant = load_assistant(&body.assistant_id).await?;
        let session_id = format!("session-{}", uuid::Uuid::new_v4());
        let session_name = build_session_name(
            body.name.clone(),
            &assistant.name,
            &session_id,
            &body.request,
        );
        let explicit_org = normalize_explicit_org(
            body.org_id.clone(),
            body.org_name.clone(),
            body.org_root_session_id.clone(),
        )?;
        let parent_session =
            load_parent_session(manager, body.parent_session_id.as_deref()).await?;
        let (resolved_model, resolved_provider) = resolve_child_session_model_provider(
            body.model.clone(),
            body.provider.clone(),
            parent_session.as_ref(),
        )?;
        let (mut agent_config, assistant_id) = build_agent_config(&assistant)?;
        let has_workspace_override = body.workspace_path.is_some();

        if let Some(path_str) = body.workspace_path.as_deref() {
            Self::validate_and_register_workspace_override(path_str, &session_id).await?;
        }

        let parent_spawn_guard =
            acquire_parent_spawn_guard(body.parent_session_id.as_deref()).await;
        let lineage_meta =
            match resolve_spawn_lineage(&session_id, &body, explicit_org.as_ref()).await {
                Ok(lineage_meta) => lineage_meta,
                Err(error) => {
                    drop(parent_spawn_guard);
                    if has_workspace_override {
                        cleanup_failed_spawn_workspace_registration(&session_id).await;
                    }
                    return Err(error);
                }
            };
        apply_lineage_to_config(&mut agent_config, &lineage_meta);

        let session = match manager
            .create_session(
                session_id.clone(),
                session_name,
                resolved_model,
                resolved_provider,
                agent_config,
            )
            .await
        {
            Ok(session) => session,
            Err(error) => {
                drop(parent_spawn_guard);
                if has_workspace_override {
                    cleanup_failed_spawn_workspace_registration(&session_id).await;
                }
                return Err(error);
            }
        };

        lineage_store()
            .write()
            .await
            .insert(session_id.clone(), lineage_meta.clone());
        drop(parent_spawn_guard);

        if let Some(parent_id) = lineage_meta.parent_session_id.as_deref() {
            if manager.get_yolo_mode(parent_id).await {
                let _ = manager.set_yolo_mode(&session_id, true).await;
            }
            if manager.get_unsafe_mode(parent_id).await {
                let _ = manager.set_unsafe_mode(&session_id, true).await;
            }
        }

        let initial_message = Message::new_user_message(
            session_id.clone(),
            body.request,
            message_source,
            assistant_id,
        );

        if let Err(error) = manager
            .start_workflow(session_id.clone(), initial_message)
            .await
        {
            log::error!(
                "Failed to start initial workflow for spawned agent: {}",
                error
            );
            remove_lineage(&session_id).await;

            if let Err(cleanup_err) = manager.delete_session(session_id.clone()).await {
                log::error!(
                    "Failed to clean up spawned session {} after workflow start error: {}",
                    session_id,
                    cleanup_err
                );
            }

            return Err(format!("Failed to start initial workflow: {}", error));
        }

        Ok(CreateSessionResponse {
            id: session.id,
            name: session.name,
            status: crate::repositories::SessionStatus::Busy
                .as_str()
                .to_string(),
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
}

async fn load_assistant(assistant_id: &str) -> Result<AssistantModel, String> {
    use crate::repositories::assistant_repository::AssistantRepository;

    crate::state::get_assistant_repository()
        .get_assistant(assistant_id)
        .await
        .map_err(|e| format!("Failed to fetch assistant: {}", e))?
        .ok_or_else(|| format!("Assistant not found: {}", assistant_id))
}

async fn load_parent_session(
    manager: &AgentSessionManager,
    parent_session_id: Option<&str>,
) -> Result<Option<crate::repositories::SessionMetadata>, String> {
    if let Some(parent_id) = parent_session_id {
        let parent_session = manager.get_session(parent_id).await?;
        if parent_session.is_none() {
            return Err(format!("Parent session not found: {}", parent_id));
        }
        Ok(parent_session)
    } else {
        Ok(None)
    }
}

fn build_agent_config(assistant: &AssistantModel) -> Result<(AgentConfig, Option<String>), String> {
    let mut agent_config = AgentConfig::from_json(&assistant.config)
        .map_err(|e| format!("Invalid assistant configuration: {}", e))?;
    agent_config.id = Some(assistant.id.clone());
    agent_config.name = assistant.name.clone();

    let assistant_id = agent_config.id.clone();
    Ok((agent_config, assistant_id))
}

fn build_session_name(
    requested_name: Option<String>,
    assistant_name: &str,
    session_id: &str,
    request: &str,
) -> Option<String> {
    requested_name.or_else(|| {
        let short_id = &session_id[session_id.len().saturating_sub(6)..];
        let preview: String = request.chars().take(40).collect();
        let trimmed = preview.trim();
        if trimmed.is_empty() {
            Some(format!("{} #{}", assistant_name, short_id))
        } else {
            Some(format!("{}: {} #{}", assistant_name, trimmed, short_id))
        }
    })
}

fn apply_lineage_to_config(agent_config: &mut AgentConfig, lineage_meta: &SessionLineageMeta) {
    agent_config.parent_session_id = lineage_meta.parent_session_id.clone();
    agent_config.lineage_id = Some(lineage_meta.lineage_id.clone());
    agent_config.depth = Some(lineage_meta.depth);
    agent_config.max_depth = lineage_meta.max_depth;
    agent_config.max_fanout = lineage_meta.max_fanout;
    agent_config.org_id = lineage_meta.org_id.clone();
    agent_config.org_name = lineage_meta.org_name.clone();
    agent_config.org_root_session_id = lineage_meta.org_root_session_id.clone();
}

async fn cleanup_failed_spawn_workspace_registration(session_id: &str) {
    let Ok(session_manager) = crate::session::get_session_manager() else {
        log::warn!(
            "Failed to get session manager while cleaning up workspace override for {}",
            session_id
        );
        return;
    };

    if let Err(error) = session_manager.remove_session(session_id).await {
        log::warn!(
            "Failed to clean up pre-registered workspace override for failed spawned session {}: {}",
            session_id,
            error
        );
    }
}
