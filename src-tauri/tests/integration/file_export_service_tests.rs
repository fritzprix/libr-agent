use tauri_mcp_agent_lib::services::FileExportService;

#[test]
fn sanitize_package_name_keeps_safe_inputs() {
    assert_eq!(
        FileExportService::sanitize_package_name("my-awesome-package"),
        "my-awesome-package"
    );
    assert_eq!(
        FileExportService::sanitize_package_name("package_name_123"),
        "package_name_123"
    );
}

#[test]
fn sanitize_package_name_blocks_directory_traversal_sequences() {
    assert_eq!(
        FileExportService::sanitize_package_name("../../../evil"),
        "_________evil"
    );
    assert_eq!(
        FileExportService::sanitize_package_name("..\\..\\..\\evil"),
        "_________evil"
    );
}

#[test]
fn sanitize_package_name_replaces_unsupported_characters() {
    assert_eq!(
        FileExportService::sanitize_package_name("package!@#name"),
        "package___name"
    );
    assert_eq!(
        FileExportService::sanitize_package_name("  trimmed package  "),
        "__trimmed_package__"
    );
}

#[test]
fn sanitize_package_name_handles_empty_or_pure_special_chars() {
    assert_eq!(
        FileExportService::sanitize_package_name(""),
        "workspace_export"
    );
    assert_eq!(
        FileExportService::sanitize_package_name("../../../"),
        "workspace_export"
    );
    assert_eq!(
        FileExportService::sanitize_package_name("!@#$%"),
        "workspace_export"
    );
}
