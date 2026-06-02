use serde_json::json;
use tauri_mcp_agent_lib::browser_sidecar::{
    browser_runtime_profile_dir, browser_runtime_profile_root, classify_browser_page,
    serialize_browser_result_value, BrowserAutomationClient, PageClassification,
};
use tauri_mcp_agent_lib::services::interactive_browser_server::{
    BrowserSession, NavigationUpdateOutcome, SessionStatus,
};

#[test]
fn browser_session_runtime_ready_tracks_generation_match() {
    let ready_session = BrowserSession {
        id: "session-1".to_string(),
        url: "about:blank".to_string(),
        current_title: Some("Ready".to_string()),
        created_at: chrono::Utc::now(),
        status: SessionStatus::Active,
        page_generation: 3,
        runtime_ready_generation: Some(3),
    };

    let stale_session = BrowserSession {
        runtime_ready_generation: Some(2),
        ..ready_session.clone()
    };

    assert!(ready_session.is_runtime_ready());
    assert!(!stale_session.is_runtime_ready());
}

#[test]
fn browser_session_ignores_stale_navigation_updates() {
    let mut session = BrowserSession {
        id: "session-1".to_string(),
        url: "https://example.com/start".to_string(),
        current_title: Some("Start".to_string()),
        created_at: chrono::Utc::now(),
        status: SessionStatus::Active,
        page_generation: 1,
        runtime_ready_generation: Some(1),
    };

    let stale_generation = session.begin_navigation(Some("https://example.com/older".to_string()));
    let current_generation =
        session.begin_navigation(Some("https://example.com/current".to_string()));

    assert_eq!(stale_generation, 2);
    assert_eq!(current_generation, 3);
    assert_eq!(session.url, "https://example.com/current");
    assert!(!session.is_runtime_ready());

    assert_eq!(
        session.finish_navigation(
            stale_generation,
            "https://example.com/older",
            Some("Older".to_string()),
        ),
        NavigationUpdateOutcome::IgnoredStale
    );
    assert_eq!(session.page_generation, current_generation);
    assert_eq!(session.url, "https://example.com/current");
    assert!(matches!(session.status, SessionStatus::Creating));
    assert!(!session.is_runtime_ready());

    assert_eq!(
        session.fail_navigation(stale_generation, "stale failure".to_string()),
        NavigationUpdateOutcome::IgnoredStale
    );
    assert!(matches!(session.status, SessionStatus::Creating));
}

#[test]
fn browser_session_finalizes_current_generation_once() {
    let mut session = BrowserSession {
        id: "session-1".to_string(),
        url: "https://example.com/start".to_string(),
        current_title: Some("Start".to_string()),
        created_at: chrono::Utc::now(),
        status: SessionStatus::Active,
        page_generation: 1,
        runtime_ready_generation: Some(1),
    };

    let generation = session.begin_navigation(Some("https://example.com/next".to_string()));
    assert_eq!(
        session.finish_navigation(
            generation,
            "https://example.com/next",
            Some("Next".to_string())
        ),
        NavigationUpdateOutcome::Applied
    );
    assert!(matches!(session.status, SessionStatus::Active));
    assert!(session.is_runtime_ready());
    assert_eq!(session.current_title.as_deref(), Some("Next"));

    assert_eq!(
        session.fail_navigation(generation, "late failure".to_string()),
        NavigationUpdateOutcome::IgnoredSettled
    );
    assert!(matches!(session.status, SessionStatus::Active));
    assert!(session.is_runtime_ready());
}

#[test]
fn browser_session_rejects_future_navigation_updates() {
    let mut session = BrowserSession {
        id: "session-1".to_string(),
        url: "https://example.com/start".to_string(),
        current_title: Some("Start".to_string()),
        created_at: chrono::Utc::now(),
        status: SessionStatus::Active,
        page_generation: 4,
        runtime_ready_generation: Some(4),
    };

    assert_eq!(
        session.finish_navigation(5, "https://example.com/future", Some("Future".to_string())),
        NavigationUpdateOutcome::RejectedFuture
    );
    assert_eq!(
        session.fail_navigation(5, "future failure".to_string()),
        NavigationUpdateOutcome::RejectedFuture
    );
    assert_eq!(session.url, "https://example.com/start");
    assert_eq!(session.current_title.as_deref(), Some("Start"));
    assert!(matches!(session.status, SessionStatus::Active));
    assert!(session.is_runtime_ready());
}

#[test]
fn browser_result_serialization_matches_legacy_string_contract() {
    assert_eq!(
        serialize_browser_result_value(None).expect("undefined should serialize"),
        "undefined"
    );
    assert_eq!(
        serialize_browser_result_value(Some(serde_json::Value::Null))
            .expect("null should serialize"),
        "null"
    );
    let serialized_object = serialize_browser_result_value(Some(json!({"ok": true, "count": 2})))
        .expect("object should serialize");
    let reparsed: serde_json::Value =
        serde_json::from_str(&serialized_object).expect("serialized object should remain JSON");
    assert_eq!(reparsed, json!({"ok": true, "count": 2}));
}

#[test]
fn classify_browser_page_detects_google_sorry_interstitials() {
    let classification = classify_browser_page(
        "https://www.google.com/sorry/index?continue=https://example.com",
        "Google Search",
        "Our systems have detected unusual traffic from your computer network.",
    );

    assert_eq!(classification, PageClassification::BlockedInterstitial);
}

#[test]
fn classify_browser_page_leaves_normal_pages_alone() {
    let classification = classify_browser_page(
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
        "Rust (programming language) - Wikipedia",
        "Rust is a multi-paradigm, general-purpose programming language.",
    );

    assert_eq!(classification, PageClassification::Normal);
}

#[test]
fn browser_runtime_profile_dirs_are_unique_and_not_the_chromiumoxide_default() {
    let first = browser_runtime_profile_dir(uuid::Uuid::new_v4());
    let second = browser_runtime_profile_dir(uuid::Uuid::new_v4());
    let chromiumoxide_default = std::env::temp_dir().join("chromiumoxide-runner");
    let profile_root = browser_runtime_profile_root();

    assert_ne!(first, second);
    assert_ne!(first, chromiumoxide_default);
    assert_ne!(second, chromiumoxide_default);
    assert!(first.starts_with(&profile_root));
    assert!(second.starts_with(&profile_root));
}

#[test]
fn browser_automation_client_uses_a_longer_bootstrap_timeout() {
    let client = BrowserAutomationClient::new(std::time::Duration::from_secs(30));

    assert_eq!(client.request_timeout(), std::time::Duration::from_secs(30));
    assert_eq!(
        client.bootstrap_timeout(),
        std::time::Duration::from_secs(180)
    );
}
