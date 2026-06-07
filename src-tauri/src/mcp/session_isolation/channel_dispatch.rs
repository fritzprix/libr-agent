use super::channel_events::{ChannelEventReceiver, SessionChannelEvent};
use crate::agent::tool_approvals::parse_channel_permission_behavior;
use log::{error, info, warn};
use tokio::task::JoinHandle;

/// Routes native MCP channel events into the active agent session.
///
/// End-to-end channel delivery tests must call `crate::state::init_channel_dispatch_agent`
/// before events arrive. Tests that only construct `SessionMCPManager` should use
/// `create_detached_channel_event_sender()` or spawn `spawn_channel_event_drain`.
pub fn spawn_session_channel_dispatch_task(
    session_id: String,
    mut event_rx: ChannelEventReceiver,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let Some(agent_manager) = crate::state::try_get_channel_dispatch_agent() else {
                warn!(
                    "Dropping channel event for session '{}' because AgentSessionManager is unavailable",
                    session_id
                );
                continue;
            };

            match event {
                SessionChannelEvent::Message {
                    server_name,
                    notification,
                } => match agent_manager
                    .inject_channel_notification(
                        session_id.clone(),
                        server_name.clone(),
                        notification,
                    )
                    .await
                {
                    Ok((message_id, triggered)) => {
                        info!(
                            "Delivered native channel notification from '{}' to session '{}' (message_id={}, triggered={})",
                            server_name, session_id, message_id, triggered
                        );
                    }
                    Err(error) => {
                        error!(
                            "Failed to deliver native channel notification from '{}' to session '{}': {}",
                            server_name, session_id, error
                        );
                    }
                },
                SessionChannelEvent::PermissionVerdict {
                    server_name,
                    verdict,
                } => {
                    let approved = match parse_channel_permission_behavior(&verdict.behavior) {
                        Ok(approved) => approved,
                        Err(error) => {
                            error!(
                                "Invalid channel permission verdict from '{}': {}",
                                server_name, error
                            );
                            continue;
                        }
                    };

                    match agent_manager
                        .respond_channel_permission(&session_id, &verdict.request_id, approved)
                        .await
                    {
                        Ok(tool_call_id) => {
                            info!(
                                "Resolved native channel permission from '{}' for session '{}' (request_id={}, tool_call_id={}, approved={})",
                                server_name,
                                session_id,
                                verdict.request_id,
                                tool_call_id,
                                approved
                            );
                        }
                        Err(error) => {
                            warn!(
                                "Failed to resolve native channel permission from '{}' for session '{}': {}",
                                server_name, session_id, error
                            );
                        }
                    }
                }
            }
        }
    })
}

/// Drains channel events without touching the global agent manager.
/// Useful in tests that exercise stdio transport but not agent injection.
pub fn spawn_channel_event_drain(mut event_rx: ChannelEventReceiver) -> JoinHandle<()> {
    tokio::spawn(async move {
        while event_rx.recv().await.is_some() {}
    })
}
