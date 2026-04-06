use super::{DbError, SessionMetadata, SessionRepository};

pub async fn build_explicit_org_layer_context(
    repo: &dyn SessionRepository,
    session_id: &str,
) -> Result<Option<String>, DbError> {
    let session = repo.get_session(session_id).await?;

    let Some(session) = session else {
        return Ok(None);
    };

    let Some(org_name) = session.org_name.clone() else {
        return Ok(None);
    };
    let Some(org_id) = session.org_id.clone() else {
        return Ok(None);
    };
    let Some(org_root_session_id) = session.org_root_session_id.clone() else {
        return Ok(None);
    };

    let all_sessions = repo.get_all_sessions().await?;
    let depth = session.depth.unwrap_or(0);
    let parent = session
        .parent_session_id
        .as_ref()
        .and_then(|parent_id| find_session(&all_sessions, parent_id));

    let siblings: Vec<&SessionMetadata> = all_sessions
        .iter()
        .filter(|candidate| candidate.id != session.id)
        .filter(|candidate| candidate.org_id.as_deref() == Some(org_id.as_str()))
        .filter(|candidate| {
            candidate.org_root_session_id.as_deref() == Some(org_root_session_id.as_str())
        })
        .filter(|candidate| candidate.depth == session.depth)
        .filter(|candidate| candidate.parent_session_id == session.parent_session_id)
        .take(5)
        .collect();

    let mut parts = vec![
        "## Explicit Org Layer".to_string(),
        String::new(),
        format!("- Org: {}", org_name),
        format!("- Depth: {}", depth),
    ];

    if let Some(parent_session) = parent {
        parts.push(format!(
            "- Parent: {}",
            format_session_label(parent_session)
        ));
    }

    if !siblings.is_empty() {
        parts.push("- Siblings at same depth:".to_string());
        for sibling in siblings {
            parts.push(format!("  - {}", format_session_label(sibling)));
        }
    }

    Ok(Some(parts.join("\n")))
}

fn find_session<'a>(
    sessions: &'a [SessionMetadata],
    session_id: &str,
) -> Option<&'a SessionMetadata> {
    sessions.iter().find(|session| session.id == session_id)
}

fn format_session_label(session: &SessionMetadata) -> String {
    match session.name.as_deref() {
        Some(name) if !name.is_empty() => format!("{} — {}", session.id, name),
        _ => session.id.clone(),
    }
}
