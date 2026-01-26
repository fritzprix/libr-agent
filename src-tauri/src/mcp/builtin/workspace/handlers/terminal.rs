use super::super::terminal_manager;
use super::super::WorkspaceServer;
use crate::mcp::builtin::error_guidance::{
    missing_param_error, not_found_error, operation_failed_error, ErrorGuidance, SuccessHint,
    ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::Value;

/// Terminal/Process management handlers
/// Extracted from mod.rs for better code organization
impl WorkspaceServer {
    /// Handle poll_process tool call
    pub async fn handle_poll_process(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        // Parse processId
        let process_id = match args.get("processId").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Ok(missing_param_error("processId", ToolGroup::Workspace));
            }
        };

        // Verify session access BEFORE write lock (optimization)
        {
            let registry = self.process_registry.read().await;
            match registry.entries.get(process_id) {
                Some(entry) if entry.session_id == session_id => {
                    // Access granted, continue
                }
                _ => {
                    return Ok(not_found_error("Process", process_id, ToolGroup::Workspace));
                }
            }
        }

        // Update poll tracking and get entry + streaming handle (write lock)
        let threshold = crate::config::poll_threshold();
        let (should_show_guidance, entry_for_response, streaming_handle) = {
            let mut registry = self.process_registry.write().await;
            let entry_clone = if let Some(entry) = registry.entries.get_mut(process_id) {
                let now = chrono::Utc::now();

                // Update poll metadata
                entry.last_poll_at = Some(now);
                entry.poll_count += 1;

                // Track consecutive running polls
                let is_running = matches!(entry.status, terminal_manager::ProcessStatus::Running);
                if is_running {
                    if entry.first_running_poll_at.is_none() {
                        entry.first_running_poll_at = Some(now);
                    }
                    entry.consecutive_running_polls += 1;
                } else {
                    // Reset counters when status changes from running
                    entry.consecutive_running_polls = 0;
                    entry.first_running_poll_at = None;
                }

                let should_guide = is_running && entry.consecutive_running_polls >= threshold;
                (should_guide, entry.clone())
            } else {
                return Ok(not_found_error("Process", process_id, ToolGroup::Workspace));
            };

            // Get streaming handle after releasing entry borrow
            let handle = registry.streaming_handles.get(process_id).cloned();
            (entry_clone.0, entry_clone.1, handle)
        };

        // Build response
        let mut response = serde_json::json!({
            "process_id": entry_for_response.id,
            "status": format!("{:?}", entry_for_response.status).to_lowercase(),
            "command": entry_for_response.command,
            "pid": entry_for_response.pid,
            "exit_code": entry_for_response.exit_code,
            "started_at": entry_for_response.started_at.to_rfc3339(),
            "finished_at": entry_for_response.finished_at.map(|t| t.to_rfc3339()),
            "stdout_size": entry_for_response.stdout_size,
            "stderr_size": entry_for_response.stderr_size,
            "streaming_available": streaming_handle.is_some(),
        });

        // Optional tail - check in-memory buffer first, fallback to file
        let mut tail_output_display = String::new();

        if let Some(tail_obj) = args.get("tail").and_then(|v| v.as_object()) {
            let src = tail_obj
                .get("src")
                .and_then(|v| v.as_str())
                .unwrap_or("stdout");
            let n = tail_obj.get("n").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

            let (lines, source) = if let Some(handle) = &streaming_handle {
                // Fast path: get from in-memory buffer
                let stream_type = if src == "stdout" {
                    terminal_manager::StreamType::Stdout
                } else {
                    terminal_manager::StreamType::Stderr
                };
                (handle.get_tail(stream_type, n).await, "buffer")
            } else {
                // Fallback: read from file (process finished or old entry)
                let file_path = if src == "stdout" {
                    std::path::PathBuf::from(&entry_for_response.stdout_path)
                } else {
                    std::path::PathBuf::from(&entry_for_response.stderr_path)
                };
                match terminal_manager::tail_lines(&file_path, n).await {
                    Ok(lines) => (lines, "file"),
                    Err(e) => {
                        tracing::warn!("Failed to read tail from file: {}", e);
                        (Vec::new(), "error")
                    }
                }
            };

            if !lines.is_empty() {
                tail_output_display = format!(
                    "\n\n--- Output (last {} lines) ---\n{}",
                    lines.len(),
                    lines.join("\n")
                );
            }

            response["tail"] = serde_json::json!({
                "src": src,
                "lines": lines,
                "source": source,
            });
        }

        // Add success hint based on process status
        let status_str = format!("{:?}", entry_for_response.status).to_lowercase();

        // ✅ FIXED: Include ALL critical details in text content for AI visibility
        let status_details = format!(
            "Process Status for {}:

- Process ID: {}
- Status: {}
- Command: {}
- PID: {}
- Exit Code: {}
- Started: {}
- Finished: {}
{}",
            process_id,
            entry_for_response.id,
            status_str,
            entry_for_response.command,
            entry_for_response
                .pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            entry_for_response
                .exit_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            entry_for_response.started_at.format("%Y-%m-%d %H:%M:%S"),
            entry_for_response
                .finished_at
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Still running".to_string()),
            if tail_output_display.is_empty() {
                String::new()
            } else {
                format!("\n{}", tail_output_display)
            }
        );

        let hint = SuccessHint::new(
            status_details,
            match entry_for_response.status {
                terminal_manager::ProcessStatus::Running => vec![
                    "Wait for process to complete before polling again".to_string(),
                    format!(
                        "Use readProcessOutput('{}', 'stdout') to view full output",
                        process_id
                    ),
                ],
                terminal_manager::ProcessStatus::Failed => vec![
                    format!(
                        "Use readProcessOutput('{}', 'stderr') to view error details",
                        process_id
                    ),
                    "Process has completed - no need to poll again".to_string(),
                ],
                terminal_manager::ProcessStatus::Finished => vec![
                    format!(
                        "Use readProcessOutput('{}', 'stdout') to view full output",
                        process_id
                    ),
                    "Process has completed - no need to poll again".to_string(),
                ],
                _ => vec!["Use listProcesses to see all processes".to_string()],
            },
        );

        // Add warning if excessive polling detected
        if should_show_guidance {
            let warning = ErrorGuidance::with_guidance(
                crate::mcp::builtin::error_guidance::ErrorCategory::InvalidState,
                format!(
                    "Excessive polling detected ({}/{} consecutive polls exceeds threshold)",
                    entry_for_response.consecutive_running_polls, threshold
                ),
                vec![
                    "Wait at least 10 seconds before next poll".to_string(),
                    "Process will continue running in background".to_string(),
                    "Status updates automatically when complete".to_string(),
                ],
                ToolGroup::Workspace,
            );
            Ok(warning.to_mcp_result())
        } else {
            Ok(hint.to_mcp_result_with_data(Some(response)))
        }
    }

    /// Handle read_process_output tool call
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

        // Get process entry
        let registry = self.process_registry.read().await;
        let entry = match registry.entries.get(process_id) {
            Some(e) => e.clone(),
            None => {
                return Ok(not_found_error("Process", process_id, ToolGroup::Workspace));
            }
        };

        // Verify session access
        if entry.session_id != session_id {
            return Ok(not_found_error("Process", process_id, ToolGroup::Workspace));
        }
        drop(registry);

        // Get file path
        let file_path = if stream == "stdout" {
            std::path::PathBuf::from(&entry.stdout_path)
        } else {
            std::path::PathBuf::from(&entry.stderr_path)
        };

        // Read lines based on mode
        let content = match mode {
            "head" => terminal_manager::head_lines(&file_path, lines).await,
            _ => terminal_manager::tail_lines(&file_path, lines).await,
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
                        "Use pollProcess(processId) to check running status".to_string(),
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
                            format!("Use pollProcess(\"{}\") to verify process status", process_id),
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

                Ok(operation_failed_error(
                    &error_title,
                    &e,
                    guidance,
                    ToolGroup::Workspace,
                ))
            }
        }
    }

    /// Handle list_processes tool call
    pub async fn handle_list_processes(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let status_filter = args
            .get("statusFilter")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        // Filter processes by session
        let registry = self.process_registry.read().await;
        let mut processes: Vec<Value> = registry
            .entries
            .values()
            .filter(|e| e.session_id == session_id)
            .filter(|e| match status_filter {
                "running" => matches!(e.status, terminal_manager::ProcessStatus::Running),
                "finished" => matches!(
                    e.status,
                    terminal_manager::ProcessStatus::Finished
                        | terminal_manager::ProcessStatus::Failed
                ),
                _ => true,
            })
            .map(|e| {
                serde_json::json!({
                    "process_id": e.id,
                    "command": e.command,
                    "status": format!("{:?}", e.status).to_lowercase(),
                    "pid": e.pid,
                    "started_at": e.started_at.to_rfc3339(),
                    "exit_code": e.exit_code,
                })
            })
            .collect();

        processes.sort_by(|a, b| {
            let a_time = a.get("started_at").and_then(|v| v.as_str()).unwrap_or("");
            let b_time = b.get("started_at").and_then(|v| v.as_str()).unwrap_or("");
            b_time.cmp(a_time) // descending order
        });

        let total = processes.len();
        let running = registry
            .entries
            .values()
            .filter(|e| e.session_id == session_id)
            .filter(|e| matches!(e.status, terminal_manager::ProcessStatus::Running))
            .count();
        let finished = registry
            .entries
            .values()
            .filter(|e| e.session_id == session_id)
            .filter(|e| {
                matches!(
                    e.status,
                    terminal_manager::ProcessStatus::Finished
                        | terminal_manager::ProcessStatus::Failed
                )
            })
            .count();

        drop(registry);

        let response = serde_json::json!({
            "processes": processes,
            "total": total,
            "running": running,
            "finished": finished,
        });

        // ✅ FIXED: Build detailed text output with FULL process details for AI visibility
        let process_list = if processes.is_empty() {
            "No processes found in current session".to_string()
        } else {
            processes
                .iter()
                .map(|p| {
                    let id = p
                        .get("process_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let status = p
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let command = p.get("command").and_then(|v| v.as_str()).unwrap_or("");
                    let pid = p
                        .get("pid")
                        .and_then(|v| v.as_u64())
                        .map(|p| format!(" (PID: {})", p))
                        .unwrap_or_default();
                    let exit_code = p
                        .get("exit_code")
                        .and_then(|v| v.as_i64())
                        .map(|c| format!(" [exit: {}]", c))
                        .unwrap_or_default();

                    // Full command visible to agent (no truncation)
                    format!(
                        "• {} [{}]{}{}\n  Command: {}",
                        id, status, pid, exit_code, command
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        };

        // Build context-aware guidance based on process statuses
        let guidance_lines = if total > 0 {
            let first_process = processes.first();
            let first_id = first_process
                .and_then(|p| p.get("process_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("processId");
            let first_status = first_process
                .and_then(|p| p.get("status"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mut lines = vec![format!("- Use pollProcess('{}') to check status", first_id)];

            // Add appropriate readProcessOutput guidance based on status
            match first_status {
                "failed" => {
                    lines.push(format!(
                        "- Use readProcessOutput('{}', 'stderr') to view error details",
                        first_id
                    ));
                }
                "finished" => {
                    lines.push(format!(
                        "- Use readProcessOutput('{}', 'stdout') to view output",
                        first_id
                    ));
                }
                "running" => {
                    lines.push(format!(
                        "- Use readProcessOutput('{}', 'stdout') to view output",
                        first_id
                    ));
                    lines.push(format!(
                        "- Use stopProcess('{}') to terminate running process",
                        first_id
                    ));
                }
                _ => {
                    lines.push(format!(
                        "- Use readProcessOutput('{}', 'stdout') to view output",
                        first_id
                    ));
                }
            }

            lines.join("\n")
        } else {
            "- No processes to manage".to_string()
        };

        let summary = format!(
            "Found {} processes ({} running, {} finished)

{}

💡 Next Steps:
{}",
            total, running, finished, process_list, guidance_lines
        );

        let hint = SuccessHint::new(summary, vec![]); // Guidance is in summary

        Ok(hint.to_mcp_result_with_data(Some(response)))
    }

    /// Handle stop_process tool call
    pub async fn handle_stop_process(
        &self,
        args: Value,
        session_id: &str,
    ) -> Result<MCPResult, String> {
        let process_id = match args.get("processId").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Ok(missing_param_error("processId", ToolGroup::Workspace));
            }
        };

        let mut registry = self.process_registry.write().await;

        // Check if process exists and belongs to session
        if let Some(entry) = registry.entries.get(process_id) {
            if entry.session_id != session_id {
                return Ok(not_found_error("Process", process_id, ToolGroup::Workspace));
            }
        } else {
            return Ok(not_found_error("Process", process_id, ToolGroup::Workspace));
        }

        // Cancel process via token
        if let Some(token) = registry.cancellation_tokens.get(process_id) {
            token.cancel();
        }

        // Update status and kill process
        if let Some(entry) = registry.entries.get_mut(process_id) {
            // Check if process is already terminated
            if matches!(
                entry.status,
                terminal_manager::ProcessStatus::Finished
                    | terminal_manager::ProcessStatus::Failed
                    | terminal_manager::ProcessStatus::Killed
            ) {
                return Ok(operation_failed_error(
                    "Stop process",
                    &format!(
                        "Process {} has already terminated with status: {:?}",
                        process_id, entry.status
                    ),
                    vec![
                        "Use listProcesses to see running processes".to_string(),
                        "Only running processes can be stopped".to_string(),
                    ],
                    ToolGroup::Workspace,
                ));
            }

            // Kill process if running
            if let Some(pid) = entry.pid {
                if matches!(
                    entry.status,
                    terminal_manager::ProcessStatus::Running
                        | terminal_manager::ProcessStatus::Starting
                ) {
                    tracing::info!("Force-killing process {} (PID {})", process_id, pid);

                    #[cfg(unix)]
                    {
                        use std::process::Command;
                        let _ = Command::new("kill")
                            .arg("-TERM")
                            .arg(pid.to_string())
                            .output();
                    }

                    #[cfg(windows)]
                    {
                        use std::process::Command;
                        let _ = Command::new("taskkill")
                            .args(["/PID", &pid.to_string(), "/F"])
                            .output();
                    }
                }
            }

            entry.status = terminal_manager::ProcessStatus::Killed;
            entry.finished_at = Some(chrono::Utc::now());
        }

        // Remove cancellation token
        registry.cancellation_tokens.remove(process_id);

        // Invalidate service context cache
        self.invalidate_context_cache().await;

        let hint = SuccessHint::new(
            format!("Process {} stopped successfully", process_id),
            vec![
                "Use listProcesses to see remaining processes".to_string(),
                "Use readProcessOutput to view output before termination".to_string(),
            ],
        );

        let response = serde_json::json!({
            "process_id": process_id,
            "stopped": true
        });

        Ok(hint.to_mcp_result_with_data(Some(response)))
    }
}
