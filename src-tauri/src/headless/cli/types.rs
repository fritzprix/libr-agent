//! Argument model, exit codes, and usage errors for `liblet` (#2889).

use crate::execution_mode::ExecutionMode;
use crate::models::workspace_isolation::WorkspaceIsolationMode;
use std::path::PathBuf;

/// Exit codes defined in `docs/specs/liblet-cli-interface.md` §3.6.
pub mod exit_codes {
    pub const SUCCESS: i32 = 0;
    pub const ERROR: i32 = 1;
    pub const USAGE_ERROR: i32 = 2;
    pub const APPROVAL_REQUIRED: i32 = 3;
    pub const QUESTION_REQUIRED: i32 = 4;
    pub const TIMEOUT: i32 = 124;
    pub const INTERRUPTED_SIGINT: i32 = 130;
    pub const TERMINATED_SIGTERM: i32 = 143;
}

/// Output format for headless execution on `stdout`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    StreamJson,
}

impl OutputFormat {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            "stream-json" => Some(Self::StreamJson),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
            Self::StreamJson => "stream-json",
        }
    }
}

/// Terminal state of standard streams at invocation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TtyState {
    pub stdin_is_tty: bool,
    pub stdout_is_tty: bool,
}

/// Validated arguments for a headless run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadlessArgs {
    pub prompt: String,
    pub cd: Option<PathBuf>,
    pub assistant: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub isolation: WorkspaceIsolationMode,
    pub execution_mode: Option<ExecutionMode>,
    pub output_format: OutputFormat,
    pub timeout_secs: Option<u64>,
    /// Persisted session to continue (#2859). The snapshot keeps this session's
    /// rows; the run executes under its id and the write-back seam merges it
    /// back into the profile DB.
    pub session: Option<String>,
}

/// Resume target for TUI mode (`#2702`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiResumeTarget {
    ContinueLatest,
    ResumePicker,
    ResumeId(String),
}

/// Validated arguments for an interactive TUI run (`#2702`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiArgs {
    pub initial_prompt: Option<String>,
    pub resume: Option<TuiResumeTarget>,
    pub name: Option<String>,
    pub cd: Option<PathBuf>,
    pub assistant: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub isolation: WorkspaceIsolationMode,
    pub execution_mode: Option<ExecutionMode>,
}

/// Validated arguments for `--list-sessions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListSessionsArgs {
    pub cd: Option<PathBuf>,
}

/// Validated arguments for `--inspect-session` (#2874): a read-only
/// postmortem view of one persisted session. Without `turn`/`message` the
/// run prints a compact digest; with one, the full detail of that message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectSessionArgs {
    pub session_id: String,
    /// Zero-based turn index to drill into (mutually exclusive with `message`).
    pub turn: Option<usize>,
    /// Message id to drill into (mutually exclusive with `turn`).
    pub message: Option<String>,
    pub output_format: OutputFormat,
}

/// Top-level command resolved from CLI flags, TTY state, and piped `stdin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibletCommand {
    Help(String),
    Version(String),
    Headless(HeadlessArgs),
    Tui(TuiArgs),
    ListSessions(ListSessionsArgs),
    InspectSession(InspectSessionArgs),
}

/// Usage error mapped to exit code `2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliUsageError {
    pub message: String,
}

impl CliUsageError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn exit_code(&self) -> i32 {
        exit_codes::USAGE_ERROR
    }
}

impl std::fmt::Display for CliUsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "error: {}\n\nRun 'liblet --help' for usage information.",
            self.message
        )
    }
}

impl std::error::Error for CliUsageError {}
