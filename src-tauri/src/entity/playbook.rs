//! Playbook entity definition for workflow automation
//!
//! This entity represents playbooks that define automated workflows.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "playbooks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub assistant_id: String,
    pub goal: String,
    pub initial_command: Option<String>,
    pub workflow: String,                 // JSON stored as TEXT (steps array)
    pub success_criteria: Option<String>, // JSON stored as TEXT
    /// Start launch pin JSON (`{ mode, sessionId? }`), stored separately from workflow.
    pub default_target_session: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_bookmarked: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
