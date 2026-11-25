//! CLI argument parsing and mode selection for `liblet` (#2703).
//!
//! Implements the accepted contract in `docs/specs/liblet-cli-interface.md`:
//! - Headless mode requires an explicit `-p` / `--prompt <text>` or non-empty
//!   piped `stdin` (when no positional prompt is given).
//! - The terminal TUI is a developer-only mode after #2861: user help does not
//!   advertise `--continue` / `--resume` / `--name`, and an interactive TTY
//!   selects TUI only when the hidden `LIBRAGENT_ENABLE_TUI=1` flag is set.
//!   Without that flag an interactive invocation is a usage error that points
//!   at `-p`.
//! - No TTY and no prompt is a usage error (exit code `2`); there is no
//!   auto-headless fallback.
//! - There is no `--hook` flag.
//!
//! Split by responsibility (#2889): [`types`] holds the argument model, exit
//! codes, and usage errors, [`help`] the user-facing text, [`parser`] the flag
//! loop and the mode decisions, [`tests`] the parsing matrix. This module
//! re-exports the same paths, so `crate::headless::cli::*` callers and the
//! public API are unchanged.

mod help;
mod parser;
mod types;

#[cfg(test)]
mod tests;

pub use help::{help_text, version_text};
pub use parser::{parse_cli, parse_cli_with_tui_gate};
pub use types::{
    exit_codes, CliUsageError, HeadlessArgs, InspectSessionArgs, LibletCommand, ListSessionsArgs,
    OutputFormat, TtyState, TuiArgs, TuiResumeTarget,
};
