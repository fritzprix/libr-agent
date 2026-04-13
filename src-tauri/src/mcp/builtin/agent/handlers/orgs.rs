use serde_json::{json, Value};

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::builtin::session_api::utils::{build_agent_tool_data, read_required_string};
use crate::mcp::types::MCPResult;
use crate::repositories::session_repository::SessionRepository;

use super::super::AgentServer;
use super::{
    caller_session_not_found_result, invalid_explicit_org_result, missing_explicit_org_result,
    read_optional_string,
};

pub async fn create_org(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session = match manager.get_session(caller_session_id).await? {
        Some(session) => session,
        None => return Ok(caller_session_not_found_result(caller_session_id)),
    };

    if session.parent_session_id.is_some() {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            "createOrg must be called from a top-level root session, not a delegated child"
                .to_string(),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Resume the top-level/root session first.".to_string(),
            "Then call createOrg(name=\"...\") from that root session.".to_string(),
        ])
        .to_mcp_result());
    }

    let org_name = read_required_string(&args, "name")?;
    if let (Some(existing_org_id), Some(existing_org_name), Some(existing_root_id)) = (
        session.org_id.clone(),
        session.org_name.clone(),
        session.org_root_session_id.clone(),
    ) {
        let message = format!(
            "Current session already owns explicit org '{}' (ID: `{}`, root session: `{}`).",
            existing_org_name, existing_org_id, existing_root_id
        );
        let mut response_data = build_agent_tool_data(
            "createOrg",
            "org",
            Some(&existing_org_id),
            &message,
            "success",
            vec![
                json!({
                    "toolName": "spawnOrgAgent",
                    "reason": "Spawn an explicit org member under this org.",
                }),
                json!({
                    "toolName": "getOrg",
                    "reason": "Inspect the existing org summary.",
                    "args": { "orgId": existing_org_id.clone() }
                }),
            ],
        );
        response_data.insert("orgId".to_string(), Value::String(existing_org_id));
        response_data.insert("orgName".to_string(), Value::String(existing_org_name));
        response_data.insert(
            "orgRootSessionId".to_string(),
            Value::String(existing_root_id),
        );
        return Ok(SuccessHint::new(message, vec![])
            .to_mcp_result_with_data(Some(Value::Object(response_data))));
    }

    let org_id = format!("org-{}", uuid::Uuid::new_v4().simple());
    let org_root_session_id = session.id.clone();
    let session_repo = crate::state::get_session_repository();
    session_repo
        .update_org_identity(
            caller_session_id,
            Some(org_id.clone()),
            Some(org_name.clone()),
            Some(org_root_session_id.clone()),
        )
        .await
        .map_err(|error| format!("Failed to persist org identity: {}", error))?;

    let message = format!(
        "Explicit org created.\n\nOrg: {} (ID: `{}`)\nRoot session: `{}`\n\nOnly sessions created through spawnOrgAgent under this org will appear in Org view.",
        org_name, org_id, org_root_session_id
    );
    let mut response_data = build_agent_tool_data(
        "createOrg",
        "org",
        Some(&org_id),
        &message,
        "success",
        vec![
            json!({
                "toolName": "spawnOrgAgent",
                "reason": "Create the first explicit org member session.",
            }),
            json!({
                "toolName": "getOrg",
                "reason": "Inspect the newly created org summary.",
                "args": { "orgId": org_id.clone() }
            }),
        ],
    );
    response_data.insert("orgId".to_string(), Value::String(org_id));
    response_data.insert("orgName".to_string(), Value::String(org_name));
    response_data.insert(
        "orgRootSessionId".to_string(),
        Value::String(org_root_session_id),
    );

    Ok(SuccessHint::new(message, vec![])
        .to_mcp_result_with_data(Some(Value::Object(response_data))))
}

pub async fn get_org(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let caller_session = match manager.get_session(caller_session_id).await? {
        Some(session) => session,
        None => return Ok(caller_session_not_found_result(caller_session_id)),
    };
    let requested_org_id = read_optional_string(&args, "orgId")?;
    let target_org_id = match requested_org_id.or_else(|| caller_session.org_id.clone()) {
        Some(org_id) => org_id,
        None => return Ok(missing_explicit_org_result()),
    };

    let sessions = manager.get_all_sessions().await?;
    let mut members: Vec<_> = sessions
        .into_iter()
        .filter(|session| session.org_id.as_deref() == Some(target_org_id.as_str()))
        .collect();

    if members.is_empty() {
        return Ok(guided_error(
            ErrorCategory::ResourceNotFound,
            format!("No sessions found for explicit org '{}'.", target_org_id),
            ToolGroup::Agent,
        )
        .to_mcp_result());
    }

    members.sort_by(|left, right| {
        let left_depth = left.depth.unwrap_or(0);
        let right_depth = right.depth.unwrap_or(0);
        left_depth
            .cmp(&right_depth)
            .then_with(|| left.created_at.cmp(&right.created_at))
    });

    let root_session = match members.iter().find(|session| {
        session
            .org_root_session_id
            .as_deref()
            .is_some_and(|root_id| root_id == session.id)
    }) {
        Some(root_session) => root_session,
        None => return Ok(invalid_explicit_org_result(&target_org_id)),
    };

    let org_name = root_session
        .org_name
        .clone()
        .unwrap_or_else(|| target_org_id.clone());

    let mut member_lines =
        String::from("| Name | Status | Depth | Session ID |\n|---|---|---|---|\n");
    member_lines.push_str(
        &members
            .iter()
            .map(|session| {
                let name_clean = session
                    .name
                    .clone()
                    .unwrap_or_else(|| session.id.clone())
                    .replace('|', "\\|")
                    .replace('\n', " ");
                format!(
                    "| {} | {} | {} | `{}` |",
                    name_clean,
                    session.status.as_str(),
                    session.depth.unwrap_or(0),
                    session.id
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );

    let busy_count = members
        .iter()
        .filter(|session| session.status == crate::repositories::SessionStatus::Busy)
        .count();
    let message = format!(
        "Explicit org summary\n\nOrg: {} (ID: `{}`)\nRoot session: `{}`\nMembers: {} (busy: {})\n\n{}",
        org_name,
        target_org_id,
        root_session.id,
        members.len(),
        busy_count,
        member_lines
    );
    let mut response_data = build_agent_tool_data(
        "getOrg",
        "org",
        Some(&target_org_id),
        &message,
        "success",
        vec![json!({
            "toolName": "spawnOrgAgent",
            "reason": "Add another explicit org member under this org.",
        })],
    );
    response_data.insert("orgId".to_string(), Value::String(target_org_id));
    response_data.insert("orgName".to_string(), Value::String(org_name));
    response_data.insert(
        "orgRootSessionId".to_string(),
        Value::String(root_session.id.clone()),
    );
    response_data.insert("memberCount".to_string(), json!(members.len()));
    response_data.insert("busyCount".to_string(), json!(busy_count));

    Ok(SuccessHint::new(message, vec![])
        .to_mcp_result_with_data(Some(Value::Object(response_data))))
}
