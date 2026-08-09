#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardSessionStatus {
    Idle,
    Busy,
    Paused,
    Other,
}

pub const ORPHANED_UI_TOOL_RESULT_ERROR: &str = "UI tool result orphaned (workflow inactive)";
pub const WORKFLOW_CANCELLED_ERROR: &str = "Workflow was cancelled";
pub const LLM_RESPONSE_SUPERSEDED_ERROR: &str = "LLM response superseded";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseAdmissionDecision {
    pub should_mark_busy: bool,
    pub skip_expected_response_id_check: bool,
}

/// Builtin UI callback tools that HTML templates post back into the agent loop.
/// Kept for callers that need to identify these tools; admission no longer
/// requires this allowlist — any non-empty tool_calls can restart Idle/Paused.
pub fn is_internal_ui_callback_tool_name(tool_name: &str) -> bool {
    matches!(tool_name, "ui__getUserAnswer" | "ui__resumeCircuitBreak")
}

pub fn should_skip_expected_response_id_for_idle_tool_entry(
    status: GuardSessionStatus,
    has_tool_calls: bool,
) -> bool {
    has_tool_calls
        && matches!(
            status,
            GuardSessionStatus::Idle | GuardSessionStatus::Paused
        )
}

pub fn inspect_response_admission(
    status: GuardSessionStatus,
    token_cancelled: bool,
    cancel_pending: bool,
    allow_idle_tool_entry: bool,
    is_ui_tool: bool,
) -> Result<ResponseAdmissionDecision, &'static str> {
    if token_cancelled || cancel_pending {
        return if is_ui_tool {
            Err(ORPHANED_UI_TOOL_RESULT_ERROR)
        } else {
            Err(WORKFLOW_CANCELLED_ERROR)
        };
    }

    if status == GuardSessionStatus::Busy {
        return Ok(ResponseAdmissionDecision {
            should_mark_busy: false,
            skip_expected_response_id_check: false,
        });
    }

    // UI resource actions (and other frontend-injected tool calls) arrive after
    // the workflow settled to Idle/Paused. Admit them so tool execution can
    // restart the loop. Cancelled tokens still reject above.
    if matches!(
        status,
        GuardSessionStatus::Idle | GuardSessionStatus::Paused
    ) && allow_idle_tool_entry
    {
        return Ok(ResponseAdmissionDecision {
            should_mark_busy: true,
            skip_expected_response_id_check: true,
        });
    }

    if is_ui_tool {
        return Err(ORPHANED_UI_TOOL_RESULT_ERROR);
    }

    Err(WORKFLOW_CANCELLED_ERROR)
}

pub fn validate_expected_response_id(
    expected_response_id: Option<&str>,
    received_message_id: &str,
    is_ui_tool: bool,
) -> Result<(), &'static str> {
    let Some(expected_response_id) = expected_response_id else {
        return if is_ui_tool {
            Err(ORPHANED_UI_TOOL_RESULT_ERROR)
        } else {
            Err(LLM_RESPONSE_SUPERSEDED_ERROR)
        };
    };

    if received_message_id != expected_response_id {
        return if is_ui_tool {
            Err(ORPHANED_UI_TOOL_RESULT_ERROR)
        } else {
            Err(LLM_RESPONSE_SUPERSEDED_ERROR)
        };
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_response_when_expected_response_id_is_missing() {
        let error = validate_expected_response_id(None, "response-1", false)
            .expect_err("missing expected response id should reject stray responses");

        assert_eq!(error, LLM_RESPONSE_SUPERSEDED_ERROR);
    }

    #[test]
    fn rejects_response_when_expected_response_id_mismatches() {
        let error = validate_expected_response_id(Some("response-1"), "response-2", false)
            .expect_err("mismatched response id should reject superseded responses");

        assert_eq!(error, LLM_RESPONSE_SUPERSEDED_ERROR);
    }

    #[test]
    fn accepts_response_when_expected_response_id_matches() {
        validate_expected_response_id(Some("response-1"), "response-1", false)
            .expect("matching response id should be accepted");
    }

    #[test]
    fn rejects_missing_expected_response_id_for_ui_tool_as_orphaned_result() {
        let error = validate_expected_response_id(None, "response-1", true)
            .expect_err("missing expected response id should reject stale UI tool results");

        assert_eq!(error, ORPHANED_UI_TOOL_RESULT_ERROR);
    }

    #[test]
    fn rejects_mismatched_expected_response_id_for_ui_tool_as_orphaned_result() {
        let error = validate_expected_response_id(Some("response-1"), "response-2", true)
            .expect_err("mismatched response id should reject stale UI tool results");

        assert_eq!(error, ORPHANED_UI_TOOL_RESULT_ERROR);
    }

    #[test]
    fn idle_or_paused_tool_entry_skips_expected_response_id() {
        assert!(should_skip_expected_response_id_for_idle_tool_entry(
            GuardSessionStatus::Idle,
            true
        ));
        assert!(should_skip_expected_response_id_for_idle_tool_entry(
            GuardSessionStatus::Paused,
            true
        ));
        assert!(!should_skip_expected_response_id_for_idle_tool_entry(
            GuardSessionStatus::Busy,
            true
        ));
        assert!(!should_skip_expected_response_id_for_idle_tool_entry(
            GuardSessionStatus::Idle,
            false
        ));
    }

    #[test]
    fn admission_allows_tool_entry_to_restart_from_idle_or_paused() {
        let idle_ui =
            inspect_response_admission(GuardSessionStatus::Idle, false, false, true, true)
                .expect("idle ui tool entry should be admitted");
        assert_eq!(
            idle_ui,
            ResponseAdmissionDecision {
                should_mark_busy: true,
                skip_expected_response_id_check: true,
            }
        );

        let paused_mcp =
            inspect_response_admission(GuardSessionStatus::Paused, false, false, true, false)
                .expect("paused mcp tool entry should be admitted");
        assert_eq!(
            paused_mcp,
            ResponseAdmissionDecision {
                should_mark_busy: true,
                skip_expected_response_id_check: true,
            }
        );
    }

    #[test]
    fn admission_rejects_idle_responses_without_tool_calls() {
        let error = inspect_response_admission(GuardSessionStatus::Idle, false, false, false, true)
            .expect_err("idle response without tool calls should be orphaned");

        assert_eq!(error, ORPHANED_UI_TOOL_RESULT_ERROR);

        let error =
            inspect_response_admission(GuardSessionStatus::Idle, false, false, false, false)
                .expect_err("idle non-ui response without tool calls should be cancelled");

        assert_eq!(error, WORKFLOW_CANCELLED_ERROR);
    }

    #[test]
    fn admission_rejects_cancelled_tool_entry_even_when_idle() {
        let error = inspect_response_admission(GuardSessionStatus::Idle, true, false, true, true)
            .expect_err("cancelled ui tool entry should stay orphaned");

        assert_eq!(error, ORPHANED_UI_TOOL_RESULT_ERROR);

        let error =
            inspect_response_admission(GuardSessionStatus::Paused, false, true, true, false)
                .expect_err("cancel_pending mcp tool entry should stay cancelled");

        assert_eq!(error, WORKFLOW_CANCELLED_ERROR);
    }

    #[test]
    fn admission_rejects_cancelled_non_ui_responses_as_workflow_cancelled() {
        let error = inspect_response_admission(GuardSessionStatus::Busy, true, false, false, false)
            .expect_err("cancelled non-ui response should be rejected");

        assert_eq!(error, WORKFLOW_CANCELLED_ERROR);
    }

    #[test]
    fn internal_ui_callback_tool_names_are_recognized() {
        assert!(is_internal_ui_callback_tool_name("ui__getUserAnswer"));
        assert!(is_internal_ui_callback_tool_name("ui__resumeCircuitBreak"));
        assert!(!is_internal_ui_callback_tool_name("ui__presentInteractive"));
        assert!(!is_internal_ui_callback_tool_name("workspace__export"));
    }
}
