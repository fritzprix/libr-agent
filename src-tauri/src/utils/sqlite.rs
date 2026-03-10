use std::path::Path;

/// Formats a filesystem path into a valid `sqlite://` connection URL.
/// On Windows, absolute paths like `C:\Users\Admin\data.db` will be converted
/// to `sqlite://C:/Users/Admin/data.db` to prevent issues with unescaped backslashes
/// in database drivers like sqlx/SeaORM.
pub fn format_sqlite_url(path_str: &str) -> String {
    let path = Path::new(path_str);
    let mut path_lossy = path.to_string_lossy().to_string();

    // Convert backslashes to forward slashes for cross-platform SQLite URL consistency
    if cfg!(target_os = "windows") || path_lossy.contains('\\') {
        path_lossy = path_lossy.replace('\\', "/");
    }

    format!("sqlite://{}", path_lossy)
}
