//! SeaORM Entity Definitions
//!
//! This module contains all entity definitions for the LibrAgent database.

// Core entities (sessions and messages)
pub mod compact_context;
pub mod message;
pub mod message_index_meta;
pub mod session;

// Planning module entities
pub mod planning_goal;
pub mod planning_scratchpad;
pub mod planning_todo;

// Content Store module entities
pub mod assistant;
pub mod chunk;
pub mod content;
pub mod knowledge;
pub mod knowledge_chunk_entity;
pub mod knowledge_chunk_v2;
pub mod knowledge_entity;
pub mod knowledge_relationship;
pub mod mcp_server;
pub mod playbook;
pub mod scheduled_task;
pub mod settings;
pub mod store;

pub mod prelude {
    // Core entities
    pub use super::compact_context::Entity as CompactContext;
    pub use super::message::Entity as Message;
    pub use super::message_index_meta::Entity as MessageIndexMeta;
    pub use super::session::Entity as Session;

    // Planning entities
    pub use super::planning_goal::Entity as PlanningGoal;
    pub use super::planning_scratchpad::Entity as PlanningScratchpad;
    pub use super::planning_todo::Entity as PlanningTodo;

    // Content Store entities
    pub use super::assistant::Entity as Assistant;
    pub use super::chunk::Entity as Chunk;
    pub use super::content::Entity as Content;
    pub use super::knowledge::Entity as Knowledge;
    pub use super::knowledge_chunk_entity::Entity as KnowledgeChunkEntity;
    pub use super::knowledge_chunk_v2::Entity as KnowledgeChunkV2;
    pub use super::knowledge_entity::Entity as KnowledgeEntity;
    pub use super::knowledge_relationship::Entity as KnowledgeRelationship;
    pub use super::mcp_server::Entity as McpServer;
    pub use super::playbook::Entity as Playbook;
    pub use super::settings::Entity as Settings;
    pub use super::store::Entity as Store;
}
