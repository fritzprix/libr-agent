//! Empty successful shell IO must be stated explicitly (not header-only).
//! Sync shell observations must bound oversized stdout/stderr streams.

use tauri_mcp_agent_lib::mcp::builtin::workspace::code_execution::shell::{
    format_command_io_message, truncate_sync_shell_stream, SYNC_SHELL_OBS_MAX_CHARS,
    SYNC_SHELL_OBS_TRUNCATION_MARKER,
};

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

#[test]
fn pipeline_failure_stdout_redirect_warning_is_generated() {
    use tauri_mcp_agent_lib::mcp::builtin::workspace::code_execution::validation::exit_zero_pipeline_failure_warning;

    let warning = exit_zero_pipeline_failure_warning(
        "ffmpeg -i video.mp4 -vn -acodec pcm_s16le audio.wav 2>&1 | tail -5",
        "bash: line 1: ffmpeg: command not found",
        "",
    );
    assert!(warning.is_some());
    let w = warning.unwrap();
    assert!(w.contains("pipefail"));
    assert!(w.contains("pipeline"));
}

#[test]
fn truncate_sync_shell_stream_keeps_short_streams() {
    let short = "hello\nworld\n";
    assert_eq!(truncate_sync_shell_stream(short), short);
}

#[test]
fn truncate_sync_shell_stream_keeps_head_and_tail() {
    let head = "HEAD-START\n";
    let mid = "m".repeat(SYNC_SHELL_OBS_MAX_CHARS);
    let tail = "\nTAIL-END";
    let full = format!("{head}{mid}{tail}");
    assert!(full.chars().count() > SYNC_SHELL_OBS_MAX_CHARS);

    let truncated = truncate_sync_shell_stream(&full);
    assert!(truncated.contains(SYNC_SHELL_OBS_TRUNCATION_MARKER));
    assert!(truncated.contains("omitted"));
    assert!(truncated.starts_with(head));
    assert!(truncated.ends_with(tail));
    assert!(!truncated.contains(&mid));
    assert!(truncated.chars().count() < full.chars().count());
}

#[test]
fn format_command_io_message_truncates_large_stdout() {
    let large = format!("BEGIN\n{}\nEND", "x".repeat(SYNC_SHELL_OBS_MAX_CHARS));
    let message = format_command_io_message("hdr", "Output", &large, "Stderr", "");
    assert!(message.starts_with("hdr\n\nOutput:\n"));
    assert!(message.contains(SYNC_SHELL_OBS_TRUNCATION_MARKER));
    assert!(message.contains("BEGIN"));
    assert!(message.contains("END"));
    assert!(message.chars().count() < large.chars().count());
}
