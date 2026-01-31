use super::super::WorkspaceServer;
use super::utils::{calculate_similarity, format_string_diff, read_file_as_string};
use crate::mcp::builtin::error_guidance::*;
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use tracing::{error, info};

impl WorkspaceServer {
    pub async fn handle_preview_replacement(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        use crate::mcp::builtin::error_guidance::ErrorCategory;

        // Parameter validation
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Parameter 'path' cannot be empty",
                    vec!["Provide a valid file path".to_string()],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        let old_string = match args.get("oldString").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Parameter 'oldString' cannot be empty",
                    vec![
                        "Extract exact text from readFile response".to_string(),
                        "Include surrounding context for uniqueness".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("oldString", ToolGroup::Workspace)),
        };

        let new_string = match args.get("newString").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return Ok(missing_param_error("newString", ToolGroup::Workspace)),
        };

        // Validate path and read file
        let safe_path = self.validate_path_with_error(path_str, session_id)?;
        let original_content = match read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(operation_failed_error(
                    "Read file for preview",
                    &e,
                    vec![
                        "Verify file exists with listDirectory".to_string(),
                        format!("Use readFile('{}') to check content", path_str),
                    ],
                    ToolGroup::Workspace,
                ));
            }
        };

        // Find matches and generate preview
        let occurrences = original_content.matches(old_string).count();

        if occurrences == 0 {
            // Similar content search
            let lines: Vec<&str> = original_content.lines().collect();
            let old_lines: Vec<&str> = old_string.lines().collect();
            let search_size = old_lines.len();

            let mut best_match: Option<(usize, f32)> = None;
            for (line_idx, window) in lines.windows(search_size.max(1)).enumerate() {
                let window_text = window.join("\n");
                let similarity = calculate_similarity(&window_text, old_string);
                if similarity > 0.3 && best_match.map_or(true, |m| similarity > m.1) {
                    best_match = Some((line_idx + 1, similarity));
                }
            }

            let suggestion = if let Some((line_num, similarity)) = best_match {
                format!(
                    "❌ Pattern NOT FOUND (but {}% similar at line {})\n\n\
                    💡 NEXT: Use readFile('{}', {}, {}) to see actual content",
                    (similarity * 100.0) as u32,
                    line_num,
                    path_str,
                    line_num,
                    line_num + search_size.saturating_sub(1)
                )
            } else {
                format!(
                    "❌ Pattern NOT FOUND in file\n\n\
                    💡 NEXT: Use readFile('{}') to see full content",
                    path_str
                )
            };

            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "Pattern not found in preview",
                vec![suggestion],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        if occurrences > 1 {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!("Pattern found {} times (not unique)", occurrences),
                vec![
                    "Include more surrounding context to make the pattern unique".to_string(),
                    format!("Use readFile('{}') to see full content", path_str),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Exactly 1 match - generate context preview
        let preview_diff = generate_replacement_context(&original_content, old_string, new_string);

        let output = format!(
            "**🔍 Preview Replacement**\n\n\
            **File:** `{}`\n\
            **Status:** ✅ EXACT MATCH FOUND\n\n\
            **Changes Preview:**\n\
            ```diff\n\
            {}\n\
            ```\n\n\
            **Next Steps:**\n\
            - ✅ Preview looks correct? Call editFile with SAME parameters\n\
            - 📖 Use readFile to see full file context",
            path_str, preview_diff
        );

        Ok(MCPResult::success_with_data(
            &output,
            json!({
                "path": path_str,
                "occurrences": 1,
                "status": "ready"
            }),
        ))
    }

    pub async fn handle_edit_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Parameter existence validation
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Parameter 'path' cannot be empty",
                    vec![
                        "Provide a valid file path: editFile({path, oldString, newString})"
                            .to_string(),
                        "Use listDirectory('.') to find files".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        // Get oldString parameter
        let old_string = match args.get("oldString").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Parameter 'oldString' cannot be empty",
                    vec![
                        "⚠️ CRITICAL: Call readFile FIRST to get exact content".to_string(),
                        "Extract text exactly as shown in readFile response".to_string(),
                        "Include surrounding context (3-5 lines) for uniqueness".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("oldString", ToolGroup::Workspace)),
        };

        // Get newString parameter (can be empty for deletion)
        let new_string = match args.get("newString").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return Ok(missing_param_error("newString", ToolGroup::Workspace)),
        };

        // Validate: Reject identical strings early (before file I/O)
        if old_string == new_string {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "oldString and newString are identical - no changes needed",
                vec![
                    "The replacement would result in no changes to the file".to_string(),
                    "Verify that newString contains the intended modifications".to_string(),
                    "If no changes are needed, consider skipping this operation".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Layer 2: Business logic - path validation and file reading
        let safe_path = self.validate_path_with_error(path_str, session_id.clone())?;

        let original_content = match read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(operation_failed_error(
                    "Read file for replacement",
                    &e,
                    vec![
                        "Verify the file exists with listDirectory".to_string(),
                        "Check file permissions".to_string(),
                        "Use readFile to see the current content".to_string(),
                    ],
                    ToolGroup::Workspace,
                ));
            }
        };

        // Count occurrences
        let occurrences = original_content.matches(old_string).count();

        if occurrences == 0 {
            // Calculate similarity for suggestions
            let lines: Vec<&str> = original_content.lines().collect();
            let old_lines: Vec<&str> = old_string.lines().collect();
            let search_size = old_lines.len();

            let mut best_match: Option<(usize, f32)> = None; // (line_num, similarity)

            // Search for similar content
            for (idx, window) in lines.windows(search_size.max(1)).enumerate() {
                let window_text = window.join("\n");
                let similarity = calculate_similarity(&window_text, old_string);

                if similarity > 0.3 {
                    // 30% threshold
                    if best_match.map_or(true, |m| similarity > m.1) {
                        best_match = Some((idx + 1, similarity));
                    }
                }
            }

            let suggestion = if let Some((line_num, similarity)) = best_match {
                format!(
                    "Similar content found at line {} ({}% match).

⚠️ MANDATORY STEPS:
1. Call readFile('{}', {}, {}) to see the ACTUAL content
2. Extract the exact text from readFile response (including whitespace)
3. Use the extracted text as oldString in your next attempt

💡 RECOMMENDED: Use previewReplacement BEFORE editFile
   → previewReplacement(path, oldString, newString) shows exact diffs
   → Catches mismatches early and shows line numbers

❌ DO NOT retry with the same oldString
❌ DO NOT reconstruct the text from previous attempts",
                    line_num,
                    (similarity * 100.0) as u32,
                    path_str,
                    line_num,
                    line_num + search_size.saturating_sub(1)
                )
            } else {
                format!(
                    "Pattern not found in file.

⚠️ MANDATORY STEPS:
1. Call readFile('{}') to see current file content
2. Extract the exact text you want to replace from readFile response
3. Use the extracted text as oldString (must match EXACTLY including whitespace)

💡 RECOMMENDED: Use previewReplacement BEFORE editFile
   → previewReplacement(path, oldString, newString) verifies without modification
   → Shows exact line numbers and context for better accuracy

❌ DO NOT retry without reading the file first
❌ DO NOT use oldString reconstructed from previous attempts or assumptions",
                    path_str
                )
            };

            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "Pattern not found",
                vec![suggestion],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        if occurrences > 1 {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!("Pattern found {} times (must be unique)", occurrences),
                vec![
                    "Include more surrounding context (5-10 lines) to make the pattern unique"
                        .to_string(),
                    format!("Use readFile('{}') to see the full content", path_str),
                    "Use previewReplacement to verify before actual replacement".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Perform replacement (exactly one match)
        let new_content = original_content.replacen(old_string, new_string, 1);

        // Write the modified content
        let file_manager = self.get_file_manager(session_id);
        match file_manager.write_file_string(path_str, &new_content).await {
            Ok(_) => {
                // Invalidate service context cache
                self.invalidate_context_cache().await;

                // Generate diff output
                let diff_output = format_string_diff(
                    &[(old_string.to_string(), new_string.to_string())],
                    path_str,
                );

                let message = format!(
                    "**✅ String Replacement Successful**\n\n\
                    **File:** `{}`\n\n\
                    {}\n\n\
                    **Next Steps:**\n\
                    - Use readFile to verify the changes\n\
                    - For multiple changes, call editFile again\n\
                    - Each replacement is atomic and independent",
                    path_str, diff_output
                );

                Ok(MCPResult::success_with_data(
                    &message,
                    json!({
                        "path": path_str,
                        "old_string_length": old_string.len(),
                        "new_string_length": new_string.len(),
                        "diff": diff_output,
                    }),
                ))
            }
            Err(e) if e.contains("Permission denied") => Ok(ErrorGuidance::with_guidance(
                ErrorCategory::PermissionDenied,
                format!("Permission denied writing to '{}'", path_str),
                vec![
                    "File may be read-only or locked by another process".to_string(),
                    "Use listDirectory to check file permissions".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result()),
            Err(e) => Ok(operation_failed_error(
                "Write file",
                &e,
                vec![
                    "File may be locked or inaccessible".to_string(),
                    format!("Use readFile('{}') to verify file still exists", path_str),
                ],
                ToolGroup::Workspace,
            )),
        }
    }
}

/// Generate contextual diff preview (shows surrounding lines)
fn generate_replacement_context(content: &str, old_string: &str, new_string: &str) -> String {
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

impl WorkspaceServer {
    pub async fn handle_edit_file_multi(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Parameter Validation
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Parameter 'path' cannot be empty",
                    vec![
                        "Provide a valid file path: editFileMulti({path, replacements})"
                            .to_string(),
                        "Use listDirectory('.') to find files".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        // Parse replacements array
        let replacements = match args.get("replacements").and_then(|v| v.as_array()) {
            Some(arr) if !arr.is_empty() => arr,
            Some(_) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    "Parameter 'replacements' cannot be empty",
                    vec![
                        "Provide at least one replacement: [{oldString, newString}]".to_string(),
                        "Use editFile for single replacements".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("replacements", ToolGroup::Workspace)),
        };

        // Normalization: Handle both objects and JSON strings
        let normalized_replacements = match normalize_replacements(replacements) {
            Ok(normalized) => normalized,
            Err((msg, idx)) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    msg,
                    vec![
                        "Each replacement must be an object: {oldString, newString}".to_string(),
                        format!("Found invalid type at index {}", idx),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        // Validate replacements structure and extract pairs
        let mut replacement_pairs: Vec<(&str, &str)> = Vec::new();
        for (idx, replacement) in normalized_replacements.iter().enumerate() {
            let obj = replacement.as_object().unwrap();

            let old_string = match obj.get("oldString").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s,
                Some(_) => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!("oldString at index {} cannot be empty", idx),
                        vec![
                            "⚠️ CRITICAL: Call readFile FIRST to get exact content".to_string(),
                            "Extract text exactly as shown in readFile response".to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
                None => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!("Missing 'oldString' at index {}", idx),
                        vec![
                            "Each replacement must have 'oldString' and 'newString' fields"
                                .to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
            };

            let new_string = match obj.get("newString").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!("Missing 'newString' at index {}", idx),
                        vec![
                            "Each replacement must have 'oldString' and 'newString' fields"
                                .to_string(),
                        ],
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
            };

            // Check for identical strings
            if old_string == new_string {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Replacement at index {}: oldString and newString are identical",
                        idx
                    ),
                    vec![
                        "No changes would be made with this replacement".to_string(),
                        "Remove this replacement from the array or modify newString".to_string(),
                    ],
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }

            replacement_pairs.push((old_string, new_string));
        }

        // Limit number of replacements
        if replacement_pairs.len() > 50 {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!(
                    "Too many replacements: {} (maximum 50)",
                    replacement_pairs.len()
                ),
                vec![
                    "Split into multiple editFileMulti calls if needed".to_string(),
                    "Or use multiple editFile calls for independent changes".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Layer 2: Path validation and file reading
        let safe_path = self.validate_path_with_error(path_str, session_id.clone())?;
        let original_content = match read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(operation_failed_error(
                    "Read file for multi-replacement",
                    &e,
                    vec![
                        "Verify the file exists with listDirectory".to_string(),
                        format!("Use readFile('{}') to check content", path_str),
                    ],
                    ToolGroup::Workspace,
                ));
            }
        };

        // Layer 3: Validate all patterns before applying any changes
        let mut validation_errors = Vec::new();
        let mut current_content = original_content.clone();

        for (idx, (old_string, _)) in replacement_pairs.iter().enumerate() {
            let occurrences = current_content.matches(old_string).count();

            if occurrences == 0 {
                // Find similar content for suggestions
                let lines: Vec<&str> = current_content.lines().collect();
                let old_lines: Vec<&str> = old_string.lines().collect();
                let search_size = old_lines.len();

                let mut best_match: Option<(usize, f32)> = None;
                for (line_idx, window) in lines.windows(search_size.max(1)).enumerate() {
                    let window_text = window.join("\n");
                    let similarity = calculate_similarity(&window_text, old_string);
                    if similarity > 0.3 && best_match.map_or(true, |m| similarity > m.1) {
                        best_match = Some((line_idx + 1, similarity));
                    }
                }

                let suggestion = if let Some((line_num, similarity)) = best_match {
                    format!(
                        "Replacement {}: Pattern NOT FOUND ({}% similar at line {})",
                        idx,
                        (similarity * 100.0) as u32,
                        line_num
                    )
                } else {
                    format!("Replacement {}: Pattern NOT FOUND in file", idx)
                };
                validation_errors.push(suggestion);
            } else if occurrences > 1 {
                validation_errors.push(format!(
                    "Replacement {}: Pattern found {} times (not unique)",
                    idx, occurrences
                ));
            } else {
                // Exactly 1 match - simulate the replacement for next validation
                current_content = current_content.replacen(old_string, replacement_pairs[idx].1, 1);
            }
        }

        // If any validation errors, return them all
        if !validation_errors.is_empty() {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "Some patterns failed validation - NO changes applied",
                vec![
                    "❌ VALIDATION ERRORS:".to_string(),
                    validation_errors.join("\n"),
                    "".to_string(),
                    "⚠️ ALL patterns must be valid for atomic operation".to_string(),
                    format!("💡 Use readFile('{}') to see current content", path_str),
                    "💡 Fix all patterns and try again".to_string(),
                ],
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        // Layer 4: Apply all replacements sequentially
        let mut modified_content = original_content.clone();
        for (old_string, new_string) in &replacement_pairs {
            modified_content = modified_content.replacen(old_string, new_string, 1);
        }

        // Layer 5: Write the modified content
        let file_manager = self.get_file_manager(session_id.clone());
        match file_manager
            .write_file_string(path_str, &modified_content)
            .await
        {
            Ok(()) => {
                info!(
                    "Successfully applied {} replacements to {}",
                    replacement_pairs.len(),
                    path_str
                );

                // Invalidate service context cache
                self.invalidate_context_cache().await;

                // Generate diff preview
                let original_lines = original_content.lines().count();
                let modified_lines = modified_content.lines().count();
                let size_change = (modified_content.len() as i64) - (original_content.len() as i64);

                let output = format!(
                    "**✅ Applied {} Replacements Successfully**\n\n\
                    **File:** `{}`\n\
                    **Original:** {} lines, {} bytes\n\
                    **Modified:** {} lines, {} bytes ({}{})\n\n\
                    **Changes Summary:**\n\
                    {}\n\n\
                    **Next Steps:**\n\
                    - 📖 Use `readFile(\"{}\")` to verify changes\n\
                    - 🔄 Use `editFile` to make further adjustments if needed",
                    replacement_pairs.len(),
                    path_str,
                    original_lines,
                    original_content.len(),
                    modified_lines,
                    modified_content.len(),
                    if size_change >= 0 { "+" } else { "" },
                    size_change,
                    replacement_pairs
                        .iter()
                        .enumerate()
                        .map(|(idx, (old, new))| {
                            let old_preview = if old.len() > 50 {
                                format!("{}...", &old[..47])
                            } else {
                                old.to_string()
                            };
                            let new_preview = if new.len() > 50 {
                                format!("{}...", &new[..47])
                            } else {
                                new.to_string()
                            };
                            format!("{}. \"{}\" → \"{}\"", idx + 1, old_preview, new_preview)
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    path_str
                );

                Ok(MCPResult::success_with_data(
                    &output,
                    json!({
                        "path": path_str,
                        "replacements_applied": replacement_pairs.len(),
                        "original_size": original_content.len(),
                        "modified_size": modified_content.len(),
                        "size_change": size_change,
                        "original_lines": original_lines,
                        "modified_lines": modified_lines
                    }),
                ))
            }
            Err(e) => {
                error!("Failed to write file {}: {}", path_str, e);
                Ok(operation_failed_error(
                    "Write multi-replacement changes",
                    &e,
                    vec![
                        "Check file permissions".to_string(),
                        "Ensure file is not locked by another process".to_string(),
                        "Original file was NOT modified".to_string(),
                    ],
                    ToolGroup::Workspace,
                ))
            }
        }
    }
}

/// Helper function to normalize replacements array
/// Returns Ok(Vec<Value>) if all replacements are valid objects (or valid JSON strings parsing to objects)
/// Returns Err((error_message, index)) if any replacement is invalid
fn normalize_replacements(replacements: &[Value]) -> Result<Vec<Value>, (String, usize)> {
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
