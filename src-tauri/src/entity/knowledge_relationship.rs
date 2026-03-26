use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "knowledge_relationships")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub assistant_id: String,
    pub source_entity_id: i32,
    pub target_entity_id: i32,
    pub relation_type: String,
    pub weight: f32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
