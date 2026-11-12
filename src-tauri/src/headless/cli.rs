//! CLI argument parsing and mode selection for `liblet` (#2703).
//!
//! Implements the accepted contract in `docs/specs/liblet-cli-interface.md`:
//! - Headless mode requires an explicit `-p` / `--prompt <text>` or non-empty
//!   piped `stdin` (when no positional prompt is given).
//! - TUI mode requires both `stdin` and `stdout` to be a TTY and `-p` to be
//!   absent.
//! - No TTY and no prompt is a usage error (exit code `2`); there is no
//!   auto-headless fallback.
//! - There is no `--hook` flag.

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

/// Top-level command resolved from CLI flags, TTY state, and piped `stdin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibletCommand {
    Help(String),
    Version(String),
    Headless(HeadlessArgs),
    Tui(TuiArgs),
    ListSessions(ListSessionsArgs),
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

/// Help text matching `docs/specs/liblet-cli-interface.md` §3.1 & §3.3.
pub fn help_text() -> String {
    concat!(
        "liblet - LibrAgent terminal & headless runner\n\n",
        "USAGE:\n",
        "  liblet [OPTIONS] [PROMPT]\n",
        "  liblet [OPTIONS] -p, --prompt <TEXT>\n",
        "  <command> | liblet [OPTIONS] [-p <TEXT>]\n\n",
        "CORE FLAGS:\n",
        "  -p, --prompt <text>           Run one prompt in headless mode and exit\n",
        "  -c, --continue                Resume the most recent session for the current directory (TUI)\n",
        "  -r, --resume [id]             Resume a session by ID or open picker (TUI)\n",
        "      --name <title>            Set the display title for a new session (TUI)\n",
        "  -C, --cd <dir>                Workspace directory for a new session (default: cwd)\n",
        "      --assistant <id-or-name>  Assistant preset for a new session\n",
        "      --model <name>            Model override for this invocation\n",
        "      --provider <name>         Provider override for this invocation\n",
        "      --isolation <mode>        Workspace isolation: host (default) or docker\n",
        "      --execution-mode <mode>   Tool approval policy: normal (default), yolo, or unsafe\n",
        "      --output-format <fmt>     Headless stdout format: text (default), json, or stream-json\n",
        "      --timeout <seconds>       Wall-clock limit for a headless run\n",
        "      --list-sessions           Print persisted sessions for the current directory and exit\n",
        "  -h, --help                    Print help and exit\n",
        "  -V, --version                 Print version and exit\n"
    )
    .to_string()
}

pub fn version_text() -> String {
    format!("liblet {}", env!("CARGO_PKG_VERSION"))
}

fn take_option_value(
    flag: &str,
    inline_val: Option<&str>,
    iter: &mut std::iter::Peekable<std::slice::Iter<'_, String>>,
) -> Result<String, CliUsageError> {
    if let Some(val) = inline_val {
        return Ok(val.to_string());
    }
    match iter.next() {
        Some(val) => Ok(val.clone()),
        None => Err(CliUsageError::new(format!(
            "option '{flag}' requires a value"
        ))),
    }
}

/// Parse CLI arguments (excluding `argv[0]`), TTY state, and optional piped `stdin`.
pub fn parse_cli(
    args: &[String],
    tty: TtyState,
    piped_stdin: Option<&str>,
) -> Result<LibletCommand, CliUsageError> {
    let mut prompt_flag: Option<String> = None;
    let mut continue_flag = false;
    let mut resume_flag: Option<Option<String>> = None;
    let mut name_flag: Option<String> = None;
    let mut cd_flag: Option<PathBuf> = None;
    let mut assistant_flag: Option<String> = None;
    let mut model_flag: Option<String> = None;
    let mut provider_flag: Option<String> = None;
    let mut isolation = WorkspaceIsolationMode::Host;
    let mut execution_mode: Option<ExecutionMode> = None;
    let mut output_format: Option<OutputFormat> = None;
    let mut timeout_secs: Option<u64> = None;
    let mut list_sessions = false;
    let mut positional: Vec<String> = Vec::new();
    let mut after_double_dash = false;

    let mut iter = args.iter().peekable();
    while let Some(raw_arg) = iter.next() {
        if after_double_dash {
            positional.push(raw_arg.clone());
            continue;
        }

        if raw_arg == "--" {
            after_double_dash = true;
            continue;
        }

        if raw_arg == "-h" || raw_arg == "--help" {
            return Ok(LibletCommand::Help(help_text()));
        }

        if raw_arg == "-V" || raw_arg == "--version" {
            return Ok(LibletCommand::Version(version_text()));
        }

        if raw_arg == "--hook" || raw_arg.starts_with("--hook=") {
            return Err(CliUsageError::new(
                "unknown option '--hook' (headless mode does not support --hook)",
            ));
        }

        let (flag, inline_val) = if let Some((f, v)) = raw_arg.split_once('=') {
            if f.starts_with('-') {
                (f, Some(v))
            } else {
                (raw_arg.as_str(), None)
            }
        } else {
            (raw_arg.as_str(), None)
        };

        match flag {
            "-p" | "--prompt" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                prompt_flag = Some(value);
            }
            "-c" | "--continue" => {
                if inline_val.is_some() {
                    return Err(CliUsageError::new(format!(
                        "option '{flag}' does not take a value"
                    )));
                }
                continue_flag = true;
            }
            "-r" | "--resume" => {
                if let Some(val) = inline_val {
                    if val.trim().is_empty() {
                        resume_flag = Some(None);
                    } else {
                        resume_flag = Some(Some(val.to_string()));
                    }
                } else if let Some(next_arg) = iter.peek() {
                    if !next_arg.starts_with('-') {
                        let id = iter.next().cloned().unwrap_or_default();
                        resume_flag = Some(Some(id));
                    } else {
                        resume_flag = Some(None);
                    }
                } else {
                    resume_flag = Some(None);
                }
            }
            "--name" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                if value.trim().is_empty() {
                    return Err(CliUsageError::new("--name cannot be empty"));
                }
                name_flag = Some(value);
            }
            "-C" | "--cd" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                if value.trim().is_empty() {
                    return Err(CliUsageError::new(
                        "--cd requires a non-empty directory path",
                    ));
                }
                cd_flag = Some(PathBuf::from(value));
            }
            "--assistant" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                if value.trim().is_empty() {
                    return Err(CliUsageError::new("--assistant cannot be empty"));
                }
                assistant_flag = Some(value);
            }
            "--model" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                if value.trim().is_empty() {
                    return Err(CliUsageError::new("--model cannot be empty"));
                }
                model_flag = Some(value);
            }
            "--provider" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                if value.trim().is_empty() {
                    return Err(CliUsageError::new("--provider cannot be empty"));
                }
                provider_flag = Some(value);
            }
            "--isolation" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                isolation = match value.trim() {
                    "host" => WorkspaceIsolationMode::Host,
                    "docker" => WorkspaceIsolationMode::Docker,
                    other => {
                        return Err(CliUsageError::new(format!(
                            "invalid --isolation value '{other}' (expected 'host' or 'docker')"
                        )))
                    }
                };
            }
            "--execution-mode" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                execution_mode = match value.trim().parse::<ExecutionMode>() {
                    Ok(mode) => Some(mode),
                    Err(_) => {
                        return Err(CliUsageError::new(format!(
                            "invalid --execution-mode value '{}' (expected 'normal', 'yolo', or 'unsafe')",
                            value.trim()
                        )))
                    }
                };
            }
            "--output-format" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                output_format = match OutputFormat::parse(&value) {
                    Some(fmt) => Some(fmt),
                    None => {
                        return Err(CliUsageError::new(format!(
                            "invalid --output-format value '{}' (expected 'text', 'json', or 'stream-json')",
                            value.trim()
                        )))
                    }
                };
            }
            "--timeout" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                let parsed = value.trim().parse::<u64>().ok().filter(|&s| s > 0);
                match parsed {
                    Some(secs) => timeout_secs = Some(secs),
                    None => {
                        return Err(CliUsageError::new(format!(
                            "invalid --timeout value '{}' (expected a positive integer number of seconds)",
                            value.trim()
                        )))
                    }
                }
            }
            "--list-sessions" => {
                if inline_val.is_some() {
                    return Err(CliUsageError::new(
                        "option '--list-sessions' does not take a value",
                    ));
                }
                list_sessions = true;
            }
            "--profile" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                if crate::profile::AppProfile::parse(&value).is_none() {
                    return Err(CliUsageError::new(format!(
                        "invalid --profile value '{}' (expected 'prod', 'dev', or 'demo')",
                        value.trim()
                    )));
                }
            }
            "--demo" => {
                if inline_val.is_some() {
                    return Err(CliUsageError::new("option '--demo' does not take a value"));
                }
            }
            other if other.starts_with('-') => {
                return Err(CliUsageError::new(format!("unknown option '{other}'")));
            }
            _ => {
                positional.push(raw_arg.clone());
            }
        }
    }

    if positional.len() > 1 {
        return Err(CliUsageError::new(
            "too many positional arguments (expected at most one initial prompt)",
        ));
    }
    let positional_prompt = positional.into_iter().next();

    if continue_flag && resume_flag.is_some() {
        return Err(CliUsageError::new(
            "--continue and --resume cannot be used together",
        ));
    }

    if (continue_flag || resume_flag.is_some()) && cd_flag.is_some() {
        return Err(CliUsageError::new(
            "--cd cannot be combined with --continue or --resume (a resumed session keeps its own workspace)",
        ));
    }

    if (continue_flag || resume_flag.is_some()) && name_flag.is_some() {
        return Err(CliUsageError::new(
            "--name applies only to a new session and cannot be combined with --continue or --resume",
        ));
    }

    let has_tui_only_flags =
        continue_flag || resume_flag.is_some() || name_flag.is_some() || list_sessions;
    let has_headless_only_flags = output_format.is_some() || timeout_secs.is_some();

    if list_sessions {
        if prompt_flag.is_some()
            || positional_prompt.is_some()
            || continue_flag
            || resume_flag.is_some()
            || name_flag.is_some()
            || has_headless_only_flags
        {
            return Err(CliUsageError::new(
                "--list-sessions cannot be combined with prompts, resume flags, or headless options",
            ));
        }
        return Ok(LibletCommand::ListSessions(ListSessionsArgs {
            cd: cd_flag,
        }));
    }

    let non_empty_piped_stdin = if !tty.stdin_is_tty {
        piped_stdin
            .map(|s| s.trim_end_matches(&['\r', '\n'][..]))
            .filter(|s| !s.trim().is_empty())
    } else {
        None
    };

    // Case 1: Explicit `-p` / `--prompt` -> Headless mode
    if let Some(raw_prompt) = prompt_flag {
        if positional_prompt.is_some() {
            return Err(CliUsageError::new(
                "positional prompt cannot be combined with -p / --prompt",
            ));
        }
        if has_tui_only_flags {
            return Err(CliUsageError::new(
                "--continue, --resume, and --name are TUI-only and cannot be used in headless mode",
            ));
        }

        let combined_prompt = match (raw_prompt.trim().is_empty(), non_empty_piped_stdin) {
            (true, Some(piped)) => piped.to_string(),
            (false, Some(piped)) => format!("{raw_prompt}\n\n{piped}"),
            (false, None) => raw_prompt,
            (true, None) => {
                return Err(CliUsageError::new(
                    "prompt passed to -p / --prompt cannot be empty",
                ))
            }
        };

        return Ok(LibletCommand::Headless(HeadlessArgs {
            prompt: combined_prompt,
            cd: cd_flag,
            assistant: assistant_flag,
            model: model_flag,
            provider: provider_flag,
            isolation,
            execution_mode,
            output_format: output_format.unwrap_or_default(),
            timeout_secs,
        }));
    }

    // Case 2: Positional prompt without `-p`
    if let Some(pos_prompt) = positional_prompt {
        if !tty.stdin_is_tty || !tty.stdout_is_tty {
            return Err(CliUsageError::new(
                "positional prompt requires an interactive TTY on stdin and stdout; pass -p / --prompt for headless mode",
            ));
        }
        if has_headless_only_flags {
            return Err(CliUsageError::new(
                "--output-format and --timeout are only valid in headless mode (use -p / --prompt)",
            ));
        }
        if pos_prompt.trim().is_empty() {
            return Err(CliUsageError::new("initial prompt cannot be empty"));
        }

        let resume = if continue_flag {
            Some(TuiResumeTarget::ContinueLatest)
        } else {
            resume_flag.map(|opt| match opt {
                Some(id) => TuiResumeTarget::ResumeId(id),
                None => TuiResumeTarget::ResumePicker,
            })
        };

        return Ok(LibletCommand::Tui(TuiArgs {
            initial_prompt: Some(pos_prompt),
            resume,
            name: name_flag,
            cd: cd_flag,
            assistant: assistant_flag,
            model: model_flag,
            provider: provider_flag,
            isolation,
            execution_mode,
        }));
    }

    // Case 3: Neither `-p` nor positional prompt
    if !tty.stdin_is_tty {
        if let Some(piped) = non_empty_piped_stdin {
            if has_tui_only_flags {
                return Err(CliUsageError::new(
                    "--continue, --resume, and --name require an interactive TTY and cannot be used with piped headless stdin",
                ));
            }
            return Ok(LibletCommand::Headless(HeadlessArgs {
                prompt: piped.to_string(),
                cd: cd_flag,
                assistant: assistant_flag,
                model: model_flag,
                provider: provider_flag,
                isolation,
                execution_mode,
                output_format: output_format.unwrap_or_default(),
                timeout_secs,
            }));
        }

        return Err(CliUsageError::new(
            "stdin is not a TTY and no prompt was provided; pass -p / --prompt <text> or pipe a non-empty prompt on stdin",
        ));
    }

    if !tty.stdout_is_tty {
        return Err(CliUsageError::new(
            "stdout is not a TTY and no prompt was provided; pass -p / --prompt <text> for headless mode",
        ));
    }

    if has_headless_only_flags {
        return Err(CliUsageError::new(
            "--output-format and --timeout are only valid in headless mode (use -p / --prompt)",
        ));
    }

    let resume = if continue_flag {
        Some(TuiResumeTarget::ContinueLatest)
    } else {
        resume_flag.map(|opt| match opt {
            Some(id) => TuiResumeTarget::ResumeId(id),
            None => TuiResumeTarget::ResumePicker,
        })
    };

    Ok(LibletCommand::Tui(TuiArgs {
        initial_prompt: None,
        resume,
        name: name_flag,
        cd: cd_flag,
        assistant: assistant_flag,
        model: model_flag,
        provider: provider_flag,
        isolation,
        execution_mode,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    const BOTH_TTY: TtyState = TtyState {
        stdin_is_tty: true,
        stdout_is_tty: true,
    };
    const NO_TTY: TtyState = TtyState {
        stdin_is_tty: false,
        stdout_is_tty: false,
    };
    const STDIN_PIPE_STDOUT_TTY: TtyState = TtyState {
        stdin_is_tty: false,
        stdout_is_tty: true,
    };
    const STDIN_TTY_STDOUT_PIPE: TtyState = TtyState {
        stdin_is_tty: true,
        stdout_is_tty: false,
    };

    #[test]
    fn explicit_prompt_runs_headless_on_tty() {
        let cmd = parse_cli(&vec_args(&["-p", "summarize the diff"]), BOTH_TTY, None)
            .expect("should parse headless");
        match cmd {
            LibletCommand::Headless(args) => {
                assert_eq!(args.prompt, "summarize the diff");
                assert_eq!(args.output_format, OutputFormat::Text);
                assert_eq!(args.isolation, WorkspaceIsolationMode::Host);
                assert_eq!(args.execution_mode, None);
            }
            other => panic!("expected Headless, got {other:?}"),
        }
    }

    #[test]
    fn explicit_prompt_appends_non_empty_piped_stdin() {
        let cmd = parse_cli(
            &vec_args(&["-p", "find root cause"]),
            STDIN_PIPE_STDOUT_TTY,
            Some("error: panicked at line 42\n"),
        )
        .expect("should combine prompt and stdin");
        match cmd {
            LibletCommand::Headless(args) => {
                assert_eq!(args.prompt, "find root cause\n\nerror: panicked at line 42");
            }
            other => panic!("expected Headless, got {other:?}"),
        }
    }

    #[test]
    fn piped_stdin_without_flag_runs_headless() {
        let cmd = parse_cli(
            &vec_args(&["--output-format", "json"]),
            NO_TTY,
            Some("explain this module\n"),
        )
        .expect("piped stdin should run headless");
        match cmd {
            LibletCommand::Headless(args) => {
                assert_eq!(args.prompt, "explain this module");
                assert_eq!(args.output_format, OutputFormat::Json);
            }
            other => panic!("expected Headless, got {other:?}"),
        }
    }

    #[test]
    fn no_tty_and_no_prompt_is_usage_error() {
        let err = parse_cli(&vec_args(&[]), NO_TTY, None).expect_err("must reject empty non-TTY");
        assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);
        assert!(err.message.contains("not a TTY"));

        let err_empty =
            parse_cli(&vec_args(&[]), NO_TTY, Some("   \n")).expect_err("must reject blank pipe");
        assert_eq!(err_empty.exit_code(), exit_codes::USAGE_ERROR);

        let err_stdout_pipe = parse_cli(&vec_args(&[]), STDIN_TTY_STDOUT_PIPE, None)
            .expect_err("must reject redirected stdout without -p");
        assert_eq!(err_stdout_pipe.exit_code(), exit_codes::USAGE_ERROR);
    }

    #[test]
    fn positional_prompt_when_not_tty_is_usage_error() {
        let err = parse_cli(
            &vec_args(&["summarize"]),
            STDIN_PIPE_STDOUT_TTY,
            Some("notes"),
        )
        .expect_err("positional prompt with piped stdin must fail");
        assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);
        assert!(err.message.contains("-p / --prompt"));

        let err2 = parse_cli(&vec_args(&["summarize"]), STDIN_TTY_STDOUT_PIPE, None)
            .expect_err("positional prompt with redirected stdout must fail");
        assert_eq!(err2.exit_code(), exit_codes::USAGE_ERROR);
    }

    #[test]
    fn hook_flag_is_rejected_with_usage_error() {
        let err = parse_cli(&vec_args(&["--hook", "cmd", "-p", "hi"]), BOTH_TTY, None)
            .expect_err("--hook must be rejected");
        assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);
        assert!(err.message.contains("--hook"));
    }

    #[test]
    fn tui_only_flags_rejected_in_headless_mode() {
        for flag in ["--continue", "--resume", "--list-sessions"] {
            let err = parse_cli(&vec_args(&["-p", "hello", flag]), BOTH_TTY, None)
                .expect_err("TUI-only flag with -p must fail");
            assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);
        }
    }

    #[test]
    fn interactive_tty_without_prompt_selects_tui() {
        let cmd = parse_cli(&vec_args(&[]), BOTH_TTY, None).expect("TUI mode");
        assert!(matches!(cmd, LibletCommand::Tui(_)));
    }
}
