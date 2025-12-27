use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

use crate::mcp::builtin::BuiltinMCPServer;
use crate::mcp::types::{
    BuiltinServerMetadata, MCPContent, MCPResult, MCPTool, ServiceContext, ServiceContextOptions,
};
use crate::mcp::utils::schema_builder::*;

/// Knowledge Server - Session-scoped knowledge base with full-text search
///
/// This server provides session-specific knowledge storage and retrieval using
/// SQLite FTS5 for efficient full-text search.
#[derive(Debug)]
pub struct KnowledgeServer {
    session_id: String,
    db_pool: Arc<SqlitePool>,
}

impl KnowledgeServer {
    /// Create a new KnowledgeServer instance for a specific session
    pub async fn new(session_id: String, db_pool: Arc<SqlitePool>) -> Result<Self, String> {
        let server = Self {
            session_id,
            db_pool,
        };

        // Initialize database tables
        server.init_tables().await?;

        Ok(server)
    }

    /// Initialize database tables for knowledge storage
    async fn init_tables(&self) -> Result<(), String> {
        // Main knowledge table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS knowledge (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                tags TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create knowledge table: {}", e))?;

        // Create index on session_id for fast filtering
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_knowledge_session
            ON knowledge(session_id)
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create session index: {}", e))?;

        // Create FTS5 virtual table for full-text search
        sqlx::query(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts
            USING fts5(title, content, content=knowledge, content_rowid=id)
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create FTS5 table: {}", e))?;

        // Create triggers to keep FTS5 in sync
        sqlx::query(
            r#"
            CREATE TRIGGER IF NOT EXISTS knowledge_ai AFTER INSERT ON knowledge BEGIN
                INSERT INTO knowledge_fts(rowid, title, content)
                VALUES (new.id, new.title, new.content);
            END
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create insert trigger: {}", e))?;

        sqlx::query(
            r#"
            CREATE TRIGGER IF NOT EXISTS knowledge_ad AFTER DELETE ON knowledge BEGIN
                INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content)
                VALUES('delete', old.id, old.title, old.content);
            END
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create delete trigger: {}", e))?;

        sqlx::query(
            r#"
            CREATE TRIGGER IF NOT EXISTS knowledge_au AFTER UPDATE ON knowledge BEGIN
                INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content)
                VALUES('delete', old.id, old.title, old.content);
                INSERT INTO knowledge_fts(rowid, title, content)
                VALUES (new.id, new.title, new.content);
            END
            "#,
        )
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to create update trigger: {}", e))?;

        Ok(())
    }

    /// Save knowledge to the database
    async fn save_knowledge(&self, args: Value) -> Result<MCPResult, String> {
        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'title' parameter")?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'content' parameter")?;

        let tags = args.get("tags").and_then(|v| v.as_str());

        let now = chrono::Utc::now().timestamp_millis();

        let result = sqlx::query(
            r#"
            INSERT INTO knowledge (session_id, title, content, tags, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&self.session_id)
        .bind(title)
        .bind(content)
        .bind(tags)
        .bind(now)
        .bind(now)
        .execute(self.db_pool.as_ref())
        .await;

        match result {
            Ok(query_result) => {
                let id = query_result.last_insert_rowid();
                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: format!("Knowledge saved successfully with ID: {}", id),
                    }]),
                    structured_content: Some(json!({
                        "success": true,
                        "id": id,
                        "title": title,
                        "session_id": self.session_id
                    })),
                    is_error: Some(false),
                })
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to save knowledge: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// Search knowledge using FTS5 full-text search
    async fn search_knowledge(&self, args: Value) -> Result<MCPResult, String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'query' parameter")?;

        let limit = args
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(10)
            .min(100);

        let result = sqlx::query_as::<_, (i64, String, String, Option<String>, i64, i64)>(
            r#"
            SELECT k.id, k.title, k.content, k.tags, k.created_at, k.updated_at
            FROM knowledge k
            JOIN knowledge_fts f ON k.id = f.rowid
            WHERE knowledge_fts MATCH ?
            AND k.session_id = ?
            ORDER BY rank
            LIMIT ?
            "#,
        )
        .bind(query)
        .bind(&self.session_id)
        .bind(limit)
        .fetch_all(self.db_pool.as_ref())
        .await;

        match result {
            Ok(rows) => {
                let results: Vec<Value> = rows
                    .into_iter()
                    .map(|(id, title, content, tags, created_at, updated_at)| {
                        json!({
                            "id": id,
                            "title": title,
                            "content": content,
                            "tags": tags,
                            "created_at": created_at,
                            "updated_at": updated_at
                        })
                    })
                    .collect();

                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: format!("Found {} results", results.len()),
                    }]),
                    structured_content: Some(json!({
                        "results": results,
                        "count": results.len()
                    })),
                    is_error: Some(false),
                })
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Search failed: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
        }
    }

    /// List all knowledge entries for this session
    async fn list_knowledge(&self, args: Value) -> Result<MCPResult, String> {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(20)
            .min(100);

        let offset = args.get("offset").and_then(|v| v.as_i64()).unwrap_or(0);

        let result = sqlx::query_as::<_, (i64, String, String, Option<String>, i64, i64)>(
            r#"
            SELECT id, title, content, tags, created_at, updated_at
            FROM knowledge
            WHERE session_id = ?
            ORDER BY updated_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(&self.session_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.db_pool.as_ref())
        .await;

        match result {
            Ok(rows) => {
                let items: Vec<Value> = rows
                    .into_iter()
                    .map(|(id, title, content, tags, created_at, updated_at)| {
                        json!({
                            "id": id,
                            "title": title,
                            "content": content,
                            "tags": tags,
                            "created_at": created_at,
                            "updated_at": updated_at
                        })
                    })
                    .collect();

                Ok(MCPResult {
                    content: Some(vec![MCPContent::Text {
                        text: format!("Listed {} knowledge entries", items.len()),
                    }]),
                    structured_content: Some(json!({
                        "items": items,
                        "count": items.len(),
                        "limit": limit,
                        "offset": offset
                    })),
                    is_error: Some(false),
                })
            }
            Err(e) => Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!("Failed to list knowledge: {}", e),
                }]),
                structured_content: None,
                is_error: Some(true),
            }),
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
            create_search_knowledge_tool(),
            create_list_knowledge_tool(),
        ]
    }

    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // Query knowledge count with error handling
        let count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM knowledge WHERE session_id = ?")
                .bind(&self.session_id)
                .fetch_one(self.db_pool.as_ref())
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
            "searchKnowledge" | "builtin_knowledge__searchKnowledge" => {
                self.search_knowledge(args).await
            }
            "listKnowledge" | "builtin_knowledge__listKnowledge" => self.list_knowledge(args).await,
            _ => Err(format!(
                "Unknown tool: {}. Available tools: saveKnowledge, searchKnowledge, listKnowledge",
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
    let mut props = HashMap::new();
    props.insert(
        "title".to_string(),
        string_prop_required("Title of the knowledge entry"),
    );
    props.insert(
        "content".to_string(),
        string_prop_required("Content/body of the knowledge entry"),
    );
    props.insert(
        "tags".to_string(),
        string_prop(None, None, Some("Optional comma-separated tags")),
    );

    MCPTool {
        name: "saveKnowledge".to_string(),
        title: Some("Save Knowledge".to_string()),
        description: "Save a knowledge entry to the session-scoped knowledge base".to_string(),
        input_schema: object_schema(props, vec!["title".to_string(), "content".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create the searchKnowledge tool definition
fn create_search_knowledge_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "query".to_string(),
        string_prop_required("Search query (FTS5 full-text search)"),
    );
    props.insert(
        "limit".to_string(),
        integer_prop_with_default(Some(1), Some(100), 10, Some("Maximum number of results")),
    );

    MCPTool {
        name: "searchKnowledge".to_string(),
        title: Some("Search Knowledge".to_string()),
        description: "Search the knowledge base using full-text search (FTS5)".to_string(),
        input_schema: object_schema(props, vec!["query".to_string()]),
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
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn create_test_pool() -> SqlitePool {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("Invalid database URL")
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .expect("Failed to create test pool");

        // Create sessions table for FOREIGN KEY constraint (not currently used but good practice)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                name TEXT,
                status TEXT DEFAULT 'idle',
                agent_config TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create sessions table");

        // Insert test sessions
        sqlx::query("INSERT OR IGNORE INTO sessions (id, name, status, created_at, updated_at) VALUES ('test-session', 'Test', 'idle', 0, 0)")
            .execute(&pool)
            .await
            .expect("Failed to insert test session");

        sqlx::query("INSERT OR IGNORE INTO sessions (id, name, status, created_at, updated_at) VALUES ('session-1', 'Session 1', 'idle', 0, 0)")
            .execute(&pool)
            .await
            .expect("Failed to insert session 1");

        sqlx::query("INSERT OR IGNORE INTO sessions (id, name, status, created_at, updated_at) VALUES ('session-2', 'Session 2', 'idle', 0, 0)")
            .execute(&pool)
            .await
            .expect("Failed to insert session 2");

        pool
    }

    #[tokio::test]
    async fn test_save_and_search_knowledge() {
        let pool = Arc::new(create_test_pool().await);
        let server = KnowledgeServer::new("test-session".to_string(), pool)
            .await
            .expect("Failed to create server");

        // Save knowledge
        let result = server
            .call_tool(
                "saveKnowledge",
                json!({
                    "title": "Rust Ownership",
                    "content": "Rust uses an ownership system to manage memory safety",
                    "tags": "rust,programming"
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
        let pool = Arc::new(create_test_pool().await);
        let server = KnowledgeServer::new("test-session".to_string(), pool)
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
        let pool = Arc::new(create_test_pool().await);

        let server1 = KnowledgeServer::new("session-1".to_string(), pool.clone())
            .await
            .expect("Failed to create server1");

        let server2 = KnowledgeServer::new("session-2".to_string(), pool)
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
}
