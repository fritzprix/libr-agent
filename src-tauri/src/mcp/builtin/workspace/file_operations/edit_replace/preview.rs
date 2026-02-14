use super::common::{find_best_match, generate_replacement_context};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::workspace::file_operations::utils::read_file_as_string;
use crate::mcp::builtin::workspace::WorkspaceServer;
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};

impl WorkspaceServer {
    pub async fn handle_preview_replacement(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Parameter validation
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'path' cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec!["Provide a valid file path".to_string()])
                .to_mcp_result());
            }
            None => {
                return Ok(missing_param_error("path", ToolGroup::Workspace));
            }
        };

        let old_string = match args.get("oldString").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'oldString' cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Extract exact text from readFile response".to_string(),
                    "Include surrounding context for uniqueness".to_string(),
                ])
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
                return Ok(
                    guided_error(ErrorCategory::OperationFailed, &e, ToolGroup::Workspace)
                        .guidance(vec![
                            "Verify file exists with listDirectory".to_string(),
                            format!("Use readFile('{}') to check content", path_str),
                        ])
                        .to_mcp_result(),
                );
            }
        };

        // Find matches and generate preview
        let occurrences = original_content.matches(old_string).count();

        if occurrences == 0 {
            // Similar content search
            let best_match = find_best_match(&original_content, old_string);
            let search_size = old_string.lines().count();

            let (error_msg, guidance) = if let Some((line_num, similarity)) = best_match {
                (
                    format!(
                        "Pattern NOT FOUND (but {}% similar at line {})",
                        (similarity * 100.0) as u32,
                        line_num
                    ),
                    vec![
                        format!(
                            "Use readFile('{}', {}, {}) to see actual content on these lines",
                            path_str,
                            line_num,
                            line_num + search_size.saturating_sub(1)
                        ),
                        "Extract the EXACT text from readFile response including ALL whitespace"
                            .to_string(),
                    ],
                )
            } else {
                (
                    "Pattern NOT FOUND in file".to_string(),
                    vec![
                        format!("Use readFile('{}') to see full current content", path_str),
                        "Verify the string you are trying to replace is exactly as it appears in the file".to_string(),
                    ],
                )
            };

            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                &error_msg,
                ToolGroup::Workspace,
            )
            .guidance(guidance)
            .to_mcp_result());
        }

        if occurrences > 1 {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Pattern found {} times (not unique)", occurrences),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Include more surrounding context (3-5 more lines) to make the pattern unique"
                    .to_string(),
                format!(
                    "Use readFile('{}') to see full content and find unique context",
                    path_str
                ),
            ])
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
            ```",
            path_str, preview_diff
        );

        let hint = SuccessHint::new(
            output,
            vec![
                "Preview looks correct? Call editFile with SAME parameters to apply".to_string(),
                "Use readFile to see full file context if needed".to_string(),
            ],
        );

        Ok(hint.to_mcp_result_with_data(Some(json!({
            "path": path_str,
            "occurrences": 1,
            "status": "ready"
        }))))
    }
}
