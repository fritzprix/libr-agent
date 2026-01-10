//! Chunk entity definition for content storage
//!
//! This entity represents text chunks that belong to content files.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "chunks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub content_id: String,
    pub chunk_index: i32,
    pub text: String,
    pub start_line: i32,
    pub end_line: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::content::Entity",
        from = "Column::ContentId",
        to = "super::content::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Content,
}

impl Related<super::content::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Content.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
