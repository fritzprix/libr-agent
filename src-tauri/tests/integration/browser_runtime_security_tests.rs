use serde_json::json;
#[allow(deprecated)]
use tauri_mcp_agent_lib::browser_sidecar::{
    agent_sticky_user_data_dir, browser_runtime_profile_dir, browser_runtime_profile_root,
    classify_browser_page, clear_agent_sticky_profile_dir, serialize_browser_result_value,
    BrowserAutomationClient, PageClassification,
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
    #[allow(deprecated)]
    let first = browser_runtime_profile_dir(uuid::Uuid::new_v4());
    #[allow(deprecated)]
    let second = browser_runtime_profile_dir(uuid::Uuid::new_v4());
    let chromiumoxide_default = std::env::temp_dir().join("chromiumoxide-runner");
    #[allow(deprecated)]
    let profile_root = browser_runtime_profile_root();

    assert_ne!(first, second);
    assert_ne!(first, chromiumoxide_default);
    assert_ne!(second, chromiumoxide_default);
    assert!(first.starts_with(&profile_root));
    assert!(second.starts_with(&profile_root));
}

#[test]
fn agent_sticky_user_data_dir_is_stable_and_under_app_data() {
    let first = agent_sticky_user_data_dir().expect("sticky dir");
    let second = agent_sticky_user_data_dir().expect("sticky dir");
    let chromiumoxide_default = std::env::temp_dir().join("chromiumoxide-runner");

    assert_eq!(first, second);
    assert_ne!(first, chromiumoxide_default);
    assert!(
        first.file_name().and_then(|name| name.to_str()) == Some("browser_agent_profile"),
        "sticky profile must use fixed browser_agent_profile dir, got {}",
        first.display()
    );
    assert!(
        !first.starts_with(&{
            #[allow(deprecated)]
            {
                browser_runtime_profile_root()
            }
        }),
        "sticky profile must live in app data, not cache UUID profiles"
    );
}

/// #1984 contract: sticky profile is wiped only via explicit clear, never via UUID cache helpers.
#[test]
fn sticky_profile_is_not_the_legacy_uuid_cache_layout() {
    let sticky = agent_sticky_user_data_dir().expect("sticky dir");
    #[allow(deprecated)]
    let legacy_root = browser_runtime_profile_root();
    assert_ne!(sticky, legacy_root);
    assert!(!sticky.starts_with(&legacy_root));
    assert_eq!(
        sticky.file_name().and_then(|n| n.to_str()),
        Some("browser_agent_profile")
    );
}

#[test]
fn clear_agent_sticky_profile_dir_is_idempotent_when_missing() {
    // Uses the real sticky path for this process; safe when the dir does not exist.
    let dir = agent_sticky_user_data_dir().expect("sticky dir");
    if dir.exists() {
        // Do not wipe a developer's real sticky profile in unit tests.
        return;
    }
    clear_agent_sticky_profile_dir().expect("clear missing sticky dir");
    clear_agent_sticky_profile_dir().expect("clear again");
}

#[test]
fn browser_automation_client_uses_a_longer_bootstrap_timeout() {
    let client = BrowserAutomationClient::new(std::time::Duration::from_secs(30));

    assert_eq!(client.request_timeout(), std::time::Duration::from_secs(30));
    // request_timeout (30s) + launch buffer (30s) => 60s floor for createSession.
    assert_eq!(
        client.bootstrap_timeout(),
        std::time::Duration::from_secs(60)
    );
}

#[test]
fn browser_session_target_parse_accepts_known_values() {
    use tauri_mcp_agent_lib::services::BrowserSessionTarget;

    assert_eq!(
        BrowserSessionTarget::parse("sidecar").unwrap(),
        BrowserSessionTarget::Sidecar
    );
    assert_eq!(
        BrowserSessionTarget::parse("userChrome").unwrap(),
        BrowserSessionTarget::UserChrome
    );
    assert_eq!(
        BrowserSessionTarget::parse("  sidecar  ").unwrap(),
        BrowserSessionTarget::Sidecar
    );
}

#[test]
fn browser_session_target_parse_rejects_unknown_values() {
    use tauri_mcp_agent_lib::services::BrowserSessionTarget;

    let err = BrowserSessionTarget::parse("auto").unwrap_err();
    assert!(err.contains("Invalid browser value"));
    assert!(err.contains("sidecar"));
    assert!(err.contains("userChrome"));

    let err = BrowserSessionTarget::parse("extension").unwrap_err();
    assert!(err.contains("Invalid browser value"));
}

#[tokio::test]
async fn user_chrome_create_fails_when_extension_disconnected() {
    use std::time::Duration;
    use tauri_mcp_agent_lib::browser_extension_bridge;
    use tauri_mcp_agent_lib::services::{BrowserSessionTarget, InteractiveBrowserServer};

    if browser_extension_bridge::is_connected() {
        // Local machine already has everyday Chrome Connected — skip to avoid opening a real tab.
        return;
    }

    let server = InteractiveBrowserServer::new(Duration::from_secs(5));
    let err = server
        .create_browser_session(
            "https://example.com",
            None,
            false,
            BrowserSessionTarget::UserChrome,
        )
        .await
        .expect_err("userChrome must fail without Connected extension");

    assert!(
        err.contains("userChrome unavailable"),
        "expected unavailable error, got: {err}"
    );
    assert!(
        err.contains("no silent sidecar fallback"),
        "expected no-fallback wording, got: {err}"
    );
    assert!(
        err.contains("browser=\"sidecar\""),
        "expected sidecar retry guidance, got: {err}"
    );
}
