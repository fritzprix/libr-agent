mod content;
mod files;
mod helpers;

use super::super::edit_mode::LINE_ANCHORS_ENABLED;
use super::super::WorkspaceServer;
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::builtin::workspace::utils::is_internal_workspace_artifact_path;
use crate::mcp::types::MCPResult;
use helpers::{parse_pagination, reject_empty_optional_str};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tracing::warn;

struct ResolvedSearchTarget {
    workspace_root: PathBuf,
    safe_path: PathBuf,
    display_path: String,
}

struct GrepSearchParams<'a> {
    workspace_root: &'a Path,
    safe_path: &'a Path,
    display_path: &'a str,
    query_str: &'a str,
    file_pattern: Option<&'a str>,
    ignore_case: bool,
    show_line_anchors: bool,
    limit: usize,
    offset: usize,
}

impl WorkspaceServer {
    pub async fn handle_glob_files(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let search_path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(missing_param_error("path", ToolGroup::Workspace)),
        };

        let file_pattern = match args.get("filePattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(missing_param_error("filePattern", ToolGroup::Workspace)),
        };

        let file_pattern = match reject_empty_optional_str(
            Some(file_pattern),
            "filePattern",
            vec!["Provide a non-empty glob like `*.rs` or `src/**/*.ts`".to_string()],
        ) {
            Ok(Some(p)) => p,
            Ok(None) => {
                return Ok(missing_param_error("filePattern", ToolGroup::Workspace));
            }
            Err(result) => return Ok(result),
        };

        let (limit, offset) = match parse_pagination(&args) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };

        let ResolvedSearchTarget {
            workspace_root,
            safe_path,
            display_path,
        } = match self.resolve_search_target(search_path, session_id).await {
            Ok(target) => target,
            Err(result) => return Ok(result),
        };

        files::search_files_only(
            &workspace_root,
            &safe_path,
            &display_path,
            file_pattern,
            limit,
            offset,
        )
        .await
    }

    pub async fn handle_grep_files(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let search_path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(missing_param_error("path", ToolGroup::Workspace)),
        };

        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return Ok(missing_param_error("query", ToolGroup::Workspace)),
        };

        let query = match reject_empty_optional_str(
            Some(query),
            "query",
            vec!["Provide a non-empty regex pattern for content search".to_string()],
        ) {
            Ok(Some(q)) => q,
            Ok(None) => return Ok(missing_param_error("query", ToolGroup::Workspace)),
            Err(result) => return Ok(result),
        };

        let file_pattern = match reject_empty_optional_str(
            args.get("filePattern").and_then(|v| v.as_str()),
            "filePattern",
            vec![
                "Provide a non-empty glob like `*.rs` or `src/**/*.ts`".to_string(),
                "Omit filePattern when you want content search across all files".to_string(),
            ],
        ) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };

        let ignore_case = args
            .get("ignoreCase")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let show_line_anchors = if LINE_ANCHORS_ENABLED {
            args.get("showLineAnchors")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        } else {
            false
        };

        let (limit, offset) = match parse_pagination(&args) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };

        let ResolvedSearchTarget {
            workspace_root,
            safe_path,
            display_path,
        } = match self.resolve_search_target(search_path, session_id).await {
            Ok(target) => target,
            Err(result) => return Ok(result),
        };

        self.search_content(GrepSearchParams {
            workspace_root: &workspace_root,
            safe_path: &safe_path,
            display_path: &display_path,
            query_str: query,
            file_pattern,
            ignore_case,
            show_line_anchors,
            limit,
            offset,
        })
        .await
    }

    /// Backward-compatible router for the hidden `searchFiles` dispatch alias.
    pub async fn handle_search(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        warn!(
            "searchFiles is deprecated; use workspace__globFiles for filename search or workspace__grepFiles for content search"
        );

        let query = args.get("query").and_then(|v| v.as_str());
        let file_pattern = args.get("filePattern").and_then(|v| v.as_str());

        if matches!(query, Some("")) {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Invalid query: query must not be empty".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Provide a non-empty regex pattern for content search with workspace__grepFiles"
                    .to_string(),
                "Use workspace__globFiles with filePattern when you only want to find files"
                    .to_string(),
            ])
            .to_mcp_result());
        }

        if matches!(file_pattern, Some("")) {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Invalid filePattern: filePattern must not be empty".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Provide a non-empty glob like `*.rs` or `src/**/*.ts`".to_string(),
                "Omit filePattern when you want content search across all files with workspace__grepFiles"
                    .to_string(),
            ])
            .to_mcp_result());
        }

        if query.is_some() {
            return self.handle_grep_files(args, session_id).await;
        }

        if file_pattern.is_some() {
            return self.handle_glob_files(args, session_id).await;
        }

        Ok(guided_error(
            ErrorCategory::MissingRequiredParam,
            "Provide either 'query' (to search text) or 'filePattern' (to find files)".to_string(),
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Use workspace__grepFiles with query for content search".to_string(),
            "Use workspace__globFiles with filePattern for filename search".to_string(),
        ])
        .to_mcp_result())
    }

    async fn resolve_search_target(
        &self,
        search_path: &str,
        session_id: Option<String>,
    ) -> Result<ResolvedSearchTarget, MCPResult> {
        let target_session_id = session_id.unwrap_or_else(|| self.session_id.clone());
        let workspace_root = self.get_workspace_dir(&target_session_id);

        let safe_path = match self
            .validate_read_path_with_skill_access(search_path, Some(target_session_id))
            .await
        {
            Ok(path) => path,
            Err(e) => {
                return Err(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {e}"),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Verify the file path is within allowed directories".to_string(),
                    "Use workspace__listDirectory to see available files".to_string(),
                ])
                .to_mcp_result());
            }
        };

        if safe_path.is_file() && is_internal_workspace_artifact_path(&workspace_root, &safe_path) {
            return Err(guided_error(
                ErrorCategory::InvalidInput,
                "Internal LibrAgent temp/export artifacts are excluded from search".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Search workspace files outside .libragent/tmp and .libragent/exports".to_string(),
                "Use workspace__readProcessOutput or workspace__listProcesses to inspect temp process output".to_string(),
                "Use workspace__export on real workspace files instead of searching generated export artifacts"
                    .to_string(),
            ])
            .to_mcp_result());
        }

        Ok(ResolvedSearchTarget {
            workspace_root,
            safe_path,
            display_path: search_path.to_string(),
        })
    }

    async fn search_content(&self, params: GrepSearchParams<'_>) -> Result<MCPResult, String> {
        let GrepSearchParams {
            workspace_root,
            safe_path,
            display_path,
            query_str,
            file_pattern,
            ignore_case,
            show_line_anchors,
            limit,
            offset,
        } = params;

        let regex = match regex::RegexBuilder::new(query_str)
            .case_insensitive(ignore_case)
            .multi_line(true)
            .crlf(true)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid regex pattern: {e}"),
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
                        format!("Invalid filePattern: {e}"),
                        ToolGroup::Workspace,
                    )
                    .to_mcp_result());
                }
            },
            None => None,
        };

        if safe_path.is_dir() {
            content::search_content_in_dir(content::SearchDirectoryRequest {
                workspace_root,
                dir: safe_path,
                file_pattern: glob_pat.as_ref(),
                search: content::SearchContentRequest {
                    display_path,
                    regex: &regex,
                    query: query_str,
                    show_hashes: show_line_anchors,
                    ignore_case,
                    limit,
                    offset,
                },
            })
            .await
        } else {
            content::search_content_in_file(
                safe_path,
                content::SearchContentRequest {
                    display_path,
                    regex: &regex,
                    query: query_str,
                    show_hashes: show_line_anchors,
                    ignore_case,
                    limit,
                    offset,
                },
            )
            .await
        }
    }
}
