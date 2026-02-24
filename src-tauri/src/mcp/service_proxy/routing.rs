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
    if tool_name.starts_with("builtin_") {
        let suffix = tool_name.strip_prefix("builtin_").unwrap();
        let (tool_id, real_tool_name) = suffix.split_once("__").ok_or_else(|| {
            format!(
                "Invalid builtin tool name format (missing '__'): {}",
                tool_name
            )
        })?;

        if tool_id.is_empty() {
            return Err(format!("Invalid builtin tool ID (empty): {}", tool_name));
        }

        Ok(ToolRouting::Builtin {
            server_id: tool_id.to_string(),
            tool_name: real_tool_name.to_string(),
        })
    } else if let Some((server_name, real_tool_name)) = tool_name.split_once("__") {
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
        let tool_name = "builtin_content_store__addContent";
        let routing = route_tool(tool_name).expect("Parsing failed");

        assert_eq!(
            routing,
            ToolRouting::Builtin {
                server_id: "content_store".to_string(),
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
}
