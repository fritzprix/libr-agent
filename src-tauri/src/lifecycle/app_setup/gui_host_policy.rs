//! Behavior lock for GUI startup hosting its own backend (#2853).
//!
//! These tests drive the extracted startup sequence instead of grepping
//! source text: the HTTP bind runs first, recovery and the scheduler start
//! only after the bind succeeds (#2823), the own-record publish wait runs
//! last and fails startup when it fails, and the second-GUI refusal is
//! classified for the user-facing dialog (#2863 G1.1/G1.2).

use super::{is_second_backend_refusal, run_gui_host_startup};
use std::sync::{Arc, Mutex};

fn bind_ok(
    steps: Arc<Mutex<Vec<&'static str>>>,
) -> impl std::future::Future<Output = Result<u16, String>> {
    async move {
        steps.lock().unwrap().push("bind");
        Ok(3030u16)
    }
}

fn bind_err(
    steps: Arc<Mutex<Vec<&'static str>>>,
) -> impl std::future::Future<Output = Result<u16, String>> {
    async move {
        steps.lock().unwrap().push("bind");
        Err("Failed to bind HTTP server on port range 3030..3040".to_string())
    }
}

fn wait_ok(
    steps: Arc<Mutex<Vec<&'static str>>>,
) -> impl std::future::Future<Output = Result<(), String>> {
    async move {
        steps.lock().unwrap().push("wait");
        Ok(())
    }
}

fn wait_err(
    steps: Arc<Mutex<Vec<&'static str>>>,
) -> impl std::future::Future<Output = Result<(), String>> {
    async move {
        steps.lock().unwrap().push("wait");
        Err("HTTP server did not publish this process's backend record".to_string())
    }
}

fn recorder(steps: Arc<Mutex<Vec<&'static str>>>, name: &'static str) -> impl FnOnce() {
    move || {
        steps.lock().unwrap().push(name);
    }
}

fn steps_of(run: &Arc<Mutex<Vec<&'static str>>>) -> Vec<&'static str> {
    run.lock().unwrap().clone()
}

#[tokio::test]
async fn failed_bind_skips_recovery_scheduler_and_publish_wait() {
    // #2823: a refused or failed bind must not leave an executor running.
    let steps = Arc::new(Mutex::new(Vec::new()));
    let result = run_gui_host_startup(
        bind_err(steps.clone()),
        recorder(steps.clone(), "recovery"),
        recorder(steps.clone(), "scheduler"),
        wait_err(steps.clone()),
    )
    .await;
    assert!(result.is_err(), "a failed bind fails the sequence");
    assert_eq!(
        steps_of(&steps),
        vec!["bind"],
        "recovery, scheduler and the publish wait must not start after a failed bind"
    );
}

#[tokio::test]
async fn successful_bind_runs_recovery_then_scheduler_then_publish_wait() {
    let steps = Arc::new(Mutex::new(Vec::new()));
    let result = run_gui_host_startup(
        bind_ok(steps.clone()),
        recorder(steps.clone(), "recovery"),
        recorder(steps.clone(), "scheduler"),
        wait_ok(steps.clone()),
    )
    .await;
    assert!(
        result.is_ok(),
        "bind + recovery + scheduler + live record succeeds"
    );
    assert_eq!(
        steps_of(&steps),
        vec!["bind", "recovery", "scheduler", "wait"],
        "the startup order is bind first, then recovery, scheduler, publish wait"
    );
}

#[tokio::test]
async fn failed_publish_wait_fails_startup_after_started_steps() {
    let steps = Arc::new(Mutex::new(Vec::new()));
    let result = run_gui_host_startup(
        bind_ok(steps.clone()),
        recorder(steps.clone(), "recovery"),
        recorder(steps.clone(), "scheduler"),
        wait_err(steps.clone()),
    )
    .await;
    assert!(
        result.is_err(),
        "setup must fail when this process never publishes a live record"
    );
    assert_eq!(
        steps_of(&steps),
        vec!["bind", "recovery", "scheduler", "wait"],
    );
}

#[test]
fn second_backend_refusal_is_classified_from_the_bind_message() {
    // server::init records exactly this shape when another live backend
    // already owns the profile; app_setup must route it to the dialog.
    let refusal = format!(
        "{} (record: C:\\data\\backend.json); refusing to bind a second HTTP server",
        crate::server::SECOND_BACKEND_REFUSAL_PREFIX
    );
    assert!(
        is_second_backend_refusal(&refusal),
        "the second-backend refusal must be classified for the dialog"
    );

    assert!(
        !is_second_backend_refusal("Failed to bind HTTP server on port range 3030..3040"),
        "a plain bind failure is a setup error, not the second-GUI case"
    );
    assert!(
        !is_second_backend_refusal(
            "HTTP server did not publish this process's backend record; refusing to start a second backend"
        ),
        "a publish timeout is a setup error, not the second-GUI case"
    );
}
