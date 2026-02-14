use serde_json::Value;

/// Generate contextual diff preview (shows surrounding lines)
pub fn generate_replacement_context(content: &str, old_string: &str, new_string: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let search_lines: Vec<&str> = old_string.lines().collect();

    // Find the match location
    for (line_idx, window) in lines.windows(search_lines.len()).enumerate() {
        if window.join("\n") == old_string {
            // Show context: 2 lines before, matched section, 2 lines after
            let context_start = line_idx.saturating_sub(2);
            let context_end = (line_idx + search_lines.len() + 2).min(lines.len());

            let mut diff_lines = Vec::new();
            diff_lines.push(format!(
                "@@ Lines {}-{} (showing context) @@",
                line_idx + 1,
                line_idx + search_lines.len()
            ));

            for (i, line) in lines[context_start..context_end].iter().enumerate() {
                let absolute_line = context_start + i + 1;
                let relative_to_match = (context_start + i) as isize - line_idx as isize;

                if relative_to_match < 0 || relative_to_match >= search_lines.len() as isize {
                    // Context lines (unchanged)
                    diff_lines.push(format!("  {:4} | {}", absolute_line, line));
                } else {
                    // Matched lines (will be replaced)
                    diff_lines.push(format!("- {:4} | {}", absolute_line, line));
                }
            }

            // Show new content
            for (i, new_line) in new_string.lines().enumerate() {
                let target_line = line_idx + i + 1;
                diff_lines.push(format!("+ {:4} | {}", target_line, new_line));
            }

            return diff_lines.join("\n");
        }
    }

    "ERROR: Match location not found (should not happen)".to_string()
}

/// Helper function to normalize replacements array
/// Returns Ok(Vec<Value>) if all replacements are valid objects (or valid JSON strings parsing to objects)
/// Returns Err((error_message, index)) if any replacement is invalid
pub fn normalize_replacements(replacements: &[Value]) -> Result<Vec<Value>, (String, usize)> {
    let mut normalized = Vec::with_capacity(replacements.len());

    for (idx, replacement) in replacements.iter().enumerate() {
        if replacement.is_object() {
            normalized.push(replacement.clone());
        } else if let Some(s) = replacement.as_str() {
            match serde_json::from_str::<Value>(s) {
                Ok(v) if v.is_object() => normalized.push(v),
                _ => {
                    return Err((
                        format!(
                            "Replacement at index {} is not a valid object or JSON string",
                            idx
                        ),
                        idx,
                    ));
                }
            }
        } else {
            return Err((
                format!("Replacement at index {} is not an object", idx),
                idx,
            ));
        }
    }

    Ok(normalized)
}

/// Find best similar match for a pattern in content
pub fn find_best_match(content: &str, pattern: &str) -> Option<(usize, f32)> {
    use crate::mcp::builtin::workspace::file_operations::utils::calculate_similarity;

    let lines: Vec<&str> = content.lines().collect();
    let old_lines: Vec<&str> = pattern.lines().collect();
    let search_size = old_lines.len();

    let mut best_match: Option<(usize, f32)> = None;
    for (line_idx, window) in lines.windows(search_size.max(1)).enumerate() {
        let window_text = window.join("\n");
        let similarity = calculate_similarity(&window_text, pattern);
        if similarity > 0.3 && best_match.as_ref().is_none_or(|m| similarity > m.1) {
            best_match = Some((line_idx + 1, similarity));
        }
    }
    best_match
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_normalize_replacements_objects() {
        let input = vec![
            json!({"oldString": "foo", "newString": "bar"}),
            json!({"oldString": "baz", "newString": "qux"}),
        ];

        let result = normalize_replacements(&input);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0]["oldString"], "foo");
        assert_eq!(normalized[1]["newString"], "qux");
    }

    #[test]
    fn test_normalize_replacements_strings() {
        let input = vec![
            json!("{\"oldString\": \"foo\", \"newString\": \"bar\"}"),
            json!("{\"oldString\": \"baz\", \"newString\": \"qux\"}"),
        ];

        let result = normalize_replacements(&input);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0]["oldString"], "foo");
        assert_eq!(normalized[1]["newString"], "qux");
    }

    #[test]
    fn test_normalize_replacements_mixed() {
        let input = vec![
            json!({"oldString": "foo", "newString": "bar"}),
            json!("{\"oldString\": \"baz\", \"newString\": \"qux\"}"),
        ];

        let result = normalize_replacements(&input);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0]["oldString"], "foo");
        assert_eq!(normalized[1]["newString"], "qux");
    }

    #[test]
    fn test_normalize_replacements_invalid_string() {
        let input = vec![
            json!({"oldString": "foo", "newString": "bar"}),
            json!("invalid-json"),
        ];

        let result = normalize_replacements(&input);
        assert!(result.is_err());
        let (msg, idx) = result.unwrap_err();
        assert_eq!(idx, 1);
        assert!(msg.contains("not a valid object or JSON string"));
    }

    #[test]
    fn test_normalize_replacements_invalid_type() {
        let input = vec![json!({"oldString": "foo", "newString": "bar"}), json!(123)];

        let result = normalize_replacements(&input);
        assert!(result.is_err());
        let (msg, idx) = result.unwrap_err();
        assert_eq!(idx, 1);
        assert!(msg.contains("not an object"));
    }
}
