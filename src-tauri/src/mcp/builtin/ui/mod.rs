use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{MCPContent, MCPResult, ServiceContext, ServiceInfo};
use crate::mcp::MCPTool;
use async_trait::async_trait;
use handlebars::Handlebars;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub mod ask_user;
pub mod tools;

// Embed templates
const CIRCUIT_BREAK_TEMPLATE: &str = include_str!("templates/circuit-break.hbs");

#[derive(Debug)]
pub struct UiServer {
    handlebars: Arc<Mutex<Handlebars<'static>>>,
}

impl Default for UiServer {
    fn default() -> Self {
        Self::new()
    }
}

impl UiServer {
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();

        // Register templates
        handlebars
            .register_template_string("circuit-break", CIRCUIT_BREAK_TEMPLATE)
            .unwrap();

        Self {
            handlebars: Arc::new(Mutex::new(handlebars)),
        }
    }

    fn get_user_answer(&self, args: Value) -> Result<MCPResult, String> {
        let _message_id = match args.get("messageId").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("messageId", ToolGroup::UI)),
        };

        let answer = args.get("answer");
        let cancelled = args
            .get("cancelled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if cancelled {
            return Ok(MCPResult::informational("User cancelled the prompt."));
        }

        let answer_str = if let Some(ans) = answer {
            if ans.is_null() {
                "null".to_string()
            } else {
                serde_json::to_string(ans).unwrap_or_else(|_| "invalid".to_string())
            }
        } else {
            "null".to_string()
        };

        Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text: format!("User replied: {}", answer_str),
            }]),
            structured_content: None,
            is_error: Some(false),
        })
    }

    fn circuit_break(&self, args: Value) -> Result<MCPResult, String> {
        let tool_name = match args.get("toolName").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("toolName", ToolGroup::UI)),
        };
        let repetition_count = args
            .get("repetitionCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let args_str = args.get("args").and_then(|v| v.as_str()).unwrap_or("");

        // Handlebars {{toolName}}/{{args}} HTML-escape once. Triple-stash {{{contextJson}}}
        // is raw — neutralize </script> breakouts from LLM-controlled strings.
        let context_json = json!({
            "toolName": tool_name,
            "repetitionCount": repetition_count,
            "args": args_str,
        });
        let context_json_for_script = context_json
            .to_string()
            .replace('<', "\\u003c")
            .replace('>', "\\u003e")
            .replace('\u{2028}', "\\u2028")
            .replace('\u{2029}', "\\u2029");

        let data = json!({
            "toolName": tool_name,
            "repetitionCount": repetition_count,
            "args": args_str,
            "contextJson": context_json_for_script,
        });

        let handlebars = self.handlebars.lock().unwrap();
        let html = match handlebars.render("circuit-break", &data) {
            Ok(h) => h,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::OperationFailed,
                    format!("Failed to render circuit breaker: {}", e),
                    ToolGroup::UI,
                )
                .with_guidance(vec![
                    "Verify internal tool name and arguments are valid".to_string()
                ])
                .to_mcp_result());
            }
        };

        let warning_message = format!(
            "⚠️ Circuit Breaker Triggered\n\nThe tool \"{}\" has been called {} times consecutively with identical parameters.\n\nThis usually indicates the agent is stuck in a loop. Please review the situation and click Resume to continue.",
            tool_name, repetition_count
        );

        Ok(crate::mcp::builtin::utils::create_resource_response(
            &format!("ui://circuit-break/{}", uuid::Uuid::new_v4()),
            "text/html",
            &html,
            "ui",
            "circuitBreak",
            Some(&warning_message),
        ))
    }

    fn resume_circuit_break(&self, args: Value) -> Result<MCPResult, String> {
        let tool_name = args
            .get("toolName")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let repetition_count = args
            .get("repetitionCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let text = format!(
            "🔄 Execution Resumed by User\n\n⚠️ IMPORTANT: You have called \"{}\" {} times with the same parameters.\n\nThis suggests your current approach is not working. Please:\n1. Analyze why the previous attempts failed (use planning__reflect if you need a structured critique → reflection → nextAction)\n2. Try a DIFFERENT approach or tool\n3. If the error persists, inform the user about the limitation\n\nDo NOT repeat the same tool call again.",
            tool_name, repetition_count
        );

        Ok(MCPResult {
            content: Some(vec![MCPContent::Text { text }]),
            structured_content: None,
            is_error: Some(false),
        })
    }

    async fn ask_user(&self, args: Value, session_id: Option<String>) -> Result<MCPResult, String> {
        let questions = match ask_user::parse_ask_user_args(&args) {
            Ok(questions) => questions,
            Err(result) => return Ok(result),
        };
        let Some(session_id) = session_id else {
            return Ok(MCPResult::error("ui__askUser needs an agent session"));
        };
        let Some(manager) = crate::state::get_app_handle()
            .and_then(|handle| handle.try_state::<crate::agent::AgentSessionManager>())
            .map(|state| state.inner().clone())
        else {
            return Ok(MCPResult::error(
                "User prompt requires the desktop UI runtime",
            ));
        };

        let payload = crate::agent::state::UserPromptPayload::Question {
            questions: questions.clone(),
        };
        match manager.request_user_prompt(&session_id, payload).await {
            Ok(outcome) => Ok(ask_user::ask_user_outcome_result(&questions, &outcome)),
            Err(message) => Ok(MCPResult::error(&message)),
        }
    }

    async fn report_result(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        let result = match non_empty_string_arg(&args, "result") {
            Some(v) => v,
            None if args.get("result").is_some() => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "result must be a non-empty string",
                    ToolGroup::UI,
                )
                .with_guidance(vec![
                    "Put the full user-facing outcome in `result`".to_string(),
                    "Include key outputs, paths, and any caveats".to_string(),
                ])
                .to_mcp_result());
            }
            None => return Ok(missing_param_error("result", ToolGroup::UI)),
        };

        // Optional: only meaningful when the request has checkable criteria.
        let criteria = non_empty_string_arg(&args, "criteria");
        let proof = non_empty_string_arg(&args, "proof");

        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");
        let title = args.get("title").and_then(|v| v.as_str());
        let status = args
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("success");

        if !["success", "partial", "blocked"].contains(&status) {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Invalid status '{}'. Supported: success, partial, blocked",
                    status
                ),
                ToolGroup::UI,
            )
            .with_guidance(vec![
                "Use success when the task is fully complete".to_string(),
                "Use partial when delivering best-effort results with known gaps".to_string(),
                "Use blocked when you cannot finish without external help".to_string(),
            ])
            .to_mcp_result());
        }

        let export_paths: Vec<String> = args
            .get("export_paths")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::trim).filter(|s| !s.is_empty()))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let mut deliverables = Vec::new();
        // Only enforce on-disk existence when we actually probed a session workspace.
        // Without session_id we must not reject (cannot safely touch the host FS).
        let mut missing_export_paths: Vec<String> = Vec::new();
        if !export_paths.is_empty() {
            for path_str in &export_paths {
                let (exists, size_bytes, extension, name, absolute_path) = if let Some(ref sid) =
                    session_id
                {
                    match crate::services::WorkspaceService::resolve_path_for_session(sid, path_str)
                        .await
                    {
                        Ok(full_path) => {
                            // Attach mode: pull container → staging so shell-written
                            // deliverables are visible before the host exists() probe.
                            // Propagate non-missing docker failures (same as
                            // sync_attach_before_host_read) — never trust stale staging.
                            if let Ok(Some(session)) =
                                crate::services::container_attach_fs::load_session(sid).await
                            {
                                if let Err(err) = crate::services::container_attach_fs::pull_container_file_to_host(
                                    &session,
                                    &full_path,
                                )
                                .await
                                {
                                    return Ok(guided_error(
                                        ErrorCategory::OperationFailed,
                                        format!(
                                            "Cannot verify export_paths: attach sync failed for {path_str}: {err}"
                                        ),
                                        ToolGroup::UI,
                                    )
                                    .with_guidance(vec![
                                        "Retry after the attach container is reachable (docker cp / sync)."
                                            .to_string(),
                                        "Do not claim status=success until export_paths can be synced from the container."
                                            .to_string(),
                                    ])
                                    .to_mcp_result_with_data(Some(json!({
                                        "attach_sync_failed_path": path_str,
                                        "error": err,
                                    }))));
                                }
                            }
                            let exists = full_path.exists();
                            let size = if exists {
                                std::fs::metadata(&full_path).map(|m| m.len()).ok()
                            } else {
                                None
                            };
                            let ext = full_path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|s| s.to_string());
                            let file_name = full_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(path_str)
                                .to_string();
                            // Strip Windows `\\?\` before slash-normalize so UI
                            // "Open" does not receive an unopenable `//?/` path.
                            let abs =
                                crate::mcp::builtin::utils::display_workspace_path(&full_path)
                                    .replace('\\', "/");
                            (exists, size, ext, file_name, Some(abs))
                        }
                        Err(_) => (false, None, None, path_str.clone(), None),
                    }
                } else {
                    // Safe fallback when no session/workspace is available: do not probe host filesystem
                    let p = std::path::Path::new(path_str);
                    let ext = p
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_string());
                    let file_name = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(path_str)
                        .to_string();
                    (false, None, ext, file_name, None)
                };

                if session_id.is_some() && !exists {
                    missing_export_paths.push(path_str.clone());
                }

                deliverables.push(json!({
                    "path": path_str,
                    "absolute_path": absolute_path,
                    "name": name,
                    "size_bytes": size_bytes,
                    "extension": extension,
                    "exists": exists,
                }));
            }
        }

        // checkSession-safe reject: do not emit Resource/STOP/type=reportResult.
        // Missing files + success+STOP would settle Idle via RecurringStop and let
        // parent checkSession (#2044) promote a hollow deliverable.
        if status == "success" && !missing_export_paths.is_empty() {
            let missing_list = missing_export_paths.join(", ");
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Cannot report status=success: export_paths not found on disk: {missing_list}"
                ),
                ToolGroup::UI,
            )
            .with_guidance(vec![
                "Write each missing deliverable with workspace__writeFile (or correct the path)."
                    .to_string(),
                "Verify the file exists (workspace__readFile / listDirectory), then call ui__reportResult again."
                    .to_string(),
                "Use status=partial only when intentionally delivering with known gaps — never claim success for missing files."
                    .to_string(),
            ])
            .to_mcp_result_with_data(Some(json!({
                "missing_export_paths": missing_export_paths,
                "deliverables": deliverables,
            }))));
        }

        let display_title = title.unwrap_or(match status {
            "partial" => "Partial result",
            "blocked" => "Blocked",
            _ => "Result",
        });

        // Keep `Result:\n{body}\n\nSTOP:` stable for parent checkSession extraction.
        let mut summary = format!(
            "Final result reported (status={status}).\n\
             Title: {display_title}\n"
        );
        if let Some(criteria) = criteria {
            summary.push_str(&format!("Acceptance criteria:\n{criteria}\n\n"));
        }
        if let Some(proof) = proof {
            summary.push_str(&format!("Verification proof:\n{proof}\n\n"));
        }
        summary.push_str(&format!(
            "Result:\n{result}\n\n\
             STOP: Do not call any more tools. The task outcome is already delivered. \
             End your turn now with at most a one-sentence confirmation."
        ));

        let structured_data = json!({
            "type": "reportResult",
            "status": status,
            "format": format,
            "title": display_title,
            "criteria": criteria,
            "proof": proof,
            "result": result,
            "deliverables": deliverables,
        });

        // Keep a minimal MCP UI resource so the existing workflow stop path
        // (`tool` message with Resource → settle idle / RecurringStop) still fires.
        // The chat UI prefers structured_content (ReportResultCard) over iframe render.
        let message_id = uuid::Uuid::new_v4().to_string();
        let stop_marker_resource = MCPContent::Resource {
            resource: json!({
                "uri": format!("ui://result/{}", message_id),
                "mimeType": "text/plain",
                "text": "",
            }),
            service_info: ServiceInfo {
                server_name: "ui".to_string(),
                tool_name: "reportResult".to_string(),
                backend_type: "BuiltInRust".to_string(),
            },
        };

        // Episode complete: drop goal/todos/scratchpad so the next user request
        // is not steered by stale planning context. partial/blocked keep planning
        // for resume; missing-export reject never reaches here.
        if status == "success" {
            if let Some(ref sid) = session_id {
                crate::agent::planning_reset::clear_planning_state_after_report_result_success(sid)
                    .await;
            }
        }

        Ok(MCPResult {
            content: Some(vec![
                MCPContent::Text { text: summary },
                stop_marker_resource,
            ]),
            structured_content: Some(structured_data),
            is_error: Some(false),
        })
    }
}

fn non_empty_string_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub const NAME: &str = "ui";

#[async_trait]
impl BuiltinMCPServer for UiServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "UI tools: askUser to ask the user questions and wait for the answers; reportResult for the final deliverable with acceptance criteria and verification proof when nothing remains to do."
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext::new(String::new())
    }

    fn tools(&self) -> Vec<MCPTool> {
        tools::all_tools()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        _session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "getUserAnswer" => self.get_user_answer(args),
            "circuitBreak" => self.circuit_break(args),
            "resumeCircuitBreak" => self.resume_circuit_break(args),
            "askUser" => self.ask_user(args, _session_id).await,
            "reportResult" => self.report_result(args, _session_id).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
