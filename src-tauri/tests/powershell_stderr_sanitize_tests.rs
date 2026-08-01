//! Regression tests for PowerShell stderr chrome sanitization (#1641).

use tauri_mcp_agent_lib::mcp::builtin::workspace::code_execution::powershell_stderr::sanitize_powershell_stderr;

#[test]
fn sanitize_strips_scriptblock_stack_trace_lines() {
    let raw = "Compiling libragent v0.8.36\n\
               at <ScriptBlock>, C:\\ws\\.libragent\\tmp\\cmd_abc_17.ps1: line 5\n\
               at <ScriptBlock>, <No file>: line 1\n";
    let cleaned = sanitize_powershell_stderr(raw);
    assert_eq!(cleaned, "Compiling libragent v0.8.36\n");
    assert!(!cleaned.contains("ScriptBlock"));
}

#[test]
fn sanitize_unwraps_native_command_error_frames() {
    let raw = "cmd : progress message\n\
               At C:\\ws\\.libragent\\tmp\\cmd_abc_0.ps1:5 char:3\n\
               +   cmd /c \"echo progress 1>&2\"\n\
               +   ~~~~~~~~~~~~~~~~~~~~~~~~~~~\n\
                   + CategoryInfo          : NotSpecified: (progress message:String) [], RemoteException\n\
                   + FullyQualifiedErrorId : NativeCommandError\n";
    let cleaned = sanitize_powershell_stderr(raw);
    assert_eq!(cleaned.trim(), "progress message");
    assert!(!cleaned.contains("CategoryInfo"));
    assert!(!cleaned.contains("cmd_abc_0.ps1"));
}

#[test]
fn sanitize_preserves_real_compiler_stderr() {
    let raw = "error: could not compile `foo`\n\nCaused by:\n  linker failed\n";
    assert_eq!(sanitize_powershell_stderr(raw), raw);
}
