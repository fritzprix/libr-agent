pub use sea_orm_migration::prelude::*;

mod m20260104_000001_create_planning_tables;
mod m20260105_000001_create_remaining_tables;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260104_000001_create_planning_tables::Migration),
            Box::new(m20260105_000001_create_remaining_tables::Migration),
        ]
    }
}
