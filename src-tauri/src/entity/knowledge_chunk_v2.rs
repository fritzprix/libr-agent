use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "knowledge_chunks_v2")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub assistant_id: String,
    pub content: String,
    pub tags: Option<String>,
    pub source: Option<String>,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
