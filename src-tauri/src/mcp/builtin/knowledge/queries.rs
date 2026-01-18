use sea_orm::*;
use serde_json::{json, Value};

use crate::entity::{knowledge, knowledge::Entity as KnowledgeEntity};
use crate::mcp::builtin::error_guidance::{
    missing_param_error, not_found_error, operation_failed_error, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;

use super::{helpers, KnowledgeServer};

/// Read a knowledge entry by ID
pub async fn read_knowledge(
    server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    let id = match args.get("id").and_then(|v| v.as_i64()) {
        Some(v) => v,
        Option::None => return Ok(missing_param_error("id", ToolGroup::Knowledge)),
    };

    let db = server.get_db();
    let result = KnowledgeEntity::find()
        .filter(knowledge::Column::Id.eq(id))
        .filter(knowledge::Column::AssistantId.eq(assistant_id))
        .one(db)
        .await;

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

            let knowledge = json!({
                "id": id,
                "assistant_id": assistant_id,
                "title": title,
                "content": content,
                "source": source,
                "tags": tags_vec,
                "created_at": created_at,
                "updated_at": updated_at
            });

            let hint = SuccessHint::new(
                format!("Knowledge [{}]: {}\n\n---\n{}\n---", id, title, content),
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
        Err(e) => Ok(operation_failed_error(
            "Read knowledge",
            &e.to_string(),
            vec![
                "Check database connectivity".to_string(),
                "Verify the ID is correct".to_string(),
                "Use listKnowledge to see available entries".to_string(),
            ],
            ToolGroup::Knowledge,
        )),
    }
}

/// Search knowledge using FTS5 full-text search
pub async fn search_knowledge(
    server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
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
        "SELECT k.id, k.title, k.content, k.source, k.tags, k.created_at, k.updated_at",
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

    sql.push_str(" WHERE k.assistant_id = ?");

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

    let mut values = vec![assistant_id.to_string().into()];

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
                    format!("- [{}] {}\n  > {}", id, title, snippet)
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
        Err(e) => Ok(operation_failed_error(
            "Search knowledge",
            &e.to_string(),
            vec![
                "Check search query format".to_string(),
                "Verify database connectivity".to_string(),
                "Use listKnowledge to see all entries".to_string(),
            ],
            ToolGroup::Knowledge,
        )),
    }
}

/// List all knowledge entries for this session
pub async fn list_knowledge(
    server: &KnowledgeServer,
    args: Value,
    assistant_id: &str,
) -> Result<MCPResult, String> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(helpers::DEFAULT_LIMIT as i64)
        .min(helpers::MAX_LIMIT as i64) as u64;

    let offset = args.get("offset").and_then(|v| v.as_i64()).unwrap_or(0) as u64;

    let db = server.get_db();
    let result = KnowledgeEntity::find()
        .filter(knowledge::Column::AssistantId.eq(assistant_id))
        .order_by_desc(knowledge::Column::UpdatedAt)
        .limit(limit)
        .offset(offset)
        .all(db)
        .await;

    match result {
        Ok(models) => {
            let items: Vec<Value> = models
                .into_iter()
                .map(|model| {
                    let tags_vec = helpers::parse_db_tags(model.tags.as_ref());

                    json!({
                        "id": model.id,
                        "title": model.title,
                        "content": model.content,
                        "source": model.source,
                        "tags": tags_vec,
                        "created_at": model.created_at,
                        "updated_at": model.updated_at
                    })
                })
                .collect();

            let items_summary = items
                .iter()
                .map(|v| {
                    let id = v["id"].as_i64().unwrap_or_default();
                    let title = v["title"].as_str().unwrap_or("Untitled");
                    format!("- [{}] {}", id, title)
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
                    vec!["Use saveKnowledge to create entries".to_string()]
                } else if items.len() as i64 == limit as i64 {
                    vec![format!("Use offset={} to see more entries", offset + limit)]
                } else {
                    vec!["Use readKnowledge to view full content".to_string()]
                },
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "items": items,
                "count": items.len(),
                "limit": limit,
                "offset": offset
            }))))
        }
        Err(e) => Ok(operation_failed_error(
            "List knowledge",
            &e.to_string(),
            vec![
                "Check database connectivity".to_string(),
                "Verify pagination parameters".to_string(),
                "Retry the operation".to_string(),
            ],
            ToolGroup::Knowledge,
        )),
    }
}
