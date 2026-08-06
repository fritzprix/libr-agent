//! Windows-safe coverage for session id display aliases, reverse lookup, and spawn ids.
//! (Standalone binary — does not pull AppHandle/WebView into the link.)
//! Org/active-session prompt composition lives in `agent_org_service_context_tests.rs`.

use regex::Regex;
use tauri_mcp_agent_lib::execution_mode::ExecutionMode;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{
    format_active_sessions_notice, SessionMetadata, SessionStatus,
};
use tauri_mcp_agent_lib::services::agent_service::spawn::generate_spawn_session_id;
use tauri_mcp_agent_lib::utils::session_id::{
    display_session_id, reject_display_token_used_as_storage_key, resolve_session_id_among,
    session_id_matches_ref, session_id_short_token, SessionIdResolve, StorageSessionId,
};

fn sample_session(id: &str, name: &str, status: SessionStatus) -> SessionMetadata {
    SessionMetadata {
        id: id.to_string(),
        name: Some(name.to_string()),
        status,
        model: "gpt-test".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: Some("parent01".to_string()),
        lineage_id: Some("lineage-1".to_string()),
        depth: Some(1),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
    }
}

#[test]
fn display_uses_short_token_without_session_prefix() {
    assert_eq!(display_session_id("a1b2c3d4e5"), "a1b2c3d4e5");
    assert_eq!(
        display_session_id("session-1735123456789012345"),
        "6789012345"
    );
    assert_eq!(display_session_id("session-a1b2c3d4e5"), "a1b2c3d4e5");
    assert_eq!(display_session_id("6789012345"), "6789012345");
    assert_eq!(session_id_short_token("child-paused"), "ild-paused");
}

#[test]
fn matches_accepts_stored_prefixed_and_bare_token() {
    let legacy = "session-1735123456789012345";
    assert!(session_id_matches_ref(legacy, legacy));
    assert!(session_id_matches_ref(legacy, "session-6789012345"));
    assert!(session_id_matches_ref(legacy, "6789012345"));

    let modern = "a1b2c3d4e5";
    assert!(session_id_matches_ref(modern, "session-a1b2c3d4e5"));
    assert!(session_id_matches_ref(modern, "a1b2c3d4e5"));
}

#[test]
fn resolve_prefers_exact_and_maps_aliases() {
    let ids = [
        "session-1735123456789012345",
        "a1b2c3d4e5",
        "session-deadbeef01",
    ];

    assert_eq!(
        resolve_session_id_among(ids, "a1b2c3d4e5"),
        SessionIdResolve::Unique("a1b2c3d4e5")
    );
    assert_eq!(
        resolve_session_id_among(ids, "session-a1b2c3d4e5"),
        SessionIdResolve::Unique("a1b2c3d4e5")
    );
    assert_eq!(
        resolve_session_id_among(ids, "6789012345"),
        SessionIdResolve::Unique("session-1735123456789012345")
    );
    assert_eq!(
        resolve_session_id_among(ids, "session-6789012345"),
        SessionIdResolve::Unique("session-1735123456789012345")
    );
    assert_eq!(
        resolve_session_id_among(ids, "session-1735123456789012345"),
        SessionIdResolve::Unique("session-1735123456789012345")
    );
}

#[test]
fn resolve_returns_ambiguous_or_missing() {
    let ambiguous = ["session-xxxx6789012345", "yyyyyy6789012345"];
    assert_eq!(
        resolve_session_id_among(ambiguous, "6789012345"),
        SessionIdResolve::Ambiguous(2)
    );
    assert_eq!(
        resolve_session_id_among(ambiguous, "session-6789012345"),
        SessionIdResolve::Ambiguous(2)
    );

    let none = ["session-deadbeef01", "a1b2c3d4e5"];
    assert_eq!(
        resolve_session_id_among(none, "missing01xx"),
        SessionIdResolve::Missing
    );
}

#[test]
fn generate_spawn_session_id_uses_short_hex() {
    let pattern = Regex::new(r"^[0-9a-f]{10}$").expect("valid regex");

    for _ in 0..20 {
        let session_id = generate_spawn_session_id();
        assert_eq!(
            session_id.len(),
            10,
            "expected 10-char session id, got {session_id}"
        );
        assert!(
            pattern.is_match(&session_id),
            "session id must be 10 hex chars, got {session_id}"
        );
        assert_eq!(display_session_id(&session_id), session_id);
    }
}

#[test]
fn active_sessions_notice_uses_short_tokens_for_legacy_and_short_ids() {
    // Regression: take(8) on legacy session-* produced a useless `session-` label.
    // Both legacy and bare-hex stored ids must render as the same short token.
    let legacy = sample_session("session-a1b2c3d4e5", "Legacy Worker", SessionStatus::Idle);
    let modern = sample_session("a1b2c3d4e5", "Short Worker", SessionStatus::Paused);

    let notice = format_active_sessions_notice(&[legacy, modern]).expect("notice should render");

    assert_eq!(
        notice.matches("`a1b2c3d4e5`").count(),
        2,
        "legacy and modern stored ids should both display as a1b2c3d4e5"
    );
    assert!(!notice.contains("`session-a1b2c3d4e5`"));
    assert!(!notice.contains("`session-` (name:"));
}

#[test]
fn http_style_resolve_among_all_candidates_accepts_short_token() {
    // Mirrors resolve_http_session_ref candidate scope (all known sessions).
    let candidates = [
        "session-1111111111111111111",
        "abcdef1234",
        "session-zzzzzzzzzz",
    ];
    assert_eq!(
        resolve_session_id_among(candidates, "1111111111"),
        SessionIdResolve::Unique("session-1111111111111111111")
    );
    assert_eq!(
        resolve_session_id_among(candidates, "abcdef1234"),
        SessionIdResolve::Unique("abcdef1234")
    );
}

/// Regression: #1689 checkSession passed `display_session_id` into message fetch.
/// Legacy storage keys have a different string than their display token — using
/// the display token as a DB key returns empty history → "No final answer yet."
#[test]
fn reject_display_token_as_storage_key_for_legacy_session() {
    let storage = "session-376d7c7c-aaaa-bbbb-cccc-dddddddd5ed777";
    let display = display_session_id(storage);
    assert_ne!(
        storage, display,
        "legacy ids must differ from display tokens (otherwise this test is vacuous)"
    );
    assert_eq!(display.len(), 10);

    let err = reject_display_token_used_as_storage_key(&display, &[storage])
        .expect_err("display token must not be accepted as storage key");
    assert!(
        err.contains("display token"),
        "error should name the footgun: {err}"
    );
    assert!(
        err.contains(storage),
        "error should point at the real storage id: {err}"
    );

    reject_display_token_used_as_storage_key(storage, &[storage])
        .expect("exact storage id must be accepted");

    // Modern spawn ids: display == storage, so display_session_id is fine for lookup.
    let modern = "a1b2c3d4e5";
    assert_eq!(display_session_id(modern), modern);
    reject_display_token_used_as_storage_key(modern, &[modern]).expect("modern id ok");

    // StorageSessionId is the typed wrapper callers must use for message fetch.
    let typed = StorageSessionId::from_resolved(storage);
    assert_eq!(typed.as_str(), storage);
}
