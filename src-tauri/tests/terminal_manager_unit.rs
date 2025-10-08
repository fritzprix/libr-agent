mod common;
use common::setup_terminal_manager;
use std::time::Duration;

#[tokio::test]
async fn test_open_new_terminal_returns_id() {
    let manager = setup_terminal_manager();
    let result = manager.open_new_terminal(None, None, None).await;
    assert!(result.is_ok(), "Should open a new terminal successfully.");
    let terminal_id = result.unwrap();
    assert!(!terminal_id.is_empty(), "Terminal ID should not be empty.");
}

#[tokio::test]
async fn test_list_terminals_returns_opened_terminal() {
    let manager = setup_terminal_manager();
    let terminal_id = manager
        .open_new_terminal(None, None, None)
        .await
        .expect("Failed to open terminal");
    let terminals = manager.list_terminals(None).await;
    assert_eq!(terminals.len(), 1, "Should list one active terminal.");
    assert_eq!(
        terminals[0].terminal_id, terminal_id,
        "The listed terminal ID should match the opened one."
    );
}

#[tokio::test]
async fn test_close_terminal_removes_from_list() {
    let manager = setup_terminal_manager();
    let terminal_id = manager
        .open_new_terminal(None, None, None)
        .await
        .expect("Failed to open terminal");

    let close_result = manager.close_terminal(&terminal_id).await;
    assert!(
        close_result.is_ok(),
        "Should close the terminal successfully."
    );

    let terminals = manager.list_terminals(None).await;
    assert!(
        terminals.is_empty(),
        "Should have no active terminals after closing."
    );
}

#[tokio::test]
async fn test_write_and_read_roundtrip() {
    let manager = setup_terminal_manager();
    let terminal_id = manager
        .open_new_terminal(None, None, None)
        .await
        .expect("Failed to open terminal");

    let write_result = manager
        .write_to_terminal(&terminal_id, "echo 'hello world'")
        .await;
    assert!(
        write_result.is_ok(),
        "Should write to the terminal successfully."
    );

    // Allow some time for the command to execute
    tokio::time::sleep(Duration::from_millis(500)).await;

    let read_result = manager.read_terminal_output(&terminal_id, None).await;
    assert!(
        read_result.is_ok(),
        "Should read from the terminal successfully."
    );

    let output = read_result.unwrap();
    let output_text: Vec<String> = output.output.into_iter().map(|o| o.line).collect();
    let joined_output = output_text.join("\n");

    assert!(
        joined_output.contains("hello world"),
        "Output should contain 'hello world'. Full output: {}",
        joined_output
    );

    manager.close_terminal(&terminal_id).await.unwrap();
}

#[tokio::test]
async fn test_read_terminal_output_incrementally() {
    let manager = setup_terminal_manager();
    let terminal_id = manager
        .open_new_terminal(None, None, None)
        .await
        .expect("Failed to open terminal");

    manager
        .write_to_terminal(&terminal_id, "echo 'line 1'")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let read_1 = manager
        .read_terminal_output(&terminal_id, None)
        .await
        .unwrap();
    assert!(
        read_1
            .output
            .iter()
            .any(|line| line.line.contains("line 1")),
        "First read should contain 'line 1'"
    );
    let next_index = read_1.next_index;

    manager
        .write_to_terminal(&terminal_id, "echo 'line 2'")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let read_2 = manager
        .read_terminal_output(&terminal_id, Some(next_index))
        .await
        .unwrap();
    assert!(
        read_2
            .output
            .iter()
            .any(|line| line.line.contains("line 2")),
        "Incremental read should contain 'line 2'"
    );
    assert!(
        !read_2
            .output
            .iter()
            .any(|line| line.line.contains("line 1")),
        "Incremental read should not contain 'line 1'"
    );

    manager.close_terminal(&terminal_id).await.unwrap();
}
