use tauri_mcp_agent_lib::agent::channel_routing::{
    resolve_auto_routed_channel_target, ChannelRouteCandidate,
};

#[test]
fn resolve_auto_routed_channel_target_requires_at_least_one_candidate() {
    let error = resolve_auto_routed_channel_target("slack", vec![])
        .expect_err("missing candidates should fail");

    assert_eq!(
        error,
        "No active session is currently connected to channel server 'slack'"
    );
}

#[test]
fn resolve_auto_routed_channel_target_returns_single_candidate() {
    let candidate = resolve_auto_routed_channel_target(
        "slack",
        vec![ChannelRouteCandidate {
            session_id: "session-1".to_string(),
            session_name: "Daily triage".to_string(),
            parent_session_id: None,
        }],
    )
    .expect("single candidate should resolve");

    assert_eq!(candidate.session_id, "session-1");
    assert_eq!(candidate.session_name, "Daily triage");
}

#[test]
fn resolve_auto_routed_channel_target_rejects_ambiguous_candidates() {
    let error = resolve_auto_routed_channel_target(
        "slack",
        vec![
            ChannelRouteCandidate {
                session_id: "session-2".to_string(),
                session_name: "Zeta".to_string(),
                parent_session_id: None,
            },
            ChannelRouteCandidate {
                session_id: "session-1".to_string(),
                session_name: "Alpha".to_string(),
                parent_session_id: None,
            },
        ],
    )
    .expect_err("ambiguous candidates should fail");

    assert_eq!(
        error,
        "Ambiguous active sessions for channel server 'slack': Alpha (session-1), Zeta (session-2). Use the session-scoped channel endpoint to target a specific session."
    );
}

#[test]
fn resolve_auto_routed_channel_target_stays_ambiguous_even_with_top_level_parent() {
    let error = resolve_auto_routed_channel_target(
        "slack",
        vec![
            ChannelRouteCandidate {
                session_id: "parent-session".to_string(),
                session_name: "Parent".to_string(),
                parent_session_id: None,
            },
            ChannelRouteCandidate {
                session_id: "child-session".to_string(),
                session_name: "Child".to_string(),
                parent_session_id: Some("parent-session".to_string()),
            },
        ],
    )
    .expect_err("parent presence should not silently override ambiguity");

    assert_eq!(
        error,
        "Ambiguous active sessions for channel server 'slack': Child (child-session), Parent (parent-session). Use the session-scoped channel endpoint to target a specific session."
    );
}
