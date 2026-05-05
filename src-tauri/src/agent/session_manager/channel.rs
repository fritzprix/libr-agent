use super::AgentSessionManager;
use crate::agent::channel_routing::{resolve_auto_routed_channel_target, ChannelRouteCandidate};
use crate::mcp::types::ChannelNotification;
use crate::models::chat::{Message, MessageSource};
use std::collections::HashMap;

pub async fn inject_channel_notification(
    manager: &AgentSessionManager,
    session_id: String,
    server_name: String,
    notification: ChannelNotification,
) -> Result<(String, bool), String> {
    manager
        .get_session(&session_id)
        .await?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    let message = build_channel_message(
        &session_id,
        &server_name,
        notification.content,
        notification.meta,
    );
    let message_id = message.id.clone();

    let should_trigger_workflow = manager.inject_messages(session_id, vec![message]).await?;

    Ok((message_id, should_trigger_workflow))
}

pub async fn resolve_channel_notification_target(
    manager: &AgentSessionManager,
    server_name: &str,
) -> Result<ChannelRouteCandidate, String> {
    let active_session_candidates = {
        let active = manager.active_sessions.read().await;
        active
            .iter()
            .map(|(session_id, session)| ChannelRouteCandidate {
                session_id: session_id.clone(),
                session_name: session
                    .metadata
                    .name
                    .clone()
                    .unwrap_or_else(|| session_id.chars().take(8).collect::<String>()),
                parent_session_id: session.metadata.parent_session_id.clone(),
            })
            .collect::<Vec<_>>()
    };

    let mut matching_candidates = Vec::new();
    for candidate in active_session_candidates {
        if manager
            .proxy_manager
            .session_has_channel_server(&candidate.session_id, server_name)
            .await
        {
            matching_candidates.push(candidate);
        }
    }

    resolve_auto_routed_channel_target(server_name, matching_candidates)
}

fn build_channel_message(
    session_id: &str,
    server_name: &str,
    content: String,
    meta: HashMap<String, String>,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    let text = format_channel_payload(server_name, &content, &meta);

    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![crate::mcp::types::MCPContent::Text {
            text,
            is_error: None,
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: None,
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        created_at: now,
        updated_at: now,
        source: Some(MessageSource::Channel),
        error: None,
        metadata: Some(serde_json::json!({
            "channel": {
                "serverName": server_name,
                "meta": meta,
            }
        })),
    }
}

fn format_channel_payload(
    server_name: &str,
    content: &str,
    meta: &HashMap<String, String>,
) -> String {
    let mut attributes = vec![format!(r#"source="{}""#, escape_xml_attr(server_name))];
    let mut sorted_meta: Vec<_> = meta.iter().collect();
    let mut invalid_meta = Vec::new();
    sorted_meta.sort_by_key(|(key, _)| *key);

    for (key, value) in sorted_meta {
        if is_safe_channel_attribute_name(key) {
            attributes.push(format!(r#"{}="{}""#, key, escape_xml_attr(value)));
            continue;
        }

        invalid_meta.push((key.as_str(), value.as_str()));
    }

    let mut escaped_body = escape_xml_text(content);
    if !invalid_meta.is_empty() {
        if !escaped_body.is_empty() {
            escaped_body.push_str("\n\n");
        }
        escaped_body.push_str("[channel_meta]\n");
        for (index, (key, value)) in invalid_meta.iter().enumerate() {
            if index > 0 {
                escaped_body.push('\n');
            }
            escaped_body.push_str(&escape_xml_text(key));
            escaped_body.push('=');
            escaped_body.push_str(&escape_xml_text(value));
        }
        escaped_body.push_str("\n[/channel_meta]");
    }

    format!(
        "<channel {}>\n{}\n</channel>",
        attributes.join(" "),
        escaped_body
    )
}

fn is_safe_channel_attribute_name(key: &str) -> bool {
    if matches!(key, "source") {
        return false;
    }

    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }

    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

#[doc(hidden)]
pub fn format_channel_payload_for_test(
    server_name: &str,
    content: &str,
    meta: &HashMap<String, String>,
) -> String {
    format_channel_payload(server_name, content, meta)
}

fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
