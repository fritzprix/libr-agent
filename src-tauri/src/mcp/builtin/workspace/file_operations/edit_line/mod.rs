use super::super::WorkspaceServer;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use std::collections::BTreeMap;

mod apply;
mod args;
mod response;
mod types;

use apply::prepare_file_edit_batch;
use args::{
    canonicalize_edit_files_args, canonicalize_legacy_edit_file_args_as_edit_files,
    parse_line_edit, validate_edit_files_arguments,
};
use response::build_edit_files_success;
use types::{LineEdit, ParsedEdit, PreparedFileEdit};

fn build_single_file_delegated_args(args: &Value, edits: Vec<Value>) -> Value {
    json!({
        "path": args.get("path").cloned().unwrap_or(Value::Null),
        "edits": edits,
    })
}

fn tag_edit_action(edit: &Value, action: &str) -> Value {
    let mut item = edit.clone();
    if let Some(obj) = item.as_object_mut() {
        obj.insert("action".to_string(), json!(action));
    }
    item
}

fn normalize_insert_after_edit(edit: &Value) -> Value {
    let mut item = json!({
        "action": "INSERT_AFTER",
        "line": edit.get("afterLine").cloned().unwrap_or(Value::Null),
        "new_value": edit.get("new_value").cloned().unwrap_or(Value::Null),
        "anchor": edit.get("anchor").cloned().unwrap_or(Value::Null),
    });

    if let (Some(obj), Some(src)) = (item.as_object_mut(), edit.as_object()) {
        for (key, value) in src {
            obj.entry(key).or_insert_with(|| value.clone());
        }
    }

    item
}

async fn write_prepared_batches(
    server: &WorkspaceServer,
    prepared_batches: &[PreparedFileEdit],
    session_id: Option<String>,
) -> Result<(), MCPResult> {
    let file_manager = server.get_file_manager(session_id);
    let mut written_paths: Vec<String> = Vec::new();

    for batch in prepared_batches {
        if let Err(error) = file_manager
            .write_file_string(&batch.path, &batch.new_content)
            .await
        {
            let mut rollback_failures = Vec::new();
            for written_path in written_paths.iter().rev() {
                if let Some(previous_batch) = prepared_batches
                    .iter()
                    .find(|candidate| &candidate.path == written_path)
                {
                    if let Err(rollback_error) = file_manager
                        .write_file_string(&previous_batch.path, &previous_batch.original_content)
                        .await
                    {
                        rollback_failures
                            .push(format!("{} ({})", previous_batch.path, rollback_error));
                    }
                }
            }

            let rollback_note = if rollback_failures.is_empty() {
                "Any earlier writes in this request were rolled back.".to_string()
            } else {
                format!("Rollback failed for: {}", rollback_failures.join(", "))
            };

            return Err(guided_error(
                ErrorCategory::OperationFailed,
                format!("Failed to write '{}': {}", batch.path, error),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                rollback_note,
                "Check file permissions and available disk space".to_string(),
                "Rerun readFile(showLineAnchors=true) before retrying if files may have changed"
                    .to_string(),
            ])
            .to_mcp_result());
        }

        written_paths.push(batch.path.clone());
    }

    Ok(())
}

impl WorkspaceServer {
    pub async fn handle_replace_lines(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        if let Some(edits) = args.get("edits").and_then(|value| value.as_array()) {
            let delegated = build_single_file_delegated_args(
                &args,
                edits
                    .iter()
                    .map(|edit| tag_edit_action(edit, "REPLACE"))
                    .collect(),
            );
            return self.handle_edit_file(delegated, session_id).await;
        }

        let delegated = build_single_file_delegated_args(
            &args,
            vec![json!({
                "action": "REPLACE",
                "line": args.get("line").cloned().unwrap_or(Value::Null),
                "endLine": args.get("endLine").cloned().unwrap_or(Value::Null),
                "new_value": args.get("new_value").cloned().unwrap_or(Value::Null),
                "anchor": args.get("anchor").cloned().unwrap_or(Value::Null),
                "endAnchor": args.get("endAnchor").cloned().unwrap_or(Value::Null),
            })],
        );

        self.handle_edit_file(delegated, session_id).await
    }

    pub async fn handle_insert_after_line(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        if let Some(edits) = args.get("edits").and_then(|value| value.as_array()) {
            let delegated = build_single_file_delegated_args(
                &args,
                edits.iter().map(normalize_insert_after_edit).collect(),
            );
            return self.handle_edit_file(delegated, session_id).await;
        }

        let delegated = build_single_file_delegated_args(
            &args,
            vec![json!({
                "action": "INSERT_AFTER",
                "line": args.get("afterLine").cloned().unwrap_or(Value::Null),
                "new_value": args.get("new_value").cloned().unwrap_or(Value::Null),
                "anchor": args.get("anchor").cloned().unwrap_or(Value::Null),
            })],
        );

        self.handle_edit_file(delegated, session_id).await
    }

    pub async fn handle_delete_lines(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        if let Some(edits) = args.get("edits").and_then(|value| value.as_array()) {
            let delegated = build_single_file_delegated_args(
                &args,
                edits
                    .iter()
                    .map(|edit| tag_edit_action(edit, "DELETE"))
                    .collect(),
            );
            return self.handle_edit_file(delegated, session_id).await;
        }

        let delegated = build_single_file_delegated_args(
            &args,
            vec![json!({
                "action": "DELETE",
                "line": args.get("line").cloned().unwrap_or(Value::Null),
                "endLine": args.get("endLine").cloned().unwrap_or(Value::Null),
                "anchor": args.get("anchor").cloned().unwrap_or(Value::Null),
                "endAnchor": args.get("endAnchor").cloned().unwrap_or(Value::Null),
            })],
        );

        self.handle_edit_file(delegated, session_id).await
    }

    pub async fn handle_edit_files(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let canonical_args = canonicalize_edit_files_args(&args);

        if let Err(validation_error) = validate_edit_files_arguments(&canonical_args) {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "editFiles arguments do not match the declared schema: {validation_error}"
                ),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Replace: [{\"path\": \"src/a.ts\", \"op\": \"replace\", \"startLine\": 10, \"startAnchor\": \"a31f2c\", \"content\": \"text\"}]".to_string(),
                "Insert top: [{\"path\": \"src/a.ts\", \"op\": \"insert_after\", \"startLine\": 0, \"content\": \"header\"}]".to_string(),
                "Delete range: [{\"path\": \"src/b.ts\", \"op\": \"delete\", \"startLine\": 10, \"endLine\": 15, \"startAnchor\": \"a31f2c\", \"endAnchor\": \"b47aa1\"}]".to_string(),
                "Use readFile(showLineAnchors=true) first to get anchor values".to_string(),
            ])
            .to_mcp_result());
        }

        let edits_array = match canonical_args
            .get("edits")
            .and_then(|value| value.as_array())
        {
            Some(edits) => edits,
            None => {
                return Ok(guided_error(
                    ErrorCategory::MissingRequiredParam,
                    "Parameter 'edits' is required and must be an array",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Replace: [{\"path\": \"src/a.ts\", \"op\": \"replace\", \"startLine\": 10, \"startAnchor\": \"a31f2c\", \"content\": \"text\"}]".to_string(),
                    "Insert-top: [{\"path\": \"src/a.ts\", \"op\": \"insert_after\", \"startLine\": 0, \"content\": \"header\"}]".to_string(),
                    "Delete range: [{\"path\": \"src/b.ts\", \"op\": \"delete\", \"startLine\": 10, \"endLine\": 15, \"startAnchor\": \"a31f2c\", \"endAnchor\": \"b47aa1\"}]".to_string(),
                    "Use readFile(showLineAnchors=true) to get anchor values first".to_string(),
                ])
                .to_mcp_result());
            }
        };

        if edits_array.is_empty() {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Parameter 'edits' cannot be empty",
                ToolGroup::Workspace,
            )
            .guidance(vec!["Provide at least one edit operation".to_string()])
            .to_mcp_result());
        }

        let mut parsed_edits = Vec::with_capacity(edits_array.len());
        for (idx, edit_value) in edits_array.iter().enumerate() {
            let edit_obj = match edit_value.as_object() {
                Some(obj) => obj,
                None => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!("Edit at index {} must be an object", idx),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Single-line: {\"path\": \"src/a.ts\", \"op\": \"replace\", \"startLine\": 10, \"startAnchor\": \"a31f2c\", \"content\": \"text\"}".to_string(),
                        "Range: {\"path\": \"src/a.ts\", \"op\": \"replace\", \"startLine\": 10, \"endLine\": 15, \"startAnchor\": \"a31f2c\", \"endAnchor\": \"b47aa1\", \"content\": \"...\"}".to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            let path = match edit_obj.get("path").and_then(|value| value.as_str()) {
                Some(path) if !path.trim().is_empty() => path.trim().to_string(),
                _ => {
                    return Ok(guided_error(
                        ErrorCategory::InvalidInput,
                        format!(
                            "Edit at index {}: 'path' field is required and must be a non-empty string",
                            idx
                        ),
                        ToolGroup::Workspace,
                    )
                    .guidance(vec![
                        "Each edit item must include its own path".to_string(),
                        "Example: {\"path\": \"src/main.rs\", ...}".to_string(),
                    ])
                    .to_mcp_result());
                }
            };

            let edit = match parse_line_edit(edit_obj, idx) {
                Ok(edit) => edit,
                Err(result) => return Ok(result),
            };

            parsed_edits.push(ParsedEdit { path, edit });
        }

        let mut grouped: BTreeMap<String, Vec<LineEdit>> = BTreeMap::new();
        for parsed in parsed_edits {
            grouped.entry(parsed.path).or_default().push(parsed.edit);
        }

        let mut prepared_batches = Vec::with_capacity(grouped.len());
        for (path, edits) in grouped {
            let prepared =
                match prepare_file_edit_batch(self, &path, edits, session_id.clone()).await {
                    Ok(batch) => batch,
                    Err(result) => return Ok(result),
                };
            prepared_batches.push(prepared);
        }

        if let Err(result) =
            write_prepared_batches(self, &prepared_batches, session_id.clone()).await
        {
            return Ok(result);
        }

        self.invalidate_context_cache().await;

        Ok(build_edit_files_success(&prepared_batches))
    }

    pub async fn handle_edit_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let canonical_args = canonicalize_legacy_edit_file_args_as_edit_files(&args);
        self.handle_edit_files(canonical_args, session_id).await
    }
}
