pub mod assistant_repository;
pub mod content_store_repository;
pub mod error;
pub mod in_memory_session_repository;
pub mod knowledge_repository;
pub mod mcp_server_repository;
pub mod message_repository;
pub mod planning_repository;
pub mod playbook_repository;
pub mod session_repository;
pub mod settings_repository;

// Re-export core types for easier imports
pub use assistant_repository::{AssistantRepository, SqliteAssistantRepository};
pub use content_store_repository::{ContentStoreRepository, SqliteContentStoreRepository};
pub use error::DbError;
pub use in_memory_session_repository::InMemorySessionRepository;
pub use knowledge_repository::{KnowledgeRepository, SqliteKnowledgeRepository};
pub use mcp_server_repository::{MCPServerRepository, SqliteMCPServerRepository};
pub use message_repository::{MessageRepository, SqliteMessageRepository};
pub use planning_repository::{PlanningRepository, SqlitePlanningRepository};
pub use playbook_repository::{
    Page, PaginationParams, PlaybookRepository, SqlitePlaybookRepository,
};
pub use session_repository::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
pub use settings_repository::{SettingsRepository, SqliteSettingsRepository};
