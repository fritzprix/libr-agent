use crate::agent::AgentConfig;
use crate::entity::assistant::Model as AssistantModel;
use crate::repositories::session_repository::SessionMetadata;

/// Resolve the assistant id for a session from the dedicated column.
pub fn extract_assistant_id_from_session(session: &SessionMetadata) -> Option<String> {
    session
        .assistant_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Build assistant-backed config from the assistants table row.
pub fn build_agent_config_from_assistant(
    assistant: &AssistantModel,
) -> Result<AgentConfig, String> {
    let mut config = AgentConfig::from_json(&assistant.config)
        .map_err(|error| format!("Invalid assistant configuration: {}", error))?;
    config.id = Some(assistant.id.clone());
    config.name = assistant.name.clone();
    Ok(config)
}

/// Overlay session lineage and org metadata onto a resolved config.
///
/// Session table columns are authoritative for lineage/org fields.
pub fn apply_session_lineage(config: &mut AgentConfig, session: &SessionMetadata) {
    config.parent_session_id = session
        .parent_session_id
        .clone()
        .or(config.parent_session_id.clone());
    config.lineage_id = session.lineage_id.clone().or(config.lineage_id.clone());
    config.depth = session.depth.or(config.depth);
    config.max_depth = session.max_depth.or(config.max_depth);
    config.max_fanout = session.max_fanout.or(config.max_fanout);
    config.org_id = session.org_id.clone().or(config.org_id.clone());
    config.org_name = session.org_name.clone().or(config.org_name.clone());
    config.org_root_session_id = session
        .org_root_session_id
        .clone()
        .or(config.org_root_session_id.clone());
}

async fn load_assistant_by_id(assistant_id: &str) -> Result<Option<AssistantModel>, String> {
    use crate::repositories::assistant_repository::AssistantRepository;

    crate::state::get_assistant_repository()
        .get_assistant(assistant_id)
        .await
        .map_err(|error| format!("Failed to fetch assistant {}: {}", assistant_id, error))
}

/// Resolve the effective runtime agent config for a session.
///
/// Loads live assistant settings from the assistants table (SSOT) using
/// `sessions.assistant_id`, then overlays session lineage/org columns.
pub async fn resolve_agent_config(session: &SessionMetadata) -> Result<AgentConfig, String> {
    let assistant_id = extract_assistant_id_from_session(session).ok_or_else(|| {
        format!(
            "Session {} has no assistant_id; start a new session with an assistant",
            session.id
        )
    })?;

    let assistant = load_assistant_by_id(&assistant_id).await?.ok_or_else(|| {
        format!(
            "Assistant {} not found for session {}",
            assistant_id, session.id
        )
    })?;

    let mut config = build_agent_config_from_assistant(&assistant)?;
    apply_session_lineage(&mut config, session);
    Ok(config)
}

/// Fingerprint assistant-derived stable prompt inputs for cache invalidation.
pub fn stable_prompt_source_key(config: &AgentConfig) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    config.id.hash(&mut hasher);
    config.name.hash(&mut hasher);
    config.system_prompt.hash(&mut hasher);
    config.description.hash(&mut hasher);
    config.temperature.map(f32::to_bits).hash(&mut hasher);
    config.max_tokens.hash(&mut hasher);
    config.mcp_server_ids.hash(&mut hasher);
    config.allowed_built_in_service_aliases.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::SessionStatus;

    fn sample_session_metadata(assistant_id: Option<String>) -> SessionMetadata {
        SessionMetadata {
            id: "session-1".to_string(),
            name: Some("Test".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            assistant_id,
            parent_session_id: Some("parent-1".to_string()),
            lineage_id: Some("lineage-1".to_string()),
            depth: Some(2),
            max_depth: Some(5),
            max_fanout: Some(3),
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
        }
    }

    #[test]
    fn extract_assistant_id_reads_column() {
        let session = sample_session_metadata(Some("from-column".to_string()));

        assert_eq!(
            extract_assistant_id_from_session(&session).as_deref(),
            Some("from-column")
        );
    }

    #[test]
    fn apply_session_lineage_prefers_session_columns() {
        let mut config = AgentConfig {
            parent_session_id: Some("blob-parent".to_string()),
            lineage_id: Some("blob-lineage".to_string()),
            depth: Some(0),
            max_depth: Some(1),
            max_fanout: Some(1),
            ..AgentConfig::default()
        };
        let session = sample_session_metadata(None);

        apply_session_lineage(&mut config, &session);

        assert_eq!(config.parent_session_id.as_deref(), Some("parent-1"));
        assert_eq!(config.lineage_id.as_deref(), Some("lineage-1"));
        assert_eq!(config.depth, Some(2));
        assert_eq!(config.max_depth, Some(5));
        assert_eq!(config.max_fanout, Some(3));
    }

    #[test]
    fn stable_prompt_source_key_changes_when_system_prompt_changes() {
        let first = AgentConfig {
            system_prompt: "You are helpful.".to_string(),
            ..Default::default()
        };

        let second = AgentConfig {
            system_prompt: "You are concise.".to_string(),
            ..first.clone()
        };

        assert_ne!(
            stable_prompt_source_key(&first),
            stable_prompt_source_key(&second)
        );
    }
}
