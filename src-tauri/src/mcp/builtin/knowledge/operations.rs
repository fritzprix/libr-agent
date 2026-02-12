use serde_json::{json, Value};

use crate::mcp::builtin::error_guidance::{
    guided_error, invalid_input_error, missing_param_error, not_found_error, ErrorCategory,
    SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::KnowledgeRepository;

use super::{helpers, KnowledgeServer};

/// Save knowledge to the database
pub async fn save_knowledge(
    _server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    let title = match args.get("title").and_then(|v| v.as_str()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("title", ToolGroup::Knowledge)),
    };

    let content = match args.get("content").and_then(|v| v.as_str()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("content", ToolGroup::Knowledge)),
    };

    let source = args
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Handle tags using helper
    let tags_val = args.get("tags");
    // Validate that if tags exist, they are an array (logic inside helper is lenient but Blueprint requires validation).
    // The blueprint says "Validate inputs BEFORE operations".
    // The original code returned `invalid_input_error` if tags wasn't an array of strings.
    // Let's preserve that validation logic if strictness is required, or trust the helper if lenience is preferred.
    // The original code was strict: `if !tags_arr.iter().all(|t| t.is_string()) ...`
    // However, the helper `parse_tags` handles `Some(val)` by filtering strings.
    // To match original strict validation behavior:
    if let Some(val) = tags_val {
        if !val.is_array() && !val.is_null() {
            return Ok(invalid_input_error(
                "Tags must be an array of strings",
                ToolGroup::Knowledge,
            ));
        }
        if let Some(arr) = val.as_array() {
            if !arr.iter().all(|t| t.is_string()) {
                return Ok(invalid_input_error(
                    "Tags must be an array of strings",
                    ToolGroup::Knowledge,
                ));
            }
        }
    }

    let tags_str = if let Some(val) = tags_val {
        match serde_json::to_string(val) {
            Ok(s) => Some(s),
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Serialize tags error: {}", e),
                    ToolGroup::Knowledge,
                )
                .with_guidance(vec!["Ensure tags are valid JSON".to_string()])
                .to_mcp_result())
            }
        }
    } else {
        None
    };

    let repo = crate::get_knowledge_repository();
    let created = repo
        .create_knowledge(
            assistant_id.to_string(),
            title.to_string(),
            content.to_string(),
            source.clone(),
            tags_str.clone(),
        )
        .await;

    match created {
        Ok(model) => {
            let id = model.id;

            // Parse tags back for response
            let tags_vec = helpers::parse_db_tags(tags_str.as_ref());

            let knowledge = json!({
                "id": id,
                "assistant_id": assistant_id,
                "title": title,
                "content": content,
                "source": source,
                "tags": tags_vec,
                "created_at": model.created_at,
                "updated_at": model.updated_at
            });

            let hint = SuccessHint::new(
                format!("Knowledge '{}' saved (ID: {})", title, id),
                vec![
                    "Use searchKnowledge to find this entry later".to_string(),
                    "Use listKnowledge to see all knowledge entries".to_string(),
                ],
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "knowledge": knowledge
            }))))
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Save knowledge error: {}", e),
            ToolGroup::Knowledge,
        )
        .with_guidance(vec![
            "Check database connectivity".to_string(),
            "Verify title and content are valid".to_string(),
            "Retry the operation".to_string(),
        ])
        .to_mcp_result()),
    }
}

/// Delete a knowledge entry by ID
pub async fn delete_knowledge(
    _server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    let id = match args.get("id").and_then(|v| v.as_i64()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("id", ToolGroup::Knowledge)),
    };

    let repo = crate::get_knowledge_repository();
    let result = repo.delete_knowledge(id, assistant_id).await;

    match result {
        Ok(_) => {
            let hint = SuccessHint::new(
                format!("Knowledge entry {} deleted successfully", id),
                vec!["Use listKnowledge to see remaining entries".to_string()],
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": id
            }))))
        }
        Err(_) => Ok(not_found_error(
            "Knowledge entry",
            &id.to_string(),
            ToolGroup::Knowledge,
        )),
    }
}
