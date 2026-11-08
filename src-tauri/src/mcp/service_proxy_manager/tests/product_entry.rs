//! Issue #2633: terminate and delete driven through the product entry points
//! (`terminate_session_with_dispatcher`, `delete_session_with_dispatcher`,
//! `delete_session_only_with_dispatcher`) with real OS processes.
#![cfg(unix)]

use super::termination_fence::{pid_alive, spawn_real_process, wait_for_pid_death};
use super::*;
use crate::agent::events::{AgentEvent, AgentEventDispatcher, WorkflowCompletionReason};
use crate::agent::lifecycle::{
    delete_session_only_with_dispatcher, delete_session_with_dispatcher,
};
use crate::agent::state::AgentSession;
use crate::agent::workflow::terminate_session_with_dispatcher;
use crate::repositories::session_repository::{SessionRepository, SqliteSessionRepository};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::RwLock;

#[derive(Default)]
pub(super) struct RecordingDispatcher {
    events: Mutex<Vec<AgentEvent>>,
}

impl RecordingDispatcher {
    pub(super) fn terminated(&self, session_id: &str) -> bool {
        self.events.lock().expect("event lock").iter().any(|event| {
            matches!(
                event,
                AgentEvent::WorkflowCompleted {
                    session_id: id,
                    reason: WorkflowCompletionReason::Terminated,
                } if id == session_id
            )
        })
    }
}

impl AgentEventDispatcher for RecordingDispatcher {
    fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
        self.events.lock().expect("event lock").push(event);
        Ok(())
    }

    fn emit_compact_state(
        &self,
        _event: crate::agent::llm::types::CompactStateEvent,
    ) -> Result<(), String> {
        Ok(())
    }

    fn emit_completion_cancel(
        &self,
        _request: crate::agent::llm::types::CompletionCancelRequest,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub(super) struct EntryDeps {
    pub(super) session_repo: Arc<dyn SessionRepository>,
    pub(super) active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    pub(super) dispatcher: RecordingDispatcher,
}

pub(super) fn entry_deps(manager: &MCPServiceProxyManager) -> EntryDeps {
    EntryDeps {
        session_repo: Arc::new(SqliteSessionRepository::new((*manager.db).clone())),
        active_sessions: Arc::new(RwLock::new(HashMap::new())),
        dispatcher: RecordingDispatcher::default(),
    }
}

pub(super) async fn set_parent(manager: &MCPServiceProxyManager, child: &str, parent: &str) {
    session::Entity::update(session::ActiveModel {
        id: Set(child.to_string()),
        parent_session_id: Set(Some(parent.to_string())),
        ..Default::default()
    })
    .exec(&*manager.db)
    .await
    .expect("failed to set parent session");
}

#[tokio::test]
async fn terminate_entry_kills_only_the_target_and_refuses_late_work() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let deps = entry_deps(manager);
    let target = "entry-terminate-target";
    let bystander = "entry-terminate-bystander";

    insert_test_session(manager, target).await;
    insert_test_session(manager, bystander).await;
    let target_pid = spawn_real_process(manager, target).await;
    let bystander_pid = spawn_real_process(manager, bystander).await;
    let token = crate::server::wake::wake_token_for(target);

    // The session is not loaded in memory, so the entry reports it after
    // running the full cleanup.
    let _ = terminate_session_with_dispatcher(
        &deps.session_repo,
        &deps.active_sessions,
        manager,
        &deps.dispatcher,
        target.to_string(),
    )
    .await;

    assert!(
        wait_for_pid_death(target_pid).await,
        "target process {target_pid} still alive after terminate"
    );
    assert!(
        pid_alive(bystander_pid),
        "another session's process ({bystander_pid}) must survive terminate"
    );
    assert!(
        !crate::server::wake::verify_wake_token(target, &token),
        "terminate must revoke the wake token"
    );
    assert!(deps.dispatcher.terminated(target));
    assert!(manager.get_proxy(target).await.is_none());

    let error = manager
        .call_tool(target, "playbook__listPlaybooks", json!({}))
        .await
        .expect_err("a terminated session must refuse late tool calls");
    assert!(error.contains("terminated"), "unexpected error: {error}");

    // A second terminate is safe.
    let _ = terminate_session_with_dispatcher(
        &deps.session_repo,
        &deps.active_sessions,
        manager,
        &deps.dispatcher,
        target.to_string(),
    )
    .await;

    manager.terminate_session_resources(bystander).await;
    assert!(wait_for_pid_death(bystander_pid).await);
}

#[tokio::test]
async fn cascade_delete_entry_kills_parent_and_child_processes_only() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let deps = entry_deps(manager);
    let parent = "entry-delete-parent";
    let child = "entry-delete-child";
    let bystander = "entry-delete-bystander";

    insert_test_session(manager, parent).await;
    insert_test_session(manager, child).await;
    insert_test_session(manager, bystander).await;
    set_parent(manager, child, parent).await;
    let parent_pid = spawn_real_process(manager, parent).await;
    let child_pid = spawn_real_process(manager, child).await;
    let bystander_pid = spawn_real_process(manager, bystander).await;

    let deleted = delete_session_with_dispatcher(
        &deps.session_repo,
        &deps.active_sessions,
        manager,
        &deps.dispatcher,
        parent.to_string(),
    )
    .await
    .expect("cascade delete must succeed");

    assert_eq!(deleted, vec![parent.to_string(), child.to_string()]);
    assert!(
        wait_for_pid_death(parent_pid).await,
        "parent process {parent_pid} still alive after delete"
    );
    assert!(
        wait_for_pid_death(child_pid).await,
        "child process {child_pid} still alive after cascade delete"
    );
    assert!(
        pid_alive(bystander_pid),
        "another session's process ({bystander_pid}) must survive delete"
    );
    assert!(deps.dispatcher.terminated(parent));
    assert!(deps.dispatcher.terminated(child));
    assert!(deps
        .session_repo
        .get_session(parent)
        .await
        .expect("session lookup")
        .is_none());

    manager.terminate_session_resources(bystander).await;
    assert!(wait_for_pid_death(bystander_pid).await);
}

#[tokio::test]
async fn delete_only_entry_kills_the_parent_and_orphans_a_live_child() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let deps = entry_deps(manager);
    let parent = "entry-delete-only-parent";
    let child = "entry-delete-only-child";

    insert_test_session(manager, parent).await;
    insert_test_session(manager, child).await;
    set_parent(manager, child, parent).await;
    let parent_pid = spawn_real_process(manager, parent).await;
    let child_pid = spawn_real_process(manager, child).await;

    let (deleted, orphaned) = delete_session_only_with_dispatcher(
        &deps.session_repo,
        &deps.active_sessions,
        manager,
        &deps.dispatcher,
        parent.to_string(),
    )
    .await
    .expect("delete-only must succeed");

    assert_eq!(deleted, parent);
    assert_eq!(orphaned, vec![child.to_string()]);
    assert!(
        wait_for_pid_death(parent_pid).await,
        "parent process {parent_pid} still alive after delete-only"
    );
    assert!(
        pid_alive(child_pid),
        "the orphaned child's process ({child_pid}) must survive delete-only"
    );
    assert!(!deps.dispatcher.terminated(child));

    let orphan = deps
        .session_repo
        .get_session(child)
        .await
        .expect("session lookup")
        .expect("the child row must remain");
    assert_eq!(orphan.parent_session_id, None);

    manager.terminate_session_resources(child).await;
    assert!(wait_for_pid_death(child_pid).await);
}
