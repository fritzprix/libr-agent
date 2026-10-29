use crate::common::setup_test_db_with_migrations;
use serde_json::json;
use tauri_mcp_agent_lib::models::advanced_settings::{AdvancedSettings, ThinkingEffort};
use tauri_mcp_agent_lib::repositories::settings_repository::{
    SettingsRepository, SqliteSettingsRepository,
};

async fn repo_with_value(value: serde_json::Value) -> SqliteSettingsRepository {
    let db = setup_test_db_with_migrations().await;
    let repo = SqliteSettingsRepository::new(db);
    repo.set("advancedSettings", value)
        .await
        .expect("set advancedSettings should succeed");
    repo
}

#[tokio::test]
async fn advanced_settings_load_returns_defaults_when_key_missing() {
    let db = setup_test_db_with_migrations().await;
    let repo = SqliteSettingsRepository::new(db);
    let settings = AdvancedSettings::load(&repo).await;
    assert_eq!(settings, AdvancedSettings::default());
}

#[tokio::test]
async fn advanced_settings_load_reads_stored_row() {
    let repo = repo_with_value(json!({
        "maxRetries": 5,
        "defaultSessionMaxDepth": 3,
        "thinkingEffort": "high",
    }))
    .await;
    let settings = AdvancedSettings::load(&repo).await;
    assert_eq!(settings.max_retries, 5);
    assert_eq!(settings.default_session_max_depth, 3);
    assert_eq!(settings.thinking_effort, ThinkingEffort::High);
    assert_eq!(settings.retry_delay, 5000); // default fallback
}

#[tokio::test]
async fn advanced_settings_load_handles_broken_json() {
    let db = setup_test_db_with_migrations().await;
    let repo = SqliteSettingsRepository::new(db);
    repo.set(
        "advancedSettings",
        serde_json::Value::String("{not json".to_string()),
    )
    .await
    .expect("set should succeed");
    let settings = AdvancedSettings::load(&repo).await;
    assert_eq!(settings, AdvancedSettings::default());
}

#[tokio::test]
fn advanced_settings_legacy_thinking_budget_maps_to_effort() {
    assert_eq!(
        AdvancedSettings::from_stored(Some(r#"{"thinkingBudget":0}"#)).thinking_effort,
        ThinkingEffort::Off
    );
    assert_eq!(
        AdvancedSettings::from_stored(Some(r#"{"thinkingBudget":-1}"#)).thinking_effort,
        ThinkingEffort::Auto
    );
    assert_eq!(
        AdvancedSettings::from_stored(Some(r#"{"thinkingBudget":2048}"#)).thinking_effort,
        ThinkingEffort::Low
    );
    assert_eq!(
        AdvancedSettings::from_stored(Some(r#"{"thinkingBudget":2049}"#)).thinking_effort,
        ThinkingEffort::Medium
    );
    assert_eq!(
        AdvancedSettings::from_stored(Some(r#"{"thinkingBudget":16384}"#)).thinking_effort,
        ThinkingEffort::Medium
    );
    assert_eq!(
        AdvancedSettings::from_stored(Some(r#"{"thinkingBudget":16385}"#)).thinking_effort,
        ThinkingEffort::High
    );
    assert_eq!(
        AdvancedSettings::from_stored(Some(r#"{"thinkingBudget":"big"}"#)).thinking_effort,
        ThinkingEffort::Off
    );
}

#[tokio::test]
fn advanced_settings_thinking_effort_wins_over_legacy_budget() {
    let settings =
        AdvancedSettings::from_stored(Some(r#"{"thinkingEffort":"low","thinkingBudget":99999}"#));
    assert_eq!(settings.thinking_effort, ThinkingEffort::Low);
}

#[tokio::test]
fn advanced_settings_thinking_effort_serializes_as_lowercase() {
    assert_eq!(
        serde_json::to_value(ThinkingEffort::Medium).unwrap(),
        serde_json::json!("medium")
    );
}
