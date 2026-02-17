use crate::db_schema_validator::validate_schema;
use crate::migration::{Migrator, MigratorTrait};
use log::{error, info, warn};
use sea_orm::DatabaseConnection;

/// Helper function to safely remove database file by renaming it
fn remove_db_file(file_path: &str) {
    let backup = format!("{}.old", file_path);
    std::fs::rename(file_path, &backup).expect("Cannot rename database file");
    info!("✅ Database file moved to: {}", backup);
}

pub async fn init_database(db_url: &str) -> DatabaseConnection {
    // Connect to database using SeaORM
    let db = match sea_orm::Database::connect(db_url).await {
        Ok(connection) => connection,
        Err(connect_error) => {
            if let Some(path_with_options) = db_url.strip_prefix("sqlite://") {
                let path = path_with_options
                    .split('?')
                    .next()
                    .unwrap_or(path_with_options);
                info!("⚙️ Database connect failed, attempting to create DB file: {path}");

                if let Some(parent) = std::path::Path::new(path).parent() {
                    if let Err(err) = std::fs::create_dir_all(parent) {
                        error!("Failed to create parent directory for DB: {err}");
                    }
                }

                if let Err(err) = std::fs::File::create(path) {
                    error!("Failed to create SQLite DB file: {err}");
                } else {
                    info!("✅ Created new SQLite DB file: {path}");
                }

                sea_orm::Database::connect(db_url)
                    .await
                    .unwrap_or_else(|retry_error| {
                        panic!("Failed to connect to database after creating file: {retry_error}")
                    })
            } else {
                panic!("Failed to connect to database: {connect_error}");
            }
        }
    };
    info!("✅ Database connected: {db_url}");

    // Run migrations with auto-recovery
    let migration_result = Migrator::up(&db, None).await;

    // Handle migration result, resetting DB if necessary
    let mut db = match migration_result {
        Ok(_) => {
            info!("✅ Database migrations applied");
            db
        }
        Err(e) => {
            error!("❌ Database migration failed: {e}");

            if let Some(path_str) = db_url.strip_prefix("sqlite://") {
                // Handle connection options like ?mode=rwc
                let path_parts: Vec<&str> = path_str.split('?').collect();
                let file_path = path_parts[0];

                warn!(
                    "⚠️ Migration failed. Attempting to reset database at: {}",
                    file_path
                );

                // Drop existing connection to release file lock
                drop(db);

                // Rename the corrupted database file
                remove_db_file(file_path);

                // Create fresh file
                if let Err(err) = std::fs::File::create(file_path) {
                    panic!("Failed to recreate database file: {err}");
                }
                info!("✅ Created fresh database file");

                // Reconnect
                let new_db = sea_orm::Database::connect(db_url)
                    .await
                    .expect("Failed to reconnect to database after reset");

                // Retry migrations on fresh DB
                Migrator::up(&new_db, None)
                    .await
                    .expect("Failed to run migrations on reset database");

                info!("✅ Database reset and migrations applied successfully");
                new_db
            } else {
                panic!("Failed to run database migrations: {e}");
            }
        }
    };

    // Validate schema after migrations
    if let Err(validation_err) = validate_schema(&db).await {
        warn!("⚠️ Schema validation failed: {}", validation_err);
        warn!("⚠️ Database schema mismatch detected. Resetting database...");

        if let Some(path_str) = db_url.strip_prefix("sqlite://") {
            let path_parts: Vec<&str> = path_str.split('?').collect();
            let file_path = path_parts[0];

            // Drop connection
            drop(db);

            // Rename the database file
            remove_db_file(file_path);

            // Recreate
            std::fs::File::create(file_path).expect("Failed to recreate database file");
            info!("✅ Created fresh database file");

            // Reconnect
            let new_db = sea_orm::Database::connect(db_url)
                .await
                .expect("Failed to reconnect after schema validation failure");

            // Run migrations
            Migrator::up(&new_db, None)
                .await
                .expect("Failed to run migrations after schema reset");

            // Validate again
            validate_schema(&new_db)
                .await
                .expect("Schema validation failed after reset");

            info!("✅ Database reset and validated successfully");
            db = new_db;
        } else {
            panic!("Schema validation failed and cannot reset non-file database");
        }
    } else {
        info!("✅ Database schema validated");
    }

    db
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_remove_db_file_success() {
        // Create temp directory
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_db_remove.db");
        let backup_file = format!("{}.old", test_file.display());

        // Create test file
        let mut file = fs::File::create(&test_file).unwrap();
        file.write_all(b"test data").unwrap();
        drop(file);

        // Verify file exists
        assert!(test_file.exists());

        // Call remove_db_file
        remove_db_file(test_file.to_str().unwrap());

        // Verify original is gone and backup exists
        assert!(!test_file.exists(), "Original file should not exist");
        assert!(
            std::path::Path::new(&backup_file).exists(),
            "Backup file should exist"
        );

        // Cleanup
        let _ = fs::remove_file(&backup_file);
    }

    #[test]
    fn test_remove_db_file_with_existing_backup() {
        // Create temp directory
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_db_existing_backup.db");
        let backup_file = format!("{}.old", test_file.display());

        // Create test file
        fs::File::create(&test_file).unwrap();

        // Create existing backup
        fs::File::create(&backup_file).unwrap();

        // Call remove_db_file (should overwrite existing backup)
        remove_db_file(test_file.to_str().unwrap());

        // Verify original is gone and backup still exists
        assert!(!test_file.exists());
        assert!(std::path::Path::new(&backup_file).exists());

        // Cleanup
        let _ = fs::remove_file(&backup_file);
    }

    #[test]
    #[should_panic(expected = "Cannot rename database file")]
    fn test_remove_db_file_nonexistent() {
        // Try to remove non-existent file
        remove_db_file("/nonexistent/path/to/file.db");
    }
}
