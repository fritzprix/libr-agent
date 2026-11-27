mod loader;
mod parser;

#[cfg(test)]
mod tests;

pub use loader::{load_hooks_from_config_files, load_hooks_from_root, merge_hooks_inline};
pub use parser::parse_hooks_inline;

use std::collections::BTreeMap;

/// Timeout a hook handler gets when its config sets none, in milliseconds (#3005).
///
/// Claude Code defaults a `command` hook to 600 s, lowers that to 30 s on
/// `UserPromptSubmit`, `PreModelSwitch`, and `PostModelSwitch` and to 10 s on
/// `MessageDisplay`, and gives `SessionEnd` hooks a 1.5 s budget.
/// <https://code.claude.com/docs/en/hooks#timeouts>
pub fn default_hook_timeout_ms(event: HookEvent) -> u64 {
    match event {
        HookEvent::UserPromptSubmit | HookEvent::PreModelSwitch | HookEvent::PostModelSwitch => {
            30_000
        }
        HookEvent::MessageDisplay => 10_000,
        HookEvent::SessionEnd => 1_500,
        _ => 600_000,
    }
}

/// Safety net for a handler that carries no resolved timeout.
pub const DEFAULT_HOOK_TIMEOUT_MS: u64 = 600_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HookEvent {
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    PermissionRequest,
    PermissionDenied,
    PostToolBatch,
    Stop,
    StopFailure,
    SubagentStart,
    SubagentStop,
    PreCompact,
    PostCompact,
    Notification,
    UserPromptExpansion,
    MessageDisplay,
    InstructionsLoaded,
    ConfigChange,
    CwdChanged,
    PreModelSwitch,
    PostModelSwitch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookDispatchMode {
    Execute,
    Stub,
}

impl HookEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::PermissionRequest => "PermissionRequest",
            Self::PermissionDenied => "PermissionDenied",
            Self::PostToolBatch => "PostToolBatch",
            Self::Stop => "Stop",
            Self::StopFailure => "StopFailure",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
            Self::PreCompact => "PreCompact",
            Self::PostCompact => "PostCompact",
            Self::Notification => "Notification",
            Self::UserPromptExpansion => "UserPromptExpansion",
            Self::MessageDisplay => "MessageDisplay",
            Self::InstructionsLoaded => "InstructionsLoaded",
            Self::ConfigChange => "ConfigChange",
            Self::CwdChanged => "CwdChanged",
            Self::PreModelSwitch => "PreModelSwitch",
            Self::PostModelSwitch => "PostModelSwitch",
        }
    }

    /// `false` for events that fire on every occurrence: a configured
    /// `matcher` is ignored, as in Claude Code.
    pub fn supports_matcher(self) -> bool {
        !matches!(self, Self::MessageDisplay | Self::CwdChanged)
    }

    /// Events where a command hook's exit code 2 blocks the action. Every
    /// other event keeps treating exit 2 as a non-blocking error.
    pub fn exit_code_2_blocks(self) -> bool {
        matches!(
            self,
            Self::UserPromptExpansion | Self::ConfigChange | Self::PreModelSwitch
        )
    }

    /// Events whose plain (non-JSON) stdout is added to the model's context.
    pub fn plain_stdout_is_context(self) -> bool {
        matches!(
            self,
            Self::SessionStart | Self::UserPromptExpansion | Self::PostModelSwitch
        )
    }

    pub fn dispatch_mode(self) -> HookDispatchMode {
        HookDispatchMode::Execute
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookHandler {
    pub command: String,
    /// Exec-form argument vector (`args` in the hook config). `Some`, even when
    /// empty, selects exec form: the command is spawned directly, without a
    /// shell (#3005).
    pub args: Option<Vec<String>>,
    pub powershell: Option<String>,
    pub timeout_ms: u64,
    pub r#async: bool,
    pub shell: Option<String>,
    /// Set for `type: "http"` handlers, which POST the event envelope to
    /// `url` instead of spawning a command (#3009).
    pub http: Option<HookHttpConfig>,
    pub r#if: Option<String>,
}

/// HTTP hook handler fields from the Hooks reference: the POST target, extra
/// headers whose `$VAR` values are interpolated only for names listed in
/// `allowed_env_vars` (#3009).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HookHttpConfig {
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub allowed_env_vars: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookMatcherGroup {
    pub matcher: Option<String>,
    pub handlers: Vec<HookHandler>,
}

/// Summary of an unrecognized/newer hook event preserved for install preview
/// so plugins declaring newer Claude Code events install cleanly instead of
/// failing validation (#2925).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedHookEntry {
    pub matcher: Option<String>,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HooksConfig {
    pub version: Option<u32>,
    pub events: BTreeMap<HookEvent, Vec<HookMatcherGroup>>,
    pub unsupported_events: BTreeMap<String, Vec<UnsupportedHookEntry>>,
    /// Handlers of not-yet-implemented types (`prompt`, `mcp_tool`, `agent`,
    /// or unknown types) on supported events, keyed by event name. They
    /// install without failing and are marked unsupported in the preview
    /// (#3009).
    pub unsupported_handlers: BTreeMap<String, Vec<UnsupportedHookEntry>>,
}

impl HooksConfig {
    /// An empty config: a plugin that declares no hook config at all.
    pub fn empty() -> Self {
        Self {
            version: None,
            events: BTreeMap::new(),
            unsupported_events: BTreeMap::new(),
            unsupported_handlers: BTreeMap::new(),
        }
    }
}
