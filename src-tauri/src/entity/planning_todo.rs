//! Planning Todo Entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "planning_todos")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub session_id: String,
    pub content: String,
    pub description: Option<String>,
    pub priority: String,
    pub parent_id: Option<i64>,
    pub is_checked: bool,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(belongs_to = "Entity", from = "Column::ParentId", to = "Column::Id")]
    SelfRef,
}

impl Related<Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SelfRef.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
