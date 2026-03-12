use crate::mcp::builtin::error_guidance::SuccessHint;
use crate::mcp::builtin::workspace::terminal_manager;
use crate::mcp::builtin::workspace::WorkspaceServer;
use crate::mcp::types::MCPResult;
use serde_json::Value;

impl WorkspaceServer {
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

            let mut lines = vec![format!(
                "- Use waitForProcess('{}', 0) to check status",
                first_id
            )];

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
}
