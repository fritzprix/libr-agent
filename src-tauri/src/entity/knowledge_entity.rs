use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "knowledge_entities")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub assistant_id: String,
    pub name: String,
    pub entity_type: Option<String>,
    pub description: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
