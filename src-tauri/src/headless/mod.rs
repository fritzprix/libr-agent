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
}
