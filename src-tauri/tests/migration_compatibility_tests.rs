use migration::MigratorTrait;
use tauri_mcp_agent_lib::migration::Migrator;

#[test]
fn includes_legacy_stores_session_index_migration() {
    let migration_names: Vec<String> = Migrator::migrations()
        .iter()
        .map(|migration| migration.name().to_string())
        .collect();

    assert!(
        migration_names.contains(&"m20260327_000025_add_stores_session_index".to_string()),
        "Migrator must retain shipped migration versions for existing user databases"
    );
}

#[test]
fn includes_gemini_context_cache_compatibility_migration() {
    let migration_names: Vec<String> = Migrator::migrations()
        .iter()
        .map(|migration| migration.name().to_string())
        .collect();

    assert!(
        migration_names.contains(&"m20260406_000028_create_gemini_context_caches".to_string()),
        "Migrator must retain migration versions that were already executed in development databases"
    );
}
