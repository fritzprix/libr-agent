use tauri_mcp_agent_lib::mcp::builtin::workspace::file_operations::utils::{
    format_file_diff, is_not_found_io_error, normalize_workspace_path_input,
};

#[test]
fn normalize_workspace_path_input_trims_and_defaults() {
    assert_eq!(
        normalize_workspace_path_input(Some("  skills/ai-daily-analyst/assets  "), ".").unwrap(),
        "skills/ai-daily-analyst/assets"
    );
    assert_eq!(normalize_workspace_path_input(None, ".").unwrap(), ".");
}

#[test]
fn normalize_workspace_path_input_rejects_blank_values() {
    assert_eq!(
        normalize_workspace_path_input(Some("   "), ".").unwrap_err(),
        "Path parameter cannot be empty"
    );
}

#[test]
fn windows_localized_not_found_error_is_classified_as_missing_path_root_cause() {
    let localized_not_found = std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "지정된 경로를 찾을 수 없습니다. (os error 3)",
    );
    let permission_denied = std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "액세스가 거부되었습니다.",
    );

    assert!(is_not_found_io_error(&localized_not_found));
    assert!(!is_not_found_io_error(&permission_denied));
}

#[test]
fn format_file_diff_counts_replacements_as_removed_and_added_lines() {
    let diff = format_file_diff(
        "line1\nline2\nline3\n",
        "line1\nline2_changed\nline3\n",
        "demo.txt",
    );

    assert!(
        diff.contains("**Changes:** 1 line(s) added, 1 line(s) removed"),
        "replacement-only diffs must still count as one add plus one remove: {diff}"
    );
    assert!(
        diff.contains("- line2"),
        "removed line should remain visible in the diff body: {diff}"
    );
    assert!(
        diff.contains("+ line2_changed"),
        "added line should remain visible in the diff body: {diff}"
    );
    assert!(
        !diff.contains("- line1"),
        "unchanged lines should not be mislabeled as removals: {diff}"
    );
}
