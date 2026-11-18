//! Headless dispatch detection (#2887).
//!
//! Flag constants, argv scanning, console attachment, and the piped-stdin rule.

use crate::headless::cli::TtyState;

/// Flag that selects in-process headless execution on `libragent` (#2859).
pub const HEADLESS_FLAG: &str = "--headless";

/// Session continuation flag; selecting it also selects headless dispatch on
/// `libragent`, so `libragent --session <id> -p "..."` works without the
/// marker (#2859).
pub const SESSION_CONTINUATION_FLAG: &str = "--session";

/// Read-only CLI views that also select in-process dispatch on `libragent`
/// (#2885): `libragent --inspect-session <id>` and `libragent --list-sessions`
/// print and exit like `liblet` — they must never boot a second GUI on the
/// profile.
pub const INSPECT_SESSION_FLAG: &str = "--inspect-session";
pub const LIST_SESSIONS_FLAG: &str = "--list-sessions";

/// Flags whose next argv token is their value, never a flag. Mirrors the
/// value-taking options of [`cli::parse_cli`]; keep the two in sync.
const ARGS_TAKING_VALUE: &[&str] = &[
    "-p",
    "--prompt",
    "-C",
    "--cd",
    "--assistant",
    "--model",
    "--provider",
    "--isolation",
    "--execution-mode",
    "--output-format",
    "--timeout",
    "--resume",
    "--name",
    "--profile",
    INSPECT_SESSION_FLAG,
];

/// True when the given arguments select in-process headless execution.
///
/// Scans argv like flags, not like substrings: scanning stops at the first
/// `--`, and the value token of a value-taking option is skipped, so a prompt
/// that happens to read `--headless` or `--session` never dispatches here.
pub fn args_want_headless(args: impl IntoIterator<Item = impl AsRef<str>>) -> bool {
    let mut skip_value = false;
    for arg in args {
        let arg = arg.as_ref();
        if arg == "--" {
            break;
        }
        if skip_value {
            skip_value = false;
            continue;
        }
        if arg == HEADLESS_FLAG
            || arg == SESSION_CONTINUATION_FLAG
            || arg == INSPECT_SESSION_FLAG
            || arg == LIST_SESSIONS_FLAG
            || arg.starts_with(&format!("{SESSION_CONTINUATION_FLAG}="))
        {
            return true;
        }
        // `--flag=value` carries its value inline and consumes no next token.
        if !arg.contains('=') && ARGS_TAKING_VALUE.contains(&arg) {
            skip_value = true;
        }
    }
    false
}

/// True when this process was started with [`HEADLESS_FLAG`],
/// `--session`, [`INSPECT_SESSION_FLAG`], or [`LIST_SESSIONS_FLAG`].
pub fn wants_headless() -> bool {
    args_want_headless(std::env::args())
}

/// Windows: join the launching terminal so a GUI-subsystem `libragent` can
/// write to its console in-process (#2859).
///
/// `libragent.exe` is `windows_subsystem = "windows"` and starts without a
/// console. Rust opens its standard streams lazily on first use, so attaching
/// before the first write is enough: `AttachConsole(ATTACH_PARENT_PROCESS)`
/// points the process standard handles at the launching terminal and the
/// streams pick them up there. No parent console (e.g. a double-click launch)
/// is not an error — the run proceeds without terminal output, like any
/// redirected headless process. Unlike `--tui`, this cannot forward to a
/// console-subsystem sibling: `liblet` is not bundled with the release app.
#[cfg(windows)]
pub fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};

    // SAFETY: ATTACH_PARENT_PROCESS is the documented "join my parent's
    // console" argument; a failure just means there is no console to join.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

/// Non-Windows: console semantics are native, there is nothing to attach.
#[cfg(not(windows))]
pub fn attach_parent_console() {}

/// Determine whether `liblet` should read piped `stdin` before parsing CLI flags.
pub fn should_read_piped_stdin(args: &[String], tty: TtyState) -> bool {
    if tty.stdin_is_tty {
        return false;
    }
    for arg in args {
        if arg == "--" {
            break;
        }
        if matches!(
            arg.as_str(),
            "-h" | "--help"
                | "-V"
                | "--version"
                | "--list-sessions"
                | "--inspect-session"
                | "--hook"
        ) || arg.starts_with("--hook=")
        {
            return false;
        }
    }
    true
}
