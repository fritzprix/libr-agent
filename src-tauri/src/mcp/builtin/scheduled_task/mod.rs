use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{
    BuiltinServerMetadata, ContextVolatility, MCPResult, MCPTool, ServiceContext,
};
use sea_orm::DatabaseConnection;

pub mod formatting;
pub mod handlers;
pub mod tools;

pub const NAME: &str = "scheduled_task";

#[derive(Debug)]
pub struct ScheduledTaskServer {
    session_id: String,
}

impl ScheduledTaskServer {
    pub async fn new(session_id: String, _db: Arc<DatabaseConnection>) -> Result<Self, String> {
        Ok(Self { session_id })
    }

    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()
    }

    pub fn metadata_static() -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: "Scheduled Tasks".to_string(),
            description:
                "Create, inspect, update, pause, and delete recurring scheduled assistant runs"
                    .to_string(),
            icon: None,
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for ScheduledTaskServer {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Persistent recurring task management for scheduled assistant execution"
    }

    fn tools(&self) -> Vec<MCPTool> {
        Self::tools_static()
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "scheduleCallback" => {
                handlers::handle_schedule_callback(self, args, session_id).await
            }
            "createScheduledTask" => {
                handlers::handle_create_scheduled_task(self, args, session_id).await
            }
            "listScheduledTasks" => handlers::handle_list_scheduled_tasks(self, args).await,
            "getScheduledTask" => handlers::handle_get_scheduled_task(self, args).await,
            "updateScheduledTask" => handlers::handle_update_scheduled_task(self, args).await,
            "toggleScheduledTask" => handlers::handle_toggle_scheduled_task(self, args).await,
            "deleteScheduledTask" => handlers::handle_delete_scheduled_task(self, args).await,
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        use crate::repositories::ScheduledTaskRepository;

        let repo = crate::state::get_scheduled_task_repository();
        let tasks = match repo.list_scheduled_tasks(None).await {
            Ok(tasks) => tasks,
            Err(error) => {
                log::warn!("Failed to load scheduled task context: {}", error);
                return ServiceContext::new(
                    "## Scheduled Tasks\n\nError loading scheduled task state",
                )
                .with_volatility(ContextVolatility::Volatile);
            }
        };

        if tasks.is_empty() {
            return ServiceContext::new("## Scheduled Tasks\n\nNo scheduled tasks configured")
                .with_structured_state(json!({
                    "total": 0,
                    "enabled": 0,
                    "disabled": 0,
                    "tasks": []
                }))
                .with_volatility(ContextVolatility::Volatile);
        }

        let enabled_count = tasks.iter().filter(|task| task.enabled).count();
        let disabled_count = tasks.len().saturating_sub(enabled_count);
        let mut upcoming_tasks = tasks.iter().filter(|task| task.enabled).collect::<Vec<_>>();
        upcoming_tasks.sort_by_key(|task| task.next_run_at.unwrap_or(i64::MAX));

        let mut context_lines = vec![
            "## Scheduled Tasks".to_string(),
            String::new(),
            format!("- Total: {}", tasks.len()),
            format!("- Enabled: {}", enabled_count),
            format!("- Disabled: {}", disabled_count),
            format!("- Caller session: {}", self.session_id),
        ];

        if !upcoming_tasks.is_empty() {
            context_lines.push(String::new());
            context_lines.push("Next scheduled runs:".to_string());
            for task in upcoming_tasks.into_iter().take(3) {
                context_lines.push(format!(
                    "- {} ({}) at {}",
                    task.name,
                    task.id,
                    formatting::format_timestamp(task.next_run_at)
                ));
            }
            if tasks.len() > 3 {
                context_lines
                    .push("Use listScheduledTasks() for the full schedule set.".to_string());
            }
        }

        ServiceContext::new(context_lines.join("\n"))
            .with_structured_state(json!({
                "total": tasks.len(),
                "enabled": enabled_count,
                "disabled": disabled_count,
                "tasks": tasks
                    .iter()
                    .take(5)
                    .map(formatting::task_to_json)
                    .collect::<Vec<_>>()
            }))
            .with_volatility(ContextVolatility::Volatile)
    }

    async fn has_active_state(&self) -> bool {
        use crate::repositories::ScheduledTaskRepository;

        crate::state::get_scheduled_task_repository()
            .list_scheduled_tasks(None)
            .await
            .map(|tasks| !tasks.is_empty())
            .unwrap_or(false)
    }
}
