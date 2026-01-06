// SeaORM imports for database operations
use crate::entity::{chunk, content, store};
use crate::entity::{content::Entity as ContentEntity, store::Entity as StoreEntity};
use sea_orm::*;
use serde::{Deserialize, Serialize};
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
    pub src_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentChunk {
    pub id: String,
    pub content_id: String,
    pub chunk_index: usize,
    pub text: String,
    pub line_range: (usize, usize), // (start_line, end_line)
}

// Convert SeaORM models to internal structs
impl From<store::Model> for ContentStore {
    fn from(model: store::Model) -> Self {
        Self {
            session_id: model.session_id,
            name: model.name,
            description: model.description,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl From<content::Model> for ContentItem {
    fn from(model: content::Model) -> Self {
        Self {
            id: model.id,
            session_id: model.session_id,
            filename: model.filename,
            mime_type: model.mime_type,
            size: model.size as usize,
            line_count: model.line_count as usize,
            preview: model.preview,
            uploaded_at: model.uploaded_at,
            chunk_count: model.chunk_count as usize,
            last_accessed_at: model.last_accessed_at,
            content: model.content,
            src_url: model.src_url,
        }
    }
}

impl From<chunk::Model> for ContentChunk {
    fn from(model: chunk::Model) -> Self {
        Self {
            id: model.id,
            content_id: model.content_id,
            chunk_index: model.chunk_index as usize,
            text: model.text,
            line_range: (model.start_line as usize, model.end_line as usize),
        }
    }
}

/// Content store storage implementation
#[derive(Debug)]
pub struct ContentStoreStorage {
    // In-memory storage
    stores: HashMap<String, ContentStore>,
    contents: HashMap<String, ContentItem>,
    chunks: HashMap<String, Vec<ContentChunk>>,
    // Database connection for persistence
    db: Option<DatabaseConnection>,
}

impl Default for ContentStoreStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentStoreStorage {
    /// Create in-memory storage (default)
    pub fn new() -> Self {
        Self {
            stores: HashMap::new(),
            contents: HashMap::new(),
            chunks: HashMap::new(),
            db: None,
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

        // Create the database file if it doesn't exist
        if !std::path::Path::new(&db_path).exists() {
            std::fs::File::create(&db_path)
                .map_err(|e| format!("Failed to create database file: {e}"))?;
        }

        // Connect using SeaORM Database
        let db = Database::connect(&format!("sqlite://{}", db_path))
            .await
            .map_err(|e| format!("Failed to connect to database: {e}"))?;

        // Note: Migrations should be run at application startup, not here

        Ok(Self {
            stores: HashMap::new(), // Keep in-memory cache for performance
            contents: HashMap::new(),
            chunks: HashMap::new(),
            db: Some(db),
        })
    }

    /// Create storage with existing SeaORM DatabaseConnection
    pub async fn new_with_db(db: sea_orm::DatabaseConnection) -> Result<Self, String> {
        // Note: Migrations should already be run at application startup

        Ok(Self {
            stores: HashMap::new(), // Keep in-memory cache for performance
            contents: HashMap::new(),
            chunks: HashMap::new(),
            db: Some(db),
        })
    }

    /// Get database connection for operations
    #[allow(dead_code)]
    fn db(&self) -> Result<&DatabaseConnection, String> {
        self.db.as_ref().ok_or_else(|| {
            "Database not initialized. Use new_sqlite() to create SQLite-backed storage."
                .to_string()
        })
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

        // Database backend
        if let Some(db) = &self.db {
            let active_model = store::ActiveModel {
                session_id: Set(session_id.clone()),
                name: Set(name),
                description: Set(description),
                created_at: Set(now.clone()),
                updated_at: Set(now),
            };

            store::Entity::insert(active_model)
                .exec(db)
                .await
                .map_err(|e| format!("Failed to create store: {e}"))?;
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
                // Use pre-calculated preview (first 200 chars)
                let preview = if content.content.len() > content.preview.len() {
                    format!("{}...", content.preview)
                } else {
                    content.preview.clone()
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
        // Check if store already exists in memory cache
        if let Some(store) = self.stores.get(&session_id) {
            return Ok(store.clone());
        }

        // If using database, check if store exists
        if let Some(db) = &self.db {
            let result = StoreEntity::find_by_id(session_id.clone())
                .one(db)
                .await
                .map_err(|e| format!("Failed to check store existence: {e}"))?;

            if let Some(model) = result {
                // Store exists in database, convert and add to memory cache
                let store = ContentStore::from(model);
                self.stores.insert(session_id.clone(), store.clone());
                return Ok(store);
            }
        }

        // Store doesn't exist, create new one
        self.create_store(session_id, name, description).await
    }

    /// Add content to a session's store
    #[allow(clippy::too_many_arguments)]
    pub async fn add_content(
        &mut self,
        session_id: &str,
        filename: &str,
        mime_type: &str,
        size: usize,
        content: &str,
        chunks: Vec<String>,
        src_url: Option<String>,
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
            src_url: src_url.clone(),
        };

        self.contents
            .insert(content_id.clone(), content_item.clone());
        self.chunks
            .insert(content_id.clone(), content_chunks.clone());

        // Database backend
        if let Some(db) = &self.db {
            // Insert content (single operation)
            let content_active_model = content::ActiveModel {
                id: Set(content_id.clone()),
                session_id: Set(session_id.to_string()),
                filename: Set(filename.to_string()),
                mime_type: Set(mime_type.to_string()),
                size: Set(size as i32),
                line_count: Set(line_count as i32),
                preview: Set(content_item.preview.clone()),
                uploaded_at: Set(content_item.uploaded_at.clone()),
                chunk_count: Set(chunk_count as i32),
                last_accessed_at: Set(content_item.last_accessed_at.clone()),
                content: Set(content.to_string()),
                src_url: Set(src_url),
            };

            content::Entity::insert(content_active_model)
                .exec(db)
                .await
                .map_err(|e| format!("Failed to save content: {e}"))?;

            // Bulk insert chunks (single SQL statement with multiple VALUES)
            if !content_chunks.is_empty() {
                let chunk_models: Vec<chunk::ActiveModel> = content_chunks
                    .iter()
                    .map(|c| chunk::ActiveModel {
                        id: Set(c.id.clone()),
                        content_id: Set(c.content_id.clone()),
                        chunk_index: Set(c.chunk_index as i32),
                        text: Set(c.text.clone()),
                        start_line: Set(c.line_range.0 as i32),
                        end_line: Set(c.line_range.1 as i32),
                    })
                    .collect();

                chunk::Entity::insert_many(chunk_models)
                    .exec(db)
                    .await
                    .map_err(|e| format!("Failed to save chunks: {e}"))?;
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

    pub async fn delete_content(&mut self, content_id: &str) -> Result<(), String> {
        // Check if content exists
        if !self.contents.contains_key(content_id) {
            return Err(format!("Content '{content_id}' not found"));
        }

        // Remove from in-memory storage
        self.contents.remove(content_id);
        self.chunks.remove(content_id);

        // Database backend - ON DELETE CASCADE handles chunks automatically
        if let Some(db) = &self.db {
            ContentEntity::delete_by_id(content_id.to_string())
                .exec(db)
                .await
                .map_err(|e| format!("Failed to delete content: {e}"))?;
        }

        Ok(())
    }
}
