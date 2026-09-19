//! Empty successful shell IO must be stated explicitly (not header-only).

use tauri_mcp_agent_lib::mcp::builtin::workspace::code_execution::shell::format_command_io_message;

#[test]
fn empty_success_io_is_explicit() {
    let message = format_command_io_message(
        "Command executed in 10ms (exit code: 0)",
        "Output",
        "",
        "Stderr",
        "",
    );
    assert_eq!(
        message,
        "Command executed in 10ms (exit code: 0)\n\n(no stdout/stderr captured)"
    );
}

#[test]
fn non_empty_streams_unchanged() {
    assert_eq!(
        format_command_io_message("hdr", "Output", "hi", "Stderr", ""),
        "hdr\n\nOutput:\nhi"
    );
    assert_eq!(
        format_command_io_message("hdr", "Output", "", "Stderr", "err"),
        "hdr\n\nStderr:\nerr"
    );
    assert_eq!(
        format_command_io_message("hdr", "Output", "hi", "Stderr", "err"),
        "hdr\n\nOutput:\nhi\n\nStderr:\nerr"
    );
}

#[test]
fn pipeline_failure_stderr_warning_is_generated() {
    use tauri_mcp_agent_lib::mcp::builtin::workspace::code_execution::validation::exit_zero_stderr_warning;

    let warning = exit_zero_stderr_warning(
        "cd /app && xxd main.db-wal | head -50",
        "xxd: main.db-wal: No such file or directory",
    );
    assert!(warning.is_some());
    let w = warning.unwrap();
    assert!(w.contains("pipefail"));
    assert!(w.contains("pipeline"));
}
