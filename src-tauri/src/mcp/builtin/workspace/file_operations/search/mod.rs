mod content;
mod files;
mod helpers;

use super::super::WorkspaceServer;
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::Value;

impl WorkspaceServer {
    pub async fn handle_search(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let search_path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(missing_param_error("path", ToolGroup::Workspace)),
        };

        let query = args.get("query").and_then(|v| v.as_str());
        let file_pattern = args.get("filePattern").and_then(|v| v.as_str());
        let ignore_case = args
            .get("ignoreCase")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let show_line_anchors = args
            .get("showLineAnchors")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let limit_raw = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
        if !(1..=1000).contains(&limit_raw) {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Invalid pagination parameter: limit must be between 1 and 1000, got {}",
                    limit_raw
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Set limit to a value between 1 and 1000".to_string(),
                "Use offset to paginate through additional results".to_string(),
            ])
            .to_mcp_result());
        }
        let limit = match usize::try_from(limit_raw) {
            Ok(value) => value,
            Err(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Invalid pagination parameter: limit is too large for this platform ({})",
                        limit_raw
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Set limit to a value between 1 and 1000".to_string(),
                    "Use smaller page sizes when paginating search results".to_string(),
                ])
                .to_mcp_result());
            }
        };

        let offset_raw = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
        let offset = match usize::try_from(offset_raw) {
            Ok(value) => value,
            Err(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Invalid pagination parameter: offset is too large for this platform ({})",
                        offset_raw
                    ),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Set offset to a smaller non-negative value".to_string(),
                    "Use limit and offset together to page through results".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Security validation
        let file_manager = self.get_file_manager(session_id.clone());
        let safe_path = match file_manager
            .get_security_validator()
            .validate_path_for_read(search_path)
        {
            Ok(path) => path,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {}", e),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the file path is within allowed directories".to_string(),
                    "Use listDirectory to see available files".to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Determine if we are doing file name search only or content search
        if query.is_none() {
            // File Name Search Only
            let pattern = match file_pattern {
                Some(p) => p,
                None => {
                    return Ok(guided_error(
                        ErrorCategory::MissingRequiredParam,
                        "Provide either 'query' (to search text) or 'filePattern' (to find files)"
                            .to_string(),
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
            };
            return files::search_files_only(&safe_path, search_path, pattern, limit, offset).await;
        }

        // Text Content Search
        let query_str = query.unwrap();
        let regex = match regex::RegexBuilder::new(query_str)
            .case_insensitive(ignore_case)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid regex pattern: {}", e),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Check regex syntax - use basic patterns like 'error|warning'".to_string(),
                    "Escape special characters with backslash: \\. \\* \\+ \\?".to_string(),
                ])
                .to_mcp_result());
            }
        };

        let glob_pat = match file_pattern {
            Some(p) => match glob::Pattern::new(p) {
                Ok(pat) => Some(pat),
                Err(e) => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Invalid filePattern: {}", e),
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
            },
            None => None,
        };

        if safe_path.is_dir() {
            content::search_content_in_dir(
                &safe_path,
                search_path,
                &regex,
                query_str,
                glob_pat.as_ref(),
                show_line_anchors,
                ignore_case,
                limit,
                offset,
            )
            .await
        } else {
            content::search_content_in_file(
                &safe_path,
                search_path,
                &regex,
                query_str,
                show_line_anchors,
                ignore_case,
                limit,
                offset,
            )
            .await
        }
    }
}
