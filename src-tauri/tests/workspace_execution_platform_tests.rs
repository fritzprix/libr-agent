//! Windows-safe coverage for execution platform resolution used in workspace context.

use tauri_mcp_agent_lib::mcp::builtin::workspace::context::ExecutionPlatform;
use tauri_mcp_agent_lib::services::workspace_runtime_manager::normalize_docker_arch;

#[test]
fn native_windows_platform_uses_host_os_and_detected_shell() {
    let platform = ExecutionPlatform::resolve(false, "windows", "x86_64", None, None);
    assert_eq!(platform.os, "windows");
    assert_eq!(platform.arch, "x86_64");
    assert!(
        platform.shell == "powershell" || platform.shell == "cmd",
        "unexpected windows shell: {}",
        platform.shell
    );
    assert_eq!(platform.platform_line(), "- Platform: windows (x86_64)");
    assert!(platform.shell_line().starts_with("- Default Shell: "));
}

#[test]
fn docker_platform_uses_linux_and_cached_shell_arch() {
    let platform =
        ExecutionPlatform::resolve(true, "windows", "x86_64", Some("sh"), Some("aarch64"));
    assert_eq!(platform.os, "linux");
    assert_eq!(platform.arch, "aarch64");
    assert_eq!(platform.shell, "sh");
    assert_eq!(platform.platform_line(), "- Platform: linux (aarch64)");
    assert_eq!(platform.shell_line(), "- Default Shell: sh");

    let json = platform.to_structured_json();
    assert_eq!(json["os"], "linux");
    assert_eq!(json["arch"], "aarch64");
    assert_eq!(json["shell"], "sh");
}

#[test]
fn docker_platform_falls_back_to_host_arch_and_bash() {
    let platform = ExecutionPlatform::resolve(true, "macos", "aarch64", None, None);
    assert_eq!(platform.os, "linux");
    assert_eq!(platform.arch, "aarch64");
    assert_eq!(platform.shell, "bash");
}

#[test]
fn normalize_docker_arch_maps_goarch_to_rust_style() {
    assert_eq!(normalize_docker_arch("amd64"), "x86_64");
    assert_eq!(normalize_docker_arch("arm64"), "aarch64");
    assert_eq!(normalize_docker_arch("x86_64"), "x86_64");
    assert_eq!(normalize_docker_arch("  ARM64  "), "aarch64");
}
