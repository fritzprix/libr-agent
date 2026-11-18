/// Custom logger setup to ensure Rust logs are written to file
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use crate::mcp::builtin::workspace::persistent_shell::secrets::mask_log_line;

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

/// Legacy single-process active log name (pre-#2858). Still treated as an
/// active file so prune never deletes another process's open log.
const LEGACY_ACTIVE_LOG_FILE_NAME: &str = "libragent.log";
const MAX_ARCHIVED_LOG_FILES: usize = 50;
const MAX_ARCHIVED_LOG_BYTES: u64 = 1024 * 1024 * 1024;
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

/// Per-process active log file (`libragent.<pid>.log`) so multiple backends
/// on one profile never share or truncate each other's file (#2858).
pub fn active_log_file_name() -> String {
    format!("libragent.{}.log", std::process::id())
}

fn is_active_log_file_name(name: &str) -> bool {
    name == LEGACY_ACTIVE_LOG_FILE_NAME || active_log_pid(name).is_some()
}

/// The pid in a per-process active log name (`libragent.<pid>.log`). `None` for
/// the legacy single-process name and for anything else.
fn active_log_pid(name: &str) -> Option<u32> {
    name.strip_prefix("libragent.")
        .and_then(|rest| rest.strip_suffix(".log"))
        .and_then(|pid| pid.parse::<u32>().ok())
}

/// Rename per-process logs left behind by dead processes to the archived form,
/// so the existing retention policy covers them (#2858 G1.3).
///
/// Rotation only renames *this* process's `libragent.<pid>.log`, which never
/// exists at startup because the pid is new. Without this sweep every earlier
/// run's `libragent.<oldpid>.log` keeps matching [`is_active_log_file_name`],
/// is never archived and never pruned, so `log_retention_days`,
/// `MAX_ARCHIVED_LOG_FILES` and `MAX_ARCHIVED_LOG_BYTES` silently stop applying.
///
/// The pid stays in the archived name so two dead logs archived within the same
/// second cannot collide.
fn archive_dead_process_logs_with(
    log_dir: &Path,
    own_pid: u32,
    pid_proven_dead: impl Fn(u32) -> bool,
) -> usize {
    let Ok(entries) = fs::read_dir(log_dir) else {
        return 0;
    };
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let mut archived = 0;
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(pid) = active_log_pid(&name) else {
            continue;
        };
        if pid == own_pid || !pid_proven_dead(pid) {
            continue;
        }
        let target = log_dir.join(format!("libragent_{timestamp}_{pid}.log"));
        if fs::rename(entry.path(), &target).is_ok() {
            archived += 1;
        }
    }
    archived
}

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
    !is_active_log_file_name(name)
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
/// the file-count and total-size limits hold. Active per-pid (and legacy)
/// `libragent*.log` files are never deleted, nor is the newest archive (the
/// previous run) unless it is past the age limit.
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
    archives.sort_by_key(|a| std::cmp::Reverse(a.1));

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

/// Every line, from any target, has the stored shell values of live sessions masked.
fn formatted_dispatch() -> fern::Dispatch {
    fern::Dispatch::new().format(|out, message, record| {
        let message = message.to_string();
        out.finish(format_args!(
            "[{}][{}][{}] {}",
            chrono::Local::now().format("%Y-%m-%d][%H:%M:%S"),
            record.target(),
            record.level(),
            mask_log_line(&message)
        ))
    })
}

pub fn setup_file_logger(log_dir: PathBuf, log_retention_days: u64) -> Result<(), String> {
    setup_logger_with_console(log_dir, log_retention_days, true, ConsoleTarget::Stdout)
}

/// Headless CLI logger (#2885): logs go to **stderr** and the log file,
/// never stdout. In headless mode stdout carries only the selected
/// `--output-format` document, so `libragent --headless --output-format
/// json` output stays parseable as a single JSON value.
pub fn setup_headless_file_logger(log_dir: PathBuf, log_retention_days: u64) -> Result<(), String> {
    setup_logger_with_console(log_dir, log_retention_days, true, ConsoleTarget::Stderr)
}

/// Console stream the logger mirrors to, chosen by the entry point.
enum ConsoleTarget {
    /// GUI backend: terminal-launched users see logs on stdout alongside
    /// the window (existing behavior).
    Stdout,
    /// Headless CLI: stdout is reserved for formatter output (#2885).
    Stderr,
}

/// `rotate_existing` renames this process's `libragent.<pid>.log` aside before
/// opening a new file. Pass `false` only in tests that need append mode; each
/// backend owns its own file so rotation never touches another process (#2858).
fn setup_logger_with_console(
    log_dir: PathBuf,
    log_retention_days: u64,
    rotate_existing: bool,
    console: ConsoleTarget,
) -> Result<(), String> {
    set_log_retention_days(log_retention_days);

    // Ensure log directory exists
    if !log_dir.exists() {
        fs::create_dir_all(&log_dir)
            .map_err(|e| format!("Failed to create log directory: {}", e))?;
    }

    let _ = LOG_DIR.set(log_dir.clone());

    let (file, pruned_files, pruned_bytes) = prepare_log_file(&log_dir, rotate_existing)?;

    let log_level = resolve_launch_log_level();

    let dispatch = formatted_dispatch().level(log_level).chain(file); // Add file output
    let dispatch = match console {
        ConsoleTarget::Stdout => dispatch.chain(std::io::stdout()), // Keep stdout
        // #2885: headless stdout is only the formatter output.
        ConsoleTarget::Stderr => dispatch.chain(std::io::stderr()),
    };
    dispatch
        .apply()
        .map_err(|e| format!("Failed to set logger: {}", e))?;

    log::info!(
        "🔥 File logger initialized at: {} (level: {})",
        log_dir.join(active_log_file_name()).display(),
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

/// Prepare this process's active log file: rotate `libragent.<pid>.log` aside
/// when `rotate_existing`, prune archives, then open the file — truncated for
/// a fresh start when rotating, appended when not. Split out from
/// [`setup_logger_with_console`] so the rotation behavior is testable
/// without installing the global logger.
/// Returns `(file, pruned_files, pruned_bytes)`.
fn prepare_log_file(
    log_dir: &Path,
    rotate_existing: bool,
) -> Result<(fs::File, usize, u64), String> {
    let log_file = log_dir.join(active_log_file_name());

    // Backup this process's existing log with a datetime suffix. Other
    // backends write `libragent.<other-pid>.log`, so this never renames
    // another process's open file (#2858).
    if rotate_existing && log_file.exists() {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let backup_file = log_dir.join(format!("libragent_{}.log", timestamp));
        fs::rename(&log_file, &backup_file)
            .map_err(|e| format!("Failed to backup log file: {}", e))?;
    }

    // Archive logs of processes that are gone before pruning, so retention
    // applies to them (#2858 G1.3).
    archive_dead_process_logs_with(log_dir, std::process::id(), |pid| {
        crate::utils::process::pid_is_proven_dead(pid)
    });

    let (pruned_files, pruned_bytes) = prune_archived_logs_in(log_dir);

    // Create new log file (truncate mode for fresh start)
    let mut file_options = OpenOptions::new();
    file_options.create(true).write(true);
    if rotate_existing {
        file_options.truncate(true);
    } else {
        file_options.append(true);
    }
    let file = file_options
        .open(&log_file)
        .map_err(|e| format!("Failed to open log file: {}", e))?;

    Ok((file, pruned_files, pruned_bytes))
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

    fn emit(logger: &dyn log::Log, target: &str, message: &str) {
        logger.log(
            &log::Record::builder()
                .args(format_args!("{message}"))
                .level(log::Level::Info)
                .target(target)
                .build(),
        );
    }

    #[tokio::test]
    async fn emitted_lines_mask_a_stored_value_and_keep_names_and_unrelated_literals() {
        use crate::mcp::builtin::workspace::persistent_shell::PersistentShellManager;

        let stored = "logger-stored-4b7e9d";
        let manager = PersistentShellManager::new();
        manager
            .store_secret("logger-mask-session", "DEMO_TOKEN", stored.to_string())
            .await
            .expect("store secret");
        let (sender, receiver) = std::sync::mpsc::channel::<String>();
        let (_, logger) = formatted_dispatch().chain(sender).into_log();

        let webview_tool_args = format!(
            "[webview] tool call {{\"name\":\"runInPersistentPowerShell\",\"arguments\":{{\"nested\":[{{\"command\":\"{}Write-Output {stored}\"}}]}}}}",
            "x".repeat(140)
        );
        emit(
            logger.as_ref(),
            "libragent::commands::log_commands",
            &webview_tool_args,
        );
        emit(
            logger.as_ref(),
            "libragent::mcp::builtin::workspace::code_execution::process",
            "Windows process command: Write-Output \"val=$env:DEMO_TOKEN\" unrelated-literal-c0de",
        );
        manager.clear_session_state("logger-mask-session").await;
        emit(logger.as_ref(), "after", &format!("forgotten {stored}"));

        let lines: Vec<String> = receiver.try_iter().collect();
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(!lines[0].contains(stored), "{}", lines[0]);
        assert!(lines[0].contains("Write-Output ***"), "{}", lines[0]);
        assert!(lines[1].contains("$env:DEMO_TOKEN"), "{}", lines[1]);
        assert!(lines[1].contains("unrelated-literal-c0de"), "{}", lines[1]);
        assert!(
            lines[2].contains(stored),
            "a forgotten value is no longer known"
        );
    }

    #[tokio::test]
    async fn emitted_lines_mask_escaped_copies_of_a_stored_value() {
        use crate::mcp::builtin::workspace::persistent_shell::PersistentShellManager;

        let stored = "p@ss\"w\\rd-91\tlog";
        let manager = PersistentShellManager::new();
        manager
            .store_secret("logger-escape-session", "DEMO_PASSWORD", stored.to_string())
            .await
            .expect("store secret");
        let (sender, receiver) = std::sync::mpsc::channel::<String>();
        let (_, logger) = formatted_dispatch().chain(sender).into_log();

        let args = serde_json::json!({ "command": format!("login --password {stored}") });
        let call = serde_json::json!({ "name": "runInPersistentShell", "arguments": args });
        let serialized_args = serde_json::json!({ "arguments": args.to_string() });
        let messages = [
            format!("[webview] tool call {call}"),
            format!("[webview] tool call {serialized_args}"),
            format!(
                "tool args {:?}",
                args["command"].as_str().unwrap_or_default()
            ),
            format!("raw args {:?}", args.to_string()),
        ];
        for message in &messages {
            emit(
                logger.as_ref(),
                "libragent::commands::log_commands",
                message,
            );
        }
        manager.clear_session_state("logger-escape-session").await;

        let lines: Vec<String> = receiver.try_iter().collect();
        assert_eq!(lines.len(), messages.len(), "{lines:?}");
        for line in &lines {
            assert!(!line.contains("p@ss"), "{line}");
            assert!(line.contains("--password ***"), "{line}");
        }
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
        write_archive(dir.path(), &active_log_file_name(), 10, 400);
        write_archive(dir.path(), "other.log", 10, 500);

        let (removed, _) = prune(dir.path(), policy(2, u64::MAX, None));

        assert_eq!(removed, 1);
        assert!(!dir.path().join("libragent_1.log").exists());
        assert!(dir.path().join("libragent_2.log.bak").exists());
        assert!(dir.path().join("libragent_3.log").exists());
        assert!(dir.path().join(active_log_file_name()).exists());
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
        write_archive(dir.path(), &active_log_file_name(), 10, 1_000);

        let (removed, _) = prune(dir.path(), policy(10, u64::MAX, Some(500)));
        assert_eq!(removed, 1);
        assert!(dir.path().join("libragent_recent.log").exists());
        assert!(!dir.path().join("libragent_expired.log").exists());
        assert!(dir.path().join(active_log_file_name()).exists());

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

    fn archived_logs(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                name.starts_with("libragent_") && name.ends_with(".log")
            })
            .collect()
    }

    #[test]
    fn append_setup_keeps_this_process_log_without_rotating() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join(active_log_file_name());
        fs::write(&log, "host line\n").unwrap();

        let (mut file, _, _) = prepare_log_file(dir.path(), false).unwrap();
        file.write_all(b"attached line\n").unwrap();
        drop(file);

        assert_eq!(
            fs::read_to_string(&log).unwrap(),
            "host line\nattached line\n"
        );
        assert!(archived_logs(dir.path()).is_empty());
    }

    #[test]
    fn rotate_setup_archives_this_process_log_only() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join(active_log_file_name());
        // A log left by a process that is gone is archived rather than kept as
        // an active file (#2858 G1.3). A live process's log is never renamed;
        // that case is asserted in
        // `dead_process_logs_are_archived_and_live_ones_are_kept`.
        let stale_other_log = dir.path().join("libragent.999999.log");
        fs::write(&log, "host line\n").unwrap();
        fs::write(&stale_other_log, "other process\n").unwrap();

        let (mut file, _, _) = prepare_log_file(dir.path(), true).unwrap();
        file.write_all(b"new line\n").unwrap();
        drop(file);

        assert_eq!(fs::read_to_string(&log).unwrap(), "new line\n");
        assert!(
            !stale_other_log.exists(),
            "a dead process's log must be archived, not left as an active file"
        );
        let backups = archived_logs(dir.path());
        assert_eq!(backups.len(), 2, "{backups:?}");
        assert!(backups
            .iter()
            .any(|path| fs::read_to_string(path).unwrap() == "host line\n"));
        assert!(backups
            .iter()
            .any(|path| fs::read_to_string(path).unwrap() == "other process\n"));
    }

    #[test]
    fn active_log_file_name_is_per_process() {
        let name = active_log_file_name();
        assert!(name.starts_with("libragent."));
        assert!(name.ends_with(".log"));
        assert!(is_active_log_file_name(&name));
        assert!(is_active_log_file_name(LEGACY_ACTIVE_LOG_FILE_NAME));
        assert!(!is_active_log_file_name("libragent_20240101_000000.log"));
    }

    /// #2858 G1.3: a dead process's `libragent.<pid>.log` must stop counting as
    /// an active file, or one file per launch survives every retention rule.
    #[test]
    fn dead_process_logs_are_archived_and_live_ones_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let dead_pid = 4_242u32;
        let live_pid = 4_243u32;
        let own_pid = std::process::id();
        write_archive(dir.path(), &format!("libragent.{dead_pid}.log"), 10, 400);
        write_archive(dir.path(), &format!("libragent.{live_pid}.log"), 10, 400);
        write_archive(dir.path(), &active_log_file_name(), 10, 400);

        // Proof-of-death predicate: only the dead pid is proven gone; the
        // live pid is unproven here, exactly like a failed probe, and must
        // be kept.
        let archived = archive_dead_process_logs_with(dir.path(), own_pid, |pid| pid == dead_pid);

        assert_eq!(archived, 1);
        assert!(
            !dir.path()
                .join(format!("libragent.{dead_pid}.log"))
                .exists(),
            "a dead process's log must stop being an active file"
        );
        assert!(
            dir.path()
                .join(format!("libragent.{live_pid}.log"))
                .exists(),
            "a live process's log must never be renamed"
        );
        assert!(
            dir.path().join(active_log_file_name()).exists(),
            "this process's own log must never be renamed"
        );

        let names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
            .collect();
        assert!(
            names.iter().any(|name| name.starts_with("libragent_")
                && name.ends_with(&format!("_{dead_pid}.log"))),
            "the dead log must be renamed with its pid: {names:?}"
        );
        assert!(
            names.iter().all(|name| !is_active_log_file_name(name)
                || active_log_pid(name) == Some(own_pid)
                || active_log_pid(name) == Some(live_pid)),
            "no dead log may keep an active name: {names:?}"
        );
        assert!(
            names.iter().any(|name| is_archived_log_file_name(name)),
            "the archived name must now be visible to the retention policy: {names:?}"
        );
    }
}
