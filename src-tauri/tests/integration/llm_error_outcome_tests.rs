use tauri_mcp_agent_lib::agent::llm::{
    completion_result_from_error_handling_outcome, AgentRuntimeError, AgentRuntimeErrorType,
    LlmErrorHandlingOutcome,
};

#[test]
fn recovered_compaction_outcome_keeps_request_flow_successful() {
    let error = AgentRuntimeError::new(
        AgentRuntimeErrorType::ContextLimitError,
        "Context limit reached",
    );

    let result = completion_result_from_error_handling_outcome(
        LlmErrorHandlingOutcome::RecoveredByCompaction,
        error,
    );

    assert!(result.is_ok());
}

#[test]
fn finalized_workflow_error_outcome_propagates_original_failure() {
    let error = AgentRuntimeError::new(
        AgentRuntimeErrorType::AiServiceError,
        "Provider request failed",
    );

    let result = completion_result_from_error_handling_outcome(
        LlmErrorHandlingOutcome::FinalizedWorkflowError,
        error,
    );

    assert_eq!(result, Err("Provider request failed".to_string()));
}
