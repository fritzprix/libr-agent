use crate::agent::ExecutionMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Clear,
    Permission { mode: ExecutionMode },
}

impl Command {
    /// Parses a raw command string like "/clear" or "/permission yolo"
    pub fn parse(text: &str) -> Option<Self> {
        let trimmed = text.trim();
        if !trimmed.starts_with('/') {
            return None;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        match parts[0] {
            "/clear" => Some(Command::Clear),
            "/permission" => {
                if parts.len() < 2 {
                    return None;
                }
                match parts[1].to_lowercase().as_str() {
                    "yolo" => Some(Command::Permission {
                        mode: ExecutionMode::Yolo,
                    }),
                    "unsafe" => Some(Command::Permission {
                        mode: ExecutionMode::Unsafe,
                    }),
                    "normal" => Some(Command::Permission {
                        mode: ExecutionMode::Normal,
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Command::Clear => "/clear".to_string(),
            Command::Permission { mode } => match mode {
                ExecutionMode::Yolo => "/permission yolo".to_string(),
                ExecutionMode::Unsafe => "/permission unsafe".to_string(),
                ExecutionMode::Normal => "/permission normal".to_string(),
            },
        }
    }

    pub fn description(&self) -> String {
        match self {
            Command::Clear => {
                "Reset session (clear messages cache and database history)".to_string()
            }
            Command::Permission { mode } => match mode {
                ExecutionMode::Yolo => {
                    "Execute tools automatically without requiring approval".to_string()
                }
                ExecutionMode::Unsafe => {
                    "Bypass standard approval and policy verification".to_string()
                }
                ExecutionMode::Normal => {
                    "Require standard approval for tool executions".to_string()
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_clear() {
        assert_eq!(Command::parse("/clear"), Some(Command::Clear));
        assert_eq!(Command::parse("  /clear   "), Some(Command::Clear));
    }

    #[test]
    fn test_parse_permission() {
        assert_eq!(
            Command::parse("/permission yolo"),
            Some(Command::Permission {
                mode: ExecutionMode::Yolo
            })
        );
        assert_eq!(
            Command::parse("/permission unsafe"),
            Some(Command::Permission {
                mode: ExecutionMode::Unsafe
            })
        );
        assert_eq!(
            Command::parse("/permission normal"),
            Some(Command::Permission {
                mode: ExecutionMode::Normal
            })
        );
        assert_eq!(Command::parse("/permission invalid"), None);
        assert_eq!(Command::parse("/permission"), None);
    }
}
