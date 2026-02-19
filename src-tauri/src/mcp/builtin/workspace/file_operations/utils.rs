use tracing::error;

// ✅ ENHANCED: Threshold for using spawn_blocking for CPU-intensive line enumeration
// Large files can block the async runtime during line enumeration
pub const LARGE_FILE_THRESHOLD: u64 = 1_048_576; // 1 MB in bytes

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

/// Compute a short MD5 hash for a line of text (first 4 chars)
pub fn compute_line_hash(line: &str) -> String {
    let digest = md5::compute(line);
    let hash_str = format!("{:x}", digest);
    hash_str[..4].to_string()
}
