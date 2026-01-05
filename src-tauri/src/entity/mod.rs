//! SeaORM Entity Definitions
//!
//! This module contains all entity definitions for the LibrAgent database.

// Planning module entities
pub mod planning_goal;
pub mod planning_scratchpad;
pub mod planning_todo;

// Content Store module entities
pub mod assistant;
pub mod chunk;
pub mod content;
pub mod knowledge;
pub mod mcp_server;
pub mod playbook;
pub mod store;

pub mod prelude {
    // Planning entities
    pub use super::planning_goal::Entity as PlanningGoal;
    pub use super::planning_scratchpad::Entity as PlanningScratchpad;
    pub use super::planning_todo::Entity as PlanningTodo;

    // Content Store entities
    pub use super::assistant::Entity as Assistant;
    pub use super::chunk::Entity as Chunk;
    pub use super::content::Entity as Content;
    pub use super::knowledge::Entity as Knowledge;
    pub use super::mcp_server::Entity as McpServer;
    pub use super::playbook::Entity as Playbook;
    pub use super::store::Entity as Store;
}
