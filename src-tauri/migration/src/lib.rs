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
        ]
    }
}
