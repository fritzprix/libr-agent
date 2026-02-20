//! Migration Verifier - SHA-256 checksum verification for migration files
//!
//! This module ensures migration file integrity by computing and verifying
//! SHA-256 checksums. This prevents accidental or malicious modification of
//! already-applied migrations.

use log::{info, warn};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Migration metadata from database
#[derive(Debug)]
pub struct MigrationRecord {
    pub version: String,
    pub checksum: String,
    pub applied_at: i64,
}

/// Migration Verifier for checksum validation
pub struct MigrationVerifier {
    db: DatabaseConnection,
    migration_dir: String,
}

impl MigrationVerifier {
    /// Create a new migration verifier
    pub fn new(db: DatabaseConnection, migration_dir: impl Into<String>) -> Self {
        Self {
            db,
            migration_dir: migration_dir.into(),
        }
    }

    /// Compute SHA-256 checksum of a file
    pub fn compute_checksum(file_path: &Path) -> Result<String, String> {
        let content = fs::read(file_path)
            .map_err(|e| format!("Failed to read file '{}': {}", file_path.display(), e))?;

        let mut hasher = Sha256::new();
        hasher.update(&content);
        let result = hasher.finalize();

        Ok(format!("{:x}", result))
    }

    /// Get stored checksums from database
    pub async fn get_stored_checksums(&self) -> Result<HashMap<String, String>, String> {
        // Check if migration_metadata table exists
        let table_exists = self
            .db
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type='table' AND name='migration_metadata'"
                    .to_string(),
            ))
            .await
            .map_err(|e| format!("Failed to check migration_metadata table: {}", e))?;

        if table_exists.is_none() {
            // Table doesn't exist yet, return empty map
            return Ok(HashMap::new());
        }

        let rows = self
            .db
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT version, checksum FROM migration_metadata WHERE checksum != 'legacy'"
                    .to_string(),
            ))
            .await
            .map_err(|e| format!("Failed to query migration_metadata: {}", e))?;

        let mut checksums = HashMap::new();
        for row in rows {
            let version: String = row
                .try_get("", "version")
                .map_err(|e| format!("Failed to get version: {}", e))?;
            let checksum: String = row
                .try_get("", "checksum")
                .map_err(|e| format!("Failed to get checksum: {}", e))?;
            checksums.insert(version, checksum);
        }

        Ok(checksums)
    }

    /// Verify all migration files against stored checksums
    pub async fn verify_all_migrations(&self) -> Result<(), String> {
        let stored_checksums = self.get_stored_checksums().await?;

        if stored_checksums.is_empty() {
            info!("ℹ️  No migration checksums found (first run or legacy migrations)");
            return Ok(());
        }

        let migration_dir = Path::new(&self.migration_dir);
        if !migration_dir.exists() {
            warn!(
                "⚠️  Migration directory not found: {}",
                migration_dir.display()
            );
            return Ok(());
        }

        let mut errors = Vec::new();

        for entry in fs::read_dir(migration_dir)
            .map_err(|e| format!("Failed to read migration directory: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            // Only check .rs files
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }

            // Skip lib.rs and mod.rs
            if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                if filename == "lib" || filename == "mod" {
                    continue;
                }

                // Extract migration version from filename
                if !filename.starts_with('m') {
                    continue;
                }

                let version = filename.to_string();

                // Check if we have a stored checksum for this migration
                if let Some(stored_checksum) = stored_checksums.get(&version) {
                    let current_checksum = Self::compute_checksum(&path)?;

                    if &current_checksum != stored_checksum {
                        errors.push(format!(
                            "❌ Migration file '{}' was modified!\n   Expected: {}\n   Found:    {}",
                            filename, stored_checksum, current_checksum
                        ));
                    } else {
                        info!("✅ Migration '{}' checksum verified", filename);
                    }
                }
            }
        }

        if !errors.is_empty() {
            return Err(format!(
                "Migration integrity check failed:\n\n{}\n\n\
                ⚠️  CRITICAL: Applied migration files were modified!\n\
                This can cause schema inconsistencies across environments.\n\
                \n\
                Solutions:\n\
                1. Revert the migration file to its original state\n\
                2. Create a NEW migration to make schema changes\n\
                3. If this is a development environment, delete the database and start fresh",
                errors.join("\n\n")
            ));
        }

        Ok(())
    }

    /// Store checksum for a migration
    pub async fn store_checksum(
        &self,
        version: &str,
        checksum: &str,
        description: Option<&str>,
        execution_time_ms: i64,
        success: bool,
    ) -> Result<(), String> {
        let applied_at = chrono::Utc::now().timestamp_millis();
        let description_value = description.unwrap_or("");

        self.db
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO migration_metadata (version, checksum, description, applied_at, execution_time_ms, success) \
                 VALUES (?, ?, ?, ?, ?, ?)",
                vec![
                    version.into(),
                    checksum.into(),
                    description_value.into(),
                    applied_at.into(),
                    execution_time_ms.into(),
                    success.into(),
                ],
            ))
            .await
            .map_err(|e| format!("Failed to store migration checksum: {}", e))?;

        Ok(())
    }

    /// Update schema version
    pub async fn update_schema_version(
        &self,
        version: &str,
        migration_count: i32,
        checksum: &str,
    ) -> Result<(), String> {
        let applied_at = chrono::Utc::now().timestamp_millis();

        // Delete old version if exists
        self.db
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "DELETE FROM schema_version".to_string(),
            ))
            .await
            .map_err(|e| format!("Failed to delete old schema version: {}", e))?;

        // Insert new version
        self.db
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO schema_version (version, migration_count, applied_at, checksum) VALUES (?, ?, ?, ?)",
                vec![
                    version.into(),
                    migration_count.into(),
                    applied_at.into(),
                    checksum.into(),
                ],
            ))
            .await
            .map_err(|e| format!("Failed to update schema version: {}", e))?;

        Ok(())
    }

    /// Get current schema version
    pub async fn get_schema_version(&self) -> Result<Option<String>, String> {
        let row = self
            .db
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT version FROM schema_version LIMIT 1".to_string(),
            ))
            .await
            .map_err(|e| format!("Failed to query schema version: {}", e))?;

        match row {
            Some(r) => {
                let version: String = r
                    .try_get("", "version")
                    .map_err(|e| format!("Failed to get version: {}", e))?;
                Ok(Some(version))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_compute_checksum() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test content").unwrap();
        file.flush().unwrap();

        let checksum = MigrationVerifier::compute_checksum(file.path()).unwrap();
        assert_eq!(checksum.len(), 64); // SHA-256 produces 64 hex characters
    }

    #[test]
    fn test_compute_checksum_consistency() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "consistent content").unwrap();
        file.flush().unwrap();

        let checksum1 = MigrationVerifier::compute_checksum(file.path()).unwrap();
        let checksum2 = MigrationVerifier::compute_checksum(file.path()).unwrap();

        assert_eq!(checksum1, checksum2);
    }
}
