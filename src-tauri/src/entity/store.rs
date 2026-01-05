//! Store entity definition for content storage
//!
//! This entity represents content stores that hold uploaded content per session.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "stores")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub session_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::content::Entity")]
    Contents,
}

impl Related<super::content::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contents.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
