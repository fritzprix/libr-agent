use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: Option<String>,
    pub status: String,
    pub model: String,
    pub provider: String,
    pub agent_config: Option<String>,
    pub parent_session_id: Option<String>,
    pub lineage_id: Option<String>,
    pub depth: Option<i32>,
    pub max_depth: Option<i32>,
    pub max_fanout: Option<i32>,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_bookmarked: bool,
    pub yolo_mode: bool,
    pub workspace_override: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::message::Entity")]
    Messages,
}

impl Related<super::message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Messages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
