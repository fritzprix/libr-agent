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
