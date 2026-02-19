use crate::db_schema_validator::validate_schema;
use crate::migration::{Migrator, MigratorTrait};
use log::{error, info, warn};
use sea_orm::DatabaseConnection;

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

                // Delete the corrupted database file
                if let Err(err) = std::fs::remove_file(file_path) {
                    error!("Failed to delete corrupted database file: {err}");
                } else {
                    info!("✅ Corrupted database file deleted");
                }

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

            // Delete database
            if let Err(err) = std::fs::remove_file(file_path) {
                error!("Failed to delete database: {err}");
                panic!("Cannot reset database after schema validation failure");
            } else {
                info!("✅ Outdated database deleted");
            }

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
