use tauri_mcp_agent_lib::agent::llm::validate_expected_response_id;

#[test]
fn rejects_response_when_expected_response_id_is_missing() {
    let error = validate_expected_response_id("session-1", None, "response-1")
        .expect_err("missing expected response id should reject stray responses");

    assert_eq!(error, "LLM response superseded");
}

#[test]
fn rejects_response_when_expected_response_id_mismatches() {
    let error = validate_expected_response_id("session-1", Some("response-1"), "response-2")
        .expect_err("mismatched response id should reject superseded responses");

    assert_eq!(error, "LLM response superseded");
}

#[test]
fn accepts_response_when_expected_response_id_matches() {
    validate_expected_response_id("session-1", Some("response-1"), "response-1")
        .expect("matching response id should be accepted");
}
