//! Background scheduler worker.
//!
//! Polls once per minute, loads due tasks from the DB, and dispatches each
//! to `runner::execute_task`.  Mirrors the `IndexingWorker` pattern.

use std::time::Duration;

use tauri::AppHandle;
use tokio::time::sleep;

use super::runner;

/// Background worker that fires due scheduled tasks.
pub struct SchedulerWorker {
    /// Held to prevent the spawned task from being dropped (cancelled).
    _task_handle: tauri::async_runtime::JoinHandle<()>,
}

impl SchedulerWorker {
    /// Start the worker.  `check_interval` defaults to 60 s in production.
    pub fn new(app_handle: AppHandle, check_interval: Duration) -> Self {
        let task_handle = tauri::async_runtime::spawn(async move {
            worker_loop(app_handle, check_interval).await;
        });

        Self {
            _task_handle: task_handle,
        }
    }
}

async fn worker_loop(app_handle: AppHandle, check_interval: Duration) {
    log::info!(
        "⏰ Scheduled task worker started (interval: {:?})",
        check_interval
    );

    // Run an immediate check on startup to catch any tasks missed while the app
    // was closed (e.g. next_run_at was 11:40, app restarted at 11:50).
    let now_ms = chrono::Utc::now().timestamp_millis();
    if let Err(e) = runner::execute_due_tasks(&app_handle, now_ms).await {
        log::error!("⏰ Scheduled task runner error (startup check): {}", e);
    }

    loop {
        sleep(check_interval).await;

        let now_ms = chrono::Utc::now().timestamp_millis();

        if let Err(e) = runner::execute_due_tasks(&app_handle, now_ms).await {
            log::error!("⏰ Scheduled task runner error: {}", e);
        }
    }
}
