use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::collections::HashMap;

/// Data models for content store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentStore {
    pub session_id: String, // Primary key: session ID (1:1 relationship with session)
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    pub id: String,
    pub session_id: String, // References ContentStore.session_id
    pub filename: String,
    pub mime_type: String,
    pub size: usize,
    pub line_count: usize,
    pub preview: String,
    pub uploaded_at: String,
    pub chunk_count: usize,
    pub last_accessed_at: String,
    // Full content storage (like web-mcp FileContent)
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentChunk {
    pub id: String,
    pub content_id: String,
    pub chunk_index: usize,
    pub text: String,
    pub line_range: (usize, usize), // (start_line, end_line)
}

/// Content store storage implementation
#[derive(Debug)]
pub struct ContentStoreStorage {
    // In-memory storage
    stores: HashMap<String, ContentStore>,
    contents: HashMap<String, ContentItem>,
    chunks: HashMap<String, Vec<ContentChunk>>,
    // SQLite connection (when using SQLite backend)
    sqlite_pool: Option<SqlitePool>,
}

impl ContentStoreStorage {
    /// Create in-memory storage (default)
    pub fn new() -> Self {
        Self {
            stores: HashMap::new(),
            contents: HashMap::new(),
            chunks: HashMap::new(),
            sqlite_pool: None,
        }
    }

    /// Create SQLite-backed storage
    pub async fn new_sqlite(database_url: String) -> Result<Self, String> {
        // Extract database path from URL and ensure directory exists
        let db_path = if let Some(path) = database_url.strip_prefix("sqlite://") {
            path.to_string()
        } else {
            database_url.clone()
        };

        // Ensure database directory exists
        if let Some(parent_dir) = std::path::Path::new(&db_path).parent() {
            std::fs::create_dir_all(parent_dir)
                .map_err(|e| format!("Failed to create database directory: {e}"))?;
        }

        // Create the database file if it doesn't exist (required for sqlx)
        if !std::path::Path::new(&db_path).exists() {
            std::fs::File::create(&db_path)
                .map_err(|e| format!("Failed to create database file: {e}"))?;
        }

        // Connect using the file path directly
        let pool = SqlitePool::connect(&db_path)
            .await
            .map_err(|e| format!("Failed to connect to SQLite: {e}"))?;

        // Create tables if they don't exist
        Self::create_tables(&pool).await?;

        Ok(Self {
            stores: HashMap::new(), // Keep in-memory cache for performance
            contents: HashMap::new(),
            chunks: HashMap::new(),
            sqlite_pool: Some(pool),
        })
    }

    /// Create database tables
    async fn create_tables(pool: &SqlitePool) -> Result<(), String> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS stores (
                session_id TEXT PRIMARY KEY,
                name TEXT,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS contents (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                mime_type TEXT NOT NULL,
                size INTEGER NOT NULL,
                line_count INTEGER NOT NULL,
                preview TEXT NOT NULL,
                uploaded_at TEXT NOT NULL,
                chunk_count INTEGER NOT NULL,
                last_accessed_at TEXT NOT NULL,
                content TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES stores(session_id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                content_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                text TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                FOREIGN KEY (content_id) REFERENCES contents(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_content_id ON chunks(content_id);
            CREATE INDEX IF NOT EXISTS idx_contents_session_id ON contents(session_id);
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create tables: {e}"))?;

        Ok(())
    }

    /// Create a new content store for a session (1:1 relationship)
    pub async fn create_store(
        &mut self,
        session_id: String,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<ContentStore, String> {
        // Check if store already exists for this session
        if self.stores.contains_key(&session_id) {
            return Err(format!(
                "Content store already exists for session: {session_id}"
            ));
        }

        let now = chrono::Utc::now().to_rfc3339();

        let store = ContentStore {
            session_id: session_id.clone(),
            name: name.clone(),
            description: description.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        // SQLite backend
        if let Some(pool) = &self.sqlite_pool {
            sqlx::query(
                "INSERT INTO stores (session_id, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(&session_id)
            .bind(&name)
            .bind(&description)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create store in SQLite: {e}"))?;
        }

        // In-memory cache (always updated for performance)
        self.stores.insert(session_id.clone(), store.clone());
        Ok(store)
    }

    /// Check if a content store exists for the given session ID
    pub fn store_exists(&self, session_id: &str) -> bool {
        self.stores.contains_key(session_id)
    }

    /// Get debug information about all stores
    #[allow(dead_code)]
    pub fn debug_stores_info(&self) -> Vec<String> {
        self.stores
            .iter()
            .map(|(id, store)| {
                format!(
                    "Store {}: name={}",
                    id,
                    store.name.as_deref().unwrap_or("unnamed")
                )
            })
            .collect()
    }

    /// Get content count for a specific session
    pub fn get_content_count(&self, session_id: &str) -> usize {
        self.contents
            .values()
            .filter(|content| content.session_id == session_id)
            .count()
    }

    /// Get detailed content summary for a specific session
    pub fn get_content_summary(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Vec<(String, usize, String)> {
        self.contents
            .values()
            .filter(|content| content.session_id == session_id)
            .take(limit)
            .map(|content| {
                // Get first 200 characters of content as preview
                let preview = if content.content.len() > 200 {
                    format!("{}...", &content.content[..200])
                } else {
                    content.content.clone()
                };
                (content.filename.clone(), content.size, preview)
            })
            .collect()
    }

    /// Get or create a content store for the given session ID
    pub async fn get_or_create_store(
        &mut self,
        session_id: String,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<ContentStore, String> {
        // Check if store already exists
        if self.store_exists(&session_id) {
            // Return existing store
            if let Some(store) = self.stores.get(&session_id) {
                Ok(store.clone())
            } else {
                Err(format!(
                    "Store exists but could not retrieve for session: {session_id}"
                ))
            }
        } else {
            // Create new store
            self.create_store(session_id, name, description).await
        }
    }

    /// Add content to a session's store
    pub async fn add_content(
        &mut self,
        session_id: &str,
        filename: &str,
        mime_type: &str,
        size: usize,
        content: &str,
        chunks: Vec<String>,
    ) -> Result<ContentItem, String> {
        // Verify store exists for this session
        if !self.stores.contains_key(session_id) {
            return Err(format!("Content store not found for session: {session_id}"));
        }

        let content_id = format!("content_{}", cuid2::create_id());
        let now = chrono::Utc::now().to_rfc3339();

        // Create content chunks
        let content_chunks: Vec<ContentChunk> = chunks
            .into_iter()
            .enumerate()
            .map(|(index, text)| {
                let start_line = index * 10 + 1; // Rough estimate
                let end_line = start_line + text.lines().count().saturating_sub(1);

                ContentChunk {
                    id: format!("chunk_{content_id}_{index}"),
                    content_id: content_id.clone(),
                    chunk_index: index,
                    text,
                    line_range: (start_line, end_line),
                }
            })
            .collect();

        let chunk_count = content_chunks.len();
        let line_count = content.lines().count();
        let preview = content.chars().take(200).collect::<String>();

        let content_item = ContentItem {
            id: content_id.clone(),
            session_id: session_id.to_string(),
            filename: filename.to_string(),
            mime_type: mime_type.to_string(),
            size,
            line_count,
            preview,
            uploaded_at: now.clone(),
            chunk_count,
            last_accessed_at: now,
            content: content.to_string(),
        };

        self.contents
            .insert(content_id.clone(), content_item.clone());
        self.chunks
            .insert(content_id.clone(), content_chunks.clone());

        // SQLite backend
        if let Some(pool) = &self.sqlite_pool {
            // Insert content
            sqlx::query(
                "INSERT INTO contents (id, session_id, filename, mime_type, size, line_count, preview, uploaded_at, chunk_count, last_accessed_at, content) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(content_id)
            .bind(session_id)
            .bind(filename)
            .bind(mime_type)
            .bind(size as i64)
            .bind(line_count as i64)
            .bind(&content_item.preview)
            .bind(&content_item.uploaded_at)
            .bind(chunk_count as i64)
            .bind(&content_item.last_accessed_at)
            .bind(content)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to save content to SQLite: {e}"))?;

            // Insert chunks
            for chunk in &content_chunks {
                sqlx::query(
                    "INSERT INTO chunks (id, content_id, chunk_index, text, start_line, end_line) VALUES (?, ?, ?, ?, ?, ?)"
                )
                .bind(&chunk.id)
                .bind(&chunk.content_id)
                .bind(chunk.chunk_index as i64)
                .bind(&chunk.text)
                .bind(chunk.line_range.0 as i64)
                .bind(chunk.line_range.1 as i64)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to save chunk to SQLite: {e}"))?;
            }
        }

        Ok(content_item)
    }

    /// List content in a session's store with pagination
    pub async fn list_content(
        &self,
        session_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<ContentItem>, usize), String> {
        // Verify store exists for this session
        if !self.stores.contains_key(session_id) {
            return Err(format!("Content store not found for session: {session_id}"));
        }

        let mut store_contents: Vec<&ContentItem> = self
            .contents
            .values()
            .filter(|c| c.session_id == session_id)
            .collect();

        // Sort by uploaded_at descending
        store_contents.sort_by(|a, b| b.uploaded_at.cmp(&a.uploaded_at));

        let total = store_contents.len();
        let paginated: Vec<ContentItem> = store_contents
            .into_iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();

        Ok((paginated, total))
    }

    /// Get session_id for a content item
    pub fn get_content_session_id(&self, content_id: &str) -> Option<String> {
        self.contents
            .get(content_id)
            .map(|content| content.session_id.clone())
    }

    /// Read content with line range
    pub async fn read_content(
        &self,
        content_id: &str,
        from_line: usize,
        to_line: Option<usize>,
    ) -> Result<String, String> {
        let chunks = self
            .chunks
            .get(content_id)
            .ok_or_else(|| format!("Content '{content_id}' not found"))?;

        let mut result = String::new();
        let target_to_line = to_line.unwrap_or(usize::MAX);

        for chunk in chunks {
            if chunk.line_range.1 >= from_line && chunk.line_range.0 <= target_to_line {
                // Chunk overlaps with requested range
                let lines: Vec<&str> = chunk.text.lines().collect();
                let start_idx = from_line.saturating_sub(chunk.line_range.0);

                let end_idx = if chunk.line_range.1 > target_to_line {
                    target_to_line - chunk.line_range.0 + 1
                } else {
                    lines.len()
                };

                for line in lines.iter().take(end_idx.min(lines.len())).skip(start_idx) {
                    result.push_str(line);
                    result.push('\n');
                }
            }
        }

        if result.is_empty() {
            return Err("No content found in specified line range".to_string());
        }

        Ok(result.trim().to_string())
    }
}
