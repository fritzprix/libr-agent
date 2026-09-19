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

    /// Ambient SC only surfaces enabled SESSION callbacks for this session.
    /// GLOBAL cron tasks and disabled callbacks stay on-demand via tools.
    fn is_active_own_session_callback(
        task: &crate::entity::scheduled_task::Model,
        session_id: &str,
    ) -> bool {
        task.enabled
            && task.task_category == crate::scheduled::TASK_CATEGORY_SESSION
            && task.session_id.as_deref() == Some(session_id)
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
            "scheduleCallback" => handlers::handle_schedule_callback(self, args, session_id).await,
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
        let mut tasks = match repo.list_scheduled_tasks(None).await {
            Ok(tasks) => tasks,
            Err(error) => {
                log::warn!("Failed to load scheduled task context: {}", error);
                return ServiceContext::new(
                    "## Scheduled Tasks\n\nError loading scheduled task state",
                )
                .with_volatility(ContextVolatility::Volatile);
            }
        };

        // Enabled SESSION callbacks for this session only — GLOBAL / disabled stay on-demand.
        tasks.retain(|task| Self::is_active_own_session_callback(task, self.session_id.as_str()));

        if tasks.is_empty() {
            // Idle: omit prompt text so volatile SC stays lean.
            return ServiceContext::new("")
                .with_structured_state(json!({
                    "total": 0,
                    "enabled": 0,
                    "disabled": 0,
                    "tasks": []
                }))
                .with_volatility(ContextVolatility::Volatile);
        }

        let pending_count = tasks.len();
        let mut upcoming_tasks = tasks.iter().collect::<Vec<_>>();
        upcoming_tasks.sort_by_key(|task| task.next_run_at.unwrap_or(i64::MAX));

        let mut context_lines = vec![
            "## Scheduled Tasks".to_string(),
            String::new(),
            format!("- Pending: {}", pending_count),
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
            if pending_count > 3 {
                context_lines.push(
                    "Use scheduled_task__listScheduledTasks() for the full schedule set."
                        .to_string(),
                );
            }
        }

        ServiceContext::new(context_lines.join("\n"))
            .with_structured_state(json!({
                "total": pending_count,
                "enabled": pending_count,
                "disabled": 0,
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

        let session_id = self.session_id.as_str();
        crate::state::get_scheduled_task_repository()
            .list_scheduled_tasks(None)
            .await
            .map(|tasks| {
                tasks
                    .iter()
                    .any(|task| Self::is_active_own_session_callback(task, session_id))
            })
            .unwrap_or(false)
    }
}
