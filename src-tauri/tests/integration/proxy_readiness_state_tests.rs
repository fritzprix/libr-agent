use tauri_mcp_agent_lib::mcp::service_proxy_manager::{
    decide_proxy_readiness_state, ProxyReadinessState,
};

#[test]
fn missing_proxy_is_not_ready() {
    assert_eq!(
        decide_proxy_readiness_state(false, false, false),
        ProxyReadinessState::MissingProxy,
        "a missing proxy must not be treated as a ready builtin-only session"
    );
}

#[test]
fn existing_builtin_only_proxy_is_ready() {
    assert_eq!(
        decide_proxy_readiness_state(true, false, false),
        ProxyReadinessState::Ready,
        "an existing proxy with no readiness signal is the builtin-only ready case"
    );
}

#[test]
fn existing_proxy_with_signal_waits_for_background_loading() {
    assert_eq!(
        decide_proxy_readiness_state(true, true, false),
        ProxyReadinessState::AwaitSignal,
        "sessions with a readiness signal must wait for external MCP discovery to finish"
    );
}

#[test]
fn runtime_ready_proxy_ignores_stale_readiness_signal() {
    assert_eq!(
        decide_proxy_readiness_state(true, true, true),
        ProxyReadinessState::Ready,
        "a runtime-ready proxy must not keep waiting on a stale readiness entry"
    );
}

#[test]
fn proxy_readiness_truth_table_remains_stable() {
    let cases = [
        ((false, false, false), ProxyReadinessState::MissingProxy),
        ((false, false, true), ProxyReadinessState::MissingProxy),
        ((false, true, false), ProxyReadinessState::MissingProxy),
        ((false, true, true), ProxyReadinessState::MissingProxy),
        ((true, false, false), ProxyReadinessState::Ready),
        ((true, false, true), ProxyReadinessState::Ready),
        ((true, true, false), ProxyReadinessState::AwaitSignal),
        ((true, true, true), ProxyReadinessState::Ready),
    ];

    for ((proxy_exists, has_readiness_signal, runtime_ready), expected) in cases {
        assert_eq!(
            decide_proxy_readiness_state(proxy_exists, has_readiness_signal, runtime_ready),
            expected,
            "unexpected readiness state for proxy_exists={proxy_exists}, has_readiness_signal={has_readiness_signal}, runtime_ready={runtime_ready}"
        );
    }
}
