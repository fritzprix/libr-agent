mod common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::agent::tool_approvals::{
    evaluate_tool_execution_policy, ToolExecutionPolicyDecision,
};
use tauri_mcp_agent_lib::lifecycle::repositories::init_repositories;
use tauri_mcp_agent_lib::mcp::builtin::workspace::code_execution::shell::policy::{
    evaluate_shell_policy, ShellPolicyAction, ShellPolicyContext,
};
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::session::SessionManager;
use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus};
use tempfile::tempdir;
use tokio::sync::OnceCell;

fn extract_text_content(result: &MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("text content expected")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_workspace_server(base_dir: &std::path::Path, session_id: &str) -> WorkspaceServer {
    let session_manager =
        SessionManager::new_with_base_dir(base_dir.to_path_buf()).expect("session manager");
    WorkspaceServer::new(session_id.to_string(), Arc::new(session_manager))
}

fn build_workspace_server_with_manager(
    base_dir: &std::path::Path,
    session_id: &str,
) -> (WorkspaceServer, Arc<SessionManager>) {
    let session_manager = Arc::new(
        SessionManager::new_with_base_dir(base_dir.to_path_buf()).expect("session manager"),
    );
    let server = WorkspaceServer::new(session_id.to_string(), session_manager.clone());
    (server, session_manager)
}

async fn ensure_settings_repository() {
    static REPOSITORIES: OnceCell<()> = OnceCell::const_new();

    REPOSITORIES
        .get_or_init(|| async {
            let db = common::setup_test_db_with_migrations().await;
            init_repositories(&db).await;
            init_session_bus(SessionBus::new());
            init_concurrency_gate(ConcurrencyGate::new(
                DEFAULT_MAX_ACTIVE_AGENTS,
                DEFAULT_MAX_SUSPENDED_AGENTS,
                DEFAULT_MAX_ACTIVE_PROCESSES,
                DEFAULT_MAX_SUSPENDED_PROCESSES,
            ));
        })
        .await;
}

#[tokio::test]
async fn tool_policy_blocks_sensitive_shell_command() {
    let decision = evaluate_tool_execution_policy(
        "workspace__runShell",
        &json!({ "command": "cat ~/.ssh/id_rsa" }),
    )
    .await;

    match decision {
        ToolExecutionPolicyDecision::Block(blocked) => {
            assert!(
                blocked.message.contains("protected path"),
                "expected protected-path block reason, got: {}",
                blocked.message
            );
        }
        other => panic!("expected block decision, got {:?}", other),
    }
}

#[tokio::test]
async fn tool_policy_blocks_attached_redirection_to_sensitive_path() {
    let decision = evaluate_tool_execution_policy(
        "workspace__runShell",
        &json!({ "command": "cat</etc/shadow" }),
    )
    .await;

    match decision {
        ToolExecutionPolicyDecision::Block(blocked) => {
            assert!(
                blocked.message.contains("protected path"),
                "expected protected-path block reason, got: {}",
                blocked.message
            );
        }
        other => panic!("expected block decision, got {:?}", other),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn tool_policy_blocks_globbed_sensitive_path() {
    let decision = evaluate_tool_execution_policy(
        "workspace__runShell",
        &json!({ "command": "cat /etc/sha*" }),
    )
    .await;

    match decision {
        ToolExecutionPolicyDecision::Block(blocked) => {
            assert!(
                blocked.message.contains("command glob"),
                "expected glob block reason, got: {}",
                blocked.message
            );
        }
        other => panic!("expected block decision, got {:?}", other),
    }
}

#[tokio::test]
async fn tool_policy_blocks_dynamic_shell_evaluation() {
    let decision = evaluate_tool_execution_policy(
        "workspace__runShell",
        &json!({ "command": "cat $(echo /etc/shadow)" }),
    )
    .await;

    match decision {
        ToolExecutionPolicyDecision::RequireHardApproval(request) => {
            assert!(
                request.description.contains("hard approval"),
                "expected hard approval description, got: {}",
                request.description
            );
            assert!(
                request.description.contains("dynamic shell evaluation")
                    || request.description.contains("substitution syntax"),
                "expected dynamic-evaluation reason, got: {}",
                request.description
            );
        }
        other => panic!("expected block decision, got {:?}", other),
    }
}

#[test]
fn shell_policy_blocks_relative_secret_after_cd_home() {
    let workspace = tempdir().expect("temp dir");
    let decision = evaluate_shell_policy(ShellPolicyContext {
        tool_name: "runInPersistentShell",
        command: "cd ~ && cat .ssh/id_rsa",
        workspace_dir: Some(workspace.path()),
        current_dir: None,
        environment: None,
        force_approval: true,
    });

    assert_eq!(decision.action, ShellPolicyAction::Block);
    assert!(
        decision.reason.contains("protected"),
        "expected protected-path reason, got: {}",
        decision.reason
    );
}

#[test]
fn shell_policy_blocks_when_current_directory_is_protected() {
    let workspace = tempdir().expect("temp dir");
    let home = dirs::home_dir().expect("home directory");
    let protected_dir = home.join(".ssh");
    let decision = evaluate_shell_policy(ShellPolicyContext {
        tool_name: "runInPersistentShell",
        command: "ls",
        workspace_dir: Some(workspace.path()),
        current_dir: Some(&protected_dir),
        environment: None,
        force_approval: true,
    });

    assert_eq!(decision.action, ShellPolicyAction::Block);
    assert!(
        decision.reason.contains("current shell directory"),
        "expected cwd protection reason, got: {}",
        decision.reason
    );
}

#[tokio::test]
async fn run_shell_returns_permission_denied_for_protected_path() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "run-shell-policy-block";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let result = server
        .handle_run_shell(
            json!({
                "command": "cat ~/.ssh/id_rsa"
            }),
            session_id,
        )
        .await
        .expect("runShell should return an MCPResult");

    let text = extract_text_content(&result);
    assert!(
        text.contains("Shell command blocked by policy"),
        "expected policy block message, got: {text}"
    );
}

#[tokio::test]
async fn spawn_process_returns_permission_denied_for_protected_path() {
    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "spawn-process-policy-block";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let result = server
        .handle_spawn_process(
            json!({
                "command": "cat ~/.aws/credentials"
            }),
            session_id,
        )
        .await
        .expect("spawnProcess should return an MCPResult");

    let text = extract_text_content(&result);
    assert!(
        text.contains("Shell command blocked by policy"),
        "expected policy block message, got: {text}"
    );
}

#[tokio::test]
async fn shell_policy_preview_keeps_command_tail() {
    let long_command = format!("echo {} && cat ~/.ssh/id_rsa", "a".repeat(220));
    let decision =
        evaluate_tool_execution_policy("workspace__runShell", &json!({ "command": long_command }))
            .await;

    match decision {
        ToolExecutionPolicyDecision::Block(blocked) => {
            assert!(
                blocked.message.contains("protected path"),
                "expected sensitive path block, got: {}",
                blocked.message
            );
        }
        other => panic!("expected block decision, got {:?}", other),
    }

    let approval = evaluate_tool_execution_policy(
        "workspace__runShell",
        &json!({ "command": format!("echo {}", "a".repeat(260)) }),
    )
    .await;

    match approval {
        ToolExecutionPolicyDecision::RequireApproval(request) => {
            assert!(
                request.input_preview.contains("echo"),
                "preview should keep command head: {}",
                request.input_preview
            );
            assert!(
                request.input_preview.contains(&"a".repeat(40)),
                "preview should retain the tail of long commands: {}",
                request.input_preview
            );
        }
        other => panic!("expected approval decision, got {:?}", other),
    }
}

#[test]
fn shell_policy_requires_approval_for_unresolved_variable_path() {
    let workspace = tempdir().expect("temp dir");
    let decision = evaluate_shell_policy(ShellPolicyContext {
        tool_name: "runShell",
        command: "cat $SECRET_DIR/id_rsa",
        workspace_dir: Some(workspace.path()),
        current_dir: None,
        environment: None,
        force_approval: false,
    });

    assert_eq!(decision.action, ShellPolicyAction::RequireHardApproval);
    assert!(
        decision.reason.contains("unresolved shell variable"),
        "expected unresolved-variable reason, got: {}",
        decision.reason
    );
}

#[tokio::test]
async fn tool_policy_requires_hard_approval_for_unresolved_variable_path() {
    let decision = evaluate_tool_execution_policy(
        "workspace__runShell",
        &json!({ "command": "cat $SECRET_DIR/id_rsa" }),
    )
    .await;

    match decision {
        ToolExecutionPolicyDecision::RequireHardApproval(request) => {
            assert!(
                request.description.contains("hard approval"),
                "expected hard approval description, got: {}",
                request.description
            );
            assert!(
                request.description.contains("unresolved shell variable"),
                "expected unresolved-variable reason, got: {}",
                request.description
            );
        }
        other => panic!("expected hard approval decision, got {:?}", other),
    }
}

#[test]
fn shell_policy_tracks_variable_across_segments() {
    let workspace = tempdir().expect("temp dir");
    let decision = evaluate_shell_policy(ShellPolicyContext {
        tool_name: "runShell",
        command: "SECRET=/etc/shadow; cat $SECRET",
        workspace_dir: Some(workspace.path()),
        current_dir: None,
        environment: None,
        force_approval: false,
    });

    assert_eq!(decision.action, ShellPolicyAction::Block);
    assert!(
        decision.reason.contains("protected"),
        "expected protected-path reason, got: {}",
        decision.reason
    );
}

#[tokio::test]
async fn tool_policy_blocks_sensitive_shell_command_from_env_variable() {
    let home = dirs::home_dir().expect("home directory");
    let decision = evaluate_tool_execution_policy(
        "workspace__runShell",
        &json!({
            "command": "cat $SECRET_DIR/id_rsa",
            "env": {
                "SECRET_DIR": home.join(".ssh").to_string_lossy().to_string()
            }
        }),
    )
    .await;

    match decision {
        ToolExecutionPolicyDecision::Block(blocked) => {
            assert!(
                blocked.message.contains("protected path"),
                "expected protected-path block reason, got: {}",
                blocked.message
            );
        }
        other => panic!("expected block decision, got {:?}", other),
    }
}

#[cfg(unix)]
#[test]
fn shell_policy_blocks_symlinked_protected_target() {
    use std::os::unix::fs::symlink;

    let workspace = tempdir().expect("temp dir");
    let link_path = workspace.path().join("etc-link");
    symlink("/etc", &link_path).expect("create symlink");

    let decision = evaluate_shell_policy(ShellPolicyContext {
        tool_name: "runShell",
        command: "cat ./etc-link/shadow",
        workspace_dir: Some(workspace.path()),
        current_dir: None,
        environment: None,
        force_approval: false,
    });

    assert_eq!(decision.action, ShellPolicyAction::Block);
    assert!(
        decision.reason.contains("protected"),
        "expected protected-path reason, got: {}",
        decision.reason
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_shell_blocks_symlinked_protected_target() {
    use std::os::unix::fs::symlink;

    ensure_settings_repository().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "run-shell-policy-symlink-block";
    let (server, session_manager) =
        build_workspace_server_with_manager(temp_dir.path(), session_id);
    let workspace_dir = session_manager.get_session_workspace_dir_by_id(session_id);
    symlink("/etc", workspace_dir.join("etc-link")).expect("create symlink");

    let result = server
        .handle_run_shell(
            json!({
                "command": "cat ./etc-link/shadow"
            }),
            session_id,
        )
        .await
        .expect("runShell should return an MCPResult");

    let text = extract_text_content(&result);
    assert!(
        text.contains("Shell command blocked by policy"),
        "expected policy block message, got: {text}"
    );
}
