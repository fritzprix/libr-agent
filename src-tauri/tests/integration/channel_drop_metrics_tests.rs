use std::collections::HashMap;

use tauri_mcp_agent_lib::mcp::session_isolation::channel_events::{
    advance_channel_drop_metrics_log_window_for_test, channel_drop_metrics_snapshot_for_test,
    create_channel_event_bus, reset_channel_drop_metrics_for_test, try_emit_channel_event,
    SessionChannelEvent, CHANNEL_EVENT_BUFFER_SIZE,
};
use tauri_mcp_agent_lib::mcp::types::ChannelNotification;

fn sample_message_event() -> SessionChannelEvent {
    SessionChannelEvent::Message {
        server_name: "test-server".to_string(),
        notification: ChannelNotification {
            content: "hello".to_string(),
            meta: HashMap::new(),
        },
    }
}

#[test]
fn channel_drop_metrics_count_buffer_full_events() {
    reset_channel_drop_metrics_for_test();

    let (tx, _rx) = create_channel_event_bus();
    let event = sample_message_event();

    for _ in 0..=CHANNEL_EVENT_BUFFER_SIZE {
        try_emit_channel_event(&tx, event.clone(), "buffer-full-test");
    }

    let (buffer_full, receiver_closed) = channel_drop_metrics_snapshot_for_test();
    assert!(buffer_full >= 1, "expected at least one buffer-full drop");
    assert_eq!(receiver_closed, 0);
}

#[test]
fn channel_drop_metrics_count_receiver_closed_events() {
    reset_channel_drop_metrics_for_test();

    let (tx, rx) = create_channel_event_bus();
    drop(rx);

    try_emit_channel_event(&tx, sample_message_event(), "receiver-closed-test");

    let (buffer_full, receiver_closed) = channel_drop_metrics_snapshot_for_test();
    assert_eq!(buffer_full, 0);
    assert_eq!(receiver_closed, 1);
}

#[test]
fn channel_drop_metrics_throttles_periodic_counter_flush() {
    reset_channel_drop_metrics_for_test();

    let (tx, _rx) = create_channel_event_bus();
    let event = sample_message_event();

    for _ in 0..=CHANNEL_EVENT_BUFFER_SIZE {
        try_emit_channel_event(&tx, event.clone(), "throttle-test");
    }

    let (buffer_full_before_flush, _) = channel_drop_metrics_snapshot_for_test();
    assert!(
        buffer_full_before_flush >= 1,
        "expected drops to accumulate while log window is active"
    );

    advance_channel_drop_metrics_log_window_for_test();
    try_emit_channel_event(&tx, event, "throttle-flush");

    let (buffer_full_after_flush, receiver_closed_after_flush) =
        channel_drop_metrics_snapshot_for_test();
    assert_eq!(
        buffer_full_after_flush, 0,
        "expired log window should flush buffer-full counter"
    );
    assert_eq!(receiver_closed_after_flush, 0);
}
