//! E2E control plane guard + handler tests (debug builds only).
//!
//! Two layers, both deterministic without env vars or process globals:
//!
//! 1. **Wiring**: the real production chain from
//!    `e2e_control_guarded_routes_with_capability` — the same function
//!    `server/routes.rs` uses — so removing the loopback Host guard or the
//!    loopback peer guard from the wiring fails these tests.
//! 2. **Handlers**: the pure dispatch decisions behind `handle_user_prompt`
//!    and `handle_tool_approval` (stale requestId, answer/cancel/unknown,
//!    approval status mapping). A live `AgentSessionManager` needs a Tauri
//!    app handle, which integration tests cannot construct, so the handlers
//!    delegate those decisions to the tested pure functions.

#[cfg(debug_assertions)]
mod tests {
    use std::convert::Infallible;
    use std::sync::Arc;

    use tauri_mcp_agent_lib::agent::state::{
        PendingUserPromptSummary, UserPromptAnswer, UserPromptKind, UserPromptPayload,
        USER_PROMPT_GONE_MESSAGE,
    };
    use tauri_mcp_agent_lib::agent::types::{ToolCall, ToolCallFunction};
    use tauri_mcp_agent_lib::agent::AgentSessionManager;
    use tauri_mcp_agent_lib::models::chat::Message;
    use tauri_mcp_agent_lib::server::access_guard::handle_rejection;
    use tauri_mcp_agent_lib::server::e2e_control::{
        capability_guard, constant_time_eq, e2e_control_guarded_routes_with_capability,
        header_matches, pending_circuit_break, plan_user_prompt_action,
        resume_circuit_break_message, tool_approval_error_status, PlannedPromptAction,
        UserPromptBody, E2E_CONTROL_HEADER,
    };
    use warp::http::{HeaderMap, StatusCode};
    use warp::Filter;

    const TOKEN: &str = "0f2a8c5e1b7d4e9a8c6f3d2b1a0e9c8d7f6e5d4c3b2a1908";
    const LOOPBACK_HOST: &str = "127.0.0.1:3030";
    const LAN_HOST: &str = "192.168.1.100:3030";
    const LOOPBACK_PEER: &str = "127.0.0.1:54321";
    const LAN_PEER: &str = "192.168.1.100:54321";

    fn guarded(
        token: Option<&str>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        let filter = capability_guard(token.map(str::to_string)).map(|| "ok");
        filter.recover(handle_rejection)
    }

    #[test]
    fn constant_time_eq_accepts_equal_and_rejects_unequal() {
        assert!(constant_time_eq(TOKEN, TOKEN));
        assert!(!constant_time_eq(TOKEN, "wrong"));
        assert!(!constant_time_eq(TOKEN, &TOKEN[..TOKEN.len() - 1]));
        assert!(!constant_time_eq("", TOKEN));
    }

    #[test]
    fn header_matches_requires_exact_capability() {
        let mut headers = HeaderMap::new();
        headers.insert(E2E_CONTROL_HEADER, TOKEN.parse().unwrap());
        assert!(header_matches(&headers, TOKEN));
        assert!(!header_matches(&headers, "not-the-token"));
        assert!(!header_matches(&HeaderMap::new(), TOKEN));
    }

    #[tokio::test]
    async fn disabled_control_plane_answers_404_even_with_a_token() {
        let filter = guarded(None);
        let res = warp::test::request()
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&filter)
            .await;
        assert_eq!(res.status(), 404);
    }

    #[tokio::test]
    async fn correct_capability_header_is_accepted() {
        let res = warp::test::request()
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&guarded(Some(TOKEN)))
            .await;
        assert_eq!(res.status(), 200);
    }

    #[tokio::test]
    async fn wrong_or_missing_capability_header_is_403() {
        let wrong = warp::test::request()
            .header(E2E_CONTROL_HEADER, "wrong-value")
            .reply(&guarded(Some(TOKEN)))
            .await;
        assert_eq!(wrong.status(), 403);

        let missing = warp::test::request().reply(&guarded(Some(TOKEN))).await;
        assert_eq!(missing.status(), 403);
    }

    // ------------------------------------------------------------------
    // Production wiring: the exact chain `server/routes.rs` mounts.
    // ------------------------------------------------------------------

    /// Manager filter that must never be extracted: every guard rejects
    /// before route handlers run, so reaching it means exposure widened.
    fn forbidden_manager(
    ) -> impl Filter<Extract = (Arc<AgentSessionManager>,), Error = Infallible> + Clone {
        warp::any().map(|| -> Arc<AgentSessionManager> {
            panic!("agent manager extracted although a guard should have rejected")
        })
    }

    fn production_chain() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
    {
        e2e_control_guarded_routes_with_capability(Some(TOKEN.to_string()), forbidden_manager())
            .recover(handle_rejection)
    }

    #[tokio::test]
    async fn status_route_passes_with_loopback_host_loopback_peer_and_token() {
        let res = warp::test::request()
            .method("GET")
            .path("/api/e2e-control/status")
            .remote_addr(LOOPBACK_PEER.parse().unwrap())
            .header("host", LOOPBACK_HOST)
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&production_chain())
            .await;
        assert_eq!(res.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(res.body()).unwrap();
        assert_eq!(body["enabled"], true);
        assert!(body["buildSha"].is_string());
        assert!(body["buildDirty"].is_boolean());
    }

    #[tokio::test]
    async fn lan_host_header_is_rejected_even_with_valid_token() {
        // Pins `access_guard(false)` in the production chain: a LAN Host must
        // fail regardless of any server-wide expose flag.
        let res = warp::test::request()
            .method("GET")
            .path("/api/e2e-control/status")
            .remote_addr(LOOPBACK_PEER.parse().unwrap())
            .header("host", LAN_HOST)
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&production_chain())
            .await;
        assert_eq!(res.status(), 403);
    }

    #[tokio::test]
    async fn non_loopback_peer_is_rejected_even_with_valid_token() {
        // Pins `loopback_peer_guard()` in the production chain.
        let res = warp::test::request()
            .method("GET")
            .path("/api/e2e-control/status")
            .remote_addr(LAN_PEER.parse().unwrap())
            .header("host", LOOPBACK_HOST)
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&production_chain())
            .await;
        assert_eq!(res.status(), 403);
    }

    #[tokio::test]
    async fn missing_capability_header_is_rejected_through_full_chain() {
        let res = warp::test::request()
            .method("GET")
            .path("/api/e2e-control/status")
            .remote_addr(LOOPBACK_PEER.parse().unwrap())
            .header("host", LOOPBACK_HOST)
            .reply(&production_chain())
            .await;
        assert_eq!(res.status(), 403);
    }

    #[tokio::test]
    async fn lan_host_rejects_mutating_routes_before_manager_extraction() {
        // POST routes extract the agent manager; the Host guard must reject
        // before that extraction (the forbidden manager filter would panic).
        let res = warp::test::request()
            .method("POST")
            .path("/api/e2e-control/sessions/sess-1/user-prompts")
            .remote_addr(LOOPBACK_PEER.parse().unwrap())
            .header("host", LAN_HOST)
            .header(E2E_CONTROL_HEADER, TOKEN)
            .json(&serde_json::json!({"requestId": "r", "action": "cancel"}))
            .reply(&production_chain())
            .await;
        assert_eq!(res.status(), 403);

        let res = warp::test::request()
            .method("POST")
            .path("/api/e2e-control/sessions/sess-1/tool-approvals")
            .remote_addr(LOOPBACK_PEER.parse().unwrap())
            .header("host", LAN_HOST)
            .header(E2E_CONTROL_HEADER, TOKEN)
            .json(&serde_json::json!({"toolCallId": "t", "approved": true}))
            .reply(&production_chain())
            .await;
        assert_eq!(res.status(), 403);
    }

    // ------------------------------------------------------------------
    // Handler decisions (pure functions the HTTP handlers delegate to).
    // ------------------------------------------------------------------

    fn pending(kind: UserPromptKind, request_id: &str) -> PendingUserPromptSummary {
        let payload = match kind {
            UserPromptKind::Secret => UserPromptPayload::Secret {
                variable_name: "E2E_SECRET".to_string(),
                label: "secret".to_string(),
                reuse_note: None,
            },
            UserPromptKind::Question => UserPromptPayload::Question { questions: vec![] },
        };
        PendingUserPromptSummary {
            request_id: request_id.to_string(),
            kind,
            payload,
            created_at: chrono::Utc::now(),
        }
    }

    fn body(request_id: &str, action: &str, value: Option<&str>) -> UserPromptBody {
        UserPromptBody {
            request_id: request_id.to_string(),
            action: action.to_string(),
            value: value.map(str::to_string),
        }
    }

    #[test]
    fn stale_request_id_is_rejected_with_gone_message() {
        let waiting = pending(UserPromptKind::Secret, "req-1");
        let err = plan_user_prompt_action(Some(&waiting), &body("req-2", "answer", Some("x")))
            .expect_err("stale requestId must be rejected");
        assert_eq!(err.0, StatusCode::NOT_FOUND);
        assert_eq!(err.1, USER_PROMPT_GONE_MESSAGE);
    }

    #[test]
    fn absent_prompt_is_rejected_like_a_stale_one() {
        let err = plan_user_prompt_action(None, &body("req-1", "answer", Some("x")))
            .expect_err("no waiting prompt must be rejected");
        assert_eq!(err.0, StatusCode::NOT_FOUND);
        assert_eq!(err.1, USER_PROMPT_GONE_MESSAGE);
    }

    #[test]
    fn answer_maps_prompt_kind_to_the_matching_answer_variant() {
        let waiting = pending(UserPromptKind::Secret, "req-1");
        let planned =
            plan_user_prompt_action(Some(&waiting), &body("req-1", "answer", Some("s3cret")))
                .expect("matching requestId + value must plan an answer");
        assert_eq!(
            planned,
            PlannedPromptAction::Answer(UserPromptAnswer::Secret("s3cret".to_string()))
        );

        let question = pending(UserPromptKind::Question, "req-1");
        let planned =
            plan_user_prompt_action(Some(&question), &body("req-1", "answer", Some("42")))
                .expect("question prompts accept an answer too");
        assert_eq!(
            planned,
            PlannedPromptAction::Answer(UserPromptAnswer::Question("42".to_string()))
        );
    }

    #[test]
    fn answer_without_value_is_a_bad_request() {
        let waiting = pending(UserPromptKind::Secret, "req-1");
        let err = plan_user_prompt_action(Some(&waiting), &body("req-1", "answer", None))
            .expect_err("answer without value must fail");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(err.1.contains("Missing value"));
    }

    #[test]
    fn cancel_action_is_planned_without_a_value() {
        let waiting = pending(UserPromptKind::Question, "req-1");
        let planned = plan_user_prompt_action(Some(&waiting), &body("req-1", "cancel", None))
            .expect("cancel needs no value");
        assert_eq!(planned, PlannedPromptAction::Cancel);
    }

    #[test]
    fn unknown_action_is_a_bad_request() {
        let waiting = pending(UserPromptKind::Secret, "req-1");
        let err = plan_user_prompt_action(Some(&waiting), &body("req-1", "explode", Some("x")))
            .expect_err("unknown action must fail");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(err.1.contains("Unknown user prompt action"));
    }

    #[test]
    fn tool_approval_error_status_maps_not_found_and_everything_else() {
        assert_eq!(
            tool_approval_error_status("tool call not found"),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            tool_approval_error_status("approval already resolved"),
            StatusCode::BAD_REQUEST
        );
    }

    // ------------------------------------------------------------------
    // Circuit-breaker resume observation + injection (#2901).
    // ------------------------------------------------------------------

    fn assistant_tool_call(name: &str, arguments: &str, id: &str) -> Message {
        let now = chrono::Utc::now().timestamp_millis();
        Message {
            id: format!("msg-{id}"),
            session_id: "sess-1".to_string(),
            role: "assistant".to_string(),
            content: vec![],
            tool_calls: Some(vec![ToolCall {
                id: id.to_string(),
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name: name.to_string(),
                    arguments: arguments.to_string(),
                },
            }]),
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            usage: None,
            prompt_tokens: None,
            created_at: now,
            updated_at: now,
            source: None,
            error: None,
            metadata: None,
        }
    }

    fn circuit_break_call() -> Message {
        assistant_tool_call(
            "ui__circuitBreak",
            r#"{"toolName":"workspace__runShell","repetitionCount":5,"args":{}}"#,
            "call-break",
        )
    }

    #[test]
    fn detects_circuit_break_waiting_for_resume() {
        let messages = vec![
            assistant_tool_call("workspace__runShell", r#"{"command":"ls"}"#, "call-a"),
            circuit_break_call(),
        ];
        let pending = pending_circuit_break(&messages).expect("break must be pending");
        assert_eq!(pending.tool_name, "workspace__runShell");
        assert_eq!(pending.repetition_count, 5);
        assert_eq!(pending.tool_call_id, "call-break");
    }

    #[test]
    fn resume_tool_call_clears_pending_circuit_break() {
        let messages = vec![
            circuit_break_call(),
            assistant_tool_call(
                "ui__resumeCircuitBreak",
                r#"{"toolName":"workspace__runShell","repetitionCount":5}"#,
                "call-resume",
            ),
        ];
        assert!(pending_circuit_break(&messages).is_none());
    }

    #[test]
    fn no_circuit_break_means_nothing_pending() {
        let messages = vec![
            assistant_tool_call("workspace__runShell", r#"{"command":"ls"}"#, "call-a"),
            assistant_tool_call("planning__addTodo", "{}", "call-b"),
        ];
        assert!(pending_circuit_break(&messages).is_none());
    }

    #[test]
    fn malformed_circuit_break_args_still_resumeable() {
        let messages = vec![assistant_tool_call(
            "ui__circuitBreak",
            "not-json",
            "call-break",
        )];
        let pending = pending_circuit_break(&messages).expect("break must still be pending");
        assert_eq!(pending.tool_name, "unknown");
        assert_eq!(pending.repetition_count, 0);
    }

    #[test]
    fn resume_message_reuses_the_gui_tool_call_shape() {
        let message = resume_circuit_break_message("sess-1", "workspace__runShell", 5);
        assert_eq!(message.session_id, "sess-1");
        assert_eq!(message.role, "assistant");
        assert!(message.content.is_empty());
        let tool_calls = message.tool_calls.expect("resume injects a tool call");
        assert_eq!(tool_calls.len(), 1);
        let name = &tool_calls[0].function.name;
        // The `ui__` prefix is what admits an idle UI-tool entry in
        // `handle_llm_response`; the name must match the GUI button exactly.
        assert!(name.starts_with("ui__"));
        assert_eq!(name, "ui__resumeCircuitBreak");
        let args: serde_json::Value = serde_json::from_str(&tool_calls[0].function.arguments)
            .expect("arguments must be valid JSON");
        assert_eq!(args["toolName"], "workspace__runShell");
        assert_eq!(args["repetitionCount"], 5);
    }

    #[tokio::test]
    async fn lan_host_rejects_circuit_break_resume_before_manager_extraction() {
        let res = warp::test::request()
            .method("POST")
            .path("/api/e2e-control/sessions/sess-1/circuit-break-resume")
            .remote_addr(LOOPBACK_PEER.parse().unwrap())
            .header("host", LAN_HOST)
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&production_chain())
            .await;
        assert_eq!(res.status(), 403);
    }

    #[tokio::test]
    async fn missing_token_rejects_circuit_break_resume() {
        let res = warp::test::request()
            .method("POST")
            .path("/api/e2e-control/sessions/sess-1/circuit-break-resume")
            .remote_addr(LOOPBACK_PEER.parse().unwrap())
            .header("host", LOOPBACK_HOST)
            .reply(&production_chain())
            .await;
        assert_eq!(res.status(), 403);
    }
}
