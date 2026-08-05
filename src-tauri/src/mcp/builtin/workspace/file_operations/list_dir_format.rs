//! Shared directory listing format helpers for host and attach-container paths.

use serde_json::{json, Value};

/// Sort listing items: directories first, then alphabetical by name.
pub fn sort_listing_items(items: &mut [Value]) {
    items.sort_by(|a, b| {
        let a_type = a.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let b_type = b.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");

        match (a_type, b_type) {
            ("directory", "file") | ("directory", "other") => std::cmp::Ordering::Less,
            ("file", "directory") | ("other", "directory") => std::cmp::Ordering::Greater,
            _ => a_name.cmp(b_name),
        }
    });
}

/// Format file size in the host listDirectory style (`8B`, `8.0KB`, `16.1MB`).
pub fn format_human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / 1024.0 / 1024.0)
    }
}

/// Build the Markdown table body (`| Type | Name | Size |` + rows).
pub fn format_listing_table(items: &[Value]) -> String {
    let mut table_lines = vec![
        "| Type | Name | Size |".to_string(),
        "|---|---|---|".to_string(),
    ];

    for item in items {
        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let type_ = item.get("type").and_then(|v| v.as_str()).unwrap_or("?");
        let size = item.get("size").and_then(|v| v.as_u64());

        let icon = match type_ {
            "directory" => "📁 dir",
            "file" => "📄 file",
            _ => "❓ other",
        };

        let size_str = size
            .map(format_human_size)
            .unwrap_or_else(|| "-".to_string());

        table_lines.push(format!("| {} | `{}` | {} |", icon, name, size_str));
    }

    table_lines.join("\n")
}

/// Pagination note appended after the table when offset/limit truncates results.
pub fn listing_truncation_note(
    offset: usize,
    page_len: usize,
    total_items: usize,
    limit: usize,
) -> String {
    let has_more = offset + page_len < total_items;
    if has_more {
        format!(
            "\n\n*(Showing {} to {} of {} items. Call workspace__listDirectory with offset: {} to see more)*",
            offset + 1,
            offset + page_len,
            total_items,
            offset + limit
        )
    } else if offset > 0 {
        format!(
            "\n\n*(Showing {} to {} of {} items)*",
            offset + 1,
            offset + page_len,
            total_items
        )
    } else {
        String::new()
    }
}

/// Build the human-readable success message for a directory listing.
pub fn build_listing_message(
    path_str: &str,
    paginated_items: &[Value],
    total_items: usize,
    offset: usize,
    limit: usize,
    header_suffix: Option<&str>,
) -> String {
    if total_items == 0 {
        return format!(
            "Directory listing for '{}':\n\n(This directory is empty)\n\nThis is a valid empty directory.",
            path_str
        );
    }

    let listing_str = format_listing_table(paginated_items);
    let truncation_note =
        listing_truncation_note(offset, paginated_items.len(), total_items, limit);
    let header = match header_suffix {
        Some(suffix) => format!("Directory listing for '{path_str}' {suffix}:"),
        None => format!("Directory listing for '{path_str}':"),
    };
    format!("{header}\n\n{listing_str}{truncation_note}")
}

/// Build a listing item JSON object matching the host contract.
pub fn listing_item(name: impl Into<String>, entry_type: &str, size: Option<u64>) -> Value {
    json!({
        "name": name.into(),
        "type": entry_type,
        "size": size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_listing_items_puts_directories_first() {
        let mut items = vec![
            listing_item("z.txt", "file", Some(1)),
            listing_item("src", "directory", None),
            listing_item("a.txt", "file", Some(2)),
            listing_item("lib", "directory", None),
        ];
        sort_listing_items(&mut items);
        let names: Vec<_> = items
            .iter()
            .map(|i| i.get("name").and_then(|v| v.as_str()).unwrap())
            .collect();
        assert_eq!(names, vec!["lib", "src", "a.txt", "z.txt"]);
    }

    #[test]
    fn format_human_size_matches_host_style() {
        assert_eq!(format_human_size(8), "8B");
        assert_eq!(format_human_size(8192), "8.0KB");
        assert_eq!(format_human_size(16_875_520), "16.1MB");
    }

    #[test]
    fn format_listing_table_includes_icons_and_sizes() {
        let items = vec![
            listing_item("src", "directory", None),
            listing_item("main.db", "file", Some(512)),
        ];
        let table = format_listing_table(&items);
        assert!(table.contains("| Type | Name | Size |"));
        assert!(table.contains("📁 dir"));
        assert!(table.contains("📄 file"));
        assert!(table.contains("`src`"));
        assert!(table.contains("`main.db`"));
        assert!(table.contains("512B"));
        assert!(table.contains("| - |"));
    }

    #[test]
    fn listing_truncation_note_signals_more_pages() {
        let note = listing_truncation_note(0, 2, 10, 2);
        assert!(note.contains("Showing 1 to 2 of 10"));
        assert!(note.contains("offset: 2"));
    }

    #[test]
    fn build_listing_message_empty_and_with_suffix() {
        let empty = build_listing_message(".", &[], 0, 0, 100, None);
        assert!(empty.contains("empty"));
        assert!(!empty.contains("| Type |"));

        let items = vec![listing_item("a.txt", "file", Some(10))];
        let msg = build_listing_message(
            ".",
            &items,
            1,
            0,
            100,
            Some("(attached container, showing 1 of 1)"),
        );
        assert!(msg.contains("attached container"));
        assert!(msg.contains("| Type | Name | Size |"));
        assert!(msg.contains("10B"));
    }
}
