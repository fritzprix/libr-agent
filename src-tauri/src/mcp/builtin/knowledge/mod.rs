use async_trait::async_trait;
use sea_orm::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::entity::{knowledge, knowledge::Entity as KnowledgeEntity};
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, not_found_error, operation_failed_error, SuccessHint,
    ToolGroup,
};
use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{
    BuiltinServerMetadata, MCPResult, MCPTool, ServiceContext, ServiceContextOptions,
};
use crate::mcp::utils::schema_builder::*;

/// Knowledge Server - Session-scoped knowledge base with full-text search
///
/// This server provides session-specific knowledge storage and retrieval using
/// SQLite FTS5 for efficient full-text search.
#[derive(Debug)]
pub struct KnowledgeServer {
    session_id: String,
    db: Arc<DatabaseConnection>,
}

impl KnowledgeServer {
    /// Create a new KnowledgeServer instance for a specific session
    pub async fn new(session_id: String, db: Arc<DatabaseConnection>) -> Result<Self, String> {
        let server = Self { session_id, db };

        Ok(server)
    }

    fn get_db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Save knowledge to the database
    async fn save_knowledge(&self, args: Value) -> Result<MCPResult, String> {
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

        // Handle tags as array of strings
        let tags_str = if let Some(tags_val) = args.get("tags") {
            if let Some(tags_arr) = tags_val.as_array() {
                // Validate all elements are strings
                if !tags_arr.iter().all(|t| t.is_string()) {
                    return Ok(invalid_input_error(
                        "Tags must be an array of strings",
                        ToolGroup::Knowledge,
                    ));
                }
                match serde_json::to_string(tags_arr) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        return Ok(operation_failed_error(
                            "Serialize tags",
                            &e.to_string(),
                            vec![
                                "Ensure tags are valid strings".to_string(),
                                "Check tag format".to_string(),
                                "Try without tags".to_string(),
                            ],
                            ToolGroup::Knowledge,
                        ))
                    }
                }
            } else {
                return Ok(invalid_input_error(
                    "Tags must be an array of strings",
                    ToolGroup::Knowledge,
                ));
            }
        } else {
            None
        };

        let now = chrono::Utc::now().timestamp_millis();
        let db = self.get_db();

        let model = knowledge::ActiveModel {
            id: NotSet,
            session_id: Set(self.session_id.clone()),
            title: Set(title.to_string()),
            content: Set(content.to_string()),
            source: Set(source.clone()),
            tags: Set(tags_str.clone()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let result = KnowledgeEntity::insert(model).exec(db).await;

        match result {
            Ok(insert_result) => {
                let id = insert_result.last_insert_id;

                // Parse tags back for response
                let tags_vec: Vec<String> = if let Some(s) = tags_str {
                    serde_json::from_str(&s).unwrap_or_default()
                } else {
                    Vec::new()
                };

                let knowledge = json!({
                    "id": id,
                    "session_id": self.session_id,
                    "title": title,
                    "content": content,
                    "source": source,
                    "tags": tags_vec,
                    "created_at": now,
                    "updated_at": now
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
            Err(e) => Ok(operation_failed_error(
                "Save knowledge",
                &e.to_string(),
                vec![
                    "Check database connectivity".to_string(),
                    "Verify title and content are valid".to_string(),
                    "Retry the operation".to_string(),
                ],
                ToolGroup::Knowledge,
            )),
        }
    }

    /// Read a knowledge entry by ID
    async fn read_knowledge(&self, args: Value) -> Result<MCPResult, String> {
        let id = match args.get("id").and_then(|v| v.as_i64()) {
            Some(v) => v,
            Option::None => return Ok(missing_param_error("id", ToolGroup::Knowledge)),
        };

        let db = self.get_db();
        let result = KnowledgeEntity::find()
            .filter(knowledge::Column::Id.eq(id))
            .filter(knowledge::Column::SessionId.eq(&self.session_id))
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
                let tags_vec: Vec<String> = if let Some(s) = tags_str {
                    serde_json::from_str(&s).unwrap_or_default()
                } else {
                    Vec::new()
                };

                let knowledge = json!({
                    "id": id,
                    "session_id": self.session_id,
                    "title": title,
                    "content": content,
                    "source": source,
                    "tags": tags_vec,
                    "created_at": created_at,
                    "updated_at": updated_at
                });

                let hint = SuccessHint::new(
                    format!("Knowledge: {}\n\n---\n{}\n---", title, content),
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

    /// Delete a knowledge entry by ID
    async fn delete_knowledge(&self, args: Value) -> Result<MCPResult, String> {
        let id = match args.get("id").and_then(|v| v.as_i64()) {
            Some(v) => v,
            Option::None => return Ok(missing_param_error("id", ToolGroup::Knowledge)),
        };

        let db = self.get_db();
        let result = KnowledgeEntity::delete_many()
            .filter(knowledge::Column::Id.eq(id))
            .filter(knowledge::Column::SessionId.eq(&self.session_id))
            .exec(db)
            .await;

        match result {
            Ok(delete_result) => {
                if delete_result.rows_affected > 0 {
                    let hint = SuccessHint::new(
                        format!("Knowledge entry {} deleted successfully", id),
                        vec!["Use listKnowledge to see remaining entries".to_string()],
                    );

                    Ok(hint.to_mcp_result_with_data(Some(json!({
                        "success": true,
                        "id": id
                    }))))
                } else {
                    Ok(not_found_error(
                        "Knowledge entry",
                        &id.to_string(),
                        ToolGroup::Knowledge,
                    ))
                }
            }
            Err(e) => Ok(operation_failed_error(
                "Delete knowledge",
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
    async fn search_knowledge(&self, args: Value) -> Result<MCPResult, String> {
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
            .unwrap_or(10)
            .min(100);

        let mut sql = String::from(
            "SELECT k.id, k.title, k.content, k.source, k.tags, k.created_at, k.updated_at",
        );

        if query_param.is_some() {
            sql.push_str(", snippet(knowledge_fts, 1, '**', '**', '...', 20) as snippet");
        } else {
            sql.push_str(", substr(k.content, 1, 150) as snippet");
        }

        sql.push_str(" FROM knowledge k");

        if query_param.is_some() {
            sql.push_str(" JOIN knowledge_fts f ON k.id = f.rowid");
        }

        sql.push_str(" WHERE k.session_id = ?");

        if query_param.is_some() {
            sql.push_str(" AND knowledge_fts MATCH ?");
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

        let mut values = vec![self.session_id.clone().into()];

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

        let db = self.get_db();
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

                        let tags_vec: Vec<String> = if let Some(s) = tags_str {
                            serde_json::from_str(&s).unwrap_or_default()
                        } else {
                            Vec::new()
                        };

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
    async fn list_knowledge(&self, args: Value) -> Result<MCPResult, String> {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(20)
            .min(100) as u64;

        let offset = args.get("offset").and_then(|v| v.as_i64()).unwrap_or(0) as u64;

        let db = self.get_db();
        let result = KnowledgeEntity::find()
            .filter(knowledge::Column::SessionId.eq(&self.session_id))
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
                        let tags_vec: Vec<String> = if let Some(s) = &model.tags {
                            serde_json::from_str(s).unwrap_or_default()
                        } else {
                            Vec::new()
                        };

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
}

#[async_trait]
impl BuiltinMCPServer for KnowledgeServer {
    fn name(&self) -> &str {
        "knowledge"
    }

    fn description(&self) -> &str {
        "Session-scoped knowledge base with full-text search"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn display_name(&self) -> String {
        "Knowledge Server".to_string()
    }

    fn metadata(&self) -> BuiltinServerMetadata {
        BuiltinServerMetadata {
            display_name: self.display_name(),
            description: self.description().to_string(),
            icon: Some("📚".to_string()),
        }
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            create_save_knowledge_tool(),
            create_read_knowledge_tool(),
            create_delete_knowledge_tool(),
            create_search_knowledge_tool(),
            create_list_knowledge_tool(),
        ]
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Query knowledge count with error handling using SeaORM
        let db = self.get_db();
        let count: u64 = KnowledgeEntity::find()
            .filter(knowledge::Column::SessionId.eq(&self.session_id))
            .count(db)
            .await
            .unwrap_or_else(|e| {
                log::warn!(
                    "Failed to query knowledge count for session '{}': {}",
                    self.session_id,
                    e
                );
                0
            });

        // Build context prompt
        let mut parts = vec!["## Knowledge Base".to_string()];

        if count == 0 {
            parts.push("\n**No knowledge entries yet.**".to_string());
            parts.push(
                "*Use saveKnowledge to store important information for future reference.*"
                    .to_string(),
            );
            parts.push(
                "*Tip: Save key facts, decisions, or context that might be useful later.*"
                    .to_string(),
            );
        } else {
            let entry_label = if count == 1 { "entry" } else { "entries" };
            parts.push(format!(
                "\n**{} knowledge {} available**",
                count, entry_label
            ));
        }

        ServiceContext {
            context_prompt: parts.join("\n"),
            structured_state: Some(json!({
                "session_id": self.session_id,
                "knowledge_count": count
            })),
        }
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
        log::debug!(
            "Knowledge server tool called: {} for session: {}",
            tool_name,
            self.session_id
        );

        match tool_name {
            "saveKnowledge" | "builtin_knowledge__saveKnowledge" => self.save_knowledge(args).await,
            "readKnowledge" | "builtin_knowledge__readKnowledge" => self.read_knowledge(args).await,
            "deleteKnowledge" | "builtin_knowledge__deleteKnowledge" => self.delete_knowledge(args).await,
            "searchKnowledge" | "builtin_knowledge__searchKnowledge" => {
                self.search_knowledge(args).await
            }
            "listKnowledge" | "builtin_knowledge__listKnowledge" => self.list_knowledge(args).await,
            _ => Err(format!(
                "Unknown tool: {}. Available tools: saveKnowledge, readKnowledge, deleteKnowledge, searchKnowledge, listKnowledge",
                tool_name
            )),
        }
    }

    async fn switch_context(&self, _options: ServiceContextOptions) -> Result<(), String> {
        // Knowledge server is session-bound, context switching not supported
        Err("Context switching not supported for session-bound knowledge server".to_string())
    }
}

/// Create the saveKnowledge tool definition
fn create_save_knowledge_tool() -> MCPTool {
    MCPTool {
        name: "saveKnowledge".to_string(),
        title: Some("Save Knowledge".to_string()),
        description: "Save a knowledge entry to the session-scoped knowledge base".to_string(),
        input_schema: serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title of the knowledge entry"
                },
                "content": {
                    "type": "string",
                    "description": "Content/body of the knowledge entry"
                },
                "source": {
                    "type": "string",
                    "description": "Source origin of the knowledge (e.g. URL, filename, 'user')"
                },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional tags for categorization"
                }
            },
            "required": ["title", "content"]
        }))
        .unwrap(),
        output_schema: None,
        annotations: None,
    }
}

/// Create the readKnowledge tool definition
fn create_read_knowledge_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "id".to_string(),
        integer_prop(None, None, Some("ID of the knowledge entry to read")),
    );

    MCPTool {
        name: "readKnowledge".to_string(),
        title: Some("Read Knowledge".to_string()),
        description: "Read a specific knowledge entry by ID".to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create the deleteKnowledge tool definition
fn create_delete_knowledge_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "id".to_string(),
        integer_prop(None, None, Some("ID of the knowledge entry to delete")),
    );

    MCPTool {
        name: "deleteKnowledge".to_string(),
        title: Some("Delete Knowledge".to_string()),
        description: "Delete a specific knowledge entry by ID".to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create the searchKnowledge tool definition
fn create_search_knowledge_tool() -> MCPTool {
    MCPTool {
        name: "searchKnowledge".to_string(),
        title: Some("Search Knowledge".to_string()),
        description: "Search the knowledge base using full-text search (FTS5) and/or tags"
            .to_string(),
        input_schema: serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (FTS5 full-text search)"
                },
                "source": {
                    "type": "string",
                    "description": "Filter by source"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Filter by tags"
                },
                "limit": {
                    "type": "integer",
                    "default": 10,
                    "maximum": 100,
                    "description": "Maximum number of results"
                }
            }
        }))
        .unwrap(),
        output_schema: None,
        annotations: None,
    }
}

/// Create the listKnowledge tool definition
fn create_list_knowledge_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "limit".to_string(),
        integer_prop_with_default(Some(1), Some(100), 20, Some("Maximum number of entries")),
    );
    props.insert(
        "offset".to_string(),
        integer_prop_with_default(Some(0), None, 0, Some("Offset for pagination")),
    );

    MCPTool {
        name: "listKnowledge".to_string(),
        title: Some("List Knowledge".to_string()),
        description: "List all knowledge entries for this session (paginated)".to_string(),
        input_schema: object_schema(props, vec![]),
        output_schema: None,
        annotations: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{knowledge, session};
    use sea_orm::{
        ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait, Schema, Set,
        Statement,
    };
    use std::sync::Arc;

    async fn create_test_db() -> Arc<DatabaseConnection> {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory database");

        let schema = Schema::new(db.get_database_backend());

        // Create sessions table
        let stmt = schema.create_table_from_entity(session::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create sessions table");

        // Create knowledge table
        let stmt = schema.create_table_from_entity(knowledge::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create knowledge table");

        // Create FTS table manually for testing search
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE VIRTUAL TABLE knowledge_fts USING fts5(title, content, tags, source, content='knowledge', content_rowid='id');".to_owned(),
        ))
        .await
        .expect("Failed to create FTS table");

        // Triggers for FTS sync
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TRIGGER knowledge_ai AFTER INSERT ON knowledge BEGIN
              INSERT INTO knowledge_fts(rowid, title, content, tags, source) VALUES (new.id, new.title, new.content, new.tags, new.source);
            END;".to_owned(),
        )).await.expect("Failed to create insert trigger");

        // Insert test sessions
        let sessions = vec!["test-session", "session-1", "session-2"];
        for id in sessions {
            let new_session = session::ActiveModel {
                id: Set(id.to_string()),
                status: Set("active".to_string()),
                created_at: Set(0),
                updated_at: Set(0),
                ..Default::default()
            };
            session::Entity::insert(new_session)
                .exec(&db)
                .await
                .expect("Failed to insert session");
        }

        Arc::new(db)
    }

    #[tokio::test]
    async fn test_save_and_search_knowledge() {
        let db = create_test_db().await;
        let server = KnowledgeServer::new("test-session".to_string(), db)
            .await
            .expect("Failed to create server");

        // Save knowledge
        let result = server
            .call_tool(
                "saveKnowledge",
                json!({
                    "title": "Rust Ownership",
                    "content": "Rust uses an ownership system to manage memory safety",
                    "tags": ["rust", "programming"]
                }),
            )
            .await
            .expect("Save should succeed");

        assert_eq!(result.is_error, Some(false));

        // Search for it
        let result = server
            .call_tool("searchKnowledge", json!({"query": "ownership"}))
            .await
            .expect("Search should succeed");

        assert_eq!(result.is_error, Some(false));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["count"], 1);
    }

    #[tokio::test]
    async fn test_list_knowledge() {
        let db = create_test_db().await;
        let server = KnowledgeServer::new("test-session".to_string(), db)
            .await
            .expect("Failed to create server");

        // Save multiple entries
        for i in 1..=3 {
            server
                .call_tool(
                    "saveKnowledge",
                    json!({
                        "title": format!("Entry {}", i),
                        "content": format!("Content {}", i)
                    }),
                )
                .await
                .expect("Save should succeed");
        }

        // List all
        let result = server
            .call_tool("listKnowledge", json!({}))
            .await
            .expect("List should succeed");

        assert_eq!(result.is_error, Some(false));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["count"], 3);
    }

    #[tokio::test]
    async fn test_session_isolation() {
        let db = create_test_db().await;

        let server1 = KnowledgeServer::new("session-1".to_string(), db.clone())
            .await
            .expect("Failed to create server1");

        let server2 = KnowledgeServer::new("session-2".to_string(), db)
            .await
            .expect("Failed to create server2");

        // Save to session 1
        server1
            .call_tool(
                "saveKnowledge",
                json!({"title": "Session 1 Data", "content": "Private"}),
            )
            .await
            .expect("Save should succeed");

        // List in session 2 - should be empty
        let result = server2
            .call_tool("listKnowledge", json!({}))
            .await
            .expect("List should succeed");

        let structured = result.structured_content.unwrap();
        assert_eq!(structured["count"], 0);
    }

    #[tokio::test]
    async fn test_knowledge_source_and_snippet() {
        let db = create_test_db().await;
        let server = KnowledgeServer::new("test-session".to_string(), db)
            .await
            .expect("Failed to create server");

        // Save knowledge with source
        let result = server
            .call_tool(
                "saveKnowledge",
                json!({
                    "title": "Document A",
                    "content": "This is a very important document content that should be snippeted. It contains specific keywords like pineapple and banana.",
                    "source": "http://example.com/doc-a",
                    "tags": ["test"]
                }),
            )
            .await
            .expect("Save should succeed");

        assert_eq!(result.is_error, Some(false));
        let structured = result.structured_content.unwrap();
        let knowledge = &structured["knowledge"];
        assert_eq!(knowledge["source"], "http://example.com/doc-a");

        // Search for it by keyword
        let result = server
            .call_tool("searchKnowledge", json!({"query": "pineapple"}))
            .await
            .expect("Search should succeed");

        assert_eq!(result.is_error, Some(false));
        let structured = result.structured_content.as_ref().unwrap();
        assert_eq!(structured["count"], 1);
        let first_result = &structured["results"][0];

        // Check source is returned
        assert_eq!(first_result["source"], "http://example.com/doc-a");

        // Check snippet is returned and contains the keyword
        let snippet = first_result["snippet"].as_str().unwrap();
        assert!(!snippet.is_empty());
        assert!(snippet.contains("pineapple") || snippet.contains("banana"));

        // Test filtering by source
        let result = server
            .call_tool(
                "searchKnowledge",
                json!({"source": "http://example.com/doc-a"}),
            )
            .await
            .expect("Source search should succeed");

        let structured = result.structured_content.as_ref().unwrap();
        assert_eq!(structured["count"], 1);

        // Test filtering by WRONG source
        let result = server
            .call_tool(
                "searchKnowledge",
                json!({"source": "http://example.com/wrong-doc"}),
            )
            .await
            .expect("Source search should succeed");

        let structured = result.structured_content.as_ref().unwrap();
        assert_eq!(structured["count"], 0);

        // Verify text response formatting
        if let Some(content_vec) = result.content {
            if let Some(crate::mcp::types::MCPContent::Text { text }) = content_vec.first() {
                println!("Text response: {}", text);
                // Note: emptiness check depends on search result, but here count is 0 so it should be "Found 0..."
                assert!(text.contains("Found 0 knowledge entries"));
            }
        }
    }
}
