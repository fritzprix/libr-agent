use super::{format_session_label, DbError, SessionMetadata, SessionRepository};

fn format_session_label_with_status(session: &SessionMetadata) -> String {
    format!(
        "{} [{}]",
        format_session_label(session),
        session.status.as_str()
    )
}

pub async fn build_explicit_org_layer_context(
    repo: &dyn SessionRepository,
    session: &SessionMetadata,
) -> Result<Option<String>, DbError> {
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

    let mut siblings: Vec<&SessionMetadata> = all_sessions
        .iter()
        .filter(|candidate| candidate.id != session.id)
        .filter(|candidate| candidate.org_id.as_deref() == Some(org_id.as_str()))
        .filter(|candidate| {
            candidate.org_root_session_id.as_deref() == Some(org_root_session_id.as_str())
        })
        .filter(|candidate| candidate.depth == session.depth)
        .filter(|candidate| candidate.parent_session_id == session.parent_session_id)
        .collect();

    // Deterministically sort siblings by updated_at DESC, then id DESC
    siblings.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| b.id.cmp(&a.id))
    });

    let siblings: Vec<&SessionMetadata> = siblings.into_iter().take(5).collect();
    let teamwork_artifact_root =
        crate::session::get_session_manager()
            .ok()
            .map(|session_manager| {
                crate::session::teamwork_artifact_dir_for_session(
                    session_manager,
                    &org_root_session_id,
                )
            });

    let mut parts = vec![
        "### Explicit Org Layer".to_string(),
        String::new(),
        format!("- Org: {} (ID: {})", org_name, org_id),
        format!("- Depth: {}", depth),
    ];

    if let Some(teamwork_artifact_root) = teamwork_artifact_root {
        parts.push(format!(
            "- Teamwork Artifact Root: {}",
            teamwork_artifact_root.display()
        ));
        parts.push(
            "- Teamwork Access Alias: @teamwork/... (e.g. @teamwork/coordination/KANBAN.md)"
                .to_string(),
        );
        parts.push(
            "- Teamwork SSOT: the teamwork artifact root is canonical; workspaceOverride does not change it."
                .to_string(),
        );
        parts.push("- Before delegating: read @teamwork/coordination/KANBAN.md".to_string());
        parts.push("- Before handoff: update @teamwork/coordination/HANDOFF.md".to_string());
        parts.push(
            "- Role context: @teamwork/agents.md, @teamwork/MISSION.md, @teamwork/ROLES.md"
                .to_string(),
        );
        parts.push(
            "- Refresh Note: teamwork scaffold updates apply on a later execution step, not retroactively in the current turn."
                .to_string(),
        );
    }

    if let Some(parent_session) = parent {
        parts.push(format!(
            "- Parent: {}",
            format_session_label_with_status(parent_session)
        ));
    }

    if !siblings.is_empty() {
        parts.push("- Siblings at same depth:".to_string());
        for sibling in siblings {
            parts.push(format!("  - {}", format_session_label_with_status(sibling)));
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
