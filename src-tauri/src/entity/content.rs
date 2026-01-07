//! Content entity definition for content storage
//!
//! This entity represents individual content files/documents stored in stores.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub session_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: i32,
    pub line_count: i32,
    pub preview: String,
    pub uploaded_at: String,
    pub chunk_count: i32,
    pub last_accessed_at: String,
    pub content: String,
    pub src_url: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::store::Entity",
        from = "Column::SessionId",
        to = "super::store::Column::SessionId",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Store,
    #[sea_orm(has_many = "super::chunk::Entity")]
    Chunks,
}

impl Related<super::store::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Store.def()
    }
}

impl Related<super::chunk::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Chunks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
