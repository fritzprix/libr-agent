use crate::entity::playbook;
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, not_found_error, operation_failed_error, ToolGroup,
};
use crate::mcp::types::{MCPContent, MCPResult};
use crate::repositories::{PaginationParams, PlaybookRepository, SessionRepository};
use handlebars::Handlebars;
use serde_json::{json, Value};
use std::cmp::Ordering;

use super::templates::PLAYBOOK_LIST_TEMPLATE;
use super::types::{
    serialize_default_target_session_column, serialize_workflow_steps, Playbook,
    TargetSessionConfig, TargetSessionMode,
};
use crate::utils::session_id::{
    resolve_session_id_among, should_try_legacy_session_resolve, SessionIdResolve,
};

fn sort_playbook_models(
    models: &mut [playbook::Model],
    sort_by: &str,
    sort_order: &str,
    bookmark_first: bool,
) {
    models.sort_by(|a, b| {
        if bookmark_first && a.is_bookmarked != b.is_bookmarked {
            return b.is_bookmarked.cmp(&a.is_bookmarked);
        }

        let ordering = match sort_by {
            "assistant" => a.assistant_id.cmp(&b.assistant_id),
            _ => a.created_at.cmp(&b.created_at),
        };
        let ordering = if sort_order == "asc" {
            ordering
        } else {
            ordering.reverse()
        };

        if ordering == Ordering::Equal {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.cmp(&b.id))
        } else {
            ordering
        }
    });
}

/// Resolve pin sessionId to storage id (exact, then legacy short/prefix fallback).
async fn canonicalize_pin_session_id(session_ref: &str) -> Result<String, MCPResult> {
    let session_ref = session_ref.trim();
    if session_ref.is_empty() {
        return Err(invalid_input_error(
            "defaultTargetSession.mode 'pin' requires a non-empty sessionId",
            ToolGroup::Playbook,
        ));
    }

    let repo = crate::get_session_repository();
    match repo.get_session(session_ref).await {
        Ok(Some(_)) => return Ok(session_ref.to_string()),
        Ok(None) => {}
        Err(e) => {
            return Err(operation_failed_error(
                "resolvePinnedSession",
                &format!("Failed to look up session '{session_ref}': {e}"),
                vec!["Retry once the session repository is available".to_string()],
                ToolGroup::Playbook,
            ))
        }
    }

    if !should_try_legacy_session_resolve(session_ref) {
        return Err(invalid_input_error(
            &format!(
                "Pinned session '{session_ref}' was not found. Use the exact session id from the current session context."
            ),
            ToolGroup::Playbook,
        ));
    }

    let sessions = match repo.get_all_sessions().await {
        Ok(sessions) => sessions,
        Err(e) => {
            return Err(operation_failed_error(
                "resolvePinnedSession",
                &format!("Failed to list sessions while resolving '{session_ref}': {e}"),
                vec!["Retry once the session repository is available".to_string()],
                ToolGroup::Playbook,
            ))
        }
    };

    let candidate_ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
    match resolve_session_id_among(candidate_ids, session_ref) {
        SessionIdResolve::Unique(resolved) => Ok(resolved.to_string()),
        SessionIdResolve::Missing => Err(invalid_input_error(
            &format!(
                "Pinned session '{session_ref}' was not found. Use the exact session id from the current session context."
            ),
            ToolGroup::Playbook,
        )),
        SessionIdResolve::Ambiguous(count) => Err(invalid_input_error(
            &format!(
                "Pinned session reference '{session_ref}' is ambiguous among {count} sessions. \
                 Retry with the exact storage session id."
            ),
            ToolGroup::Playbook,
        )),
    }
}

async fn canonicalize_default_target_session(
    mut cfg: TargetSessionConfig,
    caller_session_id: Option<&str>,
) -> Result<TargetSessionConfig, MCPResult> {
    if cfg.mode != TargetSessionMode::Pin {
        return Ok(cfg);
    }
    if cfg.pinned_session_id().is_none() {
        let Some(caller) = caller_session_id.map(str::trim).filter(|s| !s.is_empty()) else {
            return Err(invalid_input_error(
                "defaultTargetSession.mode 'pin' requires sessionId (or call from a session so the caller id can be used)",
                ToolGroup::Playbook,
            ));
        };
        cfg.session_id = Some(caller.to_string());
    }
    let session_ref = cfg
        .pinned_session_id()
        .expect("pin sessionId set above")
        .to_string();
    let storage_id = canonicalize_pin_session_id(&session_ref).await?;
    Ok(TargetSessionConfig {
        mode: TargetSessionMode::Pin,
        session_id: Some(storage_id),
    })
}

pub async fn create_playbook(
    assistant_id: &str,
    args: Value,
    caller_session_id: Option<&str>,
) -> Result<MCPResult, String> {
    let goal = match args.get("goal").and_then(|v| v.as_str()) {
        Some(g) if !g.trim().is_empty() => g,
        Some(_) => {
            return Ok(invalid_input_error(
                "Goal cannot be empty",
                ToolGroup::Playbook,
            ))
        }
        None => return Ok(missing_param_error("goal", ToolGroup::Playbook)),
    };

    let _initial_command = args.get("initialCommand").and_then(|v| v.as_str());

    let workflow = match args.get("workflow") {
        Some(w) if w.as_array().is_some_and(|arr| !arr.is_empty()) => w,
        Some(w) if w.is_array() => {
            return Ok(invalid_input_error(
                "Workflow cannot be empty array",
                ToolGroup::Playbook,
            ))
        }
        Some(_) => {
            return Ok(invalid_input_error(
                "Workflow must be an array of steps",
                ToolGroup::Playbook,
            ))
        }
        None => return Ok(missing_param_error("workflow", ToolGroup::Playbook)),
    };

    let _success_criteria = args.get("successCriteria");

    let default_target_session = match Playbook::parse_default_target_session_arg(&args) {
        Ok(cfg) => cfg,
        Err(e) => {
            return Ok(invalid_input_error(&e, ToolGroup::Playbook));
        }
    };
    let default_target_session = match default_target_session {
        Some(cfg) => match canonicalize_default_target_session(cfg, caller_session_id).await {
            Ok(cfg) => Some(cfg),
            Err(err) => return Ok(err),
        },
        None => None,
    };

    let repo = crate::get_playbook_repository();
    let id = uuid::Uuid::new_v4().to_string();

    let steps: Vec<super::types::PlaybookStep> = match serde_json::from_value(workflow.clone()) {
        Ok(steps) => steps,
        Err(e) => {
            return Ok(invalid_input_error(
                &format!("Invalid workflow format: {}", e),
                ToolGroup::Playbook,
            ))
        }
    };

    let workflow_json = match serialize_workflow_steps(&steps) {
        Ok(json) => json,
        Err(e) => {
            return Ok(invalid_input_error(
                &format!("Invalid workflow format: {}", e),
                ToolGroup::Playbook,
            ))
        }
    };
    let pin_json = match serialize_default_target_session_column(default_target_session.as_ref()) {
        Ok(json) => json,
        Err(e) => {
            return Ok(invalid_input_error(
                &format!("Invalid defaultTargetSession format: {}", e),
                ToolGroup::Playbook,
            ))
        }
    };

    let inserted = match repo
        .create_playbook(
            id.clone(),
            assistant_id.to_string(),
            goal.to_string(),
            workflow_json,
            pin_json,
        )
        .await
    {
        Ok(model) => model,
        Err(e) => {
            return Ok(operation_failed_error(
                "createPlaybook",
                &format!("Failed to save playbook to database: {}", e),
                vec![
                    "Verify database is accessible".to_string(),
                    "Check that workflow and success criteria are valid JSON".to_string(),
                ],
                ToolGroup::Playbook,
            ))
        }
    };

    let playbook = Playbook::from_model(&inserted);
    let formatted = format_playbook_summary(&playbook);

    let text_response = format!(
        "Successfully created new playbook.\nID: {}\nGoal: {}\nSteps: {}\n\n{}\n\nThe playbook is now available. Use 'playbook__listPlaybooks' to see all playbooks, or 'playbook__selectPlaybook' with ID {} to execute it.",
        id, playbook.goal, playbook.workflow.len(), formatted, id
    );

    Ok(MCPResult {
        content: Some(vec![MCPContent::Text {
            text: text_response,
        }]),
        structured_content: Some(json!({
            "success": true,
            "playbook": playbook
        })),
        is_error: Some(false),
    })
}

pub fn format_playbook_summary(p: &Playbook) -> String {
    let created = chrono::DateTime::from_timestamp_millis(p.created_at)
        .map(|dt| dt.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let pin = match &p.default_target_session {
        Some(target) if target.mode == TargetSessionMode::Pin => {
            let storage = target.pinned_session_id().unwrap_or("(missing)");
            format!(" pin:{storage}")
        }
        Some(_) => " pin:self".to_string(),
        None => String::new(),
    };
    format!(
        "id:{} goal:\"{}\" initial:\"{}\" steps:{} createdAt:{}{}",
        p.id,
        p.goal,
        p.initial_command.as_deref().unwrap_or(""),
        p.workflow.len(),
        created,
        pin
    )
}

pub async fn list_playbooks(
    assistant_id: &str,
    args: Value,
    render_ui_flag: bool,
) -> Result<MCPResult, String> {
    let repo = crate::get_playbook_repository();

    let page = args
        .get("page")
        .and_then(|v| v.as_i64())
        .unwrap_or(1)
        .max(1) as u64;
    let page_size = args
        .get("pageSize")
        .and_then(|v| v.as_i64())
        .unwrap_or(10)
        .max(1) as u64;
    let sort_by = args
        .get("sortBy")
        .and_then(|v| v.as_str())
        .unwrap_or("created_at");
    let sort_order = args
        .get("sortOrder")
        .and_then(|v| v.as_str())
        .unwrap_or("desc");
    let bookmark_first = args
        .get("bookmarkFirst")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Get all playbooks for assistant, then apply stable in-memory sorting/pagination
    let pagination = PaginationParams {
        page: 1,
        page_size: 10000,
    };
    let mut all_playbooks = repo
        .list_playbooks(Some(assistant_id), pagination)
        .await
        .map_err(|e| format!("Failed to list playbooks: {}", e))?
        .items;
    sort_playbook_models(&mut all_playbooks, sort_by, sort_order, bookmark_first);

    let total_items = all_playbooks.len() as u64;
    let offset = (page - 1) * page_size;

    // Manual pagination
    let models: Vec<_> = all_playbooks
        .into_iter()
        .skip(offset as usize)
        .take(page_size as usize)
        .collect();

    let playbooks: Vec<Playbook> = models.iter().map(Playbook::from_model).collect();
    let total_items = total_items as i64;
    let total_pages = if page_size > 0 {
        (total_items as f64 / page_size as f64).ceil() as i64
    } else {
        1
    };

    let formatted_list = if playbooks.is_empty() {
        format!("No playbooks found for assistant {}.", assistant_id)
    } else {
        playbooks
            .iter()
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "{}. {}",
                    (offset as i64) + (i as i64) + 1,
                    format_playbook_summary(p)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let page_result = json!({
        "page": page,
        "pageSize": page_size,
        "totalItems": total_items,
        "totalPages": total_pages,
        "items": playbooks
    });

    let structured = json!({
        "page": page_result,
        "formattedText": formatted_list
    });

    if render_ui_flag {
        let html = render_ui(
            &playbooks,
            page as i64,
            total_pages,
            total_items,
            page_size as i64,
        )?;
        let tool_name = "getPlaybookPage";

        let action_text = format!("Navigated to page {}", page);

        let text_response = format!(
            "[{}] {}.\nCurrent page: {} of {}\n\nPlaybooks on this page:\n{}\n\nStatus: Agent paused for user interaction (Select/Delete/Navigate buttons available).",
            tool_name, action_text, page, total_pages, formatted_list
        );

        Ok(MCPResult {
            content: Some(vec![
                MCPContent::Text {
                    text: text_response,
                },
                MCPContent::Resource {
                    resource: json!({
                        "uri": format!("ui://playbook/list/{}", assistant_id),
                        "mimeType": "text/html",
                        "text": html
                    }),
                    service_info: crate::mcp::types::ServiceInfo {
                        server_name: "playbook".to_string(),
                        tool_name: tool_name.to_string(),
                        backend_type: "BuiltInRust".to_string(),
                    },
                },
            ]),
            structured_content: Some(structured),
            is_error: Some(false),
        })
    } else {
        let text_response = if playbooks.is_empty() {
            format!(
                "[playbook__listPlaybooks] No playbooks found for assistant {}.",
                assistant_id
            )
        } else {
            format!(
                "[playbook__listPlaybooks] Found {} playbook(s) for assistant {}.\nShowing page {} of {} ({} items on this page):\n\n{}\n\nNote: Use 'playbook__getPlaybook' to view details or 'playbook__selectPlaybook' to execute a playbook.",
                total_items, assistant_id, page, total_pages, playbooks.len(), formatted_list
            )
        };

        Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text: text_response,
            }]),
            structured_content: Some(structured),
            is_error: Some(false),
        })
    }
}

pub fn render_ui(
    playbooks: &[Playbook],
    page: i64,
    total_pages: i64,
    total_items: i64,
    page_size: i64,
) -> Result<String, String> {
    let mut handlebars = Handlebars::new();
    handlebars.register_escape_fn(handlebars::html_escape);

    let view_models: Vec<Value> = playbooks
        .iter()
        .map(|p| {
            json!({
                "id": p.id,
                "goal": p.goal,
                "step_count": p.workflow.len(),
                "created_at_fmt": chrono::DateTime::from_timestamp_millis(p.created_at)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_default(),
                "is_bookmarked": p.is_bookmarked,
            })
        })
        .collect();

    let data = json!({
        "playbooks": view_models,
        "hasPlaybooks": !playbooks.is_empty(),
        "page": page,
        "totalPages": total_pages,
        "totalItems": total_items,
        "pageSize": page_size,
        "prevDisabled": if page <= 1 { "disabled" } else { "" },
        "nextDisabled": if page >= total_pages { "disabled" } else { "" }
    });

    handlebars
        .render_template(PLAYBOOK_LIST_TEMPLATE, &data)
        .map_err(|e| format!("Failed to render UI: {}", e))
}

pub async fn get_playbook(assistant_id: &str, args: Value) -> Result<MCPResult, String> {
    let repo = crate::get_playbook_repository();

    let id = match args.get("id").and_then(|v| v.as_str()) {
        Some(id) if !id.trim().is_empty() => id,
        Some(_) => {
            return Ok(invalid_input_error(
                "Playbook ID cannot be empty",
                ToolGroup::Playbook,
            ))
        }
        None => return Ok(missing_param_error("id", ToolGroup::Playbook)),
    };

    let model = match repo.get_playbook(id, assistant_id).await {
        Ok(model) => model,
        Err(e) => {
            return Ok(operation_failed_error(
                "getPlaybook",
                &format!("Database query failed: {}", e),
                vec!["Verify database is accessible".to_string()],
                ToolGroup::Playbook,
            ))
        }
    };

    match model {
        Some(model) => {
            let playbook = Playbook::from_model(&model);
            let formatted = format_playbook_detailed(&playbook);

            let text_response = format!(
                "[get_playbook] Retrieved playbook details for ID: {}\n\n{}\n\nNote: Use 'playbook__selectPlaybook' to execute this playbook, or 'playbook__updatePlaybook' to modify it.",
                id, formatted
            );

            Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: text_response,
                }]),
                structured_content: Some(json!({ "playbook": playbook })),
                is_error: Some(false),
            })
        }
        None => Ok(not_found_error("playbook", id, ToolGroup::Playbook)),
    }
}

pub async fn select_playbook(assistant_id: &str, args: Value) -> Result<MCPResult, String> {
    let repo = crate::get_playbook_repository();

    let id = match args.get("id").and_then(|v| v.as_str()) {
        Some(id) if !id.trim().is_empty() => id,
        Some(_) => {
            return Ok(invalid_input_error(
                "Playbook ID cannot be empty",
                ToolGroup::Playbook,
            ))
        }
        None => return Ok(missing_param_error("id", ToolGroup::Playbook)),
    };

    let model = match repo.get_playbook(id, assistant_id).await {
        Ok(model) => model,
        Err(e) => {
            return Ok(operation_failed_error(
                "selectPlaybook",
                &format!("Database query failed: {}", e),
                vec!["Verify database is accessible".to_string()],
                ToolGroup::Playbook,
            ))
        }
    };

    match model {
        Some(model) => {
            let playbook = Playbook::from_model(&model);
            let details = format_playbook_detailed(&playbook);
            let prompt = format!(
                "[select_playbook] Playbook \"{}\" (ID: {}) has been selected for execution.\n\nPlaybook Details:\n---\n{}\n---\n\nInstructions:\n1. Review the workflow steps and success criteria above\n2. Establish todos based on the workflow steps\n3. Begin executing the tasks according to the defined steps\n4. Track progress and verify against success criteria\n\nYou may now proceed with execution.",
                playbook.goal, playbook.id, details
            );

            Ok(MCPResult {
                content: Some(vec![MCPContent::Text { text: prompt }]),
                structured_content: Some(json!({ "playbook": playbook })),
                is_error: Some(false),
            })
        }
        None => Ok(not_found_error("playbook", id, ToolGroup::Playbook)),
    }
}

pub fn format_playbook_detailed(p: &Playbook) -> String {
    let mut lines = Vec::new();
    lines.push(format!("ID: {}", p.id));
    lines.push(format!("Goal: {}", p.goal));
    if let Some(cmd) = &p.initial_command {
        lines.push(format!("Initial Command: {}", cmd));
    }
    lines.push(format!("Steps: {}", p.workflow.len()));

    if let Some(target) = &p.default_target_session {
        match target.mode {
            TargetSessionMode::Pin => {
                let session_id = target.pinned_session_id().unwrap_or("(missing)");
                lines.push(format!("Start Launch Target: pin → session {}", session_id));
            }
            TargetSessionMode::Self_ => {
                lines.push("Start Launch Target: self (new session on Start)".to_string());
            }
        }
    }

    if !p.workflow.is_empty() {
        lines.push("\n--- Workflow ---".to_string());
        for (i, step) in p.workflow.iter().enumerate() {
            lines.push(format!(
                "{}. Step ID: {}",
                i + 1,
                step.step_id.as_deref().unwrap_or("N/A")
            ));
            lines.push(format!("   Description: {}", step.description));
            lines.push(format!(
                "   Tool: {} (Purpose: {})",
                step.action.tool_name, step.action.purpose
            ));
            if let Some(req) = &step.required_data {
                lines.push(format!("   Required Data: {}", req.join(", ")));
            }
            lines.push(format!("   Output Variable: {}", step.output_variable));
        }
    }

    if let Some(sc) = &p.success_criteria {
        lines.push("\n--- Success Criteria ---".to_string());
        lines.push(format!("Description: {}", sc.description));
        if let Some(arts) = &sc.required_artifacts {
            lines.push(format!("Required Artifacts: {}", arts.join(", ")));
        }
    }

    lines.join("\n")
}

pub async fn delete_playbook(assistant_id: &str, args: Value) -> Result<MCPResult, String> {
    let repo = crate::get_playbook_repository();

    let id = match args.get("id").and_then(|v| v.as_str()) {
        Some(id) if !id.trim().is_empty() => id,
        Some(_) => {
            return Ok(invalid_input_error(
                "Playbook ID cannot be empty",
                ToolGroup::Playbook,
            ))
        }
        None => return Ok(missing_param_error("id", ToolGroup::Playbook)),
    };

    let delete_result = repo.delete_playbook(id, assistant_id).await;

    match delete_result {
        Ok(_) => Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text: format!("Playbook '{}' deleted", id),
            }]),
            structured_content: Some(json!({ "success": true, "id": id })),
            is_error: Some(false),
        }),
        Err(e) => Ok(operation_failed_error(
            "deletePlaybook",
            &e.to_string(),
            vec![
                "Verify database is accessible".to_string(),
                "Check Playbook ID validity".to_string(),
            ],
            ToolGroup::Playbook,
        )),
    }
}

pub async fn update_playbook(
    assistant_id: &str,
    args: Value,
    caller_session_id: Option<&str>,
) -> Result<MCPResult, String> {
    let repo = crate::get_playbook_repository();

    let id = match args.get("id").and_then(|v| v.as_str()) {
        Some(id) if !id.trim().is_empty() => id,
        Some(_) => {
            return Ok(invalid_input_error(
                "Playbook ID cannot be empty",
                ToolGroup::Playbook,
            ))
        }
        None => return Ok(missing_param_error("id", ToolGroup::Playbook)),
    };

    let playbook_obj = match args.get("playbook") {
        Some(obj) if obj.is_object() => obj,
        Some(_) => {
            return Ok(invalid_input_error(
                "Playbook parameter must be an object",
                ToolGroup::Playbook,
            ))
        }
        None => return Ok(missing_param_error("playbook", ToolGroup::Playbook)),
    };

    // Fetch existing to merge
    let existing_model = match repo.get_playbook(id, assistant_id).await {
        Ok(Some(model)) => model,
        Ok(None) => return Ok(not_found_error("playbook", id, ToolGroup::Playbook)),
        Err(e) => {
            return Ok(operation_failed_error(
                "updatePlaybook",
                &format!("Database query failed: {}", e),
                vec!["Verify database is accessible".to_string()],
                ToolGroup::Playbook,
            ))
        }
    };

    let mut existing = Playbook::from_model(&existing_model);

    // Update fields if present
    if let Some(g) = playbook_obj.get("goal").and_then(|v| v.as_str()) {
        if g.trim().is_empty() {
            return Ok(invalid_input_error(
                "Goal cannot be empty",
                ToolGroup::Playbook,
            ));
        }
        existing.goal = g.to_string();
    }
    if let Some(c) = playbook_obj.get("initialCommand").and_then(|v| v.as_str()) {
        existing.initial_command = Some(c.to_string());
    }
    if let Some(w) = playbook_obj.get("workflow") {
        existing.workflow = match serde_json::from_value(w.clone()) {
            Ok(wf) => wf,
            Err(e) => {
                return Ok(invalid_input_error(
                    &format!("Invalid workflow format: {}", e),
                    ToolGroup::Playbook,
                ))
            }
        };
    }
    if let Some(s) = playbook_obj.get("successCriteria") {
        existing.success_criteria = match serde_json::from_value(s.clone()) {
            Ok(sc) => sc,
            Err(e) => {
                return Ok(invalid_input_error(
                    &format!("Invalid success criteria format: {}", e),
                    ToolGroup::Playbook,
                ))
            }
        };
    }
    if playbook_obj
        .as_object()
        .is_some_and(|obj| obj.contains_key("defaultTargetSession"))
    {
        match playbook_obj.get("defaultTargetSession") {
            Some(v) if v.is_null() => {
                existing.default_target_session = None;
            }
            Some(v) => match serde_json::from_value::<TargetSessionConfig>(v.clone()) {
                Ok(cfg) => {
                    match canonicalize_default_target_session(cfg, caller_session_id).await {
                        Ok(cfg) => {
                            existing.default_target_session = Some(cfg);
                        }
                        Err(err) => return Ok(err),
                    }
                }
                Err(e) => {
                    return Ok(invalid_input_error(
                        &format!("Invalid defaultTargetSession configuration: {}", e),
                        ToolGroup::Playbook,
                    ))
                }
            },
            None => {}
        }
    }

    let workflow_json = match serialize_workflow_steps(&existing.workflow) {
        Ok(json) => json,
        Err(e) => {
            return Ok(operation_failed_error(
                "updatePlaybook",
                &format!("Failed to serialize workflow: {}", e),
                vec!["Verify workflow structure is valid".to_string()],
                ToolGroup::Playbook,
            ))
        }
    };
    let pin_json =
        match serialize_default_target_session_column(existing.default_target_session.as_ref()) {
            Ok(json) => json,
            Err(e) => {
                return Ok(operation_failed_error(
                    "updatePlaybook",
                    &format!("Failed to serialize defaultTargetSession: {}", e),
                    vec!["Verify defaultTargetSession structure is valid".to_string()],
                    ToolGroup::Playbook,
                ))
            }
        };

    // Execute update via repository
    let updated_model = match repo
        .update_playbook(
            id,
            assistant_id,
            Some(existing.goal.clone()),
            Some(workflow_json),
            Some(pin_json),
            None,
        )
        .await
    {
        Ok(model) => model,
        Err(e) => {
            return Ok(operation_failed_error(
                "updatePlaybook",
                &format!("Database update failed: {}", e),
                vec!["Verify database is accessible".to_string()],
                ToolGroup::Playbook,
            ))
        }
    };

    let existing = Playbook::from_model(&updated_model);
    let formatted = format_playbook_summary(&existing);
    let text_response = format!(
        "Successfully updated playbook ID: {}\n\nUpdated Details:\n{}\n\nThe playbook has been modified. Changes are immediately available.",
        id, formatted
    );

    Ok(MCPResult {
        content: Some(vec![MCPContent::Text {
            text: text_response,
        }]),
        structured_content: Some(json!({
            "success": true,
            "playbook": existing
        })),
        is_error: Some(false),
    })
}
