use crate::mcp::builtin::planning::models::{ScratchpadItem, TodoDTO, TodoItem};
use crate::mcp::types::ServiceContext;
use log::info;
use serde_json::json;
use sqlx::SqlitePool;
use std::collections::HashMap;

pub async fn get_service_context(pool: &SqlitePool, session_id: &str) -> ServiceContext {
    // 1. Fetch Active Goal
    let goal: Option<String> = sqlx::query_scalar(
        "SELECT goal_text FROM planning_goals WHERE session_id = ? AND status = 'active' LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .unwrap_or_else(|e| {
        log::error!("Failed to fetch goal: {}", e);
        None
    });

    // 2. Fetch Todos (All)
    // We fetch all to calculate counts and separate checked/unchecked
    let todos: Vec<TodoItem> =
        sqlx::query_as("SELECT * FROM planning_todos WHERE session_id = ? ORDER BY created_at ASC")
            .bind(session_id)
            .fetch_all(pool)
            .await
            .unwrap_or_else(|e| {
                log::error!("Failed to fetch todos: {}", e);
                Vec::new()
            });

    // Build Todo Tree for structured state
    let mut todo_map: HashMap<i64, Vec<TodoItem>> = HashMap::new();
    let mut root_todos: Vec<TodoItem> = Vec::new();

    for todo in &todos {
        // Treat parent_id = 0 as None (root item)
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
                    subtasks: Vec::new(), // Max 1 level nesting supported
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

    // 3. Fetch Scratchpad (Recent)
    let scratchpad: Vec<ScratchpadItem> = sqlx::query_as(
        "SELECT * FROM planning_scratchpad WHERE session_id = ? ORDER BY created_at DESC LIMIT 6",
    )
    .bind(session_id)
    .fetch_all(pool)
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
    let (checked_todos, unchecked_todos): (Vec<&TodoItem>, Vec<&TodoItem>) =
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
            for (idx, t) in unchecked_todos.iter().take(5).enumerate() {
                let priority = if t.priority != "medium" {
                    format!("Priority:{}", t.priority)
                } else {
                    "Priority:medium".to_string()
                };

                let description = if let Some(desc) = &t.description {
                    if !desc.is_empty() {
                        let char_count = desc.chars().count();
                        let truncated = if char_count > 80 {
                            let s: String = desc.chars().take(80).collect();
                            format!("{}...", s)
                        } else {
                            desc.clone()
                        };
                        format!("\n     {}", truncated)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                parts.push(format!(
                    "  [{}] ID:{} | {} | {}{}",
                    idx, t.id, t.content, priority, description
                ));
            }

            if unchecked_todos.len() > 5 {
                parts.push(format!(
                    "  ...and {} more (use listTodos to see all)",
                    unchecked_todos.len() - 5
                ));
            }
            parts.push("\n*Use ID when calling checkTodo/updateTodo*".to_string());
        }

        // Checked Todos (Top 3 recent)
        if !checked_todos.is_empty() {
            parts.push("\n**Checked Items (Completed):**".to_string());
            // We want the most recently updated/created ones (which are at the end of the list since we ordered by ASC)
            // So we reverse iteration
            for t in checked_todos.iter().rev().take(3) {
                let priority = if t.priority != "medium" {
                    format!("[{}]", t.priority)
                } else {
                    String::new()
                };
                parts.push(format!("  [✓] ID:{} | {} {}", t.id, t.content, priority));
            }

            if checked_todos.len() > 3 {
                parts.push(format!(
                    "  ...and {} more completed",
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
        // Check if we have more than the limit (we fetched limit 6 to check for 'more')
        let (visible_scratchpad, has_more_scratchpad) = if scratchpad.len() > 5 {
            (&scratchpad[0..5], true)
        } else {
            (&scratchpad[..], false)
        };

        parts.push(format!("\n**Scratchpad:** {} items", scratchpad.len()));
        parts.push("".to_string()); // Spacer

        for (idx, item) in visible_scratchpad.iter().enumerate() {
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

            let content_preview = if item.title.is_some() {
                let char_count = item.content.chars().count();
                if char_count > 50 {
                    let s: String = item.content.chars().take(50).collect();
                    format!(" - {}...", s)
                } else {
                    format!(" - {}", item.content)
                }
            } else {
                let char_count = item.content.chars().count();
                if char_count > 60 {
                    let s: String = item.content.chars().take(60).collect();
                    format!("{}...", s)
                } else {
                    item.content.clone()
                }
            };

            parts.push(format!(
                "  {}. **ID:{}** {}{}{}",
                idx + 1,
                item.id,
                title_part,
                content_preview,
                tags_part
            ));
        }

        if has_more_scratchpad {
            parts.push(format!(
                "  ...and {} more items. Use listScratchpad to view all.",
                scratchpad.len() - 5
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
    let goal: Option<String> = sqlx::query_scalar(
        "SELECT goal_text FROM planning_goals WHERE session_id = ? AND status = 'active' LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let counts: Option<(i64, i64)> = sqlx::query_as(
        r#"
        SELECT 
            COUNT(*) as total,
            SUM(CASE WHEN is_checked = 0 THEN 1 ELSE 0 END) as unchecked
        FROM planning_todos 
        WHERE session_id = ?
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let (total, unchecked) = counts.unwrap_or((0, 0));
    let checked = total - unchecked;

    let goal_text = goal.unwrap_or_else(|| "No active goal".to_string());

    format!(
        "\n\nGoal: \"{}\"\n\nCurrent progress:\n  - Total: {} todos\n  - Unchecked: {}\n  - Checked: {}",
        goal_text, total, unchecked, checked
    )
}
