use super::super::WorkspaceServer;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;
use serde_json::Value;
use tokio::fs;
use tracing::info;

impl WorkspaceServer {
    pub async fn handle_import_files(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // ✅ ENHANCED: Batch file import handling with individual results tracking

        let files = match args.get("files").and_then(|v| v.as_array()) {
            Some(f) => f,
            None => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Missing required parameter: files (array)",
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        if files.is_empty() {
            return Ok(MCPResult::success("No files provided for import."));
        }

        let mut success_messages = Vec::new();
        let mut error_messages = Vec::new();

        let target_session_id = session_id.unwrap_or_else(|| self.session_id.clone());

        for file_val in files {
            // Extract srcAbsPath and destRelPath for each file
            let src_path_str = match file_val
                .get("srcAbsPath")
                .or_else(|| file_val.get("src_abs_path"))
                .and_then(|v| v.as_str())
            {
                Some(path) => path,
                None => {
                    error_messages.push("Missing srcAbsPath for a file item".to_string());
                    continue;
                }
            };

            let dest_rel_path = match file_val
                .get("destRelPath")
                .or_else(|| file_val.get("dest_rel_path"))
                .and_then(|v| v.as_str())
            {
                Some(path) => path,
                None => {
                    error_messages.push(format!("Missing destRelPath for file: {}", src_path_str));
                    continue;
                }
            };

            // Resolve and validate destination path
            let dest_abs_path = match self
                .validate_path_with_error_for_write(dest_rel_path, Some(target_session_id.clone()))
            {
                Ok(path) => path,
                Err(e) => {
                    error_messages.push(format!(
                        "Invalid destination path '{}': {}",
                        dest_rel_path, e
                    ));
                    continue;
                }
            };

            // Ensure parent directory exists
            if let Some(parent) = dest_abs_path.parent() {
                if let Err(e) = fs::create_dir_all(parent).await {
                    error_messages.push(format!(
                        "Failed to create directory '{}' for file '{}': {}",
                        parent.display(),
                        src_path_str,
                        e
                    ));
                    continue;
                }
            }

            // Perform copy
            info!(
                "Importing file: src='{}', dest='{}'",
                src_path_str,
                dest_abs_path.display()
            );
            match fs::copy(src_path_str, &dest_abs_path).await {
                Ok(_) => {
                    success_messages
                        .push(format!("Imported: {} -> {}", src_path_str, dest_rel_path));
                }
                Err(e) => {
                    error_messages.push(format!("Failed to import '{}': {}", src_path_str, e));
                }
            }
        }

        // Store flags before moving vectors
        let has_errors = !error_messages.is_empty();
        let has_success = !success_messages.is_empty();

        // Combine results
        let mut final_text = Vec::new();
        if has_success {
            final_text.push(format!(
                "Successfully imported {} file(s):",
                success_messages.len()
            ));
            final_text.extend(success_messages.into_iter().map(|s| format!("- {}", s)));
        }

        if has_errors {
            if !final_text.is_empty() {
                final_text.push("".to_string());
            }
            final_text.push(format!(
                "Failed to import {} file(s):",
                error_messages.len()
            ));
            final_text.extend(error_messages.into_iter().map(|e| format!("- {}", e)));
        }

        let result_text = final_text.join("\n");

        if has_errors && !has_success {
            // Entirely failed
            Ok(guided_error(
                ErrorCategory::InternalError,
                &result_text,
                ToolGroup::Workspace,
            )
            .to_mcp_result())
        } else {
            // Partially or fully successful
            Ok(MCPResult::success(&result_text))
        }
    }
}
