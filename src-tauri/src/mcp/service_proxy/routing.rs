use crate::mcp::builtin::service_id::{is_builtin_tool_name, parse_builtin_tool_name};

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
    if is_builtin_tool_name(tool_name) {
        let (tool_id, real_tool_name) = parse_builtin_tool_name(tool_name).ok_or_else(|| {
            format!(
                "Invalid builtin tool name format (missing '__'): {}",
                tool_name
            )
        })?;

        if tool_id.is_empty() {
            return Err(format!("Invalid builtin tool ID (empty): {}", tool_name));
        }

        if real_tool_name.is_empty() {
            return Err(format!(
                "Invalid builtin tool name (empty after '__'): {}",
                tool_name
            ));
        }

        Ok(ToolRouting::Builtin {
            server_id: tool_id.to_string(),
            tool_name: real_tool_name.to_string(),
        })
    } else if let Some((server_name, real_tool_name)) = tool_name.split_once("__") {
        if server_name.is_empty() {
            return Err(format!(
                "Invalid external tool name (empty server name): {}",
                tool_name
            ));
        }
        if real_tool_name.is_empty() {
            return Err(format!(
                "Invalid external tool name (empty tool name): {}",
                tool_name
            ));
        }
        Ok(ToolRouting::External {
            server_name: server_name.to_string(),
            tool_name: real_tool_name.to_string(),
        })
    } else {
        Err(format!(
            "Invalid tool name format (expected server__tool): {}",
            tool_name
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_tool_routing() {
        let tool_name = "builtin_attachments__addContent";
        let routing = route_tool(tool_name).expect("Parsing failed");

        assert_eq!(
            routing,
            ToolRouting::Builtin {
                server_id: "attachments".to_string(),
                tool_name: "addContent".to_string(),
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
        assert!(route_tool("builtin_").is_err());
        assert!(route_tool("builtin_no_separator").is_err());
        assert!(route_tool("no_separator").is_err());
    }

    #[test]
    fn test_builtin_empty_tool_name() {
        // builtin_planning__ has an empty real_tool_name after '__'
        assert!(route_tool("builtin_planning__").is_err());
    }

    #[test]
    fn test_external_empty_server_name() {
        // __get_forecast has an empty server name
        assert!(route_tool("__get_forecast").is_err());
    }

    #[test]
    fn test_external_empty_tool_name() {
        // weather_server__ has an empty tool name
        assert!(route_tool("weather_server__").is_err());
    }
}
