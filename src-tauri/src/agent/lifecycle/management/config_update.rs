use crate::agent::state::AgentSession;
use crate::repositories::session_repository::SessionRepository;
use crate::services::hooks::lifecycle::{
    post_model_switch, pre_model_switch, HookSources, ModelSwitch,
};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Update model/provider (and optionally assistant binding) for an existing session
#[allow(clippy::too_many_arguments)]
pub async fn update_session_config(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    _app_handle: &AppHandle,
    session_id: &str,
    model: Option<String>,
    provider: Option<String>,
    assistant_id: Option<String>,
    recursive: Option<bool>,
    switch_source: &'static str,
) -> Result<(), String> {
    let mut session_ids = vec![session_id.to_string()];

    if recursive.unwrap_or(false) {
        let descendant_ids =
            crate::services::SessionCleanupService::collect_descendant_ids(session_id).await?;
        session_ids.extend(descendant_ids);
    }

    // Every affected session's `PreModelSwitch` hooks run before anything is
    // written, so one block cancels the whole update (#3008).
    let switches = planned_model_switches(
        session_repo,
        active_sessions,
        &session_ids,
        model.as_deref(),
        provider.as_deref(),
        switch_source,
    )
    .await?;
    for (id, switch) in &switches {
        if let Some(sources) = HookSources::for_session(id) {
            pre_model_switch(&sources, id, switch).await?;
        }
    }

    for id in &session_ids {
        session_repo
            .update_session_config(id, model.clone(), provider.clone())
            .await
            .map_err(|e| format!("Failed to update session config for {}: {}", id, e))?;

        if id == session_id {
            if let Some(ref assistant_id) = assistant_id {
                session_repo
                    .update_assistant_id(id, Some(assistant_id.clone()))
                    .await
                    .map_err(|e| {
                        format!("Failed to update session assistant_id for {}: {}", id, e)
                    })?;
            }
        }
    }

    let mut active = active_sessions.write().await;
    let now = chrono::Utc::now().timestamp_millis();
    for id in &session_ids {
        if let Some(session) = active.get_mut(id) {
            if let Some(ref m) = model {
                session.metadata.model = m.clone();
            }
            if let Some(ref p) = provider {
                session.metadata.provider = p.clone();
            }
            if id == session_id {
                if let Some(ref id) = assistant_id {
                    session.metadata.assistant_id = Some(id.clone());
                }
            }
            session.metadata.updated_at = now;
            *session.cached_stable_prompt.write().await = None;
        }
    }
    drop(active);

    for (id, switch) in &switches {
        if let Some(sources) = HookSources::for_session(id) {
            post_model_switch(&sources, id, switch).await;
        }
    }

    // Emit resource update event to trigger frontend session list refresh
    crate::agent::tauri_events::emit_resource_updated(
        "session",
        "update",
        Some(session_id.to_string()),
    );

    log::info!(
        "Updated agent config for session: {} (recursive: {:?})",
        session_id,
        recursive
    );

    Ok(())
}

/// The sessions whose model or provider this update changes.
async fn planned_model_switches(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_ids: &[String],
    model: Option<&str>,
    provider: Option<&str>,
    source: &'static str,
) -> Result<Vec<(String, ModelSwitch)>, String> {
    let mut switches = Vec::new();
    for id in session_ids {
        let Some(current) = session_repo
            .get_session(id)
            .await
            .map_err(|e| format!("Failed to load session {}: {}", id, e))?
        else {
            continue;
        };
        let changes = model.is_some_and(|next| next != current.model)
            || provider.is_some_and(|next| next != current.provider);
        if !changes {
            continue;
        }
        let messages = active_sessions
            .read()
            .await
            .get(id)
            .map(|session| session.messages.clone());
        let context_tokens = match messages {
            Some(messages) => messages
                .read()
                .await
                .iter()
                .rev()
                .find_map(|message| message.prompt_tokens),
            None => None,
        };
        switches.push((
            id.clone(),
            ModelSwitch {
                to_model: model.unwrap_or(&current.model).to_string(),
                from_model: current.model,
                source,
                context_tokens,
            },
        ));
    }
    Ok(switches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::{InMemorySessionRepository, SessionMetadata, SessionStatus};

    fn metadata(session_id: &str, model: &str, provider: &str) -> SessionMetadata {
        SessionMetadata {
            id: session_id.to_string(),
            name: None,
            status: SessionStatus::Idle,
            model: model.to_string(),
            provider: provider.to_string(),
            assistant_id: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            org_id: None,
            org_name: None,
            org_root_session_id: None,
            created_at: 0,
            updated_at: 0,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            execution_mode: crate::execution_mode::ExecutionMode::Normal,
            workspace_override: None,
            workspace_isolation: crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
            docker_config: None,
            docker_container_name: None,
            docker_host_workspace_path: None,
            status_reason: None,
            forked_from: None,
        }
    }

    fn no_active_sessions() -> Arc<RwLock<HashMap<String, AgentSession>>> {
        Arc::new(RwLock::new(HashMap::new()))
    }

    #[tokio::test]
    async fn planned_model_switches_reports_only_sessions_that_change() {
        // Recursive list: two sessions move, one already matches the new
        // model, and one id is absent from the repository (#3008 G1.3).
        let repo = Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>;
        repo.upsert_session(&metadata("root", "gpt-5.4", "openai"))
            .await
            .unwrap();
        repo.upsert_session(&metadata("child", "gpt-5.4", "openai"))
            .await
            .unwrap();
        repo.upsert_session(&metadata("same", "claude", "anthropic"))
            .await
            .unwrap();

        let ids = vec![
            "root".to_string(),
            "child".to_string(),
            "same".to_string(),
            "missing".to_string(),
        ];
        let switches = planned_model_switches(
            &repo,
            &no_active_sessions(),
            &ids,
            Some("claude"),
            Some("anthropic"),
            "picker",
        )
        .await
        .unwrap();

        let planned: Vec<(&str, &str, &str)> = switches
            .iter()
            .map(|(id, switch)| {
                (
                    id.as_str(),
                    switch.from_model.as_str(),
                    switch.to_model.as_str(),
                )
            })
            .collect();
        assert_eq!(
            planned,
            vec![
                ("root", "gpt-5.4", "claude"),
                ("child", "gpt-5.4", "claude"),
            ],
            "changed sessions plan a switch; unchanged and missing sessions do not"
        );
        for (_, switch) in &switches {
            assert_eq!(switch.source, "picker");
            assert_eq!(switch.context_tokens, None);
        }
    }

    #[tokio::test]
    async fn planned_model_switches_plans_provider_only_change() {
        let repo = Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>;
        repo.upsert_session(&metadata("root", "gpt-5.4", "openai"))
            .await
            .unwrap();

        let switches = planned_model_switches(
            &repo,
            &no_active_sessions(),
            &["root".to_string()],
            None,
            Some("azure"),
            "sdk",
        )
        .await
        .unwrap();

        assert_eq!(switches.len(), 1);
        assert_eq!(switches[0].1.from_model, "gpt-5.4");
        assert_eq!(
            switches[0].1.to_model, "gpt-5.4",
            "a provider-only change keeps the model and still plans the switch"
        );
    }

    #[tokio::test]
    async fn planned_model_switches_is_empty_for_no_change() {
        let repo = Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>;
        repo.upsert_session(&metadata("root", "gpt-5.4", "openai"))
            .await
            .unwrap();

        let switches = planned_model_switches(
            &repo,
            &no_active_sessions(),
            &["root".to_string()],
            Some("gpt-5.4"),
            Some("openai"),
            "picker",
        )
        .await
        .unwrap();
        assert!(
            switches.is_empty(),
            "identical model and provider plan no switch"
        );
    }
}
