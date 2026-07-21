use crate::execution_mode::ExecutionMode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const CURRENT_FORMAT_VERSION: u32 = 1;
pub const MIN_COMPATIBLE_VERSION: u32 = 1;
pub const MAX_SINGLE_JSON_BYTES: u64 = 20 * 1024 * 1024; // 20 MB
pub const MAX_TOTAL_DECOMPRESSED_BYTES: u64 = 250 * 1024 * 1024; // 250 MB

// --- DTO structs for serialization/deserialization to bypass non-deserializable Entity models ---

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssistantRecord {
    pub id: String,
    pub name: String,
    pub config: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpServerRecord {
    pub id: String,
    pub name: String,
    pub config: String,
    pub tool_count: Option<i32>,
    pub cached_tools: Option<String>,
    pub verification_status: Option<String>,
    pub last_verification_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlaybookRecord {
    pub id: String,
    pub assistant_id: String,
    pub goal: String,
    pub initial_command: Option<String>,
    pub workflow: String,
    pub success_criteria: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_bookmarked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScheduledTaskRecord {
    pub id: String,
    pub name: String,
    pub task_category: String,
    pub cron_expression: Option<String>,
    pub schedule_timezone: String,
    pub assistant_id: String,
    pub message: String,
    #[serde(default = "default_execution_mode")]
    pub execution_mode: String,
    /// Legacy export field — used when importing backups created before execution_mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yolo_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsafe_mode: Option<bool>,
    pub created_by_session_id: Option<String>,
    pub session_id: Option<String>,
    pub workspace_override: Option<String>,
    #[serde(default)]
    pub reset_planning_state: bool,
    pub enabled: bool,
    pub last_run_at: Option<i64>,
    pub next_run_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

fn default_execution_mode() -> String {
    ExecutionMode::Normal.as_str().to_string()
}

impl ScheduledTaskRecord {
    pub(crate) fn resolved_execution_mode(&self) -> String {
        if self.yolo_mode.is_some() || self.unsafe_mode.is_some() {
            return ExecutionMode::from_runtime_flags(
                self.yolo_mode.unwrap_or(false),
                self.unsafe_mode.unwrap_or(false),
            )
            .as_str()
            .to_string();
        }

        ExecutionMode::from_db(&self.execution_mode)
            .as_str()
            .to_string()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SettingsRecord {
    pub key: String,
    pub value: String,
    pub created_at: i64,
    pub updated_at: i64,
}

// --- Tauri Command Return Structs ---

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MigrationExportInfo {
    pub file_path: String,
    pub file_size_bytes: u64,
    pub sections: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MigrationSectionReport {
    pub success: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MigrationImportResult {
    pub sections_imported: HashMap<String, MigrationSectionReport>,
    pub total_imported: usize,
    pub total_skipped: usize,
    pub total_errors: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SectionPreview {
    pub name: String,
    pub item_count: usize,
    pub size_bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CompatibilityStatus {
    Compatible,
    NewerVersion { message: String },
    Incompatible { message: String },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MigrationPreview {
    pub format_version: u32,
    pub app_version: Option<String>,
    pub exported_at: Option<String>,
    pub compatibility: CompatibilityStatus,
    pub sections: Vec<SectionPreview>,
    pub total_size_bytes: u64,
    pub file_path: String,
}
