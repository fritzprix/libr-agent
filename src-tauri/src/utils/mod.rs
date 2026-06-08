pub mod env;
#[cfg(windows)]
pub mod windows_path_discovery;
pub mod fs;
pub mod json;
pub mod pagination;
pub mod platform;
pub mod security;
pub mod sqlite;
pub mod terminal;

/// Safely truncates a string to a maximum number of characters.
/// If truncated, adds an ellipsis (...) to the end.
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    let truncated = safe_truncate(s, max_chars);
    if truncated.len() < s.len() {
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

/// Safely slices a string to a maximum number of characters without panicking.
/// Returns a slice of the original string.
pub fn safe_truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
