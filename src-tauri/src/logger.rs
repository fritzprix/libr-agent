/// Custom logger setup to ensure Rust logs are written to file
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

extern crate fern;

fn parse_log_level_arg(value: &str) -> Option<log::LevelFilter> {
    match value.trim().to_ascii_lowercase().as_str() {
        "trace" => Some(log::LevelFilter::Trace),
        "debug" => Some(log::LevelFilter::Debug),
        "info" => Some(log::LevelFilter::Info),
        "warn" | "warning" => Some(log::LevelFilter::Warn),
        "error" => Some(log::LevelFilter::Error),
        _ => None,
    }
}

pub fn resolve_launch_log_level() -> log::LevelFilter {
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--log-level=") {
            if let Some(level) = parse_log_level_arg(value) {
                return level;
            }
            continue;
        }

        if arg == "--log-level" {
            if let Some(value) = args.next() {
                if let Some(level) = parse_log_level_arg(&value) {
                    return level;
                }
            }
        }
    }

    if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    }
}

pub fn resolve_launch_log_level_name() -> &'static str {
    match resolve_launch_log_level() {
        log::LevelFilter::Trace => "trace",
        log::LevelFilter::Debug => "debug",
        log::LevelFilter::Info => "info",
        log::LevelFilter::Warn => "warn",
        log::LevelFilter::Error => "error",
        _ => "info",
    }
}

const ACTIVE_LOG_FILE_NAME: &str = "libragent.log";
const MAX_ARCHIVED_LOG_FILES: usize = 50;
const MAX_ARCHIVED_LOG_BYTES: u64 = 1024 * 1024 * 1024;
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

/// Mirrors `SystemSettings::log_retention_days`; `0` means no age limit.
static LOG_RETENTION_DAYS: AtomicU64 = AtomicU64::new(0);
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
struct ArchivePolicy {
    max_files: usize,
    max_bytes: u64,
    max_age: Option<Duration>,
}

impl ArchivePolicy {
    fn current() -> Self {
        let days = LOG_RETENTION_DAYS.load(Ordering::Relaxed);
        Self {
            max_files: MAX_ARCHIVED_LOG_FILES,
            max_bytes: MAX_ARCHIVED_LOG_BYTES,
            max_age: (days > 0).then(|| Duration::from_secs(days.saturating_mul(SECONDS_PER_DAY))),
        }
    }
}

pub fn set_log_retention_days(days: u64) {
    LOG_RETENTION_DAYS.store(days, Ordering::Relaxed);
}

fn is_archived_log_file_name(name: &str) -> bool {
    name != ACTIVE_LOG_FILE_NAME
        && name.starts_with("libragent_")
        && (name.ends_with(".log") || name.ends_with(".log.bak"))
}

/// Applies the current retention policy to `log_dir`.
pub(crate) fn prune_archived_logs_in(log_dir: &Path) -> (usize, u64) {
    prune_archived_logs(log_dir, ArchivePolicy::current(), SystemTime::now())
}

/// Applies the current retention policy to the directory the file logger
/// writes to; a no-op before [`setup_file_logger`] has run.
pub fn prune_archived_logs_now() -> (usize, u64) {
    LOG_DIR
        .get()
        .map_or((0, 0), |log_dir| prune_archived_logs_in(log_dir))
}

/// Deletes archived logs older than the age limit, then the oldest ones until
/// the file-count and total-size limits hold. The active `libragent.log` is
/// never deleted, nor is the newest archive (the previous run) unless it is
/// past the age limit.
fn prune_archived_logs(log_dir: &Path, policy: ArchivePolicy, now: SystemTime) -> (usize, u64) {
    let Ok(entries) = fs::read_dir(log_dir) else {
        return (0, 0);
    };

    let mut archives: Vec<(PathBuf, SystemTime, u64)> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(is_archived_log_file_name)
        })
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            Some((entry.path(), modified, metadata.len()))
        })
        .collect();

    // Newest first, so everything past the budget is the oldest.
    archives.sort_by(|a, b| b.1.cmp(&a.1));

    let mut kept_bytes: u64 = 0;
    let mut removed_files = 0;
    let mut removed_bytes: u64 = 0;
    for (index, (path, modified, size)) in archives.iter().enumerate() {
        let expired = policy
            .max_age
            .is_some_and(|max_age| now.duration_since(*modified).is_ok_and(|age| age > max_age));
        let within_budget =
            index < policy.max_files && kept_bytes.saturating_add(*size) <= policy.max_bytes;
        if !expired && (index == 0 || within_budget) {
            kept_bytes = kept_bytes.saturating_add(*size);
            continue;
        }
        if fs::remove_file(path).is_ok() {
            removed_files += 1;
            removed_bytes += size;
        }
    }
    (removed_files, removed_bytes)
}

pub fn setup_file_logger(log_dir: PathBuf, log_retention_days: u64) -> Result<(), String> {
    set_log_retention_days(log_retention_days);

    // Ensure log directory exists
    if !log_dir.exists() {
        fs::create_dir_all(&log_dir)
            .map_err(|e| format!("Failed to create log directory: {}", e))?;
    }

    let _ = LOG_DIR.set(log_dir.clone());
    let log_file = log_dir.join(ACTIVE_LOG_FILE_NAME);

    // Backup existing log file with datetime suffix
    if log_file.exists() {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let backup_file = log_dir.join(format!("libragent_{}.log", timestamp));
        fs::rename(&log_file, &backup_file)
            .map_err(|e| format!("Failed to backup log file: {}", e))?;
    }

    let (pruned_files, pruned_bytes) = prune_archived_logs_in(&log_dir);

    // Create new log file (truncate mode for fresh start)
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_file)
        .map_err(|e| format!("Failed to open log file: {}", e))?;

    let log_level = resolve_launch_log_level();

    // Configure fern logger
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                chrono::Local::now().format("%Y-%m-%d][%H:%M:%S"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log_level)
        .chain(std::io::stdout()) // Keep stdout
        .chain(file) // Add file output
        .apply()
        .map_err(|e| format!("Failed to set logger: {}", e))?;

    log::info!(
        "🔥 File logger initialized at: {} (level: {})",
        log_file.display(),
        resolve_launch_log_level_name()
    );
    if pruned_files > 0 {
        log::info!(
            "🧹 Pruned {} archived log file(s) ({} bytes)",
            pruned_files,
            pruned_bytes
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(max_files: usize, max_bytes: u64, max_age_secs: Option<u64>) -> ArchivePolicy {
        ArchivePolicy {
            max_files,
            max_bytes,
            max_age: max_age_secs.map(Duration::from_secs),
        }
    }

    fn prune(dir: &Path, policy: ArchivePolicy) -> (usize, u64) {
        prune_archived_logs(dir, policy, SystemTime::now())
    }

    fn write_archive(dir: &Path, name: &str, bytes: usize, age_secs: u64) {
        let path = dir.join(name);
        fs::write(&path, vec![b'x'; bytes]).unwrap();
        let modified = SystemTime::now() - Duration::from_secs(age_secs);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    #[test]
    fn prunes_oldest_archives_beyond_file_count() {
        let dir = tempfile::tempdir().unwrap();
        write_archive(dir.path(), "libragent_1.log", 10, 300);
        write_archive(dir.path(), "libragent_2.log.bak", 10, 200);
        write_archive(dir.path(), "libragent_3.log", 10, 100);
        write_archive(dir.path(), ACTIVE_LOG_FILE_NAME, 10, 400);
        write_archive(dir.path(), "other.log", 10, 500);

        let (removed, _) = prune(dir.path(), policy(2, u64::MAX, None));

        assert_eq!(removed, 1);
        assert!(!dir.path().join("libragent_1.log").exists());
        assert!(dir.path().join("libragent_2.log.bak").exists());
        assert!(dir.path().join("libragent_3.log").exists());
        assert!(dir.path().join(ACTIVE_LOG_FILE_NAME).exists());
        assert!(dir.path().join("other.log").exists());
    }

    #[test]
    fn prunes_archives_beyond_byte_budget() {
        let dir = tempfile::tempdir().unwrap();
        write_archive(dir.path(), "libragent_old.log", 60, 200);
        write_archive(dir.path(), "libragent_new.log", 60, 100);

        let (removed, removed_bytes) = prune(dir.path(), policy(10, 100, None));

        assert_eq!((removed, removed_bytes), (1, 60));
        assert!(dir.path().join("libragent_new.log").exists());
        assert!(!dir.path().join("libragent_old.log").exists());
    }

    #[test]
    fn keeps_newest_archive_even_when_it_exceeds_byte_budget() {
        let dir = tempfile::tempdir().unwrap();
        write_archive(dir.path(), "libragent_old.log", 10, 200);
        write_archive(dir.path(), "libragent_huge.log", 500, 100);

        let (removed, _) = prune(dir.path(), policy(10, 100, None));

        assert_eq!(removed, 1);
        assert!(dir.path().join("libragent_huge.log").exists());
        assert!(!dir.path().join("libragent_old.log").exists());
    }

    #[test]
    fn prunes_archives_past_the_age_limit_including_the_newest() {
        let dir = tempfile::tempdir().unwrap();
        write_archive(dir.path(), "libragent_recent.log", 10, 100);
        write_archive(dir.path(), "libragent_expired.log", 10, 1_000);
        write_archive(dir.path(), ACTIVE_LOG_FILE_NAME, 10, 1_000);

        let (removed, _) = prune(dir.path(), policy(10, u64::MAX, Some(500)));
        assert_eq!(removed, 1);
        assert!(dir.path().join("libragent_recent.log").exists());
        assert!(!dir.path().join("libragent_expired.log").exists());
        assert!(dir.path().join(ACTIVE_LOG_FILE_NAME).exists());

        let (removed, _) = prune(dir.path(), policy(10, u64::MAX, Some(50)));
        assert_eq!(removed, 1);
        assert!(!dir.path().join("libragent_recent.log").exists());
    }

    #[test]
    fn zero_retention_days_disables_the_age_limit() {
        set_log_retention_days(0);
        assert!(ArchivePolicy::current().max_age.is_none());
        set_log_retention_days(7);
        assert_eq!(
            ArchivePolicy::current().max_age,
            Some(Duration::from_secs(7 * SECONDS_PER_DAY))
        );
        set_log_retention_days(0);
    }
}
