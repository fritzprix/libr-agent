use crate::repositories::session_repository::SessionRepository;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct SessionToolAccess {
    pub session_id: Option<String>,
    pub allowed_builtin_aliases: Option<HashSet<String>>,
    pub allowed_external_server_ids: Option<HashSet<String>>,
    pub agent_id: Option<String>,
}

impl SessionToolAccess {
    pub fn builtin_status(&self, alias: &str) -> (&'static str, Option<String>) {
        match &self.allowed_builtin_aliases {
            Some(allowed) if allowed.contains(alias) => ("[Ready]", None),
            Some(_) => ("[Unsupported in current session]", None),
            None => ("[Ready]", None),
        }
    }

    pub fn external_status(
        &self,
        server_id: &str,
        server_name: &str,
    ) -> (&'static str, Option<String>) {
        match &self.allowed_external_server_ids {
            Some(allowed) if allowed.contains(server_id) => ("[Ready]", None),
            Some(_) => ("[Unsupported in current session]", None),
            None => {
                let _ = (server_name, server_id);
                ("[Unsupported in current session]", None)
            }
        }
    }

    /// Session-access label for verify/list reports.
    ///
    /// Without a session context, returns `"Unknown"` instead of `"Unsupported…"`,
    /// so text and structured `sessionStatus` stay aligned and do not imply a deny decision.
    pub fn external_access_report(
        &self,
        session_id: Option<&str>,
        server_id: &str,
        server_name: &str,
    ) -> (&'static str, String) {
        match session_id {
            Some(sid) => {
                let (status, _) = self.external_status(server_id, server_name);
                (
                    status,
                    format!(
                        "Session access for '{}': {} — verification does not grant tools to an already-running session.",
                        sid, status
                    ),
                )
            }
            None => (
                "Unknown",
                "Session access: unknown (no session context) — verification only checks connectivity."
                    .to_string(),
            ),
        }
    }
}

pub async fn load_session_tool_access(session_id: Option<&str>) -> SessionToolAccess {
    let Some(session_id) = session_id else {
        return SessionToolAccess {
            session_id: None,
            allowed_builtin_aliases: None,
            allowed_external_server_ids: None,
            agent_id: None,
        };
    };

    let repo = crate::state::get_session_repository();
    let session = match repo.get_session(session_id).await {
        Ok(Some(session)) => session,
        _ => {
            return SessionToolAccess {
                session_id: Some(session_id.to_string()),
                allowed_builtin_aliases: None,
                allowed_external_server_ids: None,
                agent_id: None,
            };
        }
    };

    let agent_config = match crate::agent::resolve_agent_config(&session).await {
        Ok(config) => config,
        Err(_) => {
            return SessionToolAccess {
                session_id: Some(session_id.to_string()),
                allowed_builtin_aliases: None,
                allowed_external_server_ids: None,
                agent_id: None,
            };
        }
    };

    let agent_id = agent_config.id.clone().or(session.assistant_id.clone());

    let allowed_builtin_aliases = Some(
        crate::agent::tools::runtime_allowed_builtin_service_aliases(&agent_config)
            .into_iter()
            .collect::<HashSet<_>>(),
    );

    let allowed_external_server_ids = Some(
        agent_config
            .mcp_server_ids
            .into_iter()
            .collect::<HashSet<_>>(),
    );

    SessionToolAccess {
        session_id: Some(session_id.to_string()),
        allowed_builtin_aliases,
        allowed_external_server_ids,
        agent_id,
    }
}
