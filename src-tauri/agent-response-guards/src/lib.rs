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

pub fn is_internal_ui_callback_tool_name(tool_name: &str) -> bool {
    matches!(tool_name, "ui__getUserAnswer" | "ui__resumeCircuitBreak")
}

pub fn should_skip_expected_response_id_for_internal_ui_callback(
    status: GuardSessionStatus,
    tool_names: &[&str],
) -> bool {
    !tool_names.is_empty()
        && matches!(
            status,
            GuardSessionStatus::Idle | GuardSessionStatus::Paused
        )
        && tool_names
            .iter()
            .all(|tool_name| is_internal_ui_callback_tool_name(tool_name))
}

pub fn inspect_response_admission(
    status: GuardSessionStatus,
    token_cancelled: bool,
    cancel_pending: bool,
    allow_idle_tool_entry: bool,
    is_ui_tool: bool,
    is_internal_ui_callback: bool,
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

    if matches!(
        status,
        GuardSessionStatus::Idle | GuardSessionStatus::Paused
    ) && allow_idle_tool_entry
        && is_internal_ui_callback
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
    fn internal_ui_callbacks_skip_expected_response_id_when_session_is_idle_or_paused() {
        assert!(should_skip_expected_response_id_for_internal_ui_callback(
            GuardSessionStatus::Idle,
            &["ui__getUserAnswer"]
        ));
        assert!(should_skip_expected_response_id_for_internal_ui_callback(
            GuardSessionStatus::Paused,
            &["ui__resumeCircuitBreak"]
        ));
    }

    #[test]
    fn non_callback_ui_tools_do_not_skip_expected_response_id() {
        assert!(!should_skip_expected_response_id_for_internal_ui_callback(
            GuardSessionStatus::Idle,
            &["ui__presentInteractive"]
        ));
        assert!(!should_skip_expected_response_id_for_internal_ui_callback(
            GuardSessionStatus::Busy,
            &["ui__getUserAnswer"]
        ));
    }

    #[test]
    fn admission_allows_internal_callbacks_to_restart_from_idle_or_paused() {
        let idle =
            inspect_response_admission(GuardSessionStatus::Idle, false, false, true, true, true)
                .expect("idle callback should be admitted");
        assert_eq!(
            idle,
            ResponseAdmissionDecision {
                should_mark_busy: true,
                skip_expected_response_id_check: true,
            }
        );

        let paused =
            inspect_response_admission(GuardSessionStatus::Paused, false, false, true, true, true)
                .expect("paused callback should be admitted");
        assert_eq!(
            paused,
            ResponseAdmissionDecision {
                should_mark_busy: true,
                skip_expected_response_id_check: true,
            }
        );
    }

    #[test]
    fn admission_rejects_non_callback_ui_tools_when_not_busy() {
        let error =
            inspect_response_admission(GuardSessionStatus::Idle, false, false, true, true, false)
                .expect_err("non-callback ui tool should be orphaned");

        assert_eq!(error, ORPHANED_UI_TOOL_RESULT_ERROR);
    }

    #[test]
    fn admission_rejects_cancelled_non_ui_responses_as_workflow_cancelled() {
        let error =
            inspect_response_admission(GuardSessionStatus::Busy, true, false, false, false, false)
                .expect_err("cancelled non-ui response should be rejected");

        assert_eq!(error, WORKFLOW_CANCELLED_ERROR);
    }
}
