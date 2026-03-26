pub mod assistant_repository;
pub mod attachments_repository;
pub mod compact_context_repository;
pub mod error;
pub mod in_memory_session_repository;
pub mod knowledge_repository;
pub mod knowledge_v2_repository;
pub mod mcp_server_repository;
pub mod message_repository;
pub mod planning_repository;
pub mod playbook_repository;
pub mod scheduled_task_repository;
pub mod session_repository;
pub mod settings_repository;

// Re-export core types for easier imports
pub use crate::utils::pagination::{Page, PaginationParams};
pub use assistant_repository::{AssistantRepository, SqliteAssistantRepository};
pub use attachments_repository::{AttachmentsRepository, SqliteAttachmentsRepository};
pub use compact_context_repository::{
    CompactContextRecord, CompactContextRepository, SqliteCompactContextRepository,
};
pub use error::DbError;
pub use in_memory_session_repository::InMemorySessionRepository;
pub use knowledge_repository::{KnowledgeRepository, SqliteKnowledgeRepository};
pub use knowledge_v2_repository::{KnowledgeV2Repository, SqliteKnowledgeV2Repository};
pub use mcp_server_repository::{MCPServerRepository, SqliteMCPServerRepository};
pub use message_repository::{MessageRepository, SqliteMessageRepository};
pub use planning_repository::{PlanningRepository, SqlitePlanningRepository};
pub use playbook_repository::{PlaybookRepository, SqlitePlaybookRepository};
pub use scheduled_task_repository::{
    CreateScheduledTaskParams, ScheduledTaskRepository, SqliteScheduledTaskRepository,
    UpdateScheduledTaskParams,
};
pub use session_repository::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
pub use settings_repository::{SettingsRepository, SqliteSettingsRepository};
