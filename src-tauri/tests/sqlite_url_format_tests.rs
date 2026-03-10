/// Integration tests for `format_sqlite_url`.
///
/// These tests verify that the utility produces well-formed `sqlite://` URLs and that
/// backslash handling is correct per platform.
use tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url;

#[test]
fn test_unix_absolute_path_produces_valid_url() {
    let url = format_sqlite_url("/home/user/data.db");
    assert_eq!(url, "sqlite:///home/user/data.db");
}

#[test]
fn test_unix_path_with_backslash_not_modified_on_non_windows() {
    // A backslash is a valid filename character on Unix; it must not be rewritten.
    let raw = "/tmp/dir\\unusual/db.sqlite";
    let url = format_sqlite_url(raw);

    #[cfg(not(target_os = "windows"))]
    assert_eq!(
        url, "sqlite:///tmp/dir\\unusual/db.sqlite",
        "Unix path with literal backslash must not be modified on non-Windows"
    );

    #[cfg(target_os = "windows")]
    assert_eq!(url, "sqlite:///tmp/dir/unusual/db.sqlite");
}

#[test]
fn test_windows_style_path_backslashes_replaced() {
    // On Windows the backslashes in "C:\Users\Admin\db.sqlite" must become forward slashes.
    // On other platforms we only verify the sqlite:// prefix is present.
    let url = format_sqlite_url("C:\\Users\\Admin\\db.sqlite");
    assert!(
        url.starts_with("sqlite://"),
        "URL must start with sqlite:// (got: {url})"
    );

    #[cfg(target_os = "windows")]
    assert_eq!(
        url, "sqlite://C:/Users/Admin/db.sqlite",
        "Backslashes must be replaced on Windows"
    );

    #[cfg(not(target_os = "windows"))]
    assert!(
        url.contains('\\'),
        "Backslashes must be preserved on non-Windows (got: {url})"
    );
}

#[test]
fn test_relative_path_produces_valid_url() {
    let url = format_sqlite_url("relative/path/db.sqlite");
    assert_eq!(url, "sqlite://relative/path/db.sqlite");
}

#[test]
fn test_sqlite_special_memory_path() {
    // ":memory:" must get the sqlite:// prefix without double-prefixing.
    let url = format_sqlite_url(":memory:");
    assert_eq!(url, "sqlite://:memory:");
}
