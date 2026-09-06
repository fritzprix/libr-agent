use super::{format_session_label, DbError, SessionMetadata, SessionRepository, SessionStatus};

/// Build service context for direct child sessions of the current session.
///
/// This lists child sessions sorted by their last activity (since `get_child_sessions` returns
/// sessions ordered by `updated_at` descending), showing their IDs, names, and execution statuses.
pub async fn build_child_sessions_context(
    repo: &dyn SessionRepository,
    session_id: &str,
) -> Result<Option<String>, DbError> {
    let children = repo.get_child_sessions(session_id).await?;
    Ok(format_child_sessions_context(&children))
}

/// Formats a list of child sessions into a markdown list.
pub fn format_child_sessions_context(children: &[SessionMetadata]) -> Option<String> {
    if children.is_empty() {
        return None;
    }

    let mut parts = vec!["## Child Sessions".to_string(), String::new()];

    let max_render = 20;
    let render_count = children.len().min(max_render);

    for child in &children[..render_count] {
        let label = format_session_label(child);
        parts.push(format!(
            "- {} {} (status: {})",
            format_assistant_tag(child),
            label,
            child.status.as_str()
        ));
    }

    if children.len() > max_render {
        parts.push(format!(
            "- ... and {} more omitted",
            children.len() - max_render
        ));
    }

    Some(parts.join("\n"))
}

/// Formats a compact status inventory of child sessions (IDs retained for targeting).
///
/// Keeps the routing guidance compact because the block is injected into synthetic
/// `<session-context>` user messages on every turn with live child sessions.
pub fn format_active_sessions_notice(children: &[SessionMetadata]) -> Option<String> {
    if children.is_empty() {
        return None;
    }

    let mut idle_sessions = Vec::new();
    let mut paused_sessions = Vec::new();
    let mut error_sessions = Vec::new();
    let mut busy_sessions = Vec::new();

    for child in children {
        match child.status {
            SessionStatus::Idle => idle_sessions.push(child),
            SessionStatus::Paused => paused_sessions.push(child),
            SessionStatus::Error => error_sessions.push(child),
            SessionStatus::Busy | SessionStatus::Queued | SessionStatus::Provisioning => {
                busy_sessions.push(child)
            }
        }
    }

    let total_count = children.len();
    let mut parts = vec![format!(
        "### Sub-Agents ({total_count}) — Reuse suitable idle sessions with the same assistant configuration via agent__messageToSession; use agent__startSession when no suitable session exists or separate parallel/isolation is needed."
    )];

    let limit_per_group = 5;

    format_group_notice(&mut parts, "Idle", &idle_sessions, limit_per_group);
    format_group_notice(&mut parts, "Paused", &paused_sessions, limit_per_group);
    format_group_notice(&mut parts, "Error", &error_sessions, limit_per_group);
    format_group_notice(&mut parts, "Busy", &busy_sessions, limit_per_group);

    Some(parts.join("\n"))
}

/// Helper: one status line with short ids (and optional truncated names).
fn format_group_notice(
    parts: &mut Vec<String>,
    status_label: &str,
    sessions: &[&SessionMetadata],
    limit: usize,
) {
    if sessions.is_empty() {
        return;
    }

    let mut entries: Vec<String> = sessions
        .iter()
        .take(limit)
        .map(|child| {
            let display_id = crate::utils::session_id::display_session_id(&child.id);
            let name_str = child.name.as_deref().unwrap_or("").trim();
            let role_tag = format_assistant_tag(child);
            if name_str.is_empty() {
                format!("`{display_id}` {role_tag}")
            } else {
                let short_name = if name_str.chars().count() > 28 {
                    format!("{}...", name_str.chars().take(25).collect::<String>())
                } else {
                    name_str.to_string()
                };
                format!("`{display_id}` {role_tag} \"{short_name}\"")
            }
        })
        .collect();

    if sessions.len() > limit {
        entries.push(format!("+{} more", sessions.len() - limit));
    }

    parts.push(format!("- {status_label}: {}", entries.join(", ")));
}

fn format_assistant_tag(session: &SessionMetadata) -> String {
    match session
        .assistant_id
        .as_deref()
        .map(str::trim)
        .filter(|assistant_id| !assistant_id.is_empty())
    {
        Some(assistant_id) => format!("[assistant:{assistant_id}]"),
        None => "[unbound]".to_string(),
    }
}
