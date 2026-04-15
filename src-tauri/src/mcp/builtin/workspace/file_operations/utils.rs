use std::io::ErrorKind;
use tracing::error;

// ✅ ENHANCED: Threshold for using spawn_blocking for CPU-intensive line enumeration
// Large files can block the async runtime during line enumeration
pub const LARGE_FILE_THRESHOLD: u64 = 1_048_576; // 1 MB in bytes

const FNV_OFFSET: u32 = 2_166_136_261;
const FNV_PRIME: u32 = 16_777_619;
const LINE_HASH_LEN: usize = 2;
const PREFIX_HASH_LEN: usize = 4;
const ANCHOR_LEN: usize = LINE_HASH_LEN + PREFIX_HASH_LEN;

/// Compute a 2-char hex content hash for anchor generation.
///
/// Uses FNV-1a 32-bit with output folding to produce a stable 2-char identifier
/// per line. This hash is embedded into opaque line anchors when `showLineAnchors`
/// is enabled, and validated by `editFile` after parsing the agent-facing anchor.
/// detect file staleness before applying edits.
pub fn compute_line_hash(content: &str) -> String {
    let mut hash = FNV_OFFSET;
    for byte in content.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    // Fold 32-bit hash to 8 bits for compact 2-char hex output
    let folded = (hash ^ (hash >> 8) ^ (hash >> 16) ^ (hash >> 24)) as u8;
    format!("{:02x}", folded)
}

/// Initial rolling state for prefix-hash computation.
pub fn initial_prefix_hash_state() -> u32 {
    FNV_OFFSET
}

/// Update rolling prefix-hash state with a single line plus its trailing newline.
pub fn update_prefix_hash_state(mut state: u32, content: &str) -> u32 {
    for byte in content.bytes().chain(std::iter::once(b'\n')) {
        state ^= byte as u32;
        state = state.wrapping_mul(FNV_PRIME);
    }
    state
}

/// Fold rolling prefix state to a compact 4-char hex identifier.
pub fn format_prefix_hash(state: u32) -> String {
    let folded = ((state >> 16) ^ (state & 0xFFFF)) as u16;
    format!("{:04x}", folded)
}

pub fn make_anchor(line_hash: &str, prefix_hash: &str) -> String {
    format!("{}{}", line_hash, prefix_hash)
}

pub fn parse_anchor(anchor: &str) -> Option<(&str, &str)> {
    if anchor.len() != ANCHOR_LEN || !anchor.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    Some(anchor.split_at(LINE_HASH_LEN))
}

pub fn compute_anchor(content: &str, prefix_state: &mut u32) -> String {
    let line_hash = compute_line_hash(content);
    *prefix_state = update_prefix_hash_state(*prefix_state, content);
    let prefix_hash = format_prefix_hash(*prefix_state);
    make_anchor(&line_hash, &prefix_hash)
}

/// Format a single anchored line and advance the rolling prefix state.
pub fn format_hashline(line_number: usize, content: &str, prefix_state: &mut u32) -> String {
    let anchor = compute_anchor(content, prefix_state);
    format!("{}:{}|{}", line_number, anchor, content)
}

/// Format content as anchored lines: `{N}:{anchor}|{line}` for each line.
pub fn format_as_hashlines(content: &str) -> String {
    let mut prefix_state = initial_prefix_hash_state();
    content
        .lines()
        .enumerate()
        .map(|(i, line)| format_hashline(i + 1, line, &mut prefix_state))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format file size in bytes to human-readable format (B, KB, MB, GB)
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// Normalize a workspace path argument from tool input.
///
/// Returns the provided default when the argument is omitted, trims surrounding
/// whitespace, and rejects blank strings so handlers can return a clear
/// InvalidInput error instead of passing empty paths into filesystem APIs.
pub fn normalize_workspace_path_input(
    path: Option<&str>,
    default_path: &str,
) -> Result<String, String> {
    let normalized = path.unwrap_or(default_path).trim();
    if normalized.is_empty() {
        return Err("Path parameter cannot be empty".to_string());
    }

    Ok(normalized.to_string())
}

/// Detect "not found" errors without depending on localized OS error strings.
pub fn is_not_found_io_error(error: &std::io::Error) -> bool {
    error.kind() == ErrorKind::NotFound
        || error.to_string().contains("No such file")
        || error.to_string().contains("not found")
}

/// Detect language/syntax highlighting identifier from file extension
pub fn detect_language(path: &std::path::Path) -> &'static str {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match extension {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "md" => "markdown",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sh" => "bash",
        "ps1" => "powershell",
        "html" => "html",
        "css" => "css",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "sql" => "sql",
        "xml" => "xml",
        "txt" | "log" => "text",
        _ => "",
    }
}

/// Validate path with security checks
pub fn validate_path_with_error(
    file_manager: &crate::services::SecureFileManager,
    path_str: &str,
) -> Result<std::path::PathBuf, String> {
    match file_manager
        .get_security_validator()
        .validate_path(path_str)
    {
        Ok(path) => Ok(path),
        Err(e) => {
            error!("Path validation failed: {}", e);
            Err(format!("Security error: {e}"))
        }
    }
}

/// Validate path for write/create operations — same as [`validate_path_with_error`]
/// but additionally blocks Windows reserved filenames (CON, NUL, COM1, …).
pub fn validate_path_with_error_for_write(
    file_manager: &crate::services::SecureFileManager,
    path_str: &str,
) -> Result<std::path::PathBuf, String> {
    match file_manager
        .get_security_validator()
        .validate_path_for_write(path_str)
    {
        Ok(path) => Ok(path),
        Err(e) => {
            error!("Path validation failed: {}", e);
            Err(format!("Security error: {e}"))
        }
    }
}

/// Read file as string (helper for edit operations)
pub async fn read_file_as_string(path: &std::path::Path) -> Result<String, String> {
    tokio::fs::read_to_string(path)
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData {
                "Failed to read file: Content appears to be binary or contains invalid UTF-8 characters. Please use a specialized tool for binary files.".to_string()
            } else {
                e.to_string()
            }
        })
}

/// Calculate text similarity (Levenshtein-based) for fuzzy matching
pub fn calculate_similarity(text1: &str, text2: &str) -> f32 {
    let len1 = text1.len();
    let len2 = text2.len();

    if len1 == 0 && len2 == 0 {
        return 1.0;
    }
    if len1 == 0 || len2 == 0 {
        return 0.0;
    }

    // Simplified similarity: count matching characters
    let matching_chars = text1
        .chars()
        .zip(text2.chars())
        .filter(|(a, b)| a == b)
        .count();

    matching_chars as f32 / len1.max(len2) as f32
}

/// Format full file diff output (Git-style unified diff)
pub fn format_file_diff(old_content: &str, new_content: &str, _file_path: &str) -> String {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();

    let added = new_lines.len().saturating_sub(old_lines.len());
    let removed = old_lines.len().saturating_sub(new_lines.len());

    let mut diff_lines = Vec::new();

    diff_lines.push(format!(
        "**Changes:** {} line(s) added, {} line(s) removed\n",
        added, removed
    ));
    diff_lines.push("**Diff:**".to_string());
    diff_lines.push("```diff".to_string());
    diff_lines.push(format!(
        "@@ -{},{} +{},{} @@",
        1,
        old_lines.len(),
        1,
        new_lines.len()
    ));

    // Show removed lines (limited to first 50 for display)
    let max_diff_lines = 50;
    let mut shown_lines = 0;

    for (idx, line) in old_lines.iter().enumerate() {
        if shown_lines >= max_diff_lines {
            diff_lines.push(format!(
                "... {} more removed lines omitted",
                old_lines.len() - idx
            ));
            break;
        }
        diff_lines.push(format!("- {}", line));
        shown_lines += 1;
    }

    shown_lines = 0;
    // Show added lines (limited to first 50 for display)
    for (idx, line) in new_lines.iter().enumerate() {
        if shown_lines >= max_diff_lines {
            diff_lines.push(format!(
                "... {} more added lines omitted",
                new_lines.len() - idx
            ));
            break;
        }
        diff_lines.push(format!("+ {}", line));
        shown_lines += 1;
    }

    diff_lines.push("```".to_string());

    diff_lines.join("\n")
}

/// Format diff output for string replacements (Git-style)
pub fn format_string_diff(replacements: &[(String, String)], file_path: &str) -> String {
    let language = detect_language(std::path::Path::new(file_path));
    let mut diff_lines = Vec::new();

    diff_lines.push("**Changes Made:**\n".to_string());
    diff_lines.push("```diff".to_string());

    for (idx, (old_str, new_str)) in replacements.iter().enumerate() {
        let old_lines: Vec<&str> = old_str.lines().collect();
        let new_lines: Vec<&str> = new_str.lines().collect();

        if idx > 0 {
            diff_lines.push(String::new()); // Separator between replacements
        }

        diff_lines.push(format!(
            "@@ Replacement #{}: {} line(s) → {} line(s) @@",
            idx + 1,
            old_lines.len(),
            new_lines.len()
        ));

        // Show removed lines
        for line in old_lines {
            diff_lines.push(format!("- {}", line));
        }

        // Show added lines
        for line in new_lines {
            diff_lines.push(format!("+ {}", line));
        }
    }

    diff_lines.push("```".to_string());

    if !language.is_empty() {
        diff_lines.push(format!("\n*Language: {}*", language));
    }

    diff_lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_line_hash_is_deterministic() {
        let h1 = compute_line_hash("fn main() {");
        let h2 = compute_line_hash("fn main() {");
        assert_eq!(h1, h2, "same input must always produce same hash");
    }

    #[test]
    fn test_compute_line_hash_differs_for_different_content() {
        let h1 = compute_line_hash("fn foo() {");
        let h2 = compute_line_hash("fn bar() {");
        assert_ne!(h1, h2, "different content should produce different hashes");
    }

    #[test]
    fn test_compute_line_hash_is_two_hex_chars() {
        let h = compute_line_hash("let x = 42;");
        assert_eq!(h.len(), 2, "hash must be exactly 2 chars");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "hash must be hex");
    }

    #[test]
    fn test_compute_line_hash_pipe_in_content_no_problem() {
        // Pipe character in content must not affect hash stability
        let h1 = compute_line_hash("let x = a | b | c;");
        let h2 = compute_line_hash("let x = a | b | c;");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_anchor_changes_when_earlier_content_changes() {
        let original = format_as_hashlines("alpha\nbeta\ngamma");
        let changed = format_as_hashlines("alpha!\nbeta\ngamma");

        let original_third = original.lines().nth(2).unwrap();
        let changed_third = changed.lines().nth(2).unwrap();

        let original_parts: Vec<&str> = original_third.splitn(2, '|').collect();
        let changed_parts: Vec<&str> = changed_third.splitn(2, '|').collect();

        assert_ne!(original_parts[0], changed_parts[0]);
    }

    #[test]
    fn test_format_as_hashlines_format() {
        let content = "fn foo() {\n    let x = 1;\n}";
        let result = format_as_hashlines(content);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3);

        for (i, line) in lines.iter().enumerate() {
            // Each line must be: {N}:{6-char-anchor}|{content}
            let colon = line.find(':').expect("must contain first ':'");
            let pipe = line.find('|').expect("must contain '|'");
            assert!(pipe > colon, "pipe must come after colon");

            let line_num: usize = line[..colon].parse().expect("prefix must be a number");
            assert_eq!(line_num, i + 1, "line numbers must be 1-based");

            let anchor = &line[colon + 1..pipe];
            assert_eq!(anchor.len(), ANCHOR_LEN, "anchor section must be 6 chars");
            assert!(anchor.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn test_format_as_hashlines_anchor_starts_with_compute_line_hash() {
        let content = "hello world\nrust is great";
        let result = format_as_hashlines(content);
        let lines: Vec<&str> = result.lines().collect();

        for (i, raw_line) in content.lines().enumerate() {
            let hashline = lines[i];
            let pipe = hashline.find('|').unwrap();
            let colon = hashline.find(':').unwrap();
            let anchor = &hashline[colon + 1..pipe];
            let embedded_hash = &anchor[..LINE_HASH_LEN];
            let expected_hash = compute_line_hash(raw_line);
            assert_eq!(
                embedded_hash,
                expected_hash,
                "embedded hash must match compute_line_hash for line {}",
                i + 1
            );
        }
    }

    #[test]
    fn test_format_as_hashlines_pipe_in_content_is_safe() {
        // Content with | must only split on the FIRST pipe
        let content = "let x = a | b | c;";
        let result = format_as_hashlines(content);
        let line = result.lines().next().unwrap();
        // After the first |, everything is content — must end with the full expression
        assert!(
            line.ends_with("let x = a | b | c;"),
            "content after first pipe must be verbatim: got '{}'",
            line
        );
    }

    #[test]
    fn test_parse_anchor_round_trips() {
        let mut prefix_state = initial_prefix_hash_state();
        let anchor = compute_anchor("alpha", &mut prefix_state);
        let (line_hash, prefix_hash) = parse_anchor(&anchor).expect("valid anchor");

        assert_eq!(line_hash.len(), LINE_HASH_LEN);
        assert_eq!(prefix_hash.len(), PREFIX_HASH_LEN);
        assert_eq!(anchor, make_anchor(line_hash, prefix_hash));
    }
}
