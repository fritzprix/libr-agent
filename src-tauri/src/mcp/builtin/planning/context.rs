use crate::entity::planning_todo;
use crate::mcp::types::ServiceContext;
use crate::repositories::PlanningRepository;
use crate::state::get_planning_repository;
use log::info;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;

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

    // 2. Fetch Todos (All)
    let todos = repo.list_todos(session_id, true).await.unwrap_or_else(|e| {
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
    let scratchpad = repo.list_scratchpad(session_id).await.unwrap_or_else(|e| {
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

            // Iterate over structured_todos (roots) to preserve hierarchy
            for t in structured_todos.iter().filter(|t| !t.checked).take(5) {
                let priority = if t.priority == "high" {
                    "🔴 High"
                } else if t.priority == "low" {
                    "🟢 Low"
                } else {
                    "🟡 Med"
                };

                let description = if let Some(desc) = &t.description {
                    if !desc.is_empty() {
                        let sanitized = desc.replace(['\n', '\r'], " ");
                        let char_count = sanitized.chars().count();
                        if char_count > 50 {
                            let s: String = sanitized.chars().take(47).collect();
                            format!("{}...", s)
                        } else {
                            sanitized
                        }
                    } else {
                        "-".to_string()
                    }
                } else {
                    "-".to_string()
                };

                let safe_content = t.title.replace(['\n', '\r'], " ").replace('|', r"\|");
                let safe_desc = description.replace('|', r"\|");

                // Optimization: If description is identical to content (e.g. derived title),
                // or if content is just a truncated version of description and we're showing the same truncation,
                // don't show description to save tokens.
                let final_desc = if safe_content == safe_desc || safe_desc == "-" {
                    "-".to_string()
                } else {
                    safe_desc
                };

                parts.push(format!(
                    "| {} | {} | {} | {} |",
                    t.id, priority, safe_content, final_desc
                ));

                // Display subtasks
                for st in t.subtasks.iter().filter(|st| !st.checked) {
                    let st_priority = if st.priority == "high" {
                        "🔴"
                    } else if st.priority == "low" {
                        "🟢"
                    } else {
                        "🟡"
                    };

                    let st_desc = if let Some(desc) = &st.description {
                        if !desc.is_empty() {
                            let sanitized = desc.replace(['\n', '\r'], " ");
                            let char_count = sanitized.chars().count();
                            if char_count > 50 {
                                let s: String = sanitized.chars().take(47).collect();
                                format!("{}...", s)
                            } else {
                                sanitized
                            }
                        } else {
                            "-".to_string()
                        }
                    } else {
                        "-".to_string()
                    };

                    let safe_st_content = st.title.replace(['\n', '\r'], " ").replace('|', r"\|");
                    let safe_st_desc = st_desc.replace('|', r"\|");

                    let final_st_desc = if safe_st_content == safe_st_desc || safe_st_desc == "-" {
                        "-".to_string()
                    } else {
                        safe_st_desc
                    };

                    parts.push(format!(
                        "| {} | {} | └─ {} | {} |",
                        st.id, st_priority, safe_st_content, final_st_desc
                    ));
                }
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
            parts.push("| ID | Status | Task | Summary |".to_string());
            parts.push("| :--- | :--- | :--- | :--- |".to_string());

            for t in checked_todos.iter().rev().take(3) {
                let safe_content = t.content.replace(['\n', '\r'], " ").replace('|', r"\|");

                // Extract summary from description field
                let summary = t
                    .description
                    .as_deref()
                    .filter(|s| !s.is_empty() && s != &"-")
                    .map(|s| {
                        let sanitized = s.replace(['\n', '\r'], " ");
                        let escaped = sanitized.replace('|', r"\|");
                        // Truncate summary if too long (max 50 chars)
                        if escaped.chars().count() > 50 {
                            let truncated: String = escaped.chars().take(47).collect();
                            format!("{}...", truncated)
                        } else {
                            escaped
                        }
                    })
                    .unwrap_or_else(|| "-".to_string());

                parts.push(format!(
                    "| {} | ✓ Done | {} | {} |",
                    t.id, safe_content, summary
                ));
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
                let safe_title = title.replace(['\n', '\r'], " ");
                let truncated_title = if safe_title.chars().count() > 50 {
                    let s: String = safe_title.chars().take(47).collect();
                    format!("**{}...**", s)
                } else {
                    format!("**{}**", safe_title)
                };
                truncated_title
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

            // Sanitize and truncate content for summary to maintain list structure
            let sanitized_content = item.content.replace(['\n', '\r'], " ");
            let truncated_content = if sanitized_content.chars().count() > 100 {
                let s: String = sanitized_content.chars().take(97).collect();
                format!("{}...", s)
            } else {
                sanitized_content
            };

            let content_part = if item.title.is_some() {
                format!(" - {}", truncated_content)
            } else {
                truncated_content
            };

            parts.push(format!(
                "{}. **ID:{}** {}{}{}",
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
