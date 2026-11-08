//! Approval checks for MCP server mutations (`tool__registerServer`, `tool__updateServer`):
//! catalog preset installs, transport validation and the masked transport preview.

pub(super) fn is_mcp_server_mutation(tool_name: &str) -> bool {
    tool_name == "tool__registerServer" || tool_name == "tool__updateServer"
}

/// Owner policy (#2180, #2570): registering a curated catalog preset with its exact
/// transport needs no approval, unless the user lists the tool for normal approval;
/// any other source still needs hard approval. Must agree with `install_provenance`
/// so the stored provenance matches the gate taken.
pub(super) fn is_catalog_preset_install(tool_name: &str, args: &serde_json::Value) -> bool {
    tool_name == "tool__registerServer"
        && args.get("authentication").is_none()
        && args
            .get("transport")
            .and_then(crate::mcp::builtin::tool::matching_catalog_preset)
            .is_some()
}

/// Rejects malformed transports before the user is asked to approve a call
/// that would fail anyway.
pub(super) fn invalid_mcp_server_transport(
    tool_name: &str,
    args: &serde_json::Value,
) -> Option<String> {
    if !is_mcp_server_mutation(tool_name) {
        return None;
    }

    const EXPECTED: &str = "Expected stdio: {\"type\":\"stdio\",\"command\":\"npx\",\"args\":[\"-y\",\"<package>\"]} or http: {\"type\":\"http\",\"url\":\"https://...\"}. Fix the transport and call again; the user has not been asked to approve yet.";

    let transport = match args.get("transport") {
        Some(value) => value,
        None => return Some(format!("Missing 'transport'. {EXPECTED}")),
    };

    match serde_json::from_value::<crate::mcp::types::TransportConfig>(transport.clone()) {
        Ok(crate::mcp::types::TransportConfig::Stdio { command, .. })
            if command.trim().is_empty() =>
        {
            Some(format!(
                "stdio transport has an empty 'command'. {EXPECTED}"
            ))
        }
        Ok(crate::mcp::types::TransportConfig::Http { url, .. }) if url.trim().is_empty() => {
            Some(format!("http transport has an empty 'url'. {EXPECTED}"))
        }
        Ok(_) => None,
        Err(error) => Some(format!("Invalid transport: {error}. {EXPECTED}")),
    }
}

/// Leads with the launch command or URL so truncated relay previews still show
/// what will run. Env and header values and URL query values are omitted because
/// previews can be forwarded to external channels; `reused` names the stored secrets
/// the call sends without asking.
pub(super) fn mcp_server_transport_summary(
    tool_name: &str,
    args: &serde_json::Value,
    reused: &[String],
) -> Option<String> {
    if !is_mcp_server_mutation(tool_name) {
        return None;
    }

    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("?");
    let transport = args.get("transport")?;
    let transport_type = transport
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("stdio");

    let object_keys = |field: &str| -> Option<String> {
        let keys = transport
            .get(field)?
            .as_object()?
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        (!keys.is_empty()).then(|| keys.join(","))
    };

    let summary = if transport_type == "stdio" {
        let command = transport
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let args_line = transport
            .get("args")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let mut line = format!("[{name}] stdio: {command}");
        if !args_line.is_empty() {
            line.push(' ');
            line.push_str(&args_line);
        }
        if let Some(env_keys) = object_keys("env") {
            line.push_str(&format!(" | env keys: {env_keys}"));
        }
        line
    } else {
        let url = transport.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let url = crate::mcp::secret_templates::mask_url_secrets(url);
        let mut line = format!("[{name}] {transport_type}: {url}");
        if let Some(header_keys) = object_keys("headers") {
            line.push_str(&format!(" | header keys: {header_keys}"));
        }
        line
    };

    if reused.is_empty() {
        Some(summary)
    } else {
        Some(format!(
            "{summary} | reuses stored secrets: {}",
            reused.join(",")
        ))
    }
}
