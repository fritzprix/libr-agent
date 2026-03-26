use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "knowledge_chunk_entities")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub chunk_id: i32,
    #[sea_orm(primary_key)]
    pub entity_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
