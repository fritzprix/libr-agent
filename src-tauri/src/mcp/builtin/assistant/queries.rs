use crate::entity::{assistant, assistant::Entity as AssistantEntity};
use crate::mcp::builtin::error_guidance::{
    missing_param_error, not_found_error, operation_failed_error, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use sea_orm::*;
use serde_json::{json, Value};

/// List all assistants with pagination support
pub async fn list_assistants(db: &DatabaseConnection, args: Value) -> Result<MCPResult, String> {
    // Legacy support: page/pageSize -> limit/offset
    let page = args
        .get("page")
        .and_then(|v| v.as_i64())
        .unwrap_or(1)
        .max(1);
    let page_size = args
        .get("pageSize")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .clamp(1, 100);

    // Also support direct limit/offset if provided (v2 native)
    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(page_size)
        .clamp(1, 100);
    let offset = args
        .get("offset")
        .and_then(|v| v.as_i64())
        .unwrap_or((page - 1) * page_size);

    // Get total count for pagination metadata
    let total_count = AssistantEntity::find().count(db).await.unwrap_or(0) as i64;

    // Fetch paginated results
    let result = AssistantEntity::find()
        .order_by_desc(assistant::Column::UpdatedAt)
        .limit(limit as u64)
        .offset(offset as u64)
        .all(db)
        .await;

    match result {
        Ok(models) => {
            let assistants: Vec<Value> = models
                .into_iter()
                .map(|model| {
                    // Parse config JSON
                    let config = serde_json::from_str::<Value>(&model.config).unwrap_or(json!({}));

                    json!({
                        "id": model.id,
                        "name": model.name,
                        "config": config,
                        "created_at": model.created_at,
                        "updated_at": model.updated_at
                    })
                })
                .collect();

            let has_more = (offset + limit) < total_count;

            let hint = SuccessHint::new(
                format!(
                    "Found {} of {} assistants (showing {} to {})",
                    total_count,
                    total_count,
                    offset + 1,
                    (offset + assistants.len() as i64).min(total_count)
                ),
                if has_more {
                    vec![format!(
                        "Use limit={} offset={} to see more assistants",
                        limit,
                        offset + limit
                    )]
                } else if total_count > 0 {
                    vec!["Use builtin_assistant__getAssistant to view details".to_string()]
                } else {
                    vec![
                        "Use builtin_assistant__createAssistant to create an assistant".to_string(),
                    ]
                },
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "assistants": assistants,
                "total": total_count,
                "limit": limit,
                "offset": offset,
                "returned": assistants.len(),
                "has_more": has_more
            }))))
        }
        Err(e) => Ok(operation_failed_error(
            "List assistants",
            &e.to_string(),
            vec![
                "Check database connectivity".to_string(),
                "Verify pagination parameters are valid integers".to_string(),
            ],
            ToolGroup::Assistant,
        )),
    }
}

/// Search assistants
pub async fn search_assistant(db: &DatabaseConnection, args: Value) -> Result<MCPResult, String> {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("query", ToolGroup::Assistant)),
    };

    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(10)
        .min(100);

    let search_pattern = format!("%{}%", query);

    let result = AssistantEntity::find()
        .filter(
            Condition::any()
                .add(assistant::Column::Name.like(&search_pattern))
                .add(assistant::Column::Config.like(&search_pattern)),
        )
        .order_by_desc(assistant::Column::UpdatedAt)
        .limit(limit as u64)
        .all(db)
        .await;

    match result {
        Ok(models) => {
            let assistants: Vec<Value> = models
                .into_iter()
                .map(|model| {
                    let config = serde_json::from_str::<Value>(&model.config).unwrap_or(json!({}));
                    json!({
                        "id": model.id,
                        "name": model.name,
                        "config": config,
                        "created_at": model.created_at,
                        "updated_at": model.updated_at
                    })
                })
                .collect();

            let hint = SuccessHint::new(
                format!("Found {} assistants matching '{}'", assistants.len(), query),
                if assistants.is_empty() {
                    vec!["Use builtin_assistant__listAssistants to see all assistants".to_string()]
                } else {
                    vec!["Use builtin_assistant__getAssistant to view details".to_string()]
                },
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "assistants": assistants,
                "count": assistants.len()
            }))))
        }
        Err(e) => Ok(operation_failed_error(
            "Search assistants",
            &e.to_string(),
            vec![
                "Check database connectivity".to_string(),
                "Verify query parameter is a valid string".to_string(),
            ],
            ToolGroup::Assistant,
        )),
    }
}

/// Get an assistant by ID
pub async fn get_assistant(db: &DatabaseConnection, args: Value) -> Result<MCPResult, String> {
    let id = match args.get("id").and_then(|v| v.as_str()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("id", ToolGroup::Assistant)),
    };

    let result = AssistantEntity::find_by_id(id).one(db).await;

    match result {
        Ok(Some(model)) => {
            // Parse config JSON
            let config = serde_json::from_str::<Value>(&model.config).unwrap_or(json!({}));

            let hint = SuccessHint::new(
                format!("Assistant: {}", model.name),
                vec![
                    "Use builtin_assistant__updateAssistant to modify configuration".to_string(),
                    "Use builtin_assistant__deleteAssistant to remove this assistant".to_string(),
                ],
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "id": model.id,
                "name": model.name,
                "config": config,
                "created_at": model.created_at,
                "updated_at": model.updated_at
            }))))
        }
        Ok(None) => Ok(not_found_error("Assistant", id, ToolGroup::Assistant)),
        Err(e) => Ok(operation_failed_error(
            "Get assistant",
            &e.to_string(),
            vec![
                "Verify the assistant ID is correct".to_string(),
                "Use builtin_assistant__listAssistants to see existing assistants".to_string(),
                "Check database connectivity".to_string(),
            ],
            ToolGroup::Assistant,
        )),
    }
}
