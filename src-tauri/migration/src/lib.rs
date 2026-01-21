pub use sea_orm_migration::prelude::*;

mod m20260104_000001_create_planning_tables;
mod m20260105_000001_create_remaining_tables;
mod m20260106_000001_create_sessions_table;
mod m20260106_000002_create_messages_tables;
mod m20260109_000001_add_source_to_knowledge;
mod m20260112_000001_create_settings_table;
mod m20260116_000001_add_assistant_id_to_playbooks;
mod m20260121_000001_add_bookmark_to_playbooks;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260104_000001_create_planning_tables::Migration),
            Box::new(m20260105_000001_create_remaining_tables::Migration),
            Box::new(m20260106_000001_create_sessions_table::Migration),
            Box::new(m20260106_000002_create_messages_tables::Migration),
            Box::new(m20260109_000001_add_source_to_knowledge::Migration),
            Box::new(m20260112_000001_create_settings_table::Migration),
            Box::new(m20260116_000001_add_assistant_id_to_playbooks::Migration),
            Box::new(m20260121_000001_add_bookmark_to_playbooks::Migration),
        ]
    }
}
