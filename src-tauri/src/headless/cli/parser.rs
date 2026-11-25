//! Flag loop and mode decisions for `liblet` CLI parsing (#2889).
//!
//! [`parse_cli`] reads the hidden developer TUI gate and hands the rest to
//! [`parse_cli_with_tui_gate`], whose `collect_flags` pass fills
//! [`ParsedFlags`] and whose `resolve_command` pass turns those flags, the TTY
//! state, and any piped `stdin` into one [`LibletCommand`].

use super::help::{help_text, version_text};
use super::types::{
    CliUsageError, HeadlessArgs, InspectSessionArgs, LibletCommand, ListSessionsArgs, OutputFormat,
    TtyState, TuiArgs, TuiResumeTarget,
};
use crate::execution_mode::ExecutionMode;
use crate::models::workspace_isolation::WorkspaceIsolationMode;
use std::path::PathBuf;

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

/// Reject `--flag=value` for an option that takes no value.
fn reject_inline_value(flag: &str, inline_val: Option<&str>) -> Result<(), CliUsageError> {
    if inline_val.is_some() {
        return Err(CliUsageError::new(format!(
            "option '{flag}' does not take a value"
        )));
    }
    Ok(())
}

/// Reject a value-taking option whose value is blank.
fn require_value(value: String, option: &str) -> Result<String, CliUsageError> {
    if value.trim().is_empty() {
        return Err(CliUsageError::new(format!("{option} cannot be empty")));
    }
    Ok(value)
}

fn parse_isolation(value: &str) -> Result<WorkspaceIsolationMode, CliUsageError> {
    match value.trim() {
        "host" => Ok(WorkspaceIsolationMode::Host),
        "docker" => Ok(WorkspaceIsolationMode::Docker),
        other => Err(CliUsageError::new(format!(
            "invalid --isolation value '{other}' (expected 'host' or 'docker')"
        ))),
    }
}

fn parse_execution_mode(value: &str) -> Result<ExecutionMode, CliUsageError> {
    value.trim().parse::<ExecutionMode>().map_err(|_| {
        CliUsageError::new(format!(
            "invalid --execution-mode value '{}' (expected 'normal', 'yolo', or 'unsafe')",
            value.trim()
        ))
    })
}

fn parse_output_format(value: &str) -> Result<OutputFormat, CliUsageError> {
    OutputFormat::parse(value).ok_or_else(|| {
        CliUsageError::new(format!(
            "invalid --output-format value '{}' (expected 'text', 'json', or 'stream-json')",
            value.trim()
        ))
    })
}

fn parse_timeout(value: &str) -> Result<u64, CliUsageError> {
    value
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|&secs| secs > 0)
        .ok_or_else(|| {
            CliUsageError::new(format!(
                "invalid --timeout value '{}' (expected a positive integer number of seconds)",
                value.trim()
            ))
        })
}

/// Parse CLI arguments (excluding `argv[0]`), TTY state, and optional piped `stdin`.
///
/// The terminal TUI is a developer-only mode after #2861, so this reads the
/// hidden `LIBRAGENT_ENABLE_TUI` flag before deciding that an interactive
/// invocation means TUI. Everything else is unaffected by the flag.
pub fn parse_cli(
    args: &[String],
    tty: TtyState,
    piped_stdin: Option<&str>,
) -> Result<LibletCommand, CliUsageError> {
    parse_cli_with_tui_gate(args, tty, piped_stdin, crate::tui::dev_tui_enabled())
}

/// [`parse_cli`] with the developer TUI gate passed in, so tests never touch
/// process-wide environment variables.
pub fn parse_cli_with_tui_gate(
    args: &[String],
    tty: TtyState,
    piped_stdin: Option<&str>,
    dev_tui: bool,
) -> Result<LibletCommand, CliUsageError> {
    match collect_flags(args)? {
        Collected::Command(command) => Ok(command),
        Collected::Flags(flags) => resolve_command(flags, tty, piped_stdin, dev_tui),
    }
}

/// Flags read from `args`, before any mode decision.
struct ParsedFlags {
    prompt: Option<String>,
    continue_latest: bool,
    resume: Option<Option<String>>,
    name: Option<String>,
    cd: Option<PathBuf>,
    assistant: Option<String>,
    model: Option<String>,
    provider: Option<String>,
    isolation: WorkspaceIsolationMode,
    execution_mode: Option<ExecutionMode>,
    output_format: Option<OutputFormat>,
    timeout_secs: Option<u64>,
    list_sessions: bool,
    inspect_session: Option<String>,
    turn: Option<usize>,
    message: Option<String>,
    headless: bool,
    session: Option<String>,
    positional: Vec<String>,
}

/// Either the command the flags decide on their own, or the flags to resolve.
enum Collected {
    Command(LibletCommand),
    Flags(ParsedFlags),
}

/// Read every flag in `args`, stopping early on `--help` / `--version`.
///
/// Only flag syntax and per-flag values are judged here; whether a combination
/// is legal is [`resolve_command`]'s job.
fn collect_flags(args: &[String]) -> Result<Collected, CliUsageError> {
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
    let mut inspect_session: Option<String> = None;
    let mut turn_flag: Option<usize> = None;
    let mut message_flag: Option<String> = None;
    let mut headless_flag = false;
    let mut session_flag: Option<String> = None;
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
            return Ok(Collected::Command(LibletCommand::Help(help_text())));
        }

        if raw_arg == "-V" || raw_arg == "--version" {
            return Ok(Collected::Command(LibletCommand::Version(version_text())));
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
                reject_inline_value(flag, inline_val)?;
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
                name_flag = Some(require_value(
                    take_option_value(flag, inline_val, &mut iter)?,
                    "--name",
                )?);
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
                assistant_flag = Some(require_value(
                    take_option_value(flag, inline_val, &mut iter)?,
                    "--assistant",
                )?);
            }
            "--model" => {
                model_flag = Some(require_value(
                    take_option_value(flag, inline_val, &mut iter)?,
                    "--model",
                )?);
            }
            "--provider" => {
                provider_flag = Some(require_value(
                    take_option_value(flag, inline_val, &mut iter)?,
                    "--provider",
                )?);
            }
            "--isolation" => {
                isolation = parse_isolation(&take_option_value(flag, inline_val, &mut iter)?)?;
            }
            "--execution-mode" => {
                execution_mode = Some(parse_execution_mode(&take_option_value(
                    flag, inline_val, &mut iter,
                )?)?);
            }
            "--output-format" => {
                output_format = Some(parse_output_format(&take_option_value(
                    flag, inline_val, &mut iter,
                )?)?);
            }
            "--timeout" => {
                timeout_secs = Some(parse_timeout(&take_option_value(
                    flag, inline_val, &mut iter,
                )?)?);
            }
            "--list-sessions" => {
                reject_inline_value(flag, inline_val)?;
                list_sessions = true;
            }
            "--inspect-session" => {
                inspect_session = Some(require_value(
                    take_option_value(flag, inline_val, &mut iter)?,
                    "--inspect-session",
                )?);
            }
            "--turn" => {
                let value = take_option_value(flag, inline_val, &mut iter)?;
                match value.trim().parse::<usize>() {
                    Ok(index) => turn_flag = Some(index),
                    Err(_) => {
                        return Err(CliUsageError::new(format!(
                            "invalid --turn value '{}' (expected a zero-based turn index)",
                            value.trim()
                        )))
                    }
                }
            }
            "--message" => {
                message_flag = Some(require_value(
                    take_option_value(flag, inline_val, &mut iter)?,
                    "--message",
                )?);
            }
            "--headless" => {
                reject_inline_value(flag, inline_val)?;
                headless_flag = true;
            }
            "--session" => {
                session_flag = Some(require_value(
                    take_option_value(flag, inline_val, &mut iter)?,
                    "--session",
                )?);
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
                reject_inline_value(flag, inline_val)?;
            }
            other if other.starts_with('-') => {
                return Err(CliUsageError::new(format!("unknown option '{other}'")));
            }
            _ => {
                positional.push(raw_arg.clone());
            }
        }
    }

    Ok(Collected::Flags(ParsedFlags {
        prompt: prompt_flag,
        continue_latest: continue_flag,
        resume: resume_flag,
        name: name_flag,
        cd: cd_flag,
        assistant: assistant_flag,
        model: model_flag,
        provider: provider_flag,
        isolation,
        execution_mode,
        output_format,
        timeout_secs,
        list_sessions,
        inspect_session,
        turn: turn_flag,
        message: message_flag,
        headless: headless_flag,
        session: session_flag,
        positional,
    }))
}

/// Decide which command the collected flags select, or why they cannot.
fn resolve_command(
    flags: ParsedFlags,
    tty: TtyState,
    piped_stdin: Option<&str>,
    dev_tui: bool,
) -> Result<LibletCommand, CliUsageError> {
    let ParsedFlags {
        prompt: prompt_flag,
        continue_latest: continue_flag,
        resume: resume_flag,
        name: name_flag,
        cd: cd_flag,
        assistant: assistant_flag,
        model: model_flag,
        provider: provider_flag,
        isolation,
        execution_mode,
        output_format,
        timeout_secs,
        list_sessions,
        inspect_session,
        turn: turn_flag,
        message: message_flag,
        headless: headless_flag,
        session: session_flag,
        positional,
    } = flags;

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

    if session_flag.is_some() && cd_flag.is_some() {
        return Err(CliUsageError::new(
            "--cd cannot be combined with --session (a continued session keeps its own workspace)",
        ));
    }

    if (continue_flag || resume_flag.is_some()) && name_flag.is_some() {
        return Err(CliUsageError::new(
            "--name applies only to a new session and cannot be combined with --continue or --resume",
        ));
    }

    // The TUI is a developer-only mode after #2861: its flags still parse, but
    // only with the hidden flag, so a user who never reads a dev doc cannot
    // enter it by accident.
    if (continue_flag || resume_flag.is_some() || name_flag.is_some()) && !dev_tui {
        return Err(CliUsageError::new(
            "--continue, --resume, and --name drive the terminal TUI, which is not available from liblet; \
             pass -p / --prompt <text> for headless execution",
        ));
    }

    let has_tui_only_flags =
        continue_flag || resume_flag.is_some() || name_flag.is_some() || list_sessions;
    let has_headless_only_flags =
        output_format.is_some() || timeout_secs.is_some() || session_flag.is_some();

    // `--inspect-session` is a read-only view (#2874): it never runs a prompt
    // and never combines with run or TUI flags. Checked before the
    // `--list-sessions` block so `--list-sessions --inspect-session` fails
    // here instead of silently listing sessions.
    if turn_flag.is_some() || message_flag.is_some() {
        if inspect_session.is_none() {
            return Err(CliUsageError::new(
                "--turn and --message require --inspect-session <id>",
            ));
        }
        if turn_flag.is_some() && message_flag.is_some() {
            return Err(CliUsageError::new(
                "--turn and --message cannot be used together",
            ));
        }
    }

    if let Some(session_id) = inspect_session {
        if prompt_flag.is_some()
            || positional_prompt.is_some()
            || continue_flag
            || resume_flag.is_some()
            || name_flag.is_some()
            || list_sessions
            || session_flag.is_some()
            || timeout_secs.is_some()
        {
            return Err(CliUsageError::new(
                "--inspect-session cannot be combined with prompts, resume flags, or run options",
            ));
        }
        if output_format == Some(OutputFormat::StreamJson) {
            return Err(CliUsageError::new(
                "--inspect-session supports --output-format text or json",
            ));
        }
        return Ok(LibletCommand::InspectSession(InspectSessionArgs {
            session_id,
            turn: turn_flag,
            message: message_flag,
            output_format: output_format.unwrap_or_default(),
        }));
    }

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
            session: session_flag.clone(),
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
                "--output-format, --timeout, and --session are only valid in headless mode (use -p / --prompt)",
            ));
        }
        if pos_prompt.trim().is_empty() {
            return Err(CliUsageError::new("initial prompt cannot be empty"));
        }
        if !dev_tui {
            return Err(CliUsageError::new(
                "a positional prompt opens the terminal TUI, which is not available from liblet; \
                 pass -p / --prompt <text> for headless execution",
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
                session: session_flag.clone(),
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

    if has_headless_only_flags || headless_flag {
        return Err(CliUsageError::new(
            "--output-format, --timeout, and --session are only valid in headless mode (use -p / --prompt)",
        ));
    }

    if !dev_tui {
        return Err(CliUsageError::new(
            "stdin is a TTY and no prompt was provided; the terminal TUI is not available from liblet; \
             pass -p / --prompt <text> for headless execution",
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
