//! Standalone regression binary for MCP stdio stderr piping.
//!
//! Kept outside `integration_tests.rs` so it also runs on Windows CI
//! (`integration_tests` is `#![cfg(not(windows))]` due to WebView DLL issues).
//!
//! These tests only inspect source via `include_str!` and do not link Tauri/WebView.

/// Background (2026-07):
/// On Windows GUI hosts, `CREATE_NO_WINDOW` + inherited stderr caused external
/// `npx` MCP servers to die during `initialize` with:
/// `Process initialization failed: connection closed: initialize response`.
///
/// Fix: always pipe (or null) stderr in `configure_mcp_child_stdio`.
/// The stderr *logger* is optional; the stderr *pipe* is not.

const CHANNEL_TRANSPORT_SOURCE: &str =
    include_str!("../src/mcp/session_isolation/channel_transport.rs");

const LIFECYCLE_SOURCE: &str =
    include_str!("../src/mcp/session_isolation/stdio_manager/lifecycle.rs");

#[test]
fn session_mcp_stdio_must_pipe_stderr_not_inherit() {
    assert!(
        CHANNEL_TRANSPORT_SOURCE.contains("fn configure_mcp_child_stdio"),
        "configure_mcp_child_stdio helper must exist so stderr policy stays centralized"
    );
    assert!(
        CHANNEL_TRANSPORT_SOURCE.contains("MCP_STDIO_STDERR_MUST_NOT_INHERIT"),
        "regression marker MCP_STDIO_STDERR_MUST_NOT_INHERIT must remain next to stderr config"
    );
    assert!(
        CHANNEL_TRANSPORT_SOURCE.contains(".stderr(Stdio::piped())")
            || CHANNEL_TRANSPORT_SOURCE.contains(".stderr(Stdio::null())"),
        "session MCP children must set stderr to piped or null; inherited stderr regresses \
         Windows CREATE_NO_WINDOW + npx initialize failures"
    );
    assert!(
        CHANNEL_TRANSPORT_SOURCE.contains("configure_mcp_child_stdio(&mut command)"),
        "spawn_channel_aware_stdio must call configure_mcp_child_stdio"
    );
}

#[test]
fn windows_create_no_window_documents_stderr_pipe_requirement() {
    assert!(
        LIFECYCLE_SOURCE.contains("CREATE_NO_WINDOW"),
        "Windows spawn path should still use CREATE_NO_WINDOW"
    );
    assert!(
        LIFECYCLE_SOURCE.contains("MCP_STDIO_STDERR_MUST_NOT_INHERIT"),
        "lifecycle.rs must cross-reference MCP_STDIO_STDERR_MUST_NOT_INHERIT so CREATE_NO_WINDOW \
         is not recombined with inherited stderr during refactors"
    );
}

#[test]
fn stderr_logger_is_optional_but_must_not_replace_pipe_requirement() {
    assert!(
        CHANNEL_TRANSPORT_SOURCE.contains("Observability only")
            || CHANNEL_TRANSPORT_SOURCE.contains("optional observability"),
        "comments must distinguish optional stderr logging from mandatory stderr piping"
    );
    assert!(
        CHANNEL_TRANSPORT_SOURCE.contains("functional correctness")
            || CHANNEL_TRANSPORT_SOURCE.contains("REGRESSION GUARD"),
        "comments must state that stderr piping is a functional correctness / regression guard"
    );
}
