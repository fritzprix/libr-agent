use serde::{Deserialize, Serialize};
use std::fmt;

/// Type-safe, stable identifier for a builtin MCP service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinServiceId {
    Planning,
    Scratchpad,
    Workspace,
    Knowledge,
    History,
    Agent,
    Skills,
    Playbook,
    Attachments,
    Ui,
    Browser,
    ScheduledTask,
    Bootstrap,
    Tool, // Unified Tool Domain
    Media,
}

/// Metadata for a builtin service used to generate registry and helper functions.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinServiceEntry {
    pub variant: BuiltinServiceId,
    pub canonical: &'static str,
    pub optional: bool,
}

impl BuiltinServiceId {
    /// Resolve a supported builtin service alias to a [`BuiltinServiceId`].
    pub fn from_alias(alias: &str) -> Option<Self> {
        match alias.trim().to_lowercase().as_str() {
            "planning" => Some(Self::Planning),
            "scratchpad" => Some(Self::Scratchpad),
            "workspace" => Some(Self::Workspace),
            "knowledge" => Some(Self::Knowledge),
            "history" => Some(Self::History),
            "agent" => Some(Self::Agent),
            "skills" => Some(Self::Skills),
            "playbook" => Some(Self::Playbook),
            "attachments" => Some(Self::Attachments),
            "ui" => Some(Self::Ui),
            "browser" => Some(Self::Browser),
            "scheduled_task" | "scheduled-task" => Some(Self::ScheduledTask),
            "bootstrap" => Some(Self::Bootstrap),
            "tool" => Some(Self::Tool),
            "media" => Some(Self::Media),
            _ => None,
        }
    }

    /// Current canonical alias for this service.
    pub fn name(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Scratchpad => "scratchpad",
            Self::Workspace => "workspace",
            Self::Knowledge => "knowledge",
            Self::History => "history",
            Self::Agent => "agent",
            Self::Skills => "skills",
            Self::Playbook => "playbook",
            Self::Attachments => "attachments",
            Self::Ui => "ui",
            Self::Browser => "browser",
            Self::ScheduledTask => "scheduled_task",
            Self::Bootstrap => "bootstrap",
            Self::Tool => "tool",
            Self::Media => "media",
        }
    }
}

// Registry SSOT
pub const BUILTIN_SERVICE_REGISTRY: &[BuiltinServiceEntry] = &[
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Planning,
        canonical: "planning",
        optional: false,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Scratchpad,
        canonical: "scratchpad",
        optional: false,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Workspace,
        canonical: "workspace",
        optional: false,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Knowledge,
        canonical: "knowledge",
        optional: true,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::History,
        canonical: "history",
        optional: true,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Agent,
        canonical: "agent",
        optional: false,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Skills,
        canonical: "skills",
        optional: false,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Playbook,
        canonical: "playbook",
        optional: false,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Attachments,
        canonical: "attachments",
        optional: false,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Ui,
        canonical: "ui",
        optional: false,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Browser,
        canonical: "browser",
        optional: true,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::ScheduledTask,
        canonical: "scheduled_task",
        optional: true,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Bootstrap,
        canonical: "bootstrap",
        optional: true,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Tool,
        canonical: "tool",
        optional: false,
    },
    BuiltinServiceEntry {
        variant: BuiltinServiceId::Media,
        canonical: "media",
        optional: true,
    },
];

pub const CORE_BUILTIN_SERVICE_ALIASES: &[&str] = &[
    "planning",
    "scratchpad",
    "workspace",
    "agent",
    "skills",
    "playbook",
    "attachments",
    "ui",
    "tool",
];

impl fmt::Display for BuiltinServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_round_trips() {
        for entry in BUILTIN_SERVICE_REGISTRY {
            assert_eq!(entry.variant.name(), entry.canonical);
        }
    }
}
