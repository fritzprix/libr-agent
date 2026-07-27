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
        parts.push(format!("- {} (status: {})", label, child.status.as_str()));
    }

    if children.len() > max_render {
        parts.push(format!(
            "- ... and {} more omitted",
            children.len() - max_render
        ));
    }

    Some(parts.join("\n"))
}

/// Formats a notice listing active sessions available for reuse via messageToSession.
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
    let mut parts = vec![
        format!("### Sub-Agent Sessions ({} total, reuse via messageToSession)", total_count),
        String::new(),
        "⚠️ **Reuse Existing Sessions First**: Avoid `startSession` — reuse idle/paused/failed sessions via `messageToSession(sessionId)` to preserve context.".to_string(),
        String::new(),
    ];

    let limit_per_group = 5;

    // Ready to Reuse (Idle)
    format_group_notice(
        &mut parts,
        "Ready to Reuse (Idle):",
        "These sessions are idle and ready for new instructions. Send a message to assign a new task.",
        &idle_sessions,
        limit_per_group,
    );

    // Suspended (Paused)
    format_group_notice(
        &mut parts,
        "Suspended (Paused):",
        "These sessions were suspended (e.g. waiting for input or approval). Send a message to resume them.",
        &paused_sessions,
        limit_per_group,
    );

    // Failed (Error)
    format_group_notice(
        &mut parts,
        "Failed (Error):",
        "These sessions encountered an error. Send a message to retry or recover them.",
        &error_sessions,
        limit_per_group,
    );

    // Running (Busy)
    format_group_notice(
        &mut parts,
        "Running (Busy):",
        "These sessions are currently executing a task. Do NOT send messages to them unless necessary; wait for them to finish.",
        &busy_sessions,
        limit_per_group,
    );

    Some(parts.join("\n"))
}

/// Helper function to format a single session status group in the reuse notice.
fn format_group_notice(
    parts: &mut Vec<String>,
    title: &str,
    description: &str,
    sessions: &[&SessionMetadata],
    limit: usize,
) {
    if sessions.is_empty() {
        return;
    }

    parts.push(format!("- **{}**", title));
    parts.push(format!("  {}", description));
    for child in sessions.iter().take(limit) {
        let short_id: String = child.id.chars().take(8).collect();
        let name_str = child.name.as_deref().unwrap_or("");
        let short_name = if name_str.chars().count() > 35 {
            format!("{}...", name_str.chars().take(32).collect::<String>())
        } else {
            name_str.to_string()
        };
        parts.push(format!("  - `{}` (name: \"{}\")", short_id, short_name));
    }
    if sessions.len() > limit {
        parts.push(format!(
            "  - ... {} more sessions in this group",
            sessions.len() - limit,
        ));
    }
}
