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
                "running" => terminal_manager::is_active_process_status(&e.status),
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
                    "name": e.name,
                    "command": e.command,
                    "status": terminal_manager::process_status_label(&e.status),
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
            .filter(|e| terminal_manager::is_active_process_status(&e.status))
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
                    let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
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
                    if name.is_empty() {
                        format!(
                            "• {} [{}]{}{}\n  Command: {}",
                            id, status, pid, exit_code, command
                        )
                    } else {
                        format!(
                            "• {} [{}]{}{}\n  Name: {}\n  Command: {}",
                            id, status, pid, exit_code, name, command
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        };

        // Build context-aware next actions based on process statuses
        let next_actions = if total > 0 {
            let first_process = processes.first();
            let first_id = first_process
                .and_then(|p| p.get("process_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("processId");
            let first_status = first_process
                .and_then(|p| p.get("status"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mut actions = Vec::new();

            match first_status {
                "failed" => {
                    actions.push(format!(
                        "Use readProcessOutput('{}', 'both') to inspect stdout and stderr",
                        first_id
                    ));
                    actions.push(
                        "Use listProcesses() again if you need another processId from this session"
                            .to_string(),
                    );
                }
                "finished" => {
                    actions.push(format!(
                        "Use readProcessOutput('{}', 'both') to inspect stdout and stderr",
                        first_id
                    ));
                    actions.push(
                        "Use listProcesses() again if you need another processId from this session"
                            .to_string(),
                    );
                }
                "running" => {
                    actions.push(format!(
                        "Use waitForProcess('{}', 0) to check status",
                        first_id
                    ));
                    actions.push(format!(
                        "Use readProcessOutput('{}', 'both') to inspect stdout and stderr",
                        first_id
                    ));
                    actions.push(format!(
                        "Use stopProcess('{}') to terminate running process",
                        first_id
                    ));
                }
                _ => {
                    actions.push(format!(
                        "Use waitForProcess('{}', 0) to check status",
                        first_id
                    ));
                    actions.push(format!(
                        "Use readProcessOutput('{}', 'both') to inspect stdout and stderr",
                        first_id
                    ));
                }
            }

            actions
        } else {
            Vec::new()
        };

        let summary = format!(
            "Found {} processes ({} running, {} finished)

{}",
            total, running, finished, process_list
        );

        let hint = SuccessHint::new(summary, next_actions);

        Ok(hint.to_mcp_result_with_data(Some(response)))
    }
}
