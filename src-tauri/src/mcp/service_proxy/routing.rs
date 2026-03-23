use crate::mcp::builtin::service_id::BuiltinServiceId;

#[derive(Debug, PartialEq)]
pub enum ToolRouting {
    Builtin {
        server_id: String,
        tool_name: String,
    },
    External {
        server_name: String,
        tool_name: String,
    },
}

pub fn route_tool(tool_name: &str) -> Result<ToolRouting, String> {
    let (server_name, real_tool_name) = tool_name.split_once("__").ok_or_else(|| {
        format!(
            "Invalid tool name format (expected server__tool): {}",
            tool_name
        )
    })?;

    if server_name.is_empty() {
        return Err(format!(
            "Invalid tool name (empty server name): {}",
            tool_name
        ));
    }
    if real_tool_name.is_empty() {
        return Err(format!(
            "Invalid tool name (empty tool name): {}",
            tool_name
        ));
    }

    if let Some(service_id) = BuiltinServiceId::from_alias(server_name) {
        Ok(ToolRouting::Builtin {
            server_id: service_id.name().to_string(),
            tool_name: real_tool_name.to_string(),
        })
    } else {
        Ok(ToolRouting::External {
            server_name: server_name.to_string(),
            tool_name: real_tool_name.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_tool_routing() {
        let tool_name = "attachments__add";
        let routing = route_tool(tool_name).expect("Parsing failed");

        assert_eq!(
            routing,
            ToolRouting::Builtin {
                server_id: "attachments".to_string(),
                tool_name: "add".to_string(),
            }
        );
    }

    #[test]
    fn test_external_tool_routing() {
        let tool_name = "weather_server__get_forecast";
        let routing = route_tool(tool_name).expect("Parsing failed");

        assert_eq!(
            routing,
            ToolRouting::External {
                server_name: "weather_server".to_string(),
                tool_name: "get_forecast".to_string(),
            }
        );
    }

    #[test]
    fn test_invalid_tool_name() {
        assert!(route_tool("no_separator").is_err());
    }

    #[test]
    fn test_builtin_empty_tool_name() {
        assert!(route_tool("planning__").is_err());
    }

    #[test]
    fn test_external_empty_server_name() {
        assert!(route_tool("__get_forecast").is_err());
    }

    #[test]
    fn test_external_empty_tool_name() {
        assert!(route_tool("weather_server__").is_err());
    }
}
