use std::path::Path;

/// Formats a filesystem path into a valid `sqlite://` connection URL.
/// On Windows, absolute paths like `C:\Users\Admin\data.db` will be converted
/// to `sqlite://C:/Users/Admin/data.db` to prevent issues with unescaped backslashes
/// in database drivers like sqlx/SeaORM.
pub fn format_sqlite_url(path_str: &str) -> String {
    let path = Path::new(path_str);
    let path_lossy = path.to_string_lossy().to_string();

    // On Windows, convert backslashes to forward slashes for SQLite URL compatibility.
    // We only do this on Windows to avoid corrupting valid Unix paths that legitimately
    // contain backslash characters.
    #[cfg(target_os = "windows")]
    let path_lossy = path_lossy.replace('\\', "/");

    format!("sqlite://{}", path_lossy)
}
