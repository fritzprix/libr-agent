//! Structured object-file editing (`workspace__editObject`).
//!
//! JSON parse/serialize helpers live in [`format`]; mutations apply to
//! `serde_json::Value`.

mod format;
mod ops;
mod pointer;

use super::super::workspace_server::path_validation_failure_guidance;
use super::super::WorkspaceServer;
use super::utils::format_file_diff;
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use format::{ensure_json_path, parse_json, serialize_json, MAX_OPS};
use ops::{apply_ops, parse_op, ApplyError, ObjectOp};
use serde_json::Value;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncReadExt;

impl WorkspaceServer {
    pub async fn handle_edit_object(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let path_str = match args.get("path").and_then(|value| value.as_str()) {
            Some(path) if !path.trim().is_empty() => path.trim(),
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Path parameter cannot be empty",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Provide a .json file path (relative paths resolve from the workspace)"
                        .to_string(),
                ])
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("path", ToolGroup::Workspace)),
        };

        if path_str.contains("..") {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Path traversal patterns (..) are not allowed",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use a normal file path without '..' traversal segments".to_string(),
            ])
            .to_mcp_result());
        }

        if let Err(message) = ensure_json_path(path_str) {
            return Ok(
                guided_error(ErrorCategory::InvalidInput, message, ToolGroup::Workspace)
                    .guidance(vec![
                "MVP supports .json only".to_string(),
                "For YAML/TOML/source files use workspace__strReplace or workspace__writeFile"
                    .to_string(),
            ])
                    .to_mcp_result(),
            );
        }

        let ops_value = match args.get("ops") {
            Some(Value::Array(items)) => items,
            Some(_) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'ops' must be an array of mutation objects",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Example: {\"path\":\"package.json\",\"ops\":[{\"op\":\"set\",\"path\":\"/scripts/dev\",\"value\":\"vite\"}]}"
                        .to_string(),
                ])
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("ops", ToolGroup::Workspace)),
        };

        if ops_value.is_empty() {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Parameter 'ops' must contain at least one mutation",
                ToolGroup::Workspace,
            )
            .guidance(vec!["Provide 1..=32 ops".to_string()])
            .to_mcp_result());
        }
        if ops_value.len() > MAX_OPS {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Too many ops ({}); maximum is {MAX_OPS}. Split into multiple editObject calls.",
                    ops_value.len()
                ),
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        let mut parsed_ops: Vec<ObjectOp> = Vec::with_capacity(ops_value.len());
        for (index, raw_op) in ops_value.iter().enumerate() {
            match parse_op(raw_op, index) {
                Ok(op) => parsed_ops.push(op),
                Err(error) => return Ok(apply_error_result(error)),
            }
        }

        let target_session_id = session_id
            .clone()
            .unwrap_or_else(|| self.session_id.clone());

        let safe_path = match self
            .validate_write_path_with_teamwork_access(path_str, Some(target_session_id))
            .await
        {
            Ok(path) => path,
            Err(error) => {
                return Ok(guided_error(
                    ErrorCategory::PermissionDenied,
                    format!("Path validation failed: {error}"),
                    ToolGroup::Workspace,
                )
                .guidance(path_validation_failure_guidance(
                    &error,
                    vec![
                        "Verify the target path is not a protected location".to_string(),
                        "Use workspace__listDirectory to see available paths".to_string(),
                    ],
                ))
                .to_mcp_result());
            }
        };

        if let Err(error) = self
            .sync_attach_before_host_read(&safe_path, session_id.as_deref())
            .await
        {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!("Failed to sync attached container file before edit: {error}"),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Verify the Harbor/Docker container is still running".to_string(),
                "Retry after confirming docker exec works".to_string(),
            ])
            .to_mcp_result());
        }

        if !safe_path.exists() {
            return Ok(not_found_error("file", path_str, ToolGroup::Workspace));
        }

        let original_content = match read_validated_utf8_file(&safe_path).await {
            Ok(content) => content,
            Err(error) => {
                return Ok(
                    guided_error(ErrorCategory::InvalidInput, error, ToolGroup::Workspace)
                        .to_mcp_result(),
                );
            }
        };

        let original_value = match parse_json(&original_content) {
            Ok(value) => value,
            Err(error) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    error,
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Repair the JSON with workspace__readFile + workspace__writeFile/strReplace, then retry editObject"
                        .to_string(),
                ])
                .to_mcp_result());
            }
        };

        let mut working = original_value;
        let applied = match apply_ops(&mut working, &parsed_ops) {
            Ok(applied) => applied,
            Err(error) => return Ok(apply_error_result(error)),
        };

        let new_content = match serialize_json(&working) {
            Ok(text) => text,
            Err(error) => {
                return Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    error,
                    ToolGroup::Workspace,
                )
                .to_mcp_result());
            }
        };

        if new_content.len() > crate::config::max_file_size() {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Result would exceed the maximum allowed file size of {} bytes",
                    crate::config::max_file_size()
                ),
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        if let Err(error) = fs::write(&safe_path, &new_content).await {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!("Failed to write file: {error}"),
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        if let Err(sync_error) = self
            .sync_attach_after_host_write(&safe_path, session_id.as_deref())
            .await
        {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!(
                    "File was updated locally but failed to sync into the attached container: {sync_error}"
                ),
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }

        let mut lines = vec![format!(
            "OK: editObject applied {} op(s) to {path_str}",
            applied.len()
        )];
        lines.push(String::new());
        for item in &applied {
            lines.push(format!(
                "- [{}] {} {} — {}",
                item.index, item.op, item.path, item.summary
            ));
        }
        lines.push(String::new());
        lines.push(
            "Wrote pretty-printed JSON (2-space indent, trailing newline). Key encounter order is preserved; whitespace is normalized."
                .to_string(),
        );

        let diff_output = format_file_diff(&original_content, &new_content, path_str);
        if !diff_output.trim().is_empty() {
            lines.push(String::new());
            lines.push(diff_output.clone());
        }

        let ops_applied: Vec<Value> = applied
            .iter()
            .map(|item| {
                serde_json::json!({
                    "index": item.index,
                    "op": item.op,
                    "path": item.path,
                    "summary": item.summary,
                })
            })
            .collect();

        Ok(
            SuccessHint::new(lines.join("\n"), vec![]).to_mcp_result_with_data(Some(
                serde_json::json!({
                    "path": path_str,
                    "opsApplied": ops_applied,
                    "unified_diff": diff_output,
                }),
            )),
        )
    }
}

fn apply_error_result(error: ApplyError) -> MCPResult {
    guided_error(
        ErrorCategory::InvalidInput,
        format!(
            "editObject failed at ops[{}] ({} {}): {}",
            error.op_index, error.op, error.path, error.message
        ),
        ToolGroup::Workspace,
    )
    .guidance(error.guidance)
    .to_mcp_result()
}

async fn read_validated_utf8_file(path: &Path) -> Result<String, String> {
    use crate::mcp::builtin::workspace::text_encoding::{decode_text_bytes, DecodedText};

    let max_size = crate::config::max_file_size();
    let metadata = fs::metadata(path)
        .await
        .map_err(|error| format!("Failed to read file metadata: {error}"))?;

    if metadata.len() > max_size as u64 {
        return Err(format!(
            "File size error: File exceeds the maximum allowed size of {max_size} bytes"
        ));
    }

    let file = fs::File::open(path)
        .await
        .map_err(|error| format!("Failed to open file: {error}"))?;

    let mut buffer = Vec::new();
    let read_limit = (max_size as u64).saturating_add(1);
    let bytes_read = file
        .take(read_limit)
        .read_to_end(&mut buffer)
        .await
        .map_err(|error| format!("Failed to read file: {error}"))?;

    if bytes_read > max_size {
        return Err(format!(
            "File size error: File exceeds the maximum allowed size of {max_size} bytes"
        ));
    }

    match decode_text_bytes(&buffer) {
        DecodedText::Binary => Err(
            "Failed to read file: content appears to be binary (embedded null bytes)".to_string(),
        ),
        DecodedText::Text { text, .. } => Ok(text),
    }
}
