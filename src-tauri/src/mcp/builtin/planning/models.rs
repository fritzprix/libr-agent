use serde::Serialize;
use sqlx::Row;

/// Todo item for frontend display
#[derive(Debug, Serialize)]
pub struct TodoDTO {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
    pub checked: bool,
    pub subtasks: Vec<TodoDTO>,
}

/// Todo item from database
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TodoItem {
    pub id: i64,
    pub content: String,
    pub description: Option<String>,
    pub priority: String,
    pub parent_id: Option<i64>,
    pub is_checked: bool,
    pub status: String, // Keep for backward compatibility if needed, or map to checked
    pub created_at: i64,
    pub updated_at: i64,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for TodoItem {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(TodoItem {
            id: row.try_get("id")?,
            content: row.try_get("content")?,
            description: row.try_get("description").ok(),
            priority: row.try_get("priority").unwrap_or("medium".to_string()),
            parent_id: row.try_get("parent_id").ok(),
            is_checked: row.try_get::<i64, _>("is_checked")? != 0,
            status: row.try_get("status")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// Scratchpad item from database
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct ScratchpadItem {
    pub id: i64,
    pub content: String,
    pub title: Option<String>,
    pub source: Option<String>,
    pub tags: Option<String>, // JSON array string
    pub created_at: i64,
    pub updated_at: i64,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for ScratchpadItem {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(ScratchpadItem {
            id: row.try_get("id")?,
            content: row.try_get("content")?,
            title: row.try_get("title").ok(),
            source: row.try_get("source").ok(),
            tags: row.try_get("tags").ok(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}
