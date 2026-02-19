use chrono::Utc;
use log::{info, warn};
use std::fs;
use std::path::{Path, PathBuf};

use super::database_error::{DatabaseError, DatabaseResult};

/// Maximum number of backups to keep
const MAX_BACKUPS: usize = 5;

/// Backup manager for database files
pub struct BackupManager {
    db_path: PathBuf,
}

impl BackupManager {
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        let db_path = db_path.into();
        let backup_dir = db_path.parent().unwrap_or(Path::new(".")).join("backups");

        if let Err(e) = fs::create_dir_all(&backup_dir) {
            log::error!("Failed to create backup directory: {}", e);
        }

        Self { db_path }
    }

    fn get_backup_dir(&self) -> PathBuf {
        self.db_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("backups")
    }

    /// Create a timestamped backup of the database
    pub fn create_backup(&self) -> DatabaseResult<PathBuf> {
        if !self.db_path.exists() {
            return Err(DatabaseError::BackupFailed {
                path: self.db_path.display().to_string(),
                error: "Database file does not exist".into(),
            });
        }

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_dir = self.get_backup_dir();

        // Ensure backup directory exists
        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir).map_err(DatabaseError::IoError)?;
        }

        let backup_path = backup_dir.join(format!(
            "{}.backup.{}.db",
            self.db_path.file_stem().unwrap().to_string_lossy(),
            timestamp
        ));

        info!("📦 Creating backup: {}", backup_path.display());

        fs::copy(&self.db_path, &backup_path).map_err(|e| DatabaseError::BackupFailed {
            path: self.db_path.display().to_string(),
            error: e.to_string(),
        })?;

        info!("✅ Backup created successfully");

        // Cleanup old backups
        self.cleanup_old_backups()?;

        Ok(backup_path)
    }

    /// Find all backup files for this database
    pub fn find_backups(&self) -> DatabaseResult<Vec<PathBuf>> {
        let backup_dir = self.get_backup_dir();

        if !backup_dir.exists() {
            return Ok(Vec::new());
        }

        let stem = self.db_path.file_stem().unwrap().to_string_lossy();
        let pattern = format!("{}.backup.", stem);

        let mut backups: Vec<PathBuf> = fs::read_dir(&backup_dir)
            .map_err(DatabaseError::IoError)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(&pattern))
                    .unwrap_or(false)
            })
            .collect();

        // Sort by modification time (newest first)
        backups.sort_by(|a, b| {
            let a_time = fs::metadata(a).and_then(|m| m.modified()).ok();
            let b_time = fs::metadata(b).and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        Ok(backups)
    }

    /// Get the most recent backup
    pub fn get_latest_backup(&self) -> DatabaseResult<Option<PathBuf>> {
        let backups = self.find_backups()?;
        Ok(backups.into_iter().next())
    }

    /// Restore database from a backup
    pub fn restore_from_backup(&self, backup_path: &Path) -> DatabaseResult<()> {
        if !backup_path.exists() {
            return Err(DatabaseError::RestoreFailed {
                backup_path: backup_path.display().to_string(),
                error: "Backup file does not exist".into(),
            });
        }

        info!("🔄 Restoring from backup: {}", backup_path.display());

        // Create a backup of current state before restoring
        if self.db_path.exists() {
            let safety_backup = self.db_path.with_extension("db.before_restore");
            fs::copy(&self.db_path, &safety_backup).ok(); // Best effort
            info!("📦 Safety backup created: {}", safety_backup.display());
        }

        fs::copy(backup_path, &self.db_path).map_err(|e| DatabaseError::RestoreFailed {
            backup_path: backup_path.display().to_string(),
            error: e.to_string(),
        })?;

        info!("✅ Database restored successfully");

        Ok(())
    }

    /// Delete old backups, keeping only the most recent N
    fn cleanup_old_backups(&self) -> DatabaseResult<()> {
        let backups = self.find_backups()?;

        if backups.len() <= MAX_BACKUPS {
            return Ok(());
        }

        let to_delete = &backups[MAX_BACKUPS..];
        info!(
            "🧹 Cleaning up {} old backups (keeping {} most recent)",
            to_delete.len(),
            MAX_BACKUPS
        );

        for backup in to_delete {
            if let Err(e) = fs::remove_file(backup) {
                warn!("⚠️ Failed to delete old backup {}: {}", backup.display(), e);
            }
        }

        Ok(())
    }

    /// Check if any backups are available
    pub fn has_backups(&self) -> bool {
        self.find_backups()
            .ok()
            .map(|b| !b.is_empty())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_backup_creation() {
        let temp_dir = std::env::temp_dir();
        let test_db = temp_dir.join("test_backup.db");

        // Create test database
        let mut file = fs::File::create(&test_db).unwrap();
        file.write_all(b"test data").unwrap();
        drop(file);

        let manager = BackupManager::new(&test_db);

        // Create backup
        let backup = manager.create_backup().unwrap();
        assert!(backup.exists());

        // Verify backup content
        let backup_content = fs::read_to_string(&backup).unwrap();
        assert_eq!(backup_content, "test data");

        // Verify backup is in 'backups' subdirectory
        assert!(backup.parent().unwrap().ends_with("backups"));

        // Cleanup
        let _ = fs::remove_file(&test_db);
        let _ = fs::remove_file(&test_db);
        let _ = fs::remove_dir_all(backup.parent().unwrap());
    }

    #[test]
    fn test_backup_cleanup() {
        let temp_dir = std::env::temp_dir();
        let test_db = temp_dir.join("test_cleanup.db");

        // Create test database
        fs::File::create(&test_db).unwrap();

        let manager = BackupManager::new(&test_db);

        // Create more than MAX_BACKUPS
        for _ in 0..7 {
            manager.create_backup().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Should only keep MAX_BACKUPS
        let backups = manager.find_backups().unwrap();
        assert_eq!(backups.len(), MAX_BACKUPS);

        // Cleanup
        let _ = fs::remove_file(&test_db);
        for backup in backups {
            let _ = fs::remove_file(backup);
        }
        let backup_dir = test_db.parent().unwrap().join("backups");
        if backup_dir.exists() {
            let _ = fs::remove_dir_all(backup_dir);
        }
    }
}
