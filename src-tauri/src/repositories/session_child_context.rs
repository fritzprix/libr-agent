use super::{format_session_label, DbError, SessionMetadata, SessionRepository};

/// Build service context for direct child sessions of the current session.
///
/// This lists child sessions sorted by their last activity (since `get_child_sessions` returns
/// sessions ordered by `updated_at` descending), showing their IDs, names, and execution statuses.
pub async fn build_child_sessions_context(
    repo: &dyn SessionRepository,
    session_id: &str,
) -> Result<Option<String>, DbError> {
    let children = repo.get_child_sessions(session_id).await?;

    if children.is_empty() {
        return Ok(None);
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

    Ok(Some(parts.join("\n")))
}
