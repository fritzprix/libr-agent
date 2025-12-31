use sqlx::SqlitePool;

/// Initialize database tables and indexes
pub async fn init_tables(pool: &SqlitePool, session_id: &str) -> Result<(), String> {
    // Create planning_goals table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS planning_goals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            goal_text TEXT NOT NULL,
            status TEXT DEFAULT 'active',
            created_at INTEGER NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create planning_goals table: {}", e))?;

    // Create planning_todos table (Updated schema)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS planning_todos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            content TEXT NOT NULL,
            description TEXT,
            priority TEXT DEFAULT 'medium',
            parent_id INTEGER,
            is_checked INTEGER DEFAULT 0,
            status TEXT DEFAULT 'pending',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create planning_todos table: {}", e))?;

    // Create planning_scratchpad table (Updated schema)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS planning_scratchpad (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            content TEXT NOT NULL,
            title TEXT,
            source TEXT,
            tags TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create planning_scratchpad table: {}", e))?;

    // Create indexes
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_planning_goals_session ON planning_goals(session_id)",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create index: {}", e))?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_planning_todos_session ON planning_todos(session_id)",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create index: {}", e))?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_planning_scratchpad_session ON planning_scratchpad(session_id)",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create index: {}", e))?;

    log::debug!(
        "Planning server tables initialized for session: {}",
        session_id
    );

    Ok(())
}
