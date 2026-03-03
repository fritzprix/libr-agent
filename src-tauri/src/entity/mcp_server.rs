//! MCP Server entity definition for MCP server configurations
//!
//! This entity represents MCP server configurations (global scope).

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "mcp_servers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String, // Immutable UUID (cuid2)
    #[sea_orm(unique)]
    pub name: String, // User-visible identifier (mutable, but unique)
    pub config: String,          // JSON stored as TEXT
    pub tool_count: Option<i32>, // Cached tool count (from last verification/connection)
    pub cached_tools: Option<String>, // JSON array of {name, description} (from last verify)
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
