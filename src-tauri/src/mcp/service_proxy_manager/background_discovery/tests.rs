use super::super::retry_policy::FailureKind;
use super::helpers::await_tool_discovery;
use super::types::{server_discovery_cap, stdio_failure_kind};
use crate::mcp::session_isolation::SessionMCPError;
use std::time::Duration;

#[test]
fn server_cap_outlasts_the_session_wait() {
    assert_eq!(server_discovery_cap(Duration::from_secs(30)).as_secs(), 600);
    assert_eq!(
        server_discovery_cap(Duration::from_secs(900)).as_secs(),
        900
    );
}

#[tokio::test]
async fn a_slow_server_is_not_given_up_at_the_session_wait() {
    let session_wait = Duration::from_millis(20);
    let slow = || async {
        tokio::time::sleep(Duration::from_millis(120)).await;
        Ok::<_, String>(3)
    };

    // The session-wait budget alone would have timed this server out (the old behavior).
    let at_session_wait =
        await_tool_discovery(slow(), session_wait, "stdio", "s", "sess", |_: &String| {
            FailureKind::Transient
        })
        .await;
    let timed_out = at_session_wait.unwrap_err();
    assert!(timed_out.message.contains("timed out"));
    assert_eq!(timed_out.kind, FailureKind::Transient);

    // With the server cap it keeps loading and becomes ready.
    let at_cap = await_tool_discovery(
        slow(),
        server_discovery_cap(session_wait),
        "stdio",
        "s",
        "sess",
        |_: &String| FailureKind::Transient,
    )
    .await;
    assert_eq!(at_cap.unwrap(), 3);
}

#[tokio::test]
async fn a_server_that_errors_is_failed_immediately() {
    let result = await_tool_discovery(
        async { Err::<(), _>("connection closed") },
        server_discovery_cap(Duration::from_secs(30)),
        "stdio",
        "s",
        "sess",
        |_: &&str| FailureKind::Transient,
    )
    .await;
    let error = result.unwrap_err().message;
    assert!(error.contains("tool discovery failed") && !error.contains("timed out"));
}

#[tokio::test]
async fn an_unavailable_session_container_is_a_permanent_failure() {
    let result = await_tool_discovery(
        async {
            Err::<(), _>(SessionMCPError::ContainerUnavailable(
                "Cannot pull Docker image 'missing:latest'".to_string(),
            ))
        },
        server_discovery_cap(Duration::from_secs(30)),
        "stdio",
        "s",
        "sess",
        stdio_failure_kind,
    )
    .await;
    let failure = result.unwrap_err();
    assert_eq!(failure.kind, FailureKind::Permanent);
    assert!(failure
        .message
        .contains("Session container is not available"));

    let crashed = SessionMCPError::InitFailed("connection closed".to_string());
    assert_eq!(stdio_failure_kind(&crashed), FailureKind::Transient);
}
