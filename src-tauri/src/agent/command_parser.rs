use crate::agent::ExecutionMode;

/// `/plugin` subcommands (#2825). Plugins installed this way are scoped to
/// the current session only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginCommand {
    /// Needs an interactive hook-approval step, so it is handled by the
    /// desktop input (preview → approval modal → install IPC), never here.
    Install {
        source: String,
    },
    Uninstall {
        target: String,
    },
    /// Turn a session or global plugin on/off (#2927). No approval needed:
    /// the files stay, so this is a state flag, not an install.
    Enable {
        target: String,
    },
    Disable {
        target: String,
    },
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Clear,
    Reload,
    Permission { mode: ExecutionMode },
    Plugin(PluginCommand),
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
            "/reload" => Some(Command::Reload),
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
            "/plugin" => {
                let action = parts.get(1)?.to_lowercase();
                // Keep the original casing/spacing of the argument (URLs).
                let arg = parts.get(2..)?.join(" ");
                match action.as_str() {
                    "list" if arg.is_empty() => Some(Command::Plugin(PluginCommand::List)),
                    "install" if !arg.is_empty() => {
                        Some(Command::Plugin(PluginCommand::Install { source: arg }))
                    }
                    "uninstall" if !arg.is_empty() => {
                        Some(Command::Plugin(PluginCommand::Uninstall { target: arg }))
                    }
                    "enable" if !arg.is_empty() => {
                        Some(Command::Plugin(PluginCommand::Enable { target: arg }))
                    }
                    "disable" if !arg.is_empty() => {
                        Some(Command::Plugin(PluginCommand::Disable { target: arg }))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Command::Clear => "/clear".to_string(),
            Command::Reload => "/reload".to_string(),
            Command::Permission { mode } => match mode {
                ExecutionMode::Yolo => "/permission yolo".to_string(),
                ExecutionMode::Unsafe => "/permission unsafe".to_string(),
                ExecutionMode::Normal => "/permission normal".to_string(),
            },
            Command::Plugin(PluginCommand::Install { .. }) => "/plugin install".to_string(),
            Command::Plugin(PluginCommand::Uninstall { .. }) => "/plugin uninstall".to_string(),
            Command::Plugin(PluginCommand::Enable { .. }) => "/plugin enable".to_string(),
            Command::Plugin(PluginCommand::Disable { .. }) => "/plugin disable".to_string(),
            Command::Plugin(PluginCommand::List) => "/plugin list".to_string(),
        }
    }

    pub fn description(&self) -> String {
        match self {
            Command::Clear => {
                "Reset session (clear messages cache and database history)".to_string()
            }
            Command::Reload => {
                "Reload session tools and workspace context without clearing history".to_string()
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
            Command::Plugin(PluginCommand::Install { .. }) => {
                "Install a plugin (git URL) for this session only".to_string()
            }
            Command::Plugin(PluginCommand::Uninstall { .. }) => {
                "Remove a session plugin by git URL or name".to_string()
            }
            Command::Plugin(PluginCommand::Enable { .. }) => {
                "Turn an installed plugin back on without downloading it again".to_string()
            }
            Command::Plugin(PluginCommand::Disable { .. }) => {
                "Turn an installed plugin off but keep its files".to_string()
            }
            Command::Plugin(PluginCommand::List) => {
                "List plugins active in this session (global or session)".to_string()
            }
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
    fn test_parse_reload() {
        assert_eq!(Command::parse("/reload"), Some(Command::Reload));
        assert_eq!(Command::parse("  /reload   "), Some(Command::Reload));
        assert_eq!(Command::Reload.display_name(), "/reload");
        assert_eq!(
            Command::Reload.description(),
            "Reload session tools and workspace context without clearing history"
        );
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

    #[test]
    fn test_parse_plugin() {
        assert_eq!(
            Command::parse("/plugin list"),
            Some(Command::Plugin(PluginCommand::List))
        );
        assert_eq!(
            Command::parse("  /plugin install https://github.com/Obra/Superpowers  "),
            Some(Command::Plugin(PluginCommand::Install {
                source: "https://github.com/Obra/Superpowers".to_string()
            }))
        );
        assert_eq!(
            Command::parse("/plugin UNINSTALL superpowers"),
            Some(Command::Plugin(PluginCommand::Uninstall {
                target: "superpowers".to_string()
            }))
        );
        assert_eq!(Command::parse("/plugin"), None);
        assert_eq!(Command::parse("/plugin install"), None);
        assert_eq!(Command::parse("/plugin uninstall"), None);
        assert_eq!(Command::parse("/plugin list extra"), None);
        assert_eq!(Command::parse("/plugin bogus x"), None);
    }

    // #2927 gatekeeper G1.2: `/plugin enable|disable` parse, keep the
    // argument as typed, and reject a missing one.
    #[test]
    fn test_parse_plugin_enable_and_disable() {
        assert_eq!(
            Command::parse("/plugin enable superpowers"),
            Some(Command::Plugin(PluginCommand::Enable {
                target: "superpowers".to_string()
            }))
        );
        assert_eq!(
            Command::parse("  /plugin DISABLE https://github.com/Obra/Superpowers  "),
            Some(Command::Plugin(PluginCommand::Disable {
                target: "https://github.com/Obra/Superpowers".to_string()
            }))
        );
        assert_eq!(
            Command::Plugin(PluginCommand::Enable {
                target: "x".to_string()
            })
            .display_name(),
            "/plugin enable"
        );
        assert_eq!(
            Command::Plugin(PluginCommand::Disable {
                target: "x".to_string()
            })
            .display_name(),
            "/plugin disable"
        );

        // No plugin named: nothing to enable, so the command is not parsed and
        // the user is not told a plugin was switched.
        assert_eq!(Command::parse("/plugin enable"), None);
        assert_eq!(Command::parse("/plugin disable"), None);
        assert_eq!(Command::parse("/plugin enable   "), None);
    }
}
