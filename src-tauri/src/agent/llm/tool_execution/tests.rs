mod pre_tool_hook_tests {
    use super::super::run::{apply_pre_tool_hook_decision, PreToolHookResolution};
    use crate::agent::state::PendingApprovalKind;
    use crate::agent::tool_approvals::ToolApprovalRequest;
    use crate::services::hooks::HookDecision;

    fn sample_req() -> ToolApprovalRequest {
        ToolApprovalRequest {
            description: "policy requires approval".to_string(),
            input_preview: "{\"cmd\":\"rm\"}".to_string(),
            masked_arguments: None,
        }
    }

    #[test]
    fn block_always_wins_and_propagates_reason() {
        for (req, hard) in [
            (None, false),
            (Some(sample_req()), false),
            (Some(sample_req()), true),
        ] {
            let res = apply_pre_tool_hook_decision(
                "workspace__runShell",
                "{}",
                req,
                hard,
                Some(HookDecision::Block),
                Some("denied by policy hook".to_string()),
            );
            assert_eq!(
                res,
                PreToolHookResolution::Block {
                    reason: "denied by policy hook".to_string()
                }
            );
        }

        let fallback = apply_pre_tool_hook_decision(
            "workspace__runShell",
            "{}",
            None,
            false,
            Some(HookDecision::Block),
            Some("   ".to_string()),
        );
        assert_eq!(
            fallback,
            PreToolHookResolution::Block {
                reason: "Tool 'workspace__runShell' blocked by PreToolUse hook.".to_string()
            }
        );
    }

    #[test]
    fn allow_clears_standard_approval_but_never_clears_hard_approval() {
        let standard = apply_pre_tool_hook_decision(
            "workspace__writeFile",
            "{}",
            Some(sample_req()),
            false,
            Some(HookDecision::Allow),
            None,
        );
        assert_eq!(standard, PreToolHookResolution::Execute);

        let hard = apply_pre_tool_hook_decision(
            "workspace__runShell",
            "{}",
            Some(sample_req()),
            true,
            Some(HookDecision::Allow),
            None,
        );
        assert_eq!(
            hard,
            PreToolHookResolution::RequestApproval {
                request: sample_req(),
                kind: PendingApprovalKind::Hard,
            }
        );
    }

    #[test]
    fn ask_forces_confirmation_even_when_runtime_had_no_approval_request() {
        // Simulates YOLO mode where `approval_request_for_runtime` returned None.
        let yolo_ask = apply_pre_tool_hook_decision(
            "workspace__runShell",
            "{\"cmd\":\"ls\"}",
            None,
            false,
            Some(HookDecision::Ask),
            Some("confirm shell command".to_string()),
        );
        assert_eq!(
            yolo_ask,
            PreToolHookResolution::RequestApproval {
                request: ToolApprovalRequest {
                    description: "confirm shell command".to_string(),
                    input_preview: "{\"cmd\":\"ls\"}".to_string(),
                    masked_arguments: None,
                },
                kind: PendingApprovalKind::Standard,
            }
        );

        let hard_ask = apply_pre_tool_hook_decision(
            "workspace__runShell",
            "{}",
            Some(sample_req()),
            true,
            Some(HookDecision::Ask),
            None,
        );
        assert_eq!(
            hard_ask,
            PreToolHookResolution::RequestApproval {
                request: sample_req(),
                kind: PendingApprovalKind::Hard,
            }
        );
    }

    #[test]
    fn none_decision_preserves_runtime_policy() {
        assert_eq!(
            apply_pre_tool_hook_decision("t", "{}", None, false, None, None),
            PreToolHookResolution::Execute
        );
        assert_eq!(
            apply_pre_tool_hook_decision("t", "{}", Some(sample_req()), false, None, None),
            PreToolHookResolution::RequestApproval {
                request: sample_req(),
                kind: PendingApprovalKind::Standard,
            }
        );
    }
}

mod pending_approval_tests {
    use super::super::approval::{build_pending_approval, ToolCallRef};
    use crate::agent::events::AgentEvent;
    use crate::agent::state::PendingApprovalKind;
    use crate::agent::tool_approvals::{
        evaluate_tool_execution_policy, ToolApprovalRequest, ToolExecutionPolicyDecision,
    };
    use tokio::sync::oneshot;

    fn event_arguments(event: &AgentEvent) -> &str {
        let AgentEvent::ToolExecutionRequiresApproval { arguments, .. } = event else {
            panic!("expected a ToolExecutionRequiresApproval event");
        };
        arguments
    }

    /// #2968 (gatekeeper G1.1): the arguments stored in `pending_approvals` (the
    /// source for the Session API listing and the resume re-emit) and the
    /// arguments emitted for the approval card both come from the request's
    /// masked arguments — a literal query value, header or fragment never
    /// reaches either.
    #[tokio::test]
    async fn mcp_server_approval_stores_and_emits_masked_arguments() {
        let args = serde_json::json!({
            "name": "svc",
            "transport": {
                "type": "http",
                "url": "http://127.0.0.1:9001/mcp?token=literal-secret&key={{secret:URLKEY}}#frag",
                "headers": { "Authorization": "Bearer literal-header" },
            },
        });
        let decision = evaluate_tool_execution_policy("tool__registerServer", &args).await;
        let ToolExecutionPolicyDecision::RequireHardApproval(request) = decision else {
            panic!("expected hard approval, got {decision:?}");
        };
        let (tx, _rx) = oneshot::channel();
        let (stored, event) = build_pending_approval(
            &ToolCallRef {
                session_id: "sess-2968",
                tool_call_id: "tc-2968",
                tool_name: "tool__registerServer",
            },
            &args.to_string(),
            &request,
            PendingApprovalKind::Hard,
            tx,
            None,
        );

        assert_eq!(stored.arguments, event_arguments(&event));
        for arguments in [&stored.arguments, event_arguments(&event)] {
            assert!(!arguments.contains("literal-secret"), "{arguments}");
            assert!(!arguments.contains("literal-header"), "{arguments}");
            assert!(!arguments.contains("#frag"), "{arguments}");
            assert!(arguments.contains("token=***"), "{arguments}");
            assert!(
                arguments.contains("{{secret:URLKEY}}"),
                "templates must stay visible: {arguments}"
            );
        }
    }

    /// The opposite case: a request with no masked arguments keeps the tool
    /// call's raw arguments on both the stored record and the event, and the
    /// kind and request id reach both payloads.
    #[test]
    fn approval_without_masked_arguments_keeps_raw_arguments() {
        let request = ToolApprovalRequest {
            description: "run the thing".to_string(),
            input_preview: "preview".to_string(),
            masked_arguments: None,
        };
        let raw = r#"{"command":"ls -la"}"#;
        let (tx, _rx) = oneshot::channel();
        let (stored, event) = build_pending_approval(
            &ToolCallRef {
                session_id: "sess-raw",
                tool_call_id: "tc-raw",
                tool_name: "workspace__runShell",
            },
            raw,
            &request,
            PendingApprovalKind::Standard,
            tx,
            Some("req-9".to_string()),
        );
        assert_eq!(stored.arguments, raw);
        assert_eq!(event_arguments(&event), raw);
        assert_eq!(stored.request_id.as_deref(), Some("req-9"));
        let AgentEvent::ToolExecutionRequiresApproval {
            request_id,
            approval_kind,
            ..
        } = &event
        else {
            panic!("expected a ToolExecutionRequiresApproval event");
        };
        assert_eq!(request_id.as_deref(), Some("req-9"));
        assert_eq!(*approval_kind, PendingApprovalKind::Standard);
    }
}
