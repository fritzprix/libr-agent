//! Cross-process scheduled-task execution claims (#2857).
//!
//! Standalone binary so it runs on Windows CI. `integration_tests` is
//! `#![cfg(not(windows))]` because it links WebView.
//!
//! Every GUI process on one profile hosts its own scheduler, so a due task
//! row alone is not exclusive. These tests drive the conditional claim the
//! runner takes before firing a task: the first backend to claim a due task
//! executes it, every other backend skips, and the claim frees again on
//! release or when a crashed holder's expiry lapses.

use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use tauri_mcp_agent_lib::execution_mode::ExecutionMode;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::scheduled_task_repository::{
    CreateScheduledTaskParams, ScheduledTaskRepository, SqliteScheduledTaskRepository,
};
use tauri_mcp_agent_lib::scheduled::TASK_CATEGORY_GLOBAL;

async fn setup_isolated_db() -> DatabaseConnection {
    tauri_mcp_agent_lib::lifecycle::database::register_sqlite_vec();
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should connect");
    Migrator::up(&db, None)
        .await
        .expect("migrations should run");
    db
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Seed one enabled GLOBAL task that is already due.
async fn seed_due_task(repo: &SqliteScheduledTaskRepository, id: &str, next_run_at: i64) {
    repo.create_scheduled_task(CreateScheduledTaskParams {
        id: id.to_string(),
        name: format!("due task {id}"),
        task_category: TASK_CATEGORY_GLOBAL.to_string(),
        cron_expression: Some("0 * * * *".to_string()),
        schedule_timezone: "local".to_string(),
        assistant_id: "asst-test".to_string(),
        message: "tick".to_string(),
        execution_mode: ExecutionMode::Normal,
        created_by_session_id: None,
        session_id: None,
        workspace_override: None,
        reset_planning_state: false,
        next_run_at: Some(next_run_at),
    })
    .await
    .expect("seed task should insert");
}

#[tokio::test]
async fn a_second_backend_cannot_claim_a_task_the_first_backend_holds() {
    let db = setup_isolated_db().await;
    let repo = SqliteScheduledTaskRepository::new(db);
    let due_at = now_ms() - 1_000;
    seed_due_task(&repo, "task-exclusive", due_at).await;

    // Both schedulers list the same due row…
    let due = repo.list_due_tasks(now_ms()).await.expect("due list");
    assert_eq!(due.len(), 1);

    // …but only the first conditional claim lands.
    let first = "run-first";
    let second = "run-second";
    let now = now_ms();
    assert!(
        repo.try_claim_run("task-exclusive", first, now, now + 600_000)
            .await
            .expect("first claim"),
        "the first backend must win the claim"
    );
    assert!(
        !repo
            .try_claim_run("task-exclusive", second, now, now + 600_000)
            .await
            .expect("second claim"),
        "a live foreign claim must block the second backend"
    );

    // The holder may re-claim (its own next tick, or a retry).
    assert!(
        repo.try_claim_run("task-exclusive", first, now + 1, now + 600_001)
            .await
            .expect("re-entrant claim"),
        "the holder keeps entering freely"
    );

    let task = repo
        .get_scheduled_task("task-exclusive")
        .await
        .expect("read task")
        .expect("task exists");
    assert_eq!(task.run_claim_run.as_deref(), Some(first));
    assert!(task.run_claim_expires_at.unwrap() > now);
}

#[tokio::test]
async fn releasing_the_claim_lets_the_next_backend_fire() {
    let db = setup_isolated_db().await;
    let repo = SqliteScheduledTaskRepository::new(db);
    let due_at = now_ms() - 1_000;
    seed_due_task(&repo, "task-release", due_at).await;

    let now = now_ms();
    assert!(repo
        .try_claim_run("task-release", "run-a", now, now + 600_000)
        .await
        .expect("claim a"));
    repo.release_run_claim("task-release", "run-a")
        .await
        .expect("release a");

    assert!(
        repo.try_claim_run("task-release", "run-b", now, now + 600_000)
            .await
            .expect("claim b"),
        "a released claim is free for the next backend"
    );

    // Releasing someone else's claim changes nothing.
    repo.release_run_claim("task-release", "run-a")
        .await
        .expect("stale release is a no-op");
    let task = repo
        .get_scheduled_task("task-release")
        .await
        .expect("read task")
        .expect("task exists");
    assert_eq!(task.run_claim_run.as_deref(), Some("run-b"));
}

#[tokio::test]
async fn a_lapsed_claim_from_a_crashed_holder_is_taken_over() {
    let db = setup_isolated_db().await;
    let repo = SqliteScheduledTaskRepository::new(db);
    let due_at = now_ms() - 1_000;
    seed_due_task(&repo, "task-stale", due_at).await;

    let now = now_ms();
    assert!(repo
        .try_claim_run("task-stale", "run-dead", now, now + 50)
        .await
        .expect("claim dead"));
    // The holder dies before releasing; once its expiry lapses the next
    // due tick anywhere on the profile takes the task over.
    let later = now + 51;
    assert!(
        !repo
            .try_claim_run("task-stale", "run-alive", now, now + 50)
            .await
            .expect("claim while live"),
        "the claim is still exclusive before expiry"
    );
    assert!(
        repo.try_claim_run("task-stale", "run-alive", later, later + 600_000)
            .await
            .expect("takeover"),
        "a lapsed claim must be free"
    );
    let task = repo
        .get_scheduled_task("task-stale")
        .await
        .expect("read task")
        .expect("task exists");
    assert_eq!(task.run_claim_run.as_deref(), Some("run-alive"));
}

#[tokio::test]
async fn record_run_keeps_the_holder_claim_and_releases_the_due_slot() {
    let db = setup_isolated_db().await;
    let repo = SqliteScheduledTaskRepository::new(db);
    let due_at = now_ms() - 1_000;
    seed_due_task(&repo, "task-record", due_at).await;

    let now = now_ms();
    assert!(repo
        .try_claim_run("task-record", "run-a", now, now + 600_000)
        .await
        .expect("claim"));

    // The holder records its run (the normal end of `execute_task`); the
    // active claim must survive that write so a second scheduler that
    // listed the task as due a moment earlier still skips it.
    let next = now + 60_000;
    repo.record_run("task-record", None, now, Some(next))
        .await
        .expect("record run");

    assert!(
        !repo
            .try_claim_run("task-record", "run-b", now, now + 600_000)
            .await
            .expect("late claim"),
        "the recorded run's holder keeps its claim until release"
    );
    assert!(
        repo.list_due_tasks(now).await.expect("due list").is_empty(),
        "record_run moved next_run_at out of the due window"
    );

    let task = repo
        .get_scheduled_task("task-record")
        .await
        .expect("read task")
        .expect("task exists");
    assert_eq!(task.run_claim_run.as_deref(), Some("run-a"));
    assert_eq!(task.last_run_at, Some(now));
    assert_eq!(task.next_run_at, Some(next));
}

#[tokio::test]
async fn a_claim_without_an_expiry_self_heals_to_free() {
    let db = setup_isolated_db().await;
    let repo = SqliteScheduledTaskRepository::new(db.clone());
    seed_due_task(&repo, "task-corrupt", now_ms() - 1_000).await;

    // A row left with a holder but no expiry is unreadable state; the next
    // claimer must not be locked out forever.
    {
        use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};
        use tauri_mcp_agent_lib::entity::scheduled_task;
        let mut active = scheduled_task::Entity::find_by_id("task-corrupt")
            .one(&db)
            .await
            .expect("find")
            .expect("exists")
            .into_active_model();
        active.run_claim_run = Set(Some("run-ghost".to_string()));
        active.run_claim_expires_at = Set(None);
        active.save(&db).await.expect("corrupt row");
    }

    let now = now_ms();
    assert!(
        repo.try_claim_run("task-corrupt", "run-new", now, now + 600_000)
            .await
            .expect("claim corrupt"),
        "a holder-without-expiry claim is treated as free"
    );
}
