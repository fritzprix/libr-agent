use chrono::TimeZone;
use tauri_mcp_agent_lib::commands::scheduled_task_commands::ScheduledTaskDto;
use tauri_mcp_agent_lib::entity::scheduled_task::Model as ScheduledTaskModel;
use tauri_mcp_agent_lib::scheduled::runner::{
    compute_next_run_for_schedule_timezone, compute_next_run_for_timezone,
};
use tauri_mcp_agent_lib::scheduled::{
    normalize_cron, TASK_CATEGORY_GLOBAL, SCHEDULE_TIMEZONE_LOCAL, SCHEDULE_TIMEZONE_UTC,
};

#[test]
fn daily_schedule_is_interpreted_in_local_time() {
    let timezone = chrono::FixedOffset::east_opt(9 * 3600).unwrap();

    // 2025-03-03 09:30:00 in UTC+09:00.
    let reference = timezone
        .with_ymd_and_hms(2025, 3, 3, 9, 30, 0)
        .single()
        .unwrap()
        .timestamp_millis();

    // A daily 09:00 schedule should fire on the next local day at 09:00,
    // not nine hours later on the same UTC day.
    let next_run = compute_next_run_for_timezone("0 9 * * *", reference, timezone).unwrap();

    let expected = timezone
        .with_ymd_and_hms(2025, 3, 4, 9, 0, 0)
        .single()
        .unwrap()
        .timestamp_millis();

    assert_eq!(next_run, expected);
}

#[test]
fn weekly_schedule_is_interpreted_in_local_time() {
    let timezone = chrono::FixedOffset::east_opt(9 * 3600).unwrap();

    // 2025-03-03 09:30:00 in UTC+09:00, which is Monday.
    let reference = timezone
        .with_ymd_and_hms(2025, 3, 3, 9, 30, 0)
        .single()
        .unwrap()
        .timestamp_millis();

    // Weekly on Monday at 09:00 should roll to the next Monday because the
    // current Monday occurrence has already passed.
    let next_run = compute_next_run_for_timezone("0 9 * * 1", reference, timezone).unwrap();

    let expected = timezone
        .with_ymd_and_hms(2025, 3, 10, 9, 0, 0)
        .single()
        .unwrap()
        .timestamp_millis();

    assert_eq!(next_run, expected);
}

#[test]
fn monthly_schedule_is_interpreted_in_local_time() {
    let timezone = chrono::FixedOffset::east_opt(9 * 3600).unwrap();

    // 2025-03-15 09:30:00 in UTC+09:00.
    let reference = timezone
        .with_ymd_and_hms(2025, 3, 15, 9, 30, 0)
        .single()
        .unwrap()
        .timestamp_millis();

    // Monthly on the 15th at 09:00 should roll to the next month because the
    // current month's occurrence has already passed.
    let next_run = compute_next_run_for_timezone("0 9 15 * *", reference, timezone).unwrap();

    let expected = timezone
        .with_ymd_and_hms(2025, 4, 15, 9, 0, 0)
        .single()
        .unwrap()
        .timestamp_millis();

    assert_eq!(next_run, expected);
}

#[test]
fn weekday_normalization_supports_common_cron_syntax() {
    assert_eq!(normalize_cron("0 9 * * 1"), "0 0 9 * * 2 *");
    assert_eq!(normalize_cron("0 9 * * 0,6"), "0 0 9 * * 1,7 *");
    assert_eq!(normalize_cron("0 9 * * 1-5"), "0 0 9 * * 2-6 *");
    assert_eq!(normalize_cron("0 9 * * 1-5/2"), "0 0 9 * * 2-6/2 *");
}

#[test]
fn schedule_timezone_mode_preserves_utc_compatibility() {
    let reference = chrono::Utc
        .with_ymd_and_hms(2025, 3, 3, 9, 30, 0)
        .single()
        .unwrap()
        .timestamp_millis();

    let utc_next =
        compute_next_run_for_schedule_timezone("0 9 * * *", reference, SCHEDULE_TIMEZONE_UTC)
            .unwrap()
            .unwrap();
    let local_next =
        compute_next_run_for_schedule_timezone("0 9 * * *", reference, SCHEDULE_TIMEZONE_LOCAL);

    let expected_utc = compute_next_run_for_timezone("0 9 * * *", reference, chrono::Utc).unwrap();

    assert_eq!(utc_next, expected_utc);
    assert!(local_next.is_ok());
}

#[test]
fn disabled_legacy_utc_tasks_expose_a_future_display_run() {
    let dto = ScheduledTaskDto::from(ScheduledTaskModel {
        id: "task-1".to_string(),
        name: "Legacy UTC task".to_string(),
        task_category: TASK_CATEGORY_GLOBAL.to_string(),
        cron_expression: Some("0 9 * * *".to_string()),
        schedule_timezone: SCHEDULE_TIMEZONE_UTC.to_string(),
        assistant_id: "assistant-1".to_string(),
        group_id: None,
        group_name: None,
        message: "hello".to_string(),
        yolo_mode: false,
        created_by_session_id: None,
        session_id: None,
        workspace_override: None,
        enabled: false,
        last_run_at: None,
        next_run_at: None,
        created_at: 0,
        updated_at: 0,
    });

    assert!(dto.next_run_at.is_some());
}
