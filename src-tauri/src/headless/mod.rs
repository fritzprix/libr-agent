//! Headless execution and CLI entry point for `liblet` (#2703).
//!
//! Architecture:
//! - `cli`: Flag parsing, TTY/stdin mode selection, and exit code constants (`0`, `1`, `2`, `3`, `4`, `124`, `130`, `143`).
//! - `output`: `text`, `json`, and `stream-json` formatters on `stdout`.
//! - `ephemeral_db`: Per-process `TempDir` and read-only SQLite snapshot of the user's profile DB with session tables cleared.
//! - `runner`: Ephemeral session execution monitor (completion, approval stop `3`, question/secret stop `4`, timeout `124`, interrupt `130`/`143`, error `1`).
//! - `launch`: headless dispatch detection (`--headless` / `--session`), console attach, piped stdin (#2887).
//! - `runtime`: ephemeral database, fork base, write-back, and cleanup lifecycle (#2887).
//! - `inspect`: `--list-sessions` / `--inspect-session` queries and previews (#2887).
//! - `webview`: hidden-WebView driver for one headless turn (#2887).

pub mod cli;
pub mod ephemeral_db;
pub mod output;
pub mod runner;
pub mod writeback;

mod inspect;
mod launch;
mod runtime;
mod webview;

use self::launch::should_read_piped_stdin;
use self::webview::run_headless_with_hidden_webview;
use crate::headless::cli::{exit_codes, parse_cli, LibletCommand, TtyState};
use std::io::{IsTerminal, Read, Write};

// The integration tests (`tests/liblet_headless_tests.rs`) drive these directly, so
// they stay on the `crate::headless` root even though they now live in submodules.
pub use inspect::{run_inspect_session, run_list_sessions};
pub use launch::{attach_parent_console, wants_headless};
pub use runtime::{init_ephemeral_database, EphemeralHeadlessRuntime, SessionForkBase};

/// Entry point for the `liblet` binary.
pub fn run_liblet_main() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tty = TtyState {
        stdin_is_tty: std::io::stdin().is_terminal(),
        stdout_is_tty: std::io::stdout().is_terminal(),
    };

    let piped_stdin = if should_read_piped_stdin(&args, tty) {
        let mut buf = String::new();
        let _ = std::io::stdin().read_to_string(&mut buf);
        Some(buf)
    } else {
        None
    };

    match parse_cli(&args, tty, piped_stdin.as_deref()) {
        Ok(LibletCommand::Help(text)) => {
            print!("{text}");
            let _ = std::io::stdout().flush();
            exit_codes::SUCCESS
        }
        Ok(LibletCommand::Version(text)) => {
            println!("{text}");
            let _ = std::io::stdout().flush();
            exit_codes::SUCCESS
        }
        Ok(LibletCommand::ListSessions(list_args)) => {
            let profile = crate::profile::resolve_profile();
            let source_db_path = crate::profile::db_path(profile);
            let mut stdout = std::io::stdout();
            let mut stderr = std::io::stderr();
            run_list_sessions(&list_args, &source_db_path, &mut stdout, &mut stderr)
        }
        Ok(LibletCommand::InspectSession(inspect_args)) => {
            let profile = crate::profile::resolve_profile();
            let source_db_path = crate::profile::db_path(profile);
            let mut stdout = std::io::stdout();
            let mut stderr = std::io::stderr();
            run_inspect_session(&inspect_args, &source_db_path, &mut stdout, &mut stderr)
        }
        Ok(LibletCommand::Headless(headless_args)) => {
            run_headless_with_hidden_webview(headless_args)
        }
        Ok(LibletCommand::Tui(_tui_args)) => run_liblet_tui_entry(),
        Err(usage_err) => {
            eprintln!("{usage_err}");
            usage_err.exit_code()
        }
    }
}

/// Entry for `liblet` interactive TUI mode (#2701, #2854).
///
/// Headless (`-p`/`--prompt`) never calls this function. Shared-backend attach
/// is removed; use `--prompt` for non-interactive execution.
fn run_liblet_tui_entry() -> i32 {
    eprintln!(
        "error: interactive TUI / shared-backend attach is not available; pass -p / --prompt <text> for headless execution."
    );
    exit_codes::ERROR
}

#[cfg(test)]
mod tests {
    use super::launch::args_want_headless;

    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn headless_and_session_flags_select_headless_dispatch() {
        assert!(args_want_headless(argv(&["--headless", "-p", "hi"])));
        assert!(args_want_headless(argv(&["-p", "hi", "--headless"])));
        assert!(args_want_headless(argv(&["--session", "s-1", "-p", "hi"])));
        assert!(args_want_headless(argv(&["--session=s-1"])));
    }

    #[test]
    fn option_values_never_select_headless_dispatch() {
        // A value token that reads like a flag is still just a value.
        assert!(!args_want_headless(argv(&["-p", "--session"])));
        assert!(!args_want_headless(argv(&["--assistant", "--headless"])));
        // After `--`, everything is a positional prompt.
        assert!(!args_want_headless(argv(&["--", "--headless"])));
        assert!(!args_want_headless(argv(&[])));
        assert!(!args_want_headless(argv(&["--headlined"])));
    }

    #[test]
    fn read_only_view_flags_select_headless_dispatch() {
        // #2885: `libragent --inspect-session` and `libragent --list-sessions`
        // must run in-process like `liblet` — never boot a second GUI.
        assert!(args_want_headless(argv(&["--inspect-session", "s-1"])));
        assert!(args_want_headless(argv(&[
            "--turn",
            "2",
            "--inspect-session",
            "s-1"
        ])));
        assert!(args_want_headless(argv(&["--list-sessions"])));
        // Values that merely read like these flags are still just values.
        assert!(!args_want_headless(argv(&["-p", "--list-sessions"])));
        assert!(!args_want_headless(argv(&["--", "--inspect-session"])));
    }

    /// Issue #2885: headless stdout carries only the `--output-format`
    /// document. The logger used to mirror every line to stdout, so
    /// `--output-format json` printed 40+ `[time][target][LEVEL]` lines
    /// before the result line and the stream failed to parse.
    ///
    /// stdout purity can only be observed from outside, so this test
    /// re-executes the test binary in "helper" mode — headless logger, a
    /// few log lines (including a `[webview]` frontend log), and one JSON
    /// final output — then asserts the helper child's captured stdout is a
    /// single JSON value and its stderr holds the log lines.
    #[test]
    fn json_output_stdout_stays_a_single_json_value() {
        use crate::execution_mode::ExecutionMode;
        use crate::headless::cli::OutputFormat;
        use crate::headless::output::{write_final_output, HeadlessResultPayload};

        const HELPER_ENV: &str = "LIBRAGENT_JSON_STDOUT_HELPER";
        if std::env::var_os(HELPER_ENV).is_some() {
            // Helper mode: mimic a headless run's output path — logger to
            // stderr + file, formatter result to stdout.
            let log_dir = tempfile::tempdir().expect("log dir");
            crate::logger::setup_headless_file_logger(log_dir.path().to_path_buf(), 1)
                .expect("headless logger");
            log::info!("startup log line");
            log::info!(target: "[webview]", "frontend ready log line");
            let payload = HeadlessResultPayload::completed(
                "42".to_string(),
                "/tmp/ws".to_string(),
                ExecutionMode::Normal,
            );
            let mut stdout = std::io::stdout();
            let mut stderr = std::io::stderr();
            write_final_output(OutputFormat::Json, &payload, &mut stdout, &mut stderr)
                .expect("write json output"); // flushes stdout
                                              // Exit right after the result so the harness prints no trailing
                                              // `ok`/summary after the JSON document.
            std::process::exit(0);
        }

        let exe = std::env::current_exe().expect("test binary path");
        let output = std::process::Command::new(exe)
            .args([
                "--exact",
                "headless::tests::json_output_stdout_stays_a_single_json_value",
                "--test-threads=1",
            ])
            .env(HELPER_ENV, "1")
            .output()
            .expect("re-execute test binary as json stdout helper");
        assert!(
            output.status.success(),
            "helper run failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // The harness writes `running 1 test` and a `test <name> ... `
        // prefix (no newline) before the helper body runs; the helper then
        // exits right after the JSON, so the first `{` starts the result
        // document and nothing may follow it but whitespace. Log lines
        // mirroring to stdout would sit in the prefix or after the JSON
        // and fail one of the two assertions below.
        let stdout = String::from_utf8_lossy(&output.stdout);
        let prefix = stdout.split('{').next().expect("split at first brace");
        assert!(
            !prefix.contains("startup log line")
                && !prefix.contains("frontend ready log line")
                && !prefix.contains("[INFO]"),
            "log lines must never reach stdout: {stdout:?}"
        );
        let json_start = stdout
            .find('{')
            .unwrap_or_else(|| panic!("expected one JSON result on stdout; got: {stdout:?}"));
        let value: serde_json::Value = serde_json::from_str(stdout[json_start..].trim())
            .unwrap_or_else(|e| panic!("stdout must hold one JSON value ({e}); got: {stdout:?}"));
        assert_eq!(value["type"], "result");
        assert_eq!(value["status"], "completed");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("startup log line") && stderr.contains("frontend ready log line"),
            "logs must go to stderr, not be lost: {stderr:?}"
        );
    }
}
