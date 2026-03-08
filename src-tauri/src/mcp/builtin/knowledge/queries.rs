use sea_orm::*;
use serde_json::{json, Value};

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, not_found_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::KnowledgeRepository;

use super::{helpers, KnowledgeServer};

/// Read a knowledge entry by ID
pub async fn read_knowledge(
    _server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    let id = match args.get("id").and_then(|v| v.as_i64()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("id", ToolGroup::Knowledge)),
    };

    let repo = crate::get_knowledge_repository();

    // Try provided assistant_id first
    let mut result = repo.get_knowledge(id, assistant_id).await;

    // If not found and assistant_id isn't "global", try "global"
    if let Ok(Option::None) = result {
        if assistant_id != "global" {
            result = repo.get_knowledge(id, "global").await;
        }
    }

    match result {
        Ok(Some(model)) => {
            let id = model.id;
            let title = model.title.clone();
            let content = model.content.clone();
            let source = model.source.clone();
            let tags_str = model.tags.clone();
            let created_at = model.created_at;
            let updated_at = model.updated_at;
            let tags_vec = helpers::parse_db_tags(tags_str.as_ref());
            let actual_assistant_id = model.assistant_id.clone();

            let knowledge = json!({
                "id": id,
                "assistant_id": actual_assistant_id,
                "title": title,
                "content": content,
                "source": source,
                "tags": tags_vec,
                "created_at": created_at,
                "updated_at": updated_at
            });

            let hint = SuccessHint::new(
                format!(
                    "Knowledge [{}]: {}\nScope: {}\n\n---\n{}\n---",
                    id,
                    title,
                    if actual_assistant_id == "global" {
                        "global"
                    } else {
                        "assistant"
                    },
                    content
                ),
                vec![
                    "Use searchKnowledge to find related entries".to_string(),
                    "Use deleteKnowledge to remove this entry".to_string(),
                ],
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "knowledge": knowledge
            }))))
        }
        Ok(Option::None) => Ok(not_found_error(
            "Knowledge entry",
            &id.to_string(),
            ToolGroup::Knowledge,
        )),
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Read knowledge error: {}", e),
            ToolGroup::Knowledge,
        )
        .with_guidance(vec![
            "Check database connectivity".to_string(),
            "Verify the ID is correct".to_string(),
            "Use listKnowledge to see available entries".to_string(),
        ])
        .to_mcp_result()),
    }
}

/// Search knowledge using FTS5 full-text search across both global and assistant-specific scopes
pub async fn search_knowledge_both(
    server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    search_internal(server, args, Some(assistant_id)).await
}

/// Search knowledge in a specific scope
pub async fn search_knowledge(
    server: &KnowledgeServer,
    args: Value,
    _assistant_id: &str,
) -> Result<MCPResult, String> {
    search_internal(server, args, None).await
}

async fn search_internal(
    server: &KnowledgeServer,
    args: Value,
    both_assistant_id: Option<&str>,
) -> Result<MCPResult, String> {
    // If both_assistant_id is Some, we search in ("global", both_assistant_id)
    // If None, we use the assistant_id from arguments or default
    let assistant_id = args
        .get("assistant_id")
        .and_then(|v| v.as_str())
        .unwrap_or("global"); // Not used if both_assistant_id is set

    let query_param = args.get("query").and_then(|v| v.as_str());
    let tags_param = args.get("tags").and_then(|v| v.as_array());
    let source_param = args.get("source").and_then(|v| v.as_str());

    if query_param.is_none() && tags_param.is_none() && source_param.is_none() {
        return Ok(missing_param_error(
            "query, tags or source",
            ToolGroup::Knowledge,
        ));
    }

    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(helpers::DEFAULT_LIMIT as i64)
        .min(helpers::MAX_LIMIT as i64);

    let mut sql = String::from(
        "SELECT k.id, k.assistant_id, k.title, k.content, k.source, k.tags, k.created_at, k.updated_at",
    );

    if query_param.is_some() {
        sql.push_str(&format!(
            ", snippet({}, 1, '**', '**', '...', {}) as snippet",
            helpers::TABLE_FTS,
            helpers::FTS_SNIPPET_LENGTH
        ));
    } else {
        sql.push_str(&format!(
            ", substr(k.content, 1, {}) as snippet",
            helpers::DEFAULT_SNIPPET_LENGTH
        ));
    }

    sql.push_str(" FROM knowledge k");

    if query_param.is_some() {
        sql.push_str(&format!(" JOIN {} f ON k.id = f.rowid", helpers::TABLE_FTS));
    }

    if both_assistant_id.is_some() {
        sql.push_str(" WHERE k.assistant_id IN ('global', ?)");
    } else {
        sql.push_str(" WHERE k.assistant_id = ?");
    }

    if query_param.is_some() {
        sql.push_str(&format!(" AND {} MATCH ?", helpers::TABLE_FTS));
    }

    if source_param.is_some() {
        sql.push_str(" AND k.source LIKE ?");
    }

    if let Some(tags) = tags_param {
        for _ in tags {
            sql.push_str(" AND k.tags LIKE ?");
        }
    }

    if query_param.is_some() {
        sql.push_str(" ORDER BY rank");
    } else {
        sql.push_str(" ORDER BY k.updated_at DESC");
    }

    sql.push_str(" LIMIT ?");

    let mut values = vec![if let Some(aid) = both_assistant_id {
        aid.to_string().into()
    } else {
        assistant_id.to_string().into()
    }];

    if let Some(q) = query_param {
        values.push(q.into());
    }

    if let Some(s) = source_param {
        values.push(format!("%{}%", s).into());
    }

    if let Some(tags) = tags_param {
        for tag in tags {
            if let Some(tag_str) = tag.as_str() {
                values.push(format!("%\"{}\"%", tag_str).into());
            } else {
                values.push("%%".into());
            }
        }
    }

    values.push(limit.into());

    let stmt = Statement::from_sql_and_values(DbBackend::Sqlite, &sql, values);

    let db = server.get_db();
    let result = db.query_all(stmt).await;

    match result {
        Ok(rows) => {
            let results: Vec<Value> = rows
                .into_iter()
                .map(|row| {
                    let id: i64 = row.try_get("", "id").unwrap_or_default();
                    let assistant_id_val: String =
                        row.try_get("", "assistant_id").unwrap_or_default();
                    let title: String = row.try_get("", "title").unwrap_or_default();
                    let content: String = row.try_get("", "content").unwrap_or_default();
                    let source: Option<String> = row.try_get("", "source").ok();
                    let snippet: String = row.try_get("", "snippet").unwrap_or_default();
                    let tags_str: Option<String> = row.try_get("", "tags").ok();
                    let created_at: i64 = row.try_get("", "created_at").unwrap_or_default();
                    let updated_at: i64 = row.try_get("", "updated_at").unwrap_or_default();

                    let tags_vec = helpers::parse_db_tags(tags_str.as_ref());

                    json!({
                        "id": id,
                        "assistant_id": assistant_id_val,
                        "title": title,
                        "content": content,
                        "snippet": snippet,
                        "source": source,
                        "tags": tags_vec,
                        "created_at": created_at,
                        "updated_at": updated_at
                    })
                })
                .collect();

            let results_summary = results
                .iter()
                .map(|v| {
                    let id = v["id"].as_i64().unwrap_or_default();
                    let title = v["title"].as_str().unwrap_or("Untitled");
                    let snippet = v["snippet"].as_str().unwrap_or("");
                    let scope = if v["assistant_id"] == "global" {
                        "global"
                    } else {
                        "assistant"
                    };
                    format!("- [{}] {} (Scope: {})\n  > {}", id, title, scope, snippet)
                })
                .collect::<Vec<_>>()
                .join("\n");

            let message = if results.is_empty() {
                "Found 0 knowledge entries".to_string()
            } else {
                format!(
                    "Found {} knowledge entries:\n{}",
                    results.len(),
                    results_summary
                )
            };

            let hint = SuccessHint::new(
                message,
                if results.is_empty() {
                    vec![
                        "Try different search terms".to_string(),
                        "Use listKnowledge to see all entries".to_string(),
                    ]
                } else {
                    vec!["Use readKnowledge to view full content".to_string()]
                },
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "results": results,
                "count": results.len()
            }))))
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Search knowledge error: {}", e),
            ToolGroup::Knowledge,
        )
        .with_guidance(vec![
            "Check search query format".to_string(),
            "Verify database connectivity".to_string(),
            "Use listKnowledge to see all entries".to_string(),
        ])
        .to_mcp_result()),
    }
}

/// List all knowledge entries for this assistant and/or global scope
pub async fn list_knowledge_both(
    server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    list_internal(server, args, Some(assistant_id)).await
}

/// List all knowledge entries for a specific scope
pub async fn list_knowledge(
    server: &KnowledgeServer,
    args: Value,
    _assistant_id: &str,
) -> Result<MCPResult, String> {
    list_internal(server, args, None).await
}

async fn list_internal(
    server: &KnowledgeServer,
    args: Value,
    both_assistant_id: Option<&str>,
) -> Result<MCPResult, String> {
    let assistant_id = args
        .get("assistant_id")
        .and_then(|v| v.as_str())
        .unwrap_or("global"); // Not used if both_assistant_id is set

    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(helpers::DEFAULT_LIMIT as i64)
        .min(helpers::MAX_LIMIT as i64) as u64;

    let offset = args.get("offset").and_then(|v| v.as_i64()).unwrap_or(0) as u64;

    let db = server.get_db();

    let mut sql = String::from("SELECT id, assistant_id, title, content, source, tags, created_at, updated_at FROM knowledge");

    if both_assistant_id.is_some() {
        sql.push_str(" WHERE assistant_id IN ('global', ?)");
    } else {
        sql.push_str(" WHERE assistant_id = ?");
    }

    sql.push_str(" ORDER BY updated_at DESC LIMIT ? OFFSET ?");

    let mut values = vec![if let Some(aid) = both_assistant_id {
        aid.to_string().into()
    } else {
        assistant_id.to_string().into()
    }];
    values.push(limit.into());
    values.push(offset.into());

    let stmt = Statement::from_sql_and_values(DbBackend::Sqlite, &sql, values);
    let result = db.query_all(stmt).await;

    match result {
        Ok(rows) => {
            let items: Vec<Value> = rows
                .into_iter()
                .map(|row| {
                    let id: i64 = row.try_get("", "id").unwrap_or_default();
                    let assistant_id_val: String =
                        row.try_get("", "assistant_id").unwrap_or_default();
                    let title: String = row.try_get("", "title").unwrap_or_default();
                    let content: String = row.try_get("", "content").unwrap_or_default();
                    let source: Option<String> = row.try_get("", "source").ok();
                    let tags_str: Option<String> = row.try_get("", "tags").ok();
                    let created_at: i64 = row.try_get("", "created_at").unwrap_or_default();
                    let updated_at: i64 = row.try_get("", "updated_at").unwrap_or_default();

                    let tags_vec = helpers::parse_db_tags(tags_str.as_ref());

                    json!({
                        "id": id,
                        "assistant_id": assistant_id_val,
                        "title": title,
                        "content": content,
                        "source": source,
                        "tags": tags_vec,
                        "created_at": created_at,
                        "updated_at": updated_at
                    })
                })
                .collect();

            let items_summary = items
                .iter()
                .map(|v| {
                    let id = v["id"].as_i64().unwrap_or_default();
                    let title = v["title"].as_str().unwrap_or("Untitled");
                    let scope = if v["assistant_id"] == "global" {
                        "global"
                    } else {
                        "assistant"
                    };
                    format!("- [{}] {} (Scope: {})", id, title, scope)
                })
                .collect::<Vec<_>>()
                .join("\n");

            let message = if items.is_empty() {
                "Listed 0 knowledge entries".to_string()
            } else {
                format!(
                    "Listed {} knowledge entries:\n{}",
                    items.len(),
                    items_summary
                )
            };

            let hint = SuccessHint::new(
                message,
                if items.is_empty() {
                    vec!["Use saveKnowledge to add entries".to_string()]
                } else {
                    vec!["Use readKnowledge to view full content".to_string()]
                },
            );

            Ok(hint.to_mcp_result_with_data(Some(
                json!({ "items": items, "page_count": items.len() }),
            )))
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("List knowledge error: {}", e),
            ToolGroup::Knowledge,
        )
        .with_guidance(vec![
            "Check database connectivity".to_string(),
            "Retry the operation".to_string(),
        ])
        .to_mcp_result()),
    }
}
