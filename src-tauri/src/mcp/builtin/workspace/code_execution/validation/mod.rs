//! Shell command validation heuristics and failure-recovery guidance.
//!
//! Split by responsibility; public surface is re-exported for stable `validation::…` call sites.

mod diagnostics;
mod guidance;
mod interactive;

pub use diagnostics::{
    exit_zero_pipeline_failure_warning, exit_zero_stderr_warning, has_shell_pipeline,
    looks_like_waiting_prompt,
};
pub use guidance::{
    is_signal_interrupt_exit, looks_like_docker_chdir_workdir_failure,
    looks_like_shell_quote_parse_error, looks_like_windows_store_alias_stub,
    shell_command_failure_guidance, shell_signal_interrupt_guidance, signal_interrupt_label,
};
pub use interactive::{
    detect_privilege_escalation, is_likely_interactive_command, resolve_stdin_delivery,
};

#[cfg(windows)]
pub use interactive::contains_unquoted_andand;
