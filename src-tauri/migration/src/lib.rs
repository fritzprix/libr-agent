pub use sea_orm_migration::prelude::*;

pub mod helpers; // Shared migration utilities (idempotency helpers, etc.)

mod m20260206_000001_create_all_tables;
mod m20260208_000002_add_llm_fields_to_sessions;
mod m20260211_000003_create_message_index_meta;
mod m20260212_000004_add_indexes;
mod m20260214_000005_add_lineage_fields_to_sessions;
mod m20260214_000006_add_max_fanout_to_sessions;
mod m20260214_000007_create_knowledge_fts;
mod m20260215_000008_add_cascade_delete_to_sessions;
mod m20260217_000009_ensure_data_integrity;
mod m20260218_000010_add_migration_metadata;
mod m20260301_000011_add_bookmark_to_sessions;
mod m20260302_000012_create_scheduled_tasks;
mod m20260303_000013_add_cached_tools_to_mcp_servers;
mod m20260306_000014_add_yolo_mode_to_sessions;
mod m20260307_000015_add_compact_context;
mod m20260309_000016_add_workspace_override_to_sessions;
mod m20260309_000017_add_usage_to_messages;
mod m20260317_000018_add_yolo_mode_to_scheduled_tasks;
mod m20260317_000019_add_workspace_override_to_scheduled_tasks;
mod m20260320_000020_add_viewed_and_message_timestamps_to_sessions;
mod m20260320_000021_add_attention_timestamps_to_sessions;
mod m20260321_000022_add_schedule_timezone_to_scheduled_tasks;
mod m20260322_000023_add_mcp_server_verification_fields;
mod m20260326_000024_create_knowledge_v2;
mod m20260327_000025_add_stores_session_index;
mod m20260405_000026_add_group_fields_to_scheduled_tasks;
mod m20260405_000027_add_org_fields_to_sessions;
mod m20260406_000028_create_gemini_context_caches;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260206_000001_create_all_tables::Migration),
            Box::new(m20260208_000002_add_llm_fields_to_sessions::Migration),
            Box::new(m20260211_000003_create_message_index_meta::Migration),
            Box::new(m20260212_000004_add_indexes::Migration),
            Box::new(m20260214_000005_add_lineage_fields_to_sessions::Migration),
            Box::new(m20260214_000006_add_max_fanout_to_sessions::Migration),
            Box::new(m20260214_000007_create_knowledge_fts::Migration),
            Box::new(m20260215_000008_add_cascade_delete_to_sessions::Migration),
            Box::new(m20260217_000009_ensure_data_integrity::Migration),
            Box::new(m20260218_000010_add_migration_metadata::Migration),
            Box::new(m20260301_000011_add_bookmark_to_sessions::Migration),
            Box::new(m20260302_000012_create_scheduled_tasks::Migration),
            Box::new(m20260303_000013_add_cached_tools_to_mcp_servers::Migration),
            Box::new(m20260306_000014_add_yolo_mode_to_sessions::Migration),
            Box::new(m20260307_000015_add_compact_context::Migration),
            Box::new(m20260309_000016_add_workspace_override_to_sessions::Migration),
            Box::new(m20260309_000017_add_usage_to_messages::Migration),
            Box::new(m20260317_000018_add_yolo_mode_to_scheduled_tasks::Migration),
            Box::new(m20260317_000019_add_workspace_override_to_scheduled_tasks::Migration),
            Box::new(m20260320_000020_add_viewed_and_message_timestamps_to_sessions::Migration),
            Box::new(m20260320_000021_add_attention_timestamps_to_sessions::Migration),
            Box::new(m20260321_000022_add_schedule_timezone_to_scheduled_tasks::Migration),
            Box::new(m20260322_000023_add_mcp_server_verification_fields::Migration),
            Box::new(m20260326_000024_create_knowledge_v2::Migration),
            Box::new(m20260327_000025_add_stores_session_index::Migration),
            Box::new(m20260405_000026_add_group_fields_to_scheduled_tasks::Migration),
            Box::new(m20260405_000027_add_org_fields_to_sessions::Migration),
            Box::new(m20260406_000028_create_gemini_context_caches::Migration),
        ]
    }
}
