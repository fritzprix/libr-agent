use super::errors::planning_follow_up_read_notice;
use crate::mcp::types::{ContextVolatility, ServiceContext};
use crate::repositories::PlanningRepository;
use crate::state::get_planning_repository;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
pub struct TodoDTO {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
    pub checked: bool,
    pub subtasks: Vec<TodoDTO>,
}

pub async fn get_service_context(
    _db: &DatabaseConnection,
    session_id: &str,
    include_checked: bool,
) -> ServiceContext {
    let repo = get_planning_repository();

    let (goal, goal_notice) = match repo.get_active_goal(session_id).await {
        Ok(goal_model) => (goal_model.map(|g| g.goal_text), None),
        Err(error) => (
            None,
            Some(planning_follow_up_read_notice("current goal state", &error)),
        ),
    };

    let (todos, todo_notice) = match repo.list_todos(session_id, include_checked).await {
        Ok(todos) => (todos, None),
        Err(error) => (
            Vec::new(),
            Some(planning_follow_up_read_notice("current todo list", &error)),
        ),
    };

    // Build flat Todo list for structured state
    // We keep the subtasks field empty for API compatibility with frontend
    let structured_todos: Vec<TodoDTO> = todos
        .iter()
        .map(|t| TodoDTO {
            id: t.id,
            title: t.content.clone(),
            description: t.description.clone(),
            priority: t.priority.clone(),
            checked: t.is_checked,
            subtasks: Vec::new(),
        })
        .collect();

    // 3. (Scratchpad moved to ScratchpadServer)

    // --- Format Output ---

    let mut parts = vec!["## Planning".to_string(), String::new()];

    // Goal Section
    parts.push("### Stable Context".to_string());
    if let Some(notice) = &goal_notice {
        parts.push("- Current Goal: unavailable".to_string());
        parts.push(format!("- {}", notice.hint));
    } else if let Some(g) = &goal {
        parts.push(format!("- Current Goal: \"{}\"", g));
    } else {
        parts.push("- No goal set".to_string());
    }

    // Todos Section
    parts.push(String::new());
    parts.push("### Live State".to_string());
    if let Some(notice) = &todo_notice {
        parts.push("- Tasks: unavailable".to_string());
        parts.push(format!("- {}", notice.hint));
    } else if !todos.is_empty() {
        let (checked_todos, unchecked_todos): (Vec<_>, Vec<_>) =
            todos.iter().enumerate().partition(|(_, t)| t.is_checked);

        parts.push(format!(
            "- Tasks: {} pending / {} completed",
            unchecked_todos.len(),
            checked_todos.len()
        ));

        if !unchecked_todos.is_empty() {
            parts.push(String::new());
            parts.push("**Pending Tasks:**".to_string());

            for (_, t) in unchecked_todos.iter().take(10) {
                let priority = match t.priority.as_str() {
                    "high" => "high",
                    "low" => "low",
                    _ => "medium",
                };

                let safe_content = t.content.replace(['\n', '\r'], " ");
                let info = t
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .replace(['\n', '\r'], " ");
                let truncated_info = if info.chars().count() > 40 {
                    let s: String = info.chars().take(37).collect();
                    format!("{}...", s)
                } else {
                    info
                };

                let info_suffix = if truncated_info.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", truncated_info)
                };

                parts.push(format!(
                    "- #{} [{}] {}{}",
                    t.id, priority, safe_content, info_suffix
                ));
            }

            if unchecked_todos.len() > 10 {
                parts.push(format!(
                    "- ...and {} more pending tasks",
                    unchecked_todos.len() - 10
                ));
            }
        }

        // Recently Checked
        if !checked_todos.is_empty() {
            parts.push(String::new());
            parts.push("**Completed Recently:**".to_string());
            for (_, t) in checked_todos.iter().rev().take(3) {
                let info = t
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .replace(['\n', '\r'], " ");

                let summary = if info.is_empty() {
                    String::new()
                } else if info.chars().count() > 50 {
                    let s: String = info.chars().take(47).collect();
                    format!(" ({}...)", s)
                } else {
                    format!(" ({})", info)
                };

                parts.push(format!("- [✓] (Todo ID {}) {}{}", t.id, t.content, summary));
            }
        }
    } else {
        parts.push("- Tasks: None".to_string());
    }

    let structured_state = json!({
         "goal": goal,
         "todos": structured_todos,
         "todos_count": todos.len()
    });

    ServiceContext::new(parts.join("\n"))
        .with_structured_state(structured_state)
        .with_volatility(ContextVolatility::Medium)
}
