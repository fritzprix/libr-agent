//! Parsing matrix for the `liblet` CLI flags (#2889).

use super::*;
use crate::models::workspace_isolation::WorkspaceIsolationMode;

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
fn headless_flag_and_session_continue_parse() {
    let cmd = parse_cli(
        &vec_args(&[
            "--headless",
            "--session",
            "abc-123",
            "-p",
            "continue the work",
        ]),
        BOTH_TTY,
        None,
    )
    .expect("--headless and --session parse with an explicit prompt");
    match cmd {
        LibletCommand::Headless(args) => {
            assert_eq!(args.session.as_deref(), Some("abc-123"));
            assert_eq!(args.prompt, "continue the work");
        }
        other => panic!("expected Headless, got {other:?}"),
    }

    let cmd = parse_cli(
        &vec_args(&["--session=abc-123"]),
        NO_TTY,
        Some("piped prompt\n"),
    )
    .expect("--session with a piped prompt runs headless");
    match cmd {
        LibletCommand::Headless(args) => {
            assert_eq!(args.session.as_deref(), Some("abc-123"))
        }
        other => panic!("expected Headless, got {other:?}"),
    }
}

#[test]
fn session_or_headless_without_a_prompt_is_a_usage_error() {
    for args in [vec!["--session", "abc-123"], vec!["--headless"]] {
        let err = parse_cli(&vec_args(&args), BOTH_TTY, None)
            .expect_err("headless flag without a prompt must fail");
        assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);
        assert!(err.message.contains("-p / --prompt"), "{}", err.message);
    }

    let err = parse_cli_with_tui_gate(&vec_args(&["--headless"]), BOTH_TTY, None, true)
        .expect_err("--headless must never open the TUI, even with the dev flag");
    assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);

    let err_empty = parse_cli(&vec_args(&["--session", "  ", "-p", "hi"]), BOTH_TTY, None)
        .expect_err("blank --session must fail");
    assert_eq!(err_empty.exit_code(), exit_codes::USAGE_ERROR);
}

#[test]
fn session_conflicts_with_cd_and_list_sessions() {
    let err = parse_cli(
        &vec_args(&["--session", "abc", "-C", ".", "-p", "hi"]),
        BOTH_TTY,
        None,
    )
    .expect_err("--session + --cd must fail");
    assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);
    assert!(err.message.contains("--session"), "{}", err.message);

    let err = parse_cli(
        &vec_args(&["--list-sessions", "--session", "abc"]),
        BOTH_TTY,
        None,
    )
    .expect_err("--list-sessions + --session must fail");
    assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);
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
fn user_help_does_not_advertise_the_tui() {
    let help = help_text();
    for hidden in ["--continue", "--resume", "--name", "(TUI)", "--tui"] {
        assert!(
            !help.contains(hidden),
            "user help must not advertise the developer TUI, found {hidden}:\n{help}"
        );
    }
    assert!(help.contains("-p, --prompt"));
    assert!(help.contains("--list-sessions"));
    assert!(help.contains("--inspect-session"));
}

#[test]
fn inspect_session_flags_parse() {
    let cmd = parse_cli(
        &vec_args(&[
            "--inspect-session",
            "s-1",
            "--turn",
            "2",
            "--output-format",
            "json",
        ]),
        BOTH_TTY,
        None,
    )
    .expect("--inspect-session with --turn must parse");
    match cmd {
        LibletCommand::InspectSession(args) => {
            assert_eq!(args.session_id, "s-1");
            assert_eq!(args.turn, Some(2));
            assert_eq!(args.message, None);
            assert_eq!(args.output_format, OutputFormat::Json);
        }
        other => panic!("expected InspectSession, got {other:?}"),
    }

    let cmd = parse_cli(&vec_args(&["--inspect-session", "s-1"]), BOTH_TTY, None)
        .expect("bare --inspect-session must parse");
    match cmd {
        LibletCommand::InspectSession(args) => {
            assert_eq!(args.turn, None);
            assert_eq!(args.output_format, OutputFormat::Text);
        }
        other => panic!("expected InspectSession, got {other:?}"),
    }
}

#[test]
fn inspect_session_validation_errors() {
    let err = parse_cli(&vec_args(&["--turn", "1"]), BOTH_TTY, None)
        .expect_err("--turn requires --inspect-session");
    assert!(err.message.contains("--inspect-session"), "{}", err.message);

    let err = parse_cli(
        &vec_args(&[
            "--inspect-session",
            "s-1",
            "--turn",
            "1",
            "--message",
            "m-1",
        ]),
        BOTH_TTY,
        None,
    )
    .expect_err("--turn and --message are exclusive");
    assert!(
        err.message.contains("cannot be used together"),
        "{}",
        err.message
    );

    let err = parse_cli(
        &vec_args(&["--inspect-session", "s-1", "--output-format", "stream-json"]),
        BOTH_TTY,
        None,
    )
    .expect_err("stream-json is not a digest format");
    assert!(err.message.contains("text or json"), "{}", err.message);

    let err = parse_cli(
        &vec_args(&["--inspect-session", "s-1", "-p", "hi"]),
        BOTH_TTY,
        None,
    )
    .expect_err("--inspect-session excludes prompts");
    assert!(
        err.message.contains("cannot be combined"),
        "{}",
        err.message
    );

    let err = parse_cli(
        &vec_args(&["--inspect-session", "s-1", "--turn", "x"]),
        BOTH_TTY,
        None,
    )
    .expect_err("--turn must be a number");
    assert!(err.message.contains("--turn"), "{}", err.message);
}

#[test]
fn interactive_tty_without_a_prompt_asks_for_prompt_without_the_dev_flag() {
    let err = parse_cli_with_tui_gate(&vec_args(&[]), BOTH_TTY, None, false)
        .expect_err("interactive TTY without the dev flag must not open the TUI");
    assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);
    assert!(err.message.contains("-p / --prompt"));
}

#[test]
fn dev_flag_keeps_the_interactive_tui_reachable_for_engineers() {
    let cmd = parse_cli_with_tui_gate(&vec_args(&[]), BOTH_TTY, None, true)
        .expect("TUI mode with the developer flag");
    assert!(matches!(cmd, LibletCommand::Tui(_)));
}

#[test]
fn positional_prompt_is_a_usage_error_without_the_dev_flag() {
    let err = parse_cli_with_tui_gate(&vec_args(&["summarize"]), BOTH_TTY, None, false)
        .expect_err("positional prompt must not open the TUI without the dev flag");
    assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);
    assert!(err.message.contains("-p / --prompt"));

    let cmd = parse_cli_with_tui_gate(&vec_args(&["summarize"]), BOTH_TTY, None, true)
        .expect("positional prompt reaches the TUI with the developer flag");
    match cmd {
        LibletCommand::Tui(args) => {
            assert_eq!(args.initial_prompt.as_deref(), Some("summarize"))
        }
        other => panic!("expected Tui, got {other:?}"),
    }
}

#[test]
fn tui_only_flags_stay_hidden_without_the_dev_flag() {
    for args in [
        vec!["--continue"],
        vec!["--resume"],
        vec!["--name", "titled"],
    ] {
        let err = parse_cli_with_tui_gate(&vec_args(&args), BOTH_TTY, None, false)
            .expect_err("TUI-only flag without the dev flag must fail");
        assert_eq!(err.exit_code(), exit_codes::USAGE_ERROR);
        assert!(err.message.contains("-p / --prompt"), "{}", err.message);
        assert!(
            !err.message.contains("LIBRAGENT_ENABLE_TUI"),
            "liblet usage errors must not advertise the developer flag: {}",
            err.message
        );
    }

    let cmd = parse_cli_with_tui_gate(&vec_args(&["--continue"]), BOTH_TTY, None, true)
        .expect("dev flag keeps --continue reachable");
    assert!(matches!(cmd, LibletCommand::Tui(_)));
}
