mod pre_tool_hook_tests {
    use super::super::{apply_pre_tool_hook_decision, PreToolHookResolution};
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

    /// The `Notification` type string a tool-approval prompt emits is the
    /// Claude Code one, so a plugin can match it by name (#3005).
    #[test]
    fn permission_prompt_is_the_claude_code_notification_type() {
        use super::super::approval::NOTIFICATION_PERMISSION_PROMPT;
        assert_eq!(
            NOTIFICATION_PERMISSION_PROMPT, "permission_prompt",
            "the emitted type string must stay the Claude Code value"
        );
        // Matchers are regexes, so an anchored pattern reaches it too.
        assert!(crate::services::hooks::matcher_matches(
            Some("permission_prompt"),
            NOTIFICATION_PERMISSION_PROMPT
        ));
        assert!(crate::services::hooks::matcher_matches(
            Some("^permission_.*$"),
            NOTIFICATION_PERMISSION_PROMPT
        ));
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

mod hook_input_rewrite_tests {
    use super::super::approval::{build_pending_approval, ToolCallRef};
    use super::super::run::{
        apply_hook_input_rewrite, EffectiveCall, HookInputRewrite, PreExecutionGuards,
    };
    use crate::agent::events::AgentEvent;
    use crate::agent::state::{AgentSession, PendingApprovalKind};
    use crate::agent::tool_approvals::{ToolApprovalRequest, ToolExecutionPolicyDecision};
    use crate::services::hooks::{HookDecision, HookDispatchResult};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::{oneshot, RwLock};

    fn empty_sessions() -> Arc<RwLock<HashMap<String, AgentSession>>> {
        Arc::new(RwLock::new(HashMap::new()))
    }

    fn allow_rewrite(updated: serde_json::Value) -> HookDispatchResult {
        HookDispatchResult {
            decision: Some(HookDecision::Allow),
            updated_input: Some(updated),
            ..HookDispatchResult::default()
        }
    }

    fn original_guards() -> PreExecutionGuards {
        PreExecutionGuards {
            policy_decision: ToolExecutionPolicyDecision::Allow,
            refusal: None,
        }
    }

    /// #3005 (first acceptance criterion, gatekeeper G1.5): the rewrite from
    /// `updatedInput` is the argument object the tool call runs with — same
    /// production step `execute_tool_calls` uses, and the same JSON string
    /// the approval and history paths carry.
    #[tokio::test]
    async fn rewritten_arguments_are_what_the_tool_receives() {
        let outcome = allow_rewrite(json!({"path": "rewritten.txt", "offset": 3}));
        let original = json!({"path": "original.txt"});

        let rewrite = apply_hook_input_rewrite(
            &empty_sessions(),
            "sess-rewrite",
            false,
            "workspace__readFile",
            &outcome,
            &original.to_string(),
            original_guards(),
        )
        .await;

        let EffectiveCall {
            args, args_json, ..
        } = match rewrite {
            HookInputRewrite::Applied(effective) => effective,
            other => panic!(
                "expected Applied, got a {} variant",
                match other {
                    HookInputRewrite::Applied(_) => "Applied",
                    HookInputRewrite::Unchanged(_) => "Unchanged",
                    HookInputRewrite::Refused(_) => "Refused",
                }
            ),
        };
        assert_eq!(args, json!({"path": "rewritten.txt", "offset": 3}));
        assert_eq!(
            args_json,
            json!({"path": "rewritten.txt", "offset": 3}).to_string()
        );
    }

    /// A rewrite that replaces the call with a destructive shell command is
    /// refused by the same policy guard that refuses the model's own
    /// `rm -rf /` — the hook cannot walk the command around it (G1.5).
    #[tokio::test]
    async fn rewrite_into_a_blocked_command_is_refused() {
        let outcome = allow_rewrite(json!({"command": "rm -rf /"}));

        let rewrite = apply_hook_input_rewrite(
            &empty_sessions(),
            "sess-rewrite-block",
            false,
            "workspace__runShell",
            &outcome,
            r#"{"command":"echo safe"}"#,
            original_guards(),
        )
        .await;

        assert!(
            matches!(rewrite, HookInputRewrite::Refused(_)),
            "rewriting runShell into a blocked command must be refused"
        );
    }

    /// A rewrite that asks for root execution is refused for a host session
    /// (G1.5), exactly like a model-originated root call.
    #[tokio::test]
    async fn rewrite_into_root_execution_is_refused() {
        let outcome = allow_rewrite(json!({"command": "id", "asRoot": true}));

        let rewrite = apply_hook_input_rewrite(
            &empty_sessions(),
            "sess-rewrite-root",
            false,
            "workspace__runShell",
            &outcome,
            r#"{"command":"id"}"#,
            original_guards(),
        )
        .await;

        assert!(
            matches!(rewrite, HookInputRewrite::Refused(_)),
            "rewriting into an asRoot call on a host session must be refused"
        );
    }

    /// A `Block` decision ignores any rewrite: the call never runs, and the
    /// original arguments and guards stand.
    #[tokio::test]
    async fn block_decision_drops_the_rewrite() {
        let outcome = HookDispatchResult {
            decision: Some(HookDecision::Block),
            reason: Some("denied".to_string()),
            updated_input: Some(json!({"path": "evil.txt"})),
            ..HookDispatchResult::default()
        };

        let rewrite = apply_hook_input_rewrite(
            &empty_sessions(),
            "sess-rewrite-blocked-decision",
            false,
            "workspace__readFile",
            &outcome,
            r#"{"path":"original.txt"}"#,
            original_guards(),
        )
        .await;

        assert!(matches!(rewrite, HookInputRewrite::Unchanged(_)));
    }

    /// The approval card shows the rewritten arguments, not the model's:
    /// the applied `args_json` is what the loop hands to the pending-approval
    /// record and event (#3005, gatekeeper G1.5).
    #[tokio::test]
    async fn approval_request_shows_the_rewritten_arguments() {
        let outcome = allow_rewrite(json!({"command": "echo rewritten"}));
        let rewrite = apply_hook_input_rewrite(
            &empty_sessions(),
            "sess-rewrite-approval",
            false,
            "workspace__runShell",
            &outcome,
            r#"{"command":"echo original"}"#,
            original_guards(),
        )
        .await;
        let HookInputRewrite::Applied(effective) = rewrite else {
            panic!("expected the rewrite to apply");
        };

        let request = ToolApprovalRequest {
            description: "run the thing".to_string(),
            input_preview: "preview".to_string(),
            masked_arguments: None,
        };
        let (tx, _rx) = oneshot::channel();
        let (stored, event) = build_pending_approval(
            &ToolCallRef {
                session_id: "sess-rewrite-approval",
                tool_call_id: "tc-rewrite",
                tool_name: "workspace__runShell",
            },
            &effective.args_json,
            &request,
            PendingApprovalKind::Standard,
            tx,
            None,
        );

        assert!(
            stored.arguments.contains("rewritten"),
            "{:?}",
            stored.arguments
        );
        assert!(
            !stored.arguments.contains("original"),
            "{:?}",
            stored.arguments
        );
        let AgentEvent::ToolExecutionRequiresApproval { arguments, .. } = &event else {
            panic!("expected a ToolExecutionRequiresApproval event");
        };
        assert!(arguments.contains("rewritten"), "{arguments}");
        assert!(!arguments.contains("original"), "{arguments}");
    }
}
