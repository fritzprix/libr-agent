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
    pub assistant_id: Option<String>,
    pub parent_session_id: Option<String>,
    pub lineage_id: Option<String>,
    pub depth: Option<i32>,
    pub max_depth: Option<i32>,
    pub max_fanout: Option<i32>,
    pub org_id: Option<String>,
    pub org_name: Option<String>,
    pub org_root_session_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_viewed_at: Option<i64>,
    pub last_message_at: Option<i64>,
    pub last_attention_at: Option<i64>,
    pub last_attention_reason: Option<String>,
    pub is_bookmarked: bool,
    pub execution_mode: String,
    pub workspace_override: Option<String>,
    pub workspace_isolation: String,
    pub docker_config_json: Option<String>,
    pub docker_container_name: Option<String>,
    pub docker_host_workspace_path: Option<String>,
    pub status_reason: Option<String>,
    /// Original session id when this session was written back as a fork by a
    /// `--session` headless run (#2860); NULL for merged or fresh sessions.
    pub forked_from: Option<String>,
    /// Monotonic counter of message row changes for this session (#2878).
    /// Maintained by the `messages_session_version_*` triggers; the headless
    /// write-back uses it as the merge-or-fork base. `default_value` keeps
    /// entity-generated test schemas (create_table_from_entity) insertable
    /// without an explicit version, matching the migration's DEFAULT 0.
    #[sea_orm(default_value = 0)]
    pub version: i64,
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

impl Model {
    pub fn parsed_execution_mode(&self) -> crate::execution_mode::ExecutionMode {
        crate::execution_mode::ExecutionMode::from_db(&self.execution_mode)
    }
}
