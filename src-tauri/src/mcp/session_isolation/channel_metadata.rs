use crate::mcp::types::ChannelServerMetadata;

pub fn extract_channel_server_metadata(
    server_name: &str,
    peer_info: Option<&rmcp::model::ServerInfo>,
) -> Option<ChannelServerMetadata> {
    let peer_info = peer_info?;
    let experimental = peer_info.capabilities.experimental.as_ref()?;

    if !experimental.contains_key("claude/channel") {
        return None;
    }

    Some(ChannelServerMetadata {
        server_name: server_name.to_string(),
        instructions: peer_info
            .instructions
            .clone()
            .filter(|text| !text.trim().is_empty()),
        supports_permission_relay: experimental.contains_key("claude/channel/permission"),
    })
}
