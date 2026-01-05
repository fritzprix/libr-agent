use crate::entity::{planning_goal, planning_scratchpad, planning_todo};
use crate::mcp::builtin::planning::models::TodoDTO;
use crate::mcp::types::ServiceContext;
use log::info;
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, SqlxSqliteConnector,
};
use serde_json::json;
use sqlx::SqlitePool;
use std::collections::HashMap;

pub async fn get_service_context(pool: &SqlitePool, session_id: &str) -> ServiceContext {
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());

    // 1. Fetch Active Goal
    let goal_model = planning_goal::Entity::find()
        .filter(planning_goal::Column::SessionId.eq(session_id))
        .filter(planning_goal::Column::Status.eq("active"))
        .one(&db)
        .await
        .unwrap_or_else(|e| {
            log::error!("Failed to fetch goal: {}", e);
            None
        });

    let goal = goal_model.map(|g| g.goal_text);

    // 2. Fetch Todos (All)
    let todos = planning_todo::Entity::find()
        .filter(planning_todo::Column::SessionId.eq(session_id))
        .order_by_asc(planning_todo::Column::CreatedAt)
        .all(&db)
        .await
        .unwrap_or_else(|e| {
            log::error!("Failed to fetch todos: {}", e);
            Vec::new()
        });

    // Build Todo Tree for structured state
    let mut todo_map: HashMap<i64, Vec<planning_todo::Model>> = HashMap::new();
    let mut root_todos: Vec<planning_todo::Model> = Vec::new();

    for todo in &todos {
        match todo.parent_id {
            Some(pid) if pid > 0 => {
                todo_map.entry(pid).or_default().push(todo.clone());
            }
            _ => {
                root_todos.push(todo.clone());
            }
        }
    }

    let structured_todos: Vec<TodoDTO> = root_todos
        .into_iter()
        .map(|t| {
            let subtasks = todo_map
                .remove(&t.id)
                .unwrap_or_default()
                .into_iter()
                .map(|st| TodoDTO {
                    id: st.id,
                    title: st.content,
                    description: st.description,
                    priority: st.priority,
                    checked: st.is_checked,
                    subtasks: Vec::new(),
                })
                .collect();

            TodoDTO {
                id: t.id,
                title: t.content,
                description: t.description,
                priority: t.priority,
                checked: t.is_checked,
                subtasks,
            }
        })
        .collect();

    // 3. Fetch Scratchpad
    let scratchpad = planning_scratchpad::Entity::find()
        .filter(planning_scratchpad::Column::SessionId.eq(session_id))
        .order_by_desc(planning_scratchpad::Column::CreatedAt)
        .all(&db)
        .await
        .unwrap_or_else(|e| {
            log::error!("Failed to fetch scratchpad: {}", e);
            Vec::new()
        });

    // --- Format Output ---

    let mut parts = vec!["## Planning".to_string()];

    // Goal Section
    if let Some(g) = &goal {
        parts.push(format!("\n**Current Goal:** \"{}\"", g));
        parts.push("*Goal is active. Track progress with todos below.*".to_string());
    } else {
        parts.push("\n**No Goal Set**".to_string());
        parts.push(
            "*Consider using createGoal to establish a clear objective for this planning session.*"
                .to_string(),
        );
    }

    // Todos Section
    let (checked_todos, unchecked_todos): (Vec<&planning_todo::Model>, Vec<&planning_todo::Model>) =
        todos.iter().partition(|t| t.is_checked);

    if !todos.is_empty() {
        parts.push(format!(
            "\n**Todos:** {} unchecked / {} checked ({} total)",
            unchecked_todos.len(),
            checked_todos.len(),
            todos.len()
        ));

        // Unchecked Todos (Top 5)
        if !unchecked_todos.is_empty() {
            parts.push("\n**Unchecked Items:**".to_string());
            parts.push("| ID | Prio | Task | Description |".to_string());
            parts.push("| :--- | :--- | :--- | :--- |".to_string());

            for t in unchecked_todos.iter().take(5) {
                let priority = if t.priority == "high" {
                    "🔴 High"
                } else if t.priority == "low" {
                    "🟢 Low"
                } else {
                    "🟡 Med"
                };

                let description = if let Some(desc) = &t.description {
                    if !desc.is_empty() {
                        let char_count = desc.chars().count();
                        if char_count > 50 {
                            let s: String = desc.chars().take(50).collect();
                            format!("{}...", s)
                        } else {
                            desc.clone()
                        }
                    } else {
                        "-".to_string()
                    }
                } else {
                    "-".to_string()
                };

                let safe_content = t.content.replace('|', r"\|");
                let safe_desc = description.replace('|', r"\|");

                parts.push(format!(
                    "| {} | {} | {} | {} |",
                    t.id, priority, safe_content, safe_desc
                ));
            }

            if unchecked_todos.len() > 5 {
                parts.push(format!(
                    "\n*...and {} more (use listTodos to see all)*",
                    unchecked_todos.len() - 5
                ));
            }
            parts.push("\n*Use ID when calling checkTodo/updateTodo*".to_string());
        }

        // Checked Todos (Top 3 recent)
        if !checked_todos.is_empty() {
            parts.push("\n**Checked Items (Recently Completed):**".to_string());
            parts.push("| ID | Status | Task |".to_string());
            parts.push("| :--- | :--- | :--- |".to_string());

            for t in checked_todos.iter().rev().take(3) {
                let safe_content = t.content.replace('|', r"\|");
                parts.push(format!("| {} | ✓ Done | {} |", t.id, safe_content));
            }

            if checked_todos.len() > 3 {
                parts.push(format!(
                    "\n*...and {} more completed*",
                    checked_todos.len() - 3
                ));
            }
        }
    } else {
        parts.push("\n**Todos:** 0 items".to_string());
        parts.push("*Use 'addTodo' to break down your goal into actionable tasks.*".to_string());
    }

    // Scratchpad Section
    if !scratchpad.is_empty() {
        parts.push(format!("\n**Scratchpad:** {} items", scratchpad.len()));
        parts.push("".to_string());

        for (idx, item) in scratchpad.iter().enumerate() {
            let title_part = if let Some(title) = &item.title {
                format!("**{}**", title)
            } else {
                String::new()
            };

            let tags_part = if let Some(tags_json) = &item.tags {
                if let Ok(tags) = serde_json::from_str::<Vec<String>>(tags_json) {
                    if !tags.is_empty() {
                        format!(" [{}]", tags.join("] ["))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let content_part = if item.title.is_some() {
                format!(" - {}", item.content)
            } else {
                item.content.clone()
            };

            parts.push(format!(
                "  {}. **ID:{}** {}{}{}",
                idx + 1,
                item.id,
                title_part,
                content_part,
                tags_part
            ));
        }
    } else {
        parts.push("\n**Scratchpad:** Empty".to_string());
        parts.push("*Use 'addScratchpad' to save important findings, IDs, or file paths for later reference.*".to_string());
    }

    let structured_state = json!({
         "goal": goal,
         "lastClearedGoal": null,
         "todos": structured_todos,
         "scratchpad": scratchpad,
         "todos_count": todos.len(),
         "scratchpad_count": scratchpad.len()
    });
    info!("structured_state: {} vs {:?}", structured_state, todos);

    ServiceContext {
        context_prompt: parts.join("\n"),
        structured_state: Some(structured_state),
    }
}

pub async fn get_planning_summary(pool: &SqlitePool, session_id: &str) -> String {
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());

    let goal_model = planning_goal::Entity::find()
        .filter(planning_goal::Column::SessionId.eq(session_id))
        .filter(planning_goal::Column::Status.eq("active"))
        .one(&db)
        .await
        .unwrap_or(None);

    let goal_text = goal_model
        .map(|g| g.goal_text)
        .unwrap_or_else(|| "No active goal".to_string());

    let total = planning_todo::Entity::find()
        .filter(planning_todo::Column::SessionId.eq(session_id))
        .count(&db)
        .await
        .unwrap_or(0);

    let unchecked = planning_todo::Entity::find()
        .filter(planning_todo::Column::SessionId.eq(session_id))
        .filter(planning_todo::Column::IsChecked.eq(false))
        .count(&db)
        .await
        .unwrap_or(0);

    let checked = total - unchecked;

    format!(
        "\n\nGoal: \"{}\"\n\nCurrent progress:\n  - Total: {} todos\n  - Unchecked: {}\n  - Checked: {}",
        goal_text, total, unchecked, checked
    )
}
