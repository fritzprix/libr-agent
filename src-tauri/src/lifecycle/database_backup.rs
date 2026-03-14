use chrono::Utc;
use log::{info, warn};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
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

    /// Create a timestamped backup of the database using VACUUM INTO for WAL-safe snapshot.
    ///
    /// `VACUUM INTO` is the only reliable way to back up a WAL-mode `SQLite` database:
    /// it performs an atomic, consistent copy that includes all committed WAL data
    /// without requiring a checkpoint or file-level copy.
    pub async fn create_backup(&self, db: &DatabaseConnection) -> DatabaseResult<PathBuf> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_dir = self.get_backup_dir();

        // Ensure backup directory exists
        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir).map_err(DatabaseError::IoError)?;
        }

        let file_stem = self
            .db_path
            .file_stem()
            .ok_or_else(|| DatabaseError::BackupFailed {
                path: self.db_path.display().to_string(),
                error: "Database path has no filename".to_string(),
            })?;

        let backup_path = backup_dir.join(format!(
            "{}.backup.{}.db",
            file_stem.to_string_lossy(),
            timestamp
        ));

        info!("📦 Creating WAL-safe backup: {}", backup_path.display());

        // VACUUM INTO performs an atomic, WAL-consistent copy — safe for WAL-mode databases.
        // Plain fs::copy would miss unflushed WAL frames and risk an inconsistent backup.
        let sql = format!("VACUUM INTO '{}'", backup_path.display());
        db.execute(Statement::from_string(DbBackend::Sqlite, sql))
            .await
            .map_err(|e| DatabaseError::BackupFailed {
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

        let stem = self
            .db_path
            .file_stem()
            .ok_or_else(|| {
                DatabaseError::IoError(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Database path has no filename",
                ))
            })?
            .to_string_lossy();
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
