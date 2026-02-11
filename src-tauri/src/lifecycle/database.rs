use crate::db_schema_validator::validate_schema;
use crate::migration::{Migrator, MigratorTrait};
use log::{error, info, warn};
use sea_orm::DatabaseConnection;
use std::fs;
use std::path::Path;

pub async fn init_db(
    db_url: &str,
) -> Result<DatabaseConnection, Box<dyn std::error::Error + Send + Sync>> {
    // Connect to database using SeaORM
    let mut db = match sea_orm::Database::connect(db_url).await {
        Ok(connection) => {
            info!("✅ Database connected: {db_url}");
            connection
        }
        Err(e) => {
            // If this looks like a file-backed sqlite URL, try to create the file
            if let Some(path_str) = db_url.strip_prefix("sqlite://") {
                info!("⚙️ Database connect failed, attempting to create DB file: {path_str}");

                // Handle connection options like ?mode=rwc
                let path_parts: Vec<&str> = path_str.split('?').collect();
                let file_path = path_parts[0];

                if let Some(parent) = Path::new(file_path).parent() {
                    if let Err(err) = fs::create_dir_all(parent) {
                        error!("Failed to create parent directory for DB: {err}");
                    }
                }

                if let Err(err) = fs::File::create(file_path) {
                    error!("Failed to create SQLite DB file: {err}");
                } else {
                    info!("✅ Created new SQLite DB file: {file_path}");
                }

                // Retry connection once
                sea_orm::Database::connect(db_url)
                    .await
                    .map_err(|err| format!("Failed to connect to database after creating file: {err}"))?
            } else {
                return Err(Box::new(e));
            }
        }
    };

    // Run migrations with auto-recovery
    let migration_result = Migrator::up(&db, None).await;

    // Handle migration result, resetting DB if necessary
    db = match migration_result {
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

                // Delete the corrupted database file
                if let Err(err) = fs::remove_file(file_path) {
                    error!("Failed to delete corrupted database file: {err}");
                } else {
                    info!("✅ Corrupted database file deleted");
                }

                // Create fresh file
                if let Err(err) = fs::File::create(file_path) {
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

            // Delete database
            if let Err(err) = fs::remove_file(file_path) {
                error!("Failed to delete database: {err}");
                panic!("Cannot reset database after schema validation failure");
            } else {
                info!("✅ Outdated database deleted");
            }

            // Recreate
            fs::File::create(file_path).expect("Failed to recreate database file");
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

    Ok(db)
}
