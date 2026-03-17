//! Scheduled task entity definition.
//!
//! Represents a cron-backed recurring task that triggers an agent workflow
//! with a specific assistant. The `message` field supports `@mention` syntax
//! (e.g. `@playbook:goal`, `@skill:name`) which is expanded by the existing
//! `resolve_message_references` pipeline at execution time.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "scheduled_tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// User-defined label for the task
    pub name: String,
    /// Standard cron expression (5 or 6 fields, UTC)
    pub cron_expression: String,
    /// Assistant (agent) that owns and executes this task
    pub assistant_id: String,
    /// Message to inject as a user turn; supports @playbook:name, @skill:name mentions
    pub message: String,
    /// Whether tools should execute without approval
    pub yolo_mode: bool,
    /// Reused session ID — populated after first run, None for fresh tasks
    pub session_id: Option<String>,
    /// Optional workspace override path applied to the pinned session at execution time
    pub workspace_override: Option<String>,
    /// Whether the task is active
    pub enabled: bool,
    /// Epoch milliseconds of last successful trigger (None = never run)
    pub last_run_at: Option<i64>,
    /// Epoch milliseconds of next scheduled trigger (computed on save/run)
    pub next_run_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
