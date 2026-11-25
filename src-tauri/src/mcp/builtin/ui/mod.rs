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

/// The `kind` argument value of `ui__circuitBreak` / `ui__resumeCircuitBreak`
/// for a break caused by invalid tool-call arguments (#2957). One shared
/// constant: the invalid-args guard writes it, the resume selection compares
/// it, and the tests assert it — a typo on either side is now a compile
/// error instead of silently wrong guidance.
pub const CIRCUIT_BREAK_KIND_INVALID_TOOL_ARGS: &str = "invalidToolArgs";

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

    /// Renders the `ui__circuitBreak` card. `pub` only so integration tests
    /// can drive the exact production path (guard → card → resume, #2957
    /// G1.2); tool dispatch goes through `execute_tool`.
    pub fn circuit_break(&self, args: Value) -> Result<MCPResult, String> {
        let tool_name = match args.get("toolName").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return Ok(missing_param_error("toolName", ToolGroup::UI)),
        };
        let repetition_count = args
            .get("repetitionCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let args_str = args.get("args").and_then(|v| v.as_str()).unwrap_or("");
        // Callers other than the identical-call loop guard (for example, the
        // invalid tool-call arguments guard) give their own reason. When set,
        // the card shows it instead of the identical-parameters wording.
        let reason = non_empty_string_arg(&args, "reason");

        // Handlebars {{toolName}}/{{args}} HTML-escape once. Triple-stash {{{contextJson}}}
        // is raw — neutralize </script> breakouts from LLM-controlled strings.
        // `kind` (#2957) travels through the card to the Resume call so the
        // resume guidance can match the cause (`invalidToolArgs` vs loop).
        let kind = non_empty_string_arg(&args, "kind");
        let context_json = json!({
            "toolName": tool_name,
            "repetitionCount": repetition_count,
            "args": args_str,
            "kind": kind.unwrap_or(""),
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
            "reason": reason.unwrap_or(""),
            "kind": kind.unwrap_or(""),
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

        let warning_message = match reason {
            Some(reason) => format!("⚠️ Circuit Breaker Triggered\n\n{}", reason),
            None => format!(
                "⚠️ Circuit Breaker Triggered\n\nThe tool \"{}\" has been called {} times consecutively with identical parameters.\n\nThis usually indicates the agent is stuck in a loop. Please review the situation and click Resume to continue.",
                tool_name, repetition_count
            ),
        };

        Ok(crate::mcp::builtin::utils::create_resource_response(
            &format!("ui://circuit-break/{}", uuid::Uuid::new_v4()),
            "text/html",
            &html,
            "ui",
            "circuitBreak",
            Some(&warning_message),
        ))
    }

    /// Builds the `ui__resumeCircuitBreak` tool result. `pub` only so
    /// integration tests can drive the exact production resume path (#2957
    /// G1.2); tool dispatch goes through `execute_tool`.
    pub fn resume_circuit_break(&self, args: Value) -> Result<MCPResult, String> {
        let tool_name = args
            .get("toolName")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let repetition_count = args
            .get("repetitionCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        // #2957: the break card passes its cause kind through. An
        // invalid-arguments break never ran the tool, so the resume text
        // must guide toward smaller output, not "different approach".
        let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("");

        let text = if kind == CIRCUIT_BREAK_KIND_INVALID_TOOL_ARGS {
            format!(
                "🔄 Execution Resumed by User\n\n⚠️ IMPORTANT: Your previous tool-call arguments for \"{}\" were cut off or not valid JSON, so the tool did NOT run (it was attempted {} time(s)).\n\nThe arguments may have been too large for one tool call. Please:\n1. Split the output into smaller parts — for example, write a large file in several smaller calls instead of one\n2. Keep each tool call's arguments short\n3. Do NOT send the same large payload again",
                tool_name, repetition_count
            )
        } else {
            format!(
                "🔄 Execution Resumed by User\n\n⚠️ IMPORTANT: You have called \"{}\" {} times with the same parameters.\n\nThis suggests your current approach is not working. Please:\n1. Analyze why the previous attempts failed (use planning__reflect if you need a structured critique → reflection → nextAction)\n2. Try a DIFFERENT approach or tool\n3. If the error persists, inform the user about the limitation\n\nDo NOT repeat the same tool call again.",
                tool_name, repetition_count
            )
        };

        Ok(MCPResult {
            content: Some(vec![MCPContent::Text { text }]),
            structured_content: None,
            is_error: Some(false),
        })
    }

    async fn ask_user(&self, args: Value, session_id: Option<String>) -> Result<MCPResult, String> {
        let (questions, warnings) = match ask_user::parse_ask_user_args(&args) {
            Ok(parsed) => parsed,
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
            Ok(outcome) => Ok(ask_user::ask_user_outcome_result(
                &questions, &outcome, &warnings,
            )),
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
        // Load attach metadata once: the sync branch below and the Docker hint in
        // the missing-paths error both need it (#2972 item 5).
        let attach_metadata = match session_id.as_deref() {
            Some(sid) if !export_paths.is_empty() => {
                crate::services::container_attach_fs::load_session(sid)
                    .await
                    .ok()
                    .flatten()
            }
            _ => None,
        };
        let docker_session = attach_metadata.as_ref().is_some_and(|session| {
            session.workspace_isolation
                == crate::models::workspace_isolation::WorkspaceIsolationMode::Docker
        });
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
                            if let Some(session) = attach_metadata.as_ref() {
                                if let Err(err) = crate::services::container_attach_fs::pull_container_file_to_host(
                                    session,
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
                                    .with_guidance(attach_sync_failure_guidance(&err))
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
            // #2731 2697 D-1, #2972 review G1.1: this branch is only reached
            // after any attach sync succeeded (or none was needed), so daemon
            // unreachability cannot be claimed here — the attach-sync-failure
            // path above names it when a Docker call actually failed. What is
            // known: a Docker session can hold files inside the container,
            // outside the mounted workspace, where no sync can see them.
            let docker_suffix = if docker_session {
                " This session is Docker-isolated: files written inside the \
                 container outside the mounted workspace stay there and never \
                 appear on the host."
            } else {
                ""
            };
            let mut guidance = vec![
                "Write each missing deliverable with workspace__writeFile (or correct the path)."
                    .to_string(),
                "Verify the file exists (workspace__readFile / listDirectory), then call ui__reportResult again."
                    .to_string(),
            ];
            if docker_session {
                guidance.push(
                    "Write deliverables inside the session's mounted workspace so they \
                     can be synced to the host."
                        .to_string(),
                );
            }
            guidance.push(
                "Use status=partial only when intentionally delivering with known gaps — never claim success for missing files."
                    .to_string(),
            );
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Cannot report status=success: export_paths not found on disk: {missing_list}.{docker_suffix}"
                ),
                ToolGroup::UI,
            )
            .with_guidance(guidance)
            .to_mcp_result_with_data(Some(json!({
                "missing_export_paths": missing_export_paths,
                "docker_session": docker_session,
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
            "circuitBreak" => self.circuit_break(args),
            "resumeCircuitBreak" => self.resume_circuit_break(args),
            "askUser" => self.ask_user(args, _session_id).await,
            "reportResult" => self.report_result(args, _session_id).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}

/// Guidance for a failed attach sync in `reportResult`: name the Docker
/// daemon only when the error actually reports one unreachable — a missing
/// or stopped container needs a different fix than a stopped daemon
/// (#2972 review G1.1).
pub(crate) fn attach_sync_failure_guidance(err: &str) -> Vec<String> {
    let first = if crate::services::container_attach_fs::is_daemon_unreachable_error(err) {
        "The Docker daemon is unreachable — start Docker (or Docker Desktop) so the \
         container file can be synced, then retry."
            .to_string()
    } else {
        "Retry after the attach container is reachable (docker cp / sync).".to_string()
    };
    vec![
        first,
        "Do not claim status=success until export_paths can be synced from the container."
            .to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_sync_guidance_names_daemon_only_when_observed() {
        // #2972 review G1.1: "daemon unreachable" must come from the actual
        // Docker error, never just from the session being Docker-isolated.
        let daemon_err = "docker cp abc123:/app/work/task.txt /tmp/task.txt failed: \
                          Cannot connect to the Docker daemon at unix:///var/run/docker.sock. \
                          Is the docker daemon running?";
        let guidance = attach_sync_failure_guidance(daemon_err);
        assert!(
            guidance[0].contains("Docker daemon is unreachable"),
            "observed daemon failure must name the daemon: {guidance:?}"
        );

        let container_err = "docker cp failed: Error response from daemon: No such container: xyz";
        let guidance = attach_sync_failure_guidance(container_err);
        assert!(
            !guidance[0].contains("daemon is unreachable"),
            "a missing container must not be reported as a dead daemon: {guidance:?}"
        );
        assert!(
            guidance[0].contains("attach container is reachable"),
            "{guidance:?}"
        );
    }

    fn first_text(result: MCPResult) -> String {
        result
            .content
            .expect("content")
            .into_iter()
            .find_map(|item| match item {
                MCPContent::Text { text } => Some(text),
                _ => None,
            })
            .expect("warning text content")
    }

    fn embedded_html(result: MCPResult) -> String {
        result
            .content
            .expect("content")
            .into_iter()
            .find_map(|item| match item {
                MCPContent::Resource { resource, .. } => resource
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                _ => None,
            })
            .expect("embedded HTML resource")
    }

    #[test]
    fn circuit_break_card_shows_reason_instead_of_loop_text() {
        // The chat UI renders the HTML card, so the reason must be in the
        // card, not only in the tool result text (#2955).
        let html = embedded_html(
            UiServer::new()
                .circuit_break(json!({
                    "toolName": "workspace__writeFile",
                    "repetitionCount": 4,
                    "args": "{\"path\": \"a",
                    "reason": "The model sent invalid arguments for the tool \"workspace__writeFile\" in 4 replies in a row (truncated_tool_args_string).",
                }))
                .expect("resource response"),
        );
        assert!(html.contains("invalid arguments for the tool"));
        assert!(html.contains("truncated_tool_args_string"));
        assert!(!html.contains("identical\n        parameters"));
        assert!(!html.contains("stuck in a loop"));
    }

    #[test]
    fn circuit_break_card_keeps_loop_text_without_reason() {
        for args in [
            json!({ "toolName": "workspace__writeFile", "repetitionCount": 5, "args": "{}" }),
            json!({
                "toolName": "workspace__writeFile",
                "repetitionCount": 5,
                "args": "{}",
                "reason": "   ",
            }),
        ] {
            let html = embedded_html(
                UiServer::new()
                    .circuit_break(args)
                    .expect("resource response"),
            );
            assert!(html.contains("identical\n        parameters"));
            assert!(html.contains("stuck in a loop"));
        }
    }

    #[test]
    fn circuit_break_card_html_escapes_reason() {
        // reason is LLM/caller-controlled; the card must not render it raw.
        let html = embedded_html(
            UiServer::new()
                .circuit_break(json!({
                    "toolName": "x__y",
                    "repetitionCount": 4,
                    "args": "{}",
                    "reason": "bad <script>alert(1)</script> args",
                }))
                .expect("resource response"),
        );
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
    }

    #[test]
    fn circuit_break_uses_caller_reason_when_provided() {
        // Callers other than the identical-call loop guard (for example, the
        // invalid tool-call arguments guard) pass their own reason.
        let text = first_text(
            UiServer::new()
                .circuit_break(json!({
                    "toolName": "workspace__writeFile",
                    "repetitionCount": 4,
                    "args": "{}",
                    "reason": "The model sent invalid arguments for the tool \"x\".",
                }))
                .expect("resource response"),
        );
        assert!(text.starts_with("⚠️ Circuit Breaker Triggered"));
        assert!(text.contains("The model sent invalid arguments for the tool \"x\"."));
        assert!(!text.contains("identical parameters"));
    }

    #[test]
    fn circuit_break_falls_back_to_identical_call_text() {
        // Missing or blank reason keeps the loop-guard wording.
        for args in [
            json!({ "toolName": "workspace__writeFile", "repetitionCount": 5, "args": "{}" }),
            json!({
                "toolName": "workspace__writeFile",
                "repetitionCount": 5,
                "args": "{}",
                "reason": "   ",
            }),
        ] {
            let text = first_text(
                UiServer::new()
                    .circuit_break(args)
                    .expect("resource response"),
            );
            assert!(text.starts_with("⚠️ Circuit Breaker Triggered"));
            assert!(
                text.contains("has been called 5 times consecutively with identical parameters")
            );
        }
    }

    // --- #2957: the Resume guidance must match the break cause -------------

    #[test]
    fn resume_after_invalid_args_break_gives_split_output_guidance() {
        let text = first_text(
            UiServer::new()
                .resume_circuit_break(json!({
                    "toolName": "workspace__writeFile",
                    "repetitionCount": 4,
                    "kind": CIRCUIT_BREAK_KIND_INVALID_TOOL_ARGS,
                }))
                .expect("resume result"),
        );
        assert!(text.starts_with("🔄 Execution Resumed by User"));
        assert!(
            text.contains("cut off or not valid JSON"),
            "resume text names the actual cause: {text}"
        );
        assert!(text.contains("did NOT run"), "the tool never ran: {text}");
        assert!(
            text.contains("smaller parts"),
            "resume text tells the model to split the output: {text}"
        );
        assert!(
            !text.contains("same parameters"),
            "the identical-loop wording must not appear: {text}"
        );
        assert!(
            !text.contains("DIFFERENT approach"),
            "\"try a different tool\" is wrong guidance here: {text}"
        );
    }

    #[test]
    fn resume_after_identical_call_loop_keeps_loop_text() {
        let text = first_text(
            UiServer::new()
                .resume_circuit_break(json!({
                    "toolName": "workspace__runShell",
                    "repetitionCount": 5,
                }))
                .expect("resume result"),
        );
        assert!(text.contains("times with the same parameters"));
        assert!(text.contains("DIFFERENT approach"));
        assert!(
            !text.contains("cut off or not valid JSON"),
            "the invalid-args wording must not appear: {text}"
        );
    }

    #[test]
    fn resume_with_missing_or_unknown_kind_falls_back_to_loop_text() {
        for args in [
            json!({ "toolName": "workspace__runShell", "repetitionCount": 5 }),
            json!({ "toolName": "workspace__runShell", "repetitionCount": 5, "kind": "" }),
            json!({ "toolName": "workspace__runShell", "repetitionCount": 5, "kind": "   " }),
            json!({ "toolName": "workspace__runShell", "repetitionCount": 5, "kind": "somethingElse" }),
        ] {
            let text = first_text(
                UiServer::new()
                    .resume_circuit_break(args)
                    .expect("resume result"),
            );
            assert!(
                text.contains("times with the same parameters"),
                "unknown kind falls back to the loop text: {text}"
            );
        }
    }

    #[test]
    fn circuit_break_card_carries_kind_into_the_resume_context() {
        // The card's resume button sends `kind` from the context JSON
        // (#2957), so the rendered script must contain the actual value.
        let html = embedded_html(
            UiServer::new()
                .circuit_break(json!({
                    "toolName": "workspace__writeFile",
                    "repetitionCount": 4,
                    "args": "{}",
                    "kind": CIRCUIT_BREAK_KIND_INVALID_TOOL_ARGS,
                }))
                .expect("resource response"),
        );
        assert!(html.contains(CIRCUIT_BREAK_KIND_INVALID_TOOL_ARGS));
        // S1.2: without a kind the context JSON itself must render
        // `"kind":""` — the button then sends an empty kind, and the
        // resume tool falls back to the loop text.
        let html = embedded_html(
            UiServer::new()
                .circuit_break(json!({
                    "toolName": "workspace__writeFile",
                    "repetitionCount": 5,
                    "args": "{}",
                }))
                .expect("resource response"),
        );
        assert!(
            html.contains(r#""kind":"""#),
            "context JSON must carry an empty kind: {}",
            html.lines()
                .find(|line| line.contains("contextData"))
                .unwrap_or("<contextData line not found>")
        );
    }
}
