//! Outbound channel permission relay to MCP servers that advertise
//! `supports_permission_relay` in initialize metadata.
//!
//! Flow:
//! 1. Agent tool needs user approval → `broadcast_permission_request` sends
//!    `notifications/claude/channel/permission_request` to the MCP server.
//! 2. MCP server may surface its own UI / Telegram prompt and later send
//!    `claude/channel/permission` (intercepted by `channel_transport`).
//! 3. Verdict is routed to `respond_channel_permission` via `channel_dispatch`.
use crate::mcp::session_isolation::error::SessionMCPError;
use crate::mcp::types::ChannelPermissionRequest;
use rmcp::{
    model::{ClientNotification, CustomClientNotification},
    service::{RoleClient, RunningService},
};

use super::SessionMCPManager;

impl SessionMCPManager {
    pub async fn broadcast_permission_request(
        &self,
        request: ChannelPermissionRequest,
    ) -> Result<(), SessionMCPError> {
        let metadata = self.channel_metadata.read().await;
        let processes = self.active_processes.read().await;
        let params = serde_json::to_value(&request).map_err(|error| {
            SessionMCPError::InitFailed(format!(
                "Failed to serialize channel permission request: {}",
                error
            ))
        })?;

        for (server_name, channel_meta) in metadata.iter() {
            if !channel_meta.supports_permission_relay {
                continue;
            }

            let Some(process) = processes.get(server_name) else {
                continue;
            };

            if let Err(error) =
                send_permission_request_notification(&process.client, params.clone()).await
            {
                log::warn!(
                    "Failed to send channel permission request to server '{}' for session '{}': {}",
                    server_name,
                    self.session_id,
                    error
                );
            }
        }

        Ok(())
    }
}

async fn send_permission_request_notification(
    client: &RunningService<RoleClient, ()>,
    params: serde_json::Value,
) -> Result<(), String> {
    client
        .peer()
        .send_notification(ClientNotification::CustomClientNotification(
            CustomClientNotification::new(
                "notifications/claude/channel/permission_request",
                Some(params),
            ),
        ))
        .await
        .map_err(|error| error.to_string())
}
