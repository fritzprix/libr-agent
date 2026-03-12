use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::workspace::terminal_manager;
use crate::mcp::builtin::workspace::WorkspaceServer;
use crate::mcp::types::MCPResult;
use serde_json::Value;

impl WorkspaceServer {
    pub async fn handle_read_process_output(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        // Parse parameters
        let process_id = match args.get("processId").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Ok(missing_param_error("processId", ToolGroup::Workspace));
            }
        };

        let stream = match args.get("stream").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return Ok(missing_param_error("stream", ToolGroup::Workspace));
            }
        };

        let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("tail");

        let lines = args.get("lines").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

        let start_line = args
            .get("start_line")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let end_line = args
            .get("end_line")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        // Get process entry
        let registry = self.process_registry.read().await;
        let entry = match registry.entries.get(process_id) {
            Some(e) => e.clone(),
            None => {
                // ✅ ENHANCED: Process-specific error with available process IDs
                let available: Vec<String> = registry
                    .entries
                    .values()
                    .filter(|e| e.session_id == session_id)
                    .take(5)
                    .map(|e| format!("{} [{}]", e.id, e.command))
                    .collect();

                let available_text = if available.is_empty() {
                    "No processes found in this session".to_string()
                } else {
                    format!("Available processes: {}", available.join(", "))
                };

                return Ok(guided_error(
                    ErrorCategory::ResourceNotFound,
                    format!("Process '{}' not found", process_id),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    available_text,
                    "Use listProcesses() to see all processes with IDs".to_string(),
                    "Check if process has finished - finished processes are kept for 24 hours"
                        .to_string(),
                ])
                .to_mcp_result());
            }
        };

        // Verify session access
        if entry.session_id != session_id {
            // ✅ ENHANCED: Better error message for session mismatch
            return Ok(guided_error(
                ErrorCategory::PermissionDenied,
                format!("Process '{}' not found in current session", process_id),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Process may belong to a different session".to_string(),
                "Use listProcesses() to see processes in your session".to_string(),
            ])
            .to_mcp_result());
        }
        drop(registry);

        // Get file path
        let file_path = if stream == "stdout" {
            std::path::PathBuf::from(&entry.stdout_path)
        } else {
            std::path::PathBuf::from(&entry.stderr_path)
        };

        // Read lines based on mode or range
        let content = if let (Some(start), Some(end)) = (start_line, end_line) {
            terminal_manager::read_lines_range(&file_path, start, end).await
        } else {
            match mode {
                "head" => terminal_manager::head_lines(&file_path, lines).await,
                _ => terminal_manager::tail_lines(&file_path, lines).await,
            }
        };

        match content {
            Ok(lines_vec) => {
                let content_display = lines_vec.join("\n");
                let response = serde_json::json!({
                    "process_id": process_id,
                    "stream": stream,
                    "mode": mode,
                    "lines_requested": lines.min(100),
                    "lines_returned": lines_vec.len(),
                    "content": lines_vec,
                    "total_size": terminal_manager::get_file_size(&file_path).await,
                    "note": "Text output only. Max 100 lines per request.",
                });

                let hint = SuccessHint::new(
                    format!(
                        "Read {} lines from {} {}:\n\n{}",
                        lines_vec.len(),
                        stream,
                        mode,
                        content_display
                    ),
                    vec![
                        "Use waitForProcess(processId, 0) to check running status".to_string(),
                        format!(
                            "Try mode=\"{}\" to read the {} of output instead",
                            if mode == "head" { "tail" } else { "head" },
                            if mode == "head" { "end" } else { "beginning" }
                        ),
                        "Increase lines parameter to get more output (max 100)".to_string(),
                    ],
                );

                Ok(hint.to_mcp_result_with_data(Some(response)))
            }
            Err(e) => {
                // ✅ ENHANCED: Context-specific error guidance based on error type
                let error_lower = e.to_lowercase();

                let (error_title, guidance) = if error_lower.contains("not found")
                    || error_lower.contains("no such file")
                {
                    // Process hasn't generated output yet
                    (
                        format!("No {} output file found", stream),
                        vec![
                            "The process may not have started yet".to_string(),
                            format!("Use waitForProcess(\"{}\", 0) to verify process status", process_id),
                            "Wait a moment and try again - the process may not have generated output".to_string(),
                            "Check if the process ran with output redirected elsewhere".to_string(),
                        ],
                    )
                } else if error_lower.contains("permission") || error_lower.contains("denied") {
                    // Permission denied accessing output file
                    (
                        "Permission denied reading output".to_string(),
                        vec![
                            format!(
                                "Cannot read {} stream for process \"{}\"",
                                stream, process_id
                            ),
                            "Check process permissions and ownership".to_string(),
                            "Try running as elevated user if needed".to_string(),
                            "Use listProcesses to view process details".to_string(),
                        ],
                    )
                } else if error_lower.contains("too large") || error_lower.contains("too big") {
                    // File is too large to read entirely
                    (
                        "Output file too large".to_string(),
                        vec![
                            "Maximum 100 lines per request".to_string(),
                            "Reduce 'lines' parameter to read less data".to_string(),
                            "Use mode=\"head\" for beginning or mode=\"tail\" for end".to_string(),
                            "Consider grep or other text processing tools for filtering"
                                .to_string(),
                        ],
                    )
                } else if error_lower.contains("invalid") || error_lower.contains("utf") {
                    // Output contains invalid UTF-8
                    (
                        "Output contains non-UTF-8 data".to_string(),
                        vec![
                            "The process output contains binary or invalid UTF-8 data".to_string(),
                            "Try reading stderr instead if it contains error messages".to_string(),
                            "Check if the process generated text output or binary data".to_string(),
                        ],
                    )
                } else {
                    // Generic error
                    (
                        "Failed to read process output".to_string(),
                        vec![
                            format!("Verify process {} exists: use listProcesses()", process_id),
                            format!("Check stream=\"{}\" is correct (stdout or stderr)", stream),
                            "Ensure the process has generated output".to_string(),
                            "Check file permissions and disk space".to_string(),
                        ],
                    )
                };

                Ok(guided_error(
                    ErrorCategory::InvalidState,
                    error_title,
                    ToolGroup::Workspace,
                )
                .guidance(guidance)
                .to_mcp_result())
            }
        }
    }
}
