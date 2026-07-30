use tauri_mcp_agent_lib::mcp::service_proxy_manager::{
    decide_proxy_readiness_state, ProxyReadinessState,
};

#[test]
fn missing_proxy_is_not_ready() {
    assert_eq!(
        decide_proxy_readiness_state(false, false),
        ProxyReadinessState::MissingProxy,
        "a missing proxy must not be treated as a ready builtin-only session"
    );
    assert_eq!(
        decide_proxy_readiness_state(false, true),
        ProxyReadinessState::MissingProxy,
        "a missing proxy must not be treated as ready even when a stale signal exists"
    );
}

#[test]
fn existing_builtin_only_proxy_is_ready() {
    assert_eq!(
        decide_proxy_readiness_state(true, false),
        ProxyReadinessState::Ready,
        "an existing proxy with no readiness signal is the builtin-only ready case"
    );
}

#[test]
fn existing_proxy_with_signal_waits_for_background_loading() {
    assert_eq!(
        decide_proxy_readiness_state(true, true),
        ProxyReadinessState::AwaitSignal,
        "sessions with a readiness signal must wait for external MCP discovery to finish"
    );
}

#[test]
fn proxy_readiness_truth_table_remains_stable() {
    let cases = [
        ((false, false), ProxyReadinessState::MissingProxy),
        ((false, true), ProxyReadinessState::MissingProxy),
        ((true, false), ProxyReadinessState::Ready),
        ((true, true), ProxyReadinessState::AwaitSignal),
    ];

    for ((proxy_exists, has_readiness_signal), expected) in cases {
        assert_eq!(
            decide_proxy_readiness_state(proxy_exists, has_readiness_signal),
            expected,
            "unexpected readiness state for proxy_exists={proxy_exists}, has_readiness_signal={has_readiness_signal}"
        );
    }
}
