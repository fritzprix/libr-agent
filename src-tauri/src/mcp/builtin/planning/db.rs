use sqlx::{Row, SqlitePool};
use std::collections::HashSet;

/// Define a table column structure
struct Column {
    name: &'static str,
    def: &'static str,
}

/// Define a table schema with columns and table-level constraints
struct TableSchema {
    name: &'static str,
    columns: &'static [Column],
    constraints: &'static [&'static str], // e.g., FOREIGN KEY, PRIMARY KEY constraints not inline
}

impl TableSchema {
    /// Sync the table schema: Create if not exists, or add missing columns if exists.
    pub async fn sync(&self, pool: &SqlitePool) -> Result<(), String> {
        // 1. Construct CREATE TABLE SQL dynamically
        let mut col_defs: Vec<String> = self
            .columns
            .iter()
            .map(|c| format!("{} {}", c.name, c.def))
            .collect();

        for constraint in self.constraints {
            col_defs.push(constraint.to_string());
        }

        let create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {} ({})",
            self.name,
            col_defs.join(", ")
        );

        sqlx::query(&create_sql)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to ensure table {}: {}", self.name, e))?;

        // 2. Check existing columns using PRAGMA table_info
        let pragma_sql = format!("PRAGMA table_info({})", self.name);
        let rows = sqlx::query(&pragma_sql)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to fetch schema info for {}: {}", self.name, e))?;

        let existing_columns: HashSet<String> =
            rows.iter().map(|r| r.get::<String, _>("name")).collect();

        // 3. Diff and Migrate: Add missing columns
        for col in self.columns {
            if !existing_columns.contains(col.name) {
                log::info!(
                    "Migrating table '{}': Adding missing column '{}'",
                    self.name,
                    col.name
                );

                let alter_sql = format!(
                    "ALTER TABLE {} ADD COLUMN {} {}",
                    self.name, col.name, col.def
                );

                if let Err(e) = sqlx::query(&alter_sql).execute(pool).await {
                    // Ignore duplicate column errors if race conditions occur, otherwise report
                    if !e.to_string().contains("duplicate column name") {
                        return Err(format!(
                            "Failed to add column {} to {}: {}",
                            col.name, self.name, e
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

/// Initialize database tables and indexes using the Generic Schema Manager
pub async fn init_tables(pool: &SqlitePool, session_id: &str) -> Result<(), String> {
    // FORCE RESET: Drop scratchpad table to fix UNIQUE constraint issue
    // This is necessary because the legacy schema had a UNIQUE constraint on session_id
    let _ = sqlx::query("DROP TABLE IF EXISTS planning_scratchpad")
        .execute(pool)
        .await;

    // 1. Define Planning Goals Schema
    let goals_schema = TableSchema {
        name: "planning_goals",
        columns: &[
            Column {
                name: "id",
                def: "INTEGER PRIMARY KEY AUTOINCREMENT",
            },
            Column {
                name: "session_id",
                def: "TEXT NOT NULL",
            },
            Column {
                name: "goal_text",
                def: "TEXT NOT NULL",
            },
            Column {
                name: "status",
                def: "TEXT DEFAULT 'active'",
            },
            Column {
                name: "created_at",
                def: "INTEGER NOT NULL",
            },
        ],
        constraints: &["FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE"],
    };

    // 2. Define Planning Todos Schema
    let todos_schema = TableSchema {
        name: "planning_todos",
        columns: &[
            Column {
                name: "id",
                def: "INTEGER PRIMARY KEY AUTOINCREMENT",
            },
            Column {
                name: "session_id",
                def: "TEXT NOT NULL",
            },
            Column {
                name: "content",
                def: "TEXT NOT NULL",
            },
            Column {
                name: "description",
                def: "TEXT",
            },
            // Removed 'active_form' as requested
            Column {
                name: "priority",
                def: "TEXT DEFAULT 'medium'",
            },
            Column {
                name: "parent_id",
                def: "INTEGER",
            },
            Column {
                name: "is_checked",
                def: "INTEGER DEFAULT 0",
            },
            Column {
                name: "status",
                def: "TEXT DEFAULT 'pending'",
            },
            Column {
                name: "created_at",
                def: "INTEGER NOT NULL",
            },
            Column {
                name: "updated_at",
                def: "INTEGER DEFAULT 0",
            }, // Added default for robustness
        ],
        constraints: &["FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE"],
    };

    // 3. Define Planning Scratchpad Schema
    let scratchpad_schema = TableSchema {
        name: "planning_scratchpad",
        columns: &[
            Column {
                name: "id",
                def: "INTEGER PRIMARY KEY AUTOINCREMENT",
            },
            Column {
                name: "session_id",
                def: "TEXT NOT NULL",
            },
            Column {
                name: "content",
                def: "TEXT NOT NULL",
            },
            Column {
                name: "title",
                def: "TEXT",
            },
            Column {
                name: "source",
                def: "TEXT",
            },
            Column {
                name: "tags",
                def: "TEXT",
            },
            Column {
                name: "created_at",
                def: "INTEGER DEFAULT 0",
            }, // Ensure default for existing rows
            Column {
                name: "updated_at",
                def: "INTEGER DEFAULT 0",
            },
        ],
        constraints: &["FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE"],
    };

    // Sync all tables
    goals_schema.sync(pool).await?;
    todos_schema.sync(pool).await?;
    scratchpad_schema.sync(pool).await?;

    // Create Indexes
    // Note: CREATE INDEX IF NOT EXISTS is already idempotent, so we can keep it simple.
    let indexes = [
        "DROP INDEX IF EXISTS idx_planning_goals_session",
        "CREATE INDEX IF NOT EXISTS idx_planning_goals_session ON planning_goals(session_id)",
        "DROP INDEX IF EXISTS idx_planning_todos_session",
        "CREATE INDEX IF NOT EXISTS idx_planning_todos_session ON planning_todos(session_id)",
        "DROP INDEX IF EXISTS idx_planning_scratchpad_session",
        "CREATE INDEX IF NOT EXISTS idx_planning_scratchpad_session ON planning_scratchpad(session_id)",
    ];

    for idx_sql in indexes {
        sqlx::query(idx_sql)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create index: {}", e))?;
    }

    // Optional legacy cleanup (Best effort)
    // Dropping columns in SQLite is expensive (requires copy), so we do it only if necessary.
    // For now, we only attempt to drop the specific deprecated column if it exists to be clean.
    let _ = sqlx::query("ALTER TABLE planning_todos DROP COLUMN active_form")
        .execute(pool)
        .await;

    log::debug!(
        "Planning server tables synced and initialized for session: {}",
        session_id
    );

    Ok(())
}
