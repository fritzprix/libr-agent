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
    pub session_id: String, // Track which session created this playbook
    pub goal: String,
    pub initial_command: Option<String>,
    pub workflow: String,                 // JSON stored as TEXT
    pub success_criteria: Option<String>, // JSON stored as TEXT
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
