use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::workspace::terminal_manager;
use crate::mcp::builtin::workspace::WorkspaceServer;
use crate::mcp::types::MCPResult;
use serde_json::json;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Clone, Copy)]
enum OutputSelection {
    Stdout,
    Stderr,
    Both,
}

impl OutputSelection {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "stdout" => Some(Self::Stdout),
            "stderr" => Some(Self::Stderr),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
            Self::Both => "both",
        }
    }

    fn stream_names(self) -> &'static [&'static str] {
        match self {
            Self::Stdout => &["stdout"],
            Self::Stderr => &["stderr"],
            Self::Both => &["stdout", "stderr"],
        }
    }
}

fn parse_offset(args: &Value) -> Result<Option<usize>, MCPResult> {
    let Some(value) = args.get("offset") else {
        return Ok(None);
    };

    match value {
        Value::Null => Ok(None),
        Value::Number(number) => match number.as_u64() {
            Some(offset) => usize::try_from(offset).map(Some).map_err(|_| {
                guided_error(
                    ErrorCategory::InvalidInput,
                    "Invalid offset: value is too large",
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Use a non-negative line offset that fits the platform".to_string(),
                ])
                .to_mcp_result()
            }),
            None => Err(guided_error(
                ErrorCategory::InvalidInput,
                "Invalid offset: expected a non-negative integer",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use offset=0 for the first page".to_string(),
                "Use the returned next_offset for the following page".to_string(),
            ])
            .to_mcp_result()),
        },
        _ => Err(guided_error(
            ErrorCategory::InvalidInput,
            "Invalid offset: expected a non-negative integer",
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Use offset=0 for the first page".to_string(),
            "Use the returned next_offset for the following page".to_string(),
        ])
        .to_mcp_result()),
    }
}

fn status_label(status: &terminal_manager::ProcessStatus) -> String {
    terminal_manager::process_status_label(status)
}

fn stream_file_path(entry: &terminal_manager::ProcessEntry, stream: &str) -> PathBuf {
    match stream {
        "stdout" => PathBuf::from(&entry.stdout_path),
        "stderr" => PathBuf::from(&entry.stderr_path),
        _ => unreachable!("validated stream name"),
    }
}

async fn read_stream_output(
    file_path: &PathBuf,
    mode: &str,
    lines: usize,
) -> Result<Vec<String>, String> {
    match mode {
        "head" => terminal_manager::head_lines(file_path, lines).await,
        "tail" => terminal_manager::tail_lines(file_path, lines).await,
        _ => Err(format!("Unsupported mode '{mode}'")),
    }
}

fn stream_type_for_name(stream: &str) -> terminal_manager::StreamType {
    match stream {
        "stderr" => terminal_manager::StreamType::Stderr,
        _ => terminal_manager::StreamType::Stdout,
    }
}

async fn read_stream_prefer_live(
    streaming: Option<&std::sync::Arc<terminal_manager::StreamingHandle>>,
    file_path: &PathBuf,
    stream_name: &str,
    mode: &str,
    lines: usize,
) -> Result<Vec<String>, String> {
    if let Some(handle) = streaming {
        let stream_type = stream_type_for_name(stream_name);
        let from_memory = match mode {
            "head" => handle.get_head(stream_type, lines).await,
            "tail" => handle.get_tail(stream_type, lines).await,
            _ => {
                return Err(format!("Unsupported mode '{mode}'"));
            }
        };
        if !from_memory.is_empty() {
            return Ok(from_memory);
        }
    }

    // Fall back to on-disk files (sync timeout-handoff path, or empty start).
    // Missing files while still starting should read as empty, not hard-fail.
    match read_stream_output(file_path, mode, lines).await {
        Ok(content) => Ok(content),
        Err(error) => {
            let lower = error.to_lowercase();
            if lower.contains("not found") || lower.contains("no such file") {
                Ok(Vec::new())
            } else {
                Err(error)
            }
        }
    }
}

fn format_stream_section(stream: &str, lines: &[String]) -> String {
    let body = if lines.is_empty() {
        "(no output)".to_string()
    } else {
        lines.join("\n")
    };

    format!("[{}]\n{}", stream.to_uppercase(), body)
}

fn build_read_error(process_id: &str, stream_label: &str, error: &str) -> MCPResult {
    let error_lower = error.to_lowercase();

    let (error_title, guidance) =
        if error_lower.contains("not found") || error_lower.contains("no such file") {
            (
                format!("No {stream_label} output file found"),
                vec![
                    "The process may not have started yet".to_string(),
                    format!(
                    "Use workspace__waitForProcess(\"{process_id}\", 0) to verify process status"
                ),
                    "Wait a moment and try again - the process may not have generated output"
                        .to_string(),
                    "Check if the process ran with output redirected elsewhere".to_string(),
                ],
            )
        } else if error_lower.contains("permission") || error_lower.contains("denied") {
            (
                "Permission denied reading output".to_string(),
                vec![
                    format!("Cannot read {stream_label} stream for process \"{process_id}\""),
                    "Check process permissions and ownership".to_string(),
                    "Try running as elevated user if needed".to_string(),
                    "Use workspace__listProcesses to view process details".to_string(),
                ],
            )
        } else if error_lower.contains("too large") || error_lower.contains("too big") {
            (
                "Output file too large".to_string(),
                vec![
                    "Maximum 100 lines per request".to_string(),
                    "Reduce 'lines' parameter to read less data".to_string(),
                    "Use mode=\"head\" for beginning or mode=\"tail\" for end".to_string(),
                    "Use output_paths with workspace__readFile/search for deeper inspection"
                        .to_string(),
                ],
            )
        } else if error_lower.contains("invalid") || error_lower.contains("utf") {
            (
                "Output contains non-UTF-8 data".to_string(),
                vec![
                    "The process output contains binary or invalid UTF-8 data".to_string(),
                    "Try reading the other stream if it contains text diagnostics".to_string(),
                    "Check if the process generated text output or binary data".to_string(),
                ],
            )
        } else {
            (
                "Failed to read process output".to_string(),
                vec![
                    format!("Verify process {process_id} exists: use workspace__listProcesses()"),
                    format!("Check stream=\"{stream_label}\" is correct (stdout, stderr, or both)"),
                    "Ensure the process has generated output".to_string(),
                    "Check file permissions and disk space".to_string(),
                ],
            )
        };

    guided_error(
        ErrorCategory::OperationFailed,
        error_title,
        ToolGroup::Workspace,
    )
    .guidance(guidance)
    .to_mcp_result()
}

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
        let selection = match OutputSelection::parse(stream) {
            Some(selection) => selection,
            None => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid stream '{stream}'"),
                    ToolGroup::Workspace,
                )
                .guidance(vec![
                    "Use stream=\"stdout\", stream=\"stderr\", or stream=\"both\"".to_string(),
                    "Use stream=\"both\" when you need both streams in one response".to_string(),
                ])
                .to_mcp_result());
            }
        };

        let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("tail");
        if !matches!(mode, "tail" | "head") {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("Invalid mode '{mode}'"),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use mode=\"tail\" to read the latest lines".to_string(),
                "Use mode=\"head\" to read the earliest lines".to_string(),
            ])
            .to_mcp_result());
        }
        let lines = args
            .get("lines")
            .and_then(|v| v.as_u64())
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(20)
            .min(100);
        if lines == 0 {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "lines must be greater than zero",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use lines between 1 and 100".to_string(),
                "Use the returned next_offset to continue pagination".to_string(),
            ])
            .to_mcp_result());
        }
        let offset = match parse_offset(&args) {
            Ok(offset) => offset,
            Err(error) => return Ok(error),
        };
        if offset.is_some() && mode != "head" {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "offset can only be used with mode=\"head\"",
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use mode=\"head\" with offset=0 for the first page".to_string(),
                "Omit offset to preserve the default tail behavior".to_string(),
            ])
            .to_mcp_result());
        }
        // Get process entry + live streaming handle (if registered)
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
                    format!("Process '{}' is not registered in this session", process_id),
                    ToolGroup::Workspace,
                )
                .guidance(super::not_found::process_not_found_guidance(available_text))
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
                "Use workspace__listProcesses() to see processes in your session".to_string(),
            ])
            .to_mcp_result());
        }

        let streaming = registry.streaming_handles.get(process_id).cloned();
        drop(registry);

        let status = status_label(&entry.status);
        let is_process_running = matches!(
            entry.status,
            terminal_manager::ProcessStatus::Starting | terminal_manager::ProcessStatus::Running
        );

        let mut stream_sections = Vec::new();
        let mut outputs = serde_json::Map::new();
        let mut output_paths = serde_json::Map::new();
        let mut output_path_lines = Vec::new();
        let mut next_offset = None;

        for stream_name in selection.stream_names() {
            let file_path = stream_file_path(&entry, stream_name);
            let (content, total_lines, truncated, stream_next_offset) = if let Some(offset) = offset
            {
                let page = match terminal_manager::read_lines_page(&file_path, offset, lines).await
                {
                    Ok(page) => page,
                    Err(error) => {
                        return Ok(build_read_error(process_id, stream_name, &error));
                    }
                };
                let next = if offset.saturating_add(lines) < page.total_lines {
                    Some(offset.saturating_add(lines))
                } else {
                    None
                };
                (page.lines, Some(page.total_lines), page.truncated, next)
            } else {
                let content = match read_stream_prefer_live(
                    streaming.as_ref(),
                    &file_path,
                    stream_name,
                    mode,
                    lines,
                )
                .await
                {
                    Ok(content) => content,
                    Err(error) => {
                        return Ok(build_read_error(process_id, stream_name, &error));
                    }
                };
                (content, None, false, None)
            };
            let lines_returned = content.len();
            let file_path_string = file_path.to_string_lossy().to_string();

            if let Some(stream_next_offset) = stream_next_offset {
                next_offset = Some(next_offset.unwrap_or(0).max(stream_next_offset));
            }

            stream_sections.push(format_stream_section(stream_name, &content));
            output_paths.insert((*stream_name).to_string(), json!(file_path_string));
            output_path_lines.push(format!("- {}: {}", stream_name, file_path_string));
            let mut output = serde_json::Map::from_iter([
                ("content".to_string(), json!(content)),
                ("lines_returned".to_string(), json!(lines_returned)),
                (
                    "total_size_bytes".to_string(),
                    json!(terminal_manager::get_file_size(&file_path).await),
                ),
            ]);
            if let Some(total_lines) = total_lines {
                output.insert("total_lines".to_string(), json!(total_lines));
                output.insert("truncated".to_string(), json!(truncated));
            }
            outputs.insert((*stream_name).to_string(), Value::Object(output));
        }

        let mut text = format!(
            "Read output from process {} (stream: {}, mode: {}, status: {})\n\nInternal output files (absolute paths, not workspace-relative):\n{}\n\n{}",
            process_id,
            selection.as_str(),
            mode,
            status,
            output_path_lines.join("\n"),
            stream_sections.join("\n\n")
        );
        if let Some(offset) = offset {
            let pagination_note = match next_offset {
                Some(next_offset) => format!(
                    "\n\nPagination: page starts at offset {offset}. Read the next page with offset={next_offset}."
                ),
                None if is_process_running => format!(
                    "\n\nPagination: no more output is available now at offset {offset}. The process is still running; retry with offset={offset} after it produces more output."
                ),
                None => "\n\nPagination: this is the final page.".to_string(),
            };
            text.push_str(&pagination_note);
        }

        let mut response = serde_json::Map::from_iter([
            ("process_id".to_string(), json!(process_id)),
            ("stream".to_string(), json!(selection.as_str())),
            ("mode".to_string(), json!(mode)),
            ("status".to_string(), json!(status)),
            ("is_process_running".to_string(), json!(is_process_running)),
            ("output_paths".to_string(), Value::Object(output_paths)),
            ("outputs".to_string(), Value::Object(outputs)),
            ("lines_requested".to_string(), json!(lines.min(100))),
            (
                "note".to_string(),
                json!(
                    "Text output only. Max 100 lines per request. output_paths are absolute internal diagnostics paths; do not pass them to workspace__readFile or workspace__listDirectory."
                ),
            ),
        ]);
        if let Some(offset) = offset {
            response.insert("offset".to_string(), json!(offset));
            response.insert("next_offset".to_string(), json!(next_offset));
        }

        // Finished reads already include the captured output — no patronizing follow-ups.
        let next_actions = if is_process_running {
            vec![
                format!(
                    "Use workspace__waitForProcess('{}', 0) to check whether it has finished",
                    process_id
                ),
                format!(
                    "Use workspace__stopProcess('{}') only if the running process is stuck",
                    process_id
                ),
            ]
        } else {
            vec![]
        };

        let hint = SuccessHint::new(text, next_actions);

        Ok(hint.to_mcp_result_with_data(Some(Value::Object(response))))
    }
}
