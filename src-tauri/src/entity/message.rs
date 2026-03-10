use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub session_id: String,
    pub role: String,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    pub tool_calls: Option<String>,
    pub tool_call_id: Option<String>,
    pub is_streaming: Option<i32>,
    pub thinking: Option<String>,
    pub thinking_signature: Option<String>,
    pub assistant_id: Option<String>,
    pub attachments: Option<String>,
    pub tool_use: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub source: Option<String>,
    pub error: Option<String>,
    pub usage: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::session::Entity",
        from = "Column::SessionId",
        to = "super::session::Column::Id",
        on_delete = "Cascade"
    )]
    Session,
}

impl Related<super::session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Session.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
