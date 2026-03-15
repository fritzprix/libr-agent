use crate::mcp::types::ServiceContext;
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

pub async fn get_service_context(_db: &DatabaseConnection, session_id: &str) -> ServiceContext {
    let repo = get_planning_repository();

    // 1. Fetch Active Goal
    let goal_model = repo.get_active_goal(session_id).await.unwrap_or_else(|e| {
        log::error!("Failed to fetch goal: {}", e);
        None
    });

    let goal = goal_model.map(|g| g.goal_text);

    // 2. Fetch Todos (All, ordered by CreatedAt)
    let todos = repo.list_todos(session_id, true).await.unwrap_or_else(|e| {
        log::error!("Failed to fetch todos: {}", e);
        Vec::new()
    });

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

    let mut parts = vec!["## Planning".to_string()];

    // Goal Section
    if let Some(g) = &goal {
        parts.push(format!("\n**Current Goal:** \"{}\"", g));
    } else {
        parts.push("\n**No Goal Set**".to_string());
    }

    // Todos Section
    if !todos.is_empty() {
        let (checked_todos, unchecked_todos): (Vec<_>, Vec<_>) =
            todos.iter().enumerate().partition(|(_, t)| t.is_checked);

        parts.push(format!(
            "\n**Tasks:** {} pending / {} completed",
            unchecked_todos.len(),
            checked_todos.len()
        ));

        // Unchecked Todos
        if !unchecked_todos.is_empty() {
            parts.push("\n**Pending Tasks:**".to_string());
            parts.push("| Index | Prio | Task | Info |".to_string());
            parts.push("| :--- | :--- | :--- | :--- |".to_string());

            for (idx, t) in unchecked_todos.iter().take(10) {
                let priority_emoji = if t.priority == "high" {
                    "🔴"
                } else if t.priority == "low" {
                    "🟢"
                } else {
                    "🟡"
                };

                let safe_content = t.content.replace(['\n', '\r'], " ").replace('|', r"\|");
                let info = t
                    .description
                    .as_deref()
                    .unwrap_or("-")
                    .replace(['\n', '\r'], " ");
                let truncated_info = if info.chars().count() > 40 {
                    let s: String = info.chars().take(37).collect();
                    format!("{}...", s)
                } else {
                    info
                };

                parts.push(format!(
                    "| {} | {} | {} | {} |",
                    idx, priority_emoji, safe_content, truncated_info
                ));
            }

            if unchecked_todos.len() > 10 {
                parts.push(format!(
                    "*...and {} more pending tasks*",
                    unchecked_todos.len() - 10
                ));
            }
        }

        // Recently Checked
        if !checked_todos.is_empty() {
            parts.push("\n**Completed Recently:**".to_string());
            for (idx, t) in checked_todos.iter().rev().take(3) {
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

                parts.push(format!("- [✓] (Index {}) {}{}", idx, t.content, summary));
            }
        }

        parts.push("\n*Use 'index' when calling updateTodo.*".to_string());
    } else {
        parts.push("\n**Tasks:** None".to_string());
        parts.push("*Use 'addTodo' to create your first task.*".to_string());
    }

    let structured_state = json!({
         "goal": goal,
         "todos": structured_todos,
         "todos_count": todos.len()
    });

    ServiceContext {
        context_prompt: parts.join("\n"),
        structured_state: Some(structured_state),
    }
}
