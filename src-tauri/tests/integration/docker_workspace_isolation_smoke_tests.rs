use crate::common;

use sea_orm_migration::MigratorTrait;
use std::collections::HashMap;
use std::process::Command;
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::builtin::workspace::persistent_shell::PersistentShellManager;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::models::workspace_isolation::{
    DockerWorkspaceConfig, WorkspaceIsolationMode,
};
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::services::WorkspaceRuntimeManager;
use tauri_mcp_agent_lib::session_isolation::{
    IsolatedProcessConfig, IsolationLevel, SessionIsolationManager,
};

#[tokio::test]
#[ignore = "requires local Docker daemon and pulls/runs a bash-capable image"]
async fn docker_workspace_run_shell_smoke_test() {
    if !docker_available() {
        eprintln!("Docker daemon is not available; skipping Docker workspace smoke test");
        return;
    }

    let db = common::setup_test_db().await;
    Migrator::up(&db, None)
        .await
        .expect("migrations should run for Docker smoke test");
    let repo = SqliteSessionRepository::new(db);
    tauri_mcp_agent_lib::set_session_repository(repo.clone());

    let temp_dir = tempfile::tempdir().expect("temporary workspace should be created");
    let workspace_path = temp_dir.path().to_path_buf();
    let session_id = format!("docker-smoke-{}", cuid2::create_id());
    let container_name = format!("libragent-session-{session_id}");

    let mut docker_env = HashMap::new();
    docker_env.insert("LIBRAGENT_DOCKER_SMOKE".to_string(), "smoke-ok".to_string());

    let session = SessionMetadata {
        id: session_id.clone(),
        name: Some("Docker smoke test".to_string()),
        status: SessionStatus::Idle,
        model: "test-model".to_string(),
        provider: "test-provider".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: Some(session_id.clone()),
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Docker,
        docker_config: Some(DockerWorkspaceConfig {
            image: "ubuntu:24.04".to_string(),
            env: docker_env,
        }),
        docker_container_name: Some(container_name.clone()),
        docker_host_workspace_path: Some(workspace_path.to_string_lossy().to_string()),
    };

    repo.upsert_session(&session)
        .await
        .expect("Docker smoke session should persist");

    let isolation_manager = SessionIsolationManager::new();
    let mut cmd = isolation_manager
        .create_isolated_command(IsolatedProcessConfig {
            session_id: session_id.clone(),
            workspace_path: workspace_path.clone(),
            command:
                "pwd && echo \"$LIBRAGENT_DOCKER_SMOKE\" > docker-smoke.txt && cat docker-smoke.txt"
                    .to_string(),
            args: Vec::new(),
            env_vars: HashMap::new(),
            isolation_level: IsolationLevel::Medium,
            shell_type: None,
        })
        .await
        .expect("Docker isolated command should be created");

    let output = cmd
        .output()
        .await
        .expect("Docker isolated command should run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Docker command failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("/workspace"),
        "stdout should include container cwd: {stdout}"
    );
    assert!(
        stdout.contains("smoke-ok"),
        "stdout should include env-propagated marker: {stdout}"
    );

    let smoke_file = workspace_path.join("docker-smoke.txt");
    assert!(
        smoke_file.is_file(),
        "Docker command should create file on host bind mount: {}",
        smoke_file.display()
    );
    let file_content =
        std::fs::read_to_string(&smoke_file).expect("smoke file should be readable on host");
    assert_eq!(file_content.trim(), "smoke-ok");

    let label = docker_inspect_label(&container_name);
    assert_eq!(label.as_deref(), Some(session_id.as_str()));

    cleanup_container(&container_name);
}

#[tokio::test]
#[ignore = "requires local Docker daemon and pulls/runs a bash-capable image"]
async fn docker_workspace_persistent_shell_smoke_test() {
    if !docker_available() {
        eprintln!("Docker daemon is not available; skipping Docker persistent shell smoke test");
        return;
    }

    let db = common::setup_test_db().await;
    Migrator::up(&db, None)
        .await
        .expect("migrations should run for Docker persistent shell smoke test");
    let repo = SqliteSessionRepository::new(db);
    tauri_mcp_agent_lib::set_session_repository(repo.clone());

    let temp_dir = tempfile::tempdir().expect("temporary workspace should be created");
    let workspace_path = temp_dir.path().to_path_buf();
    let session_id = format!("docker-persistent-{}", cuid2::create_id());
    let container_name = format!("libragent-session-{session_id}");

    let session = SessionMetadata {
        id: session_id.clone(),
        name: Some("Docker persistent shell smoke test".to_string()),
        status: SessionStatus::Idle,
        model: "test-model".to_string(),
        provider: "test-provider".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: Some(session_id.clone()),
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Docker,
        docker_config: Some(DockerWorkspaceConfig {
            image: "ubuntu:24.04".to_string(),
            env: HashMap::new(),
        }),
        docker_container_name: Some(container_name.clone()),
        docker_host_workspace_path: Some(workspace_path.to_string_lossy().to_string()),
    };

    repo.upsert_session(&session)
        .await
        .expect("Docker persistent smoke session should persist");

    let shell_manager = PersistentShellManager::new();
    let (stdout, stderr, exit_code, cwd) = shell_manager
        .execute(
            session_id.clone(),
            workspace_path.clone(),
            "mkdir -p nested && cd nested && export LIBRAGENT_PERSISTENT_MARKER=ok && pwd",
        )
        .await
        .expect("first persistent shell command should run");

    assert_eq!(
        exit_code, 0,
        "first persistent command failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(cwd, "/workspace/nested");
    assert!(
        stdout.contains("/workspace/nested"),
        "first stdout should include nested cwd: {stdout}"
    );

    let (stdout, stderr, exit_code, cwd) = shell_manager
        .execute(
            session_id.clone(),
            workspace_path.clone(),
            "pwd && echo \"$LIBRAGENT_PERSISTENT_MARKER\" > persistent.txt && cat persistent.txt",
        )
        .await
        .expect("second persistent shell command should reuse state");

    assert_eq!(
        exit_code, 0,
        "second persistent command failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(cwd, "/workspace/nested");
    assert!(
        stdout.contains("/workspace/nested"),
        "second stdout should preserve cwd: {stdout}"
    );
    assert!(
        stdout.contains("ok"),
        "second stdout should preserve exported env: {stdout}"
    );

    let marker_file = workspace_path.join("nested/persistent.txt");
    assert!(
        marker_file.is_file(),
        "persistent shell should write through bind mount: {}",
        marker_file.display()
    );
    let file_content =
        std::fs::read_to_string(&marker_file).expect("persistent marker file should be readable");
    assert_eq!(file_content.trim(), "ok");

    shell_manager
        .terminate_shell(&session_id)
        .await
        .expect("persistent shell should terminate");
    cleanup_container(&container_name);
}

#[tokio::test]
#[ignore = "requires local Docker daemon and pulls/runs a bash-capable image"]
async fn docker_workspace_stale_container_sweeper_smoke_test() {
    if !docker_available() {
        eprintln!("Docker daemon is not available; skipping Docker stale sweeper smoke test");
        return;
    }

    let session_id = format!("docker-stale-{}", cuid2::create_id());
    let container_name = format!("libragent-session-{session_id}");
    cleanup_container(&container_name);

    let label = format!("com.libragent.session_id={session_id}");
    let output = Command::new("docker")
        .args([
            "run",
            "-d",
            "--name",
            &container_name,
            "--label",
            &label,
            "ubuntu:24.04",
            "tail",
            "-f",
            "/dev/null",
        ])
        .output()
        .expect("docker run should execute");

    assert!(
        output.status.success(),
        "failed to create stale container\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let active_sessions = std::collections::HashSet::new();
    let removed = WorkspaceRuntimeManager::sweep_stale_containers(&active_sessions)
        .await
        .expect("stale sweeper should run");

    assert!(
        removed.contains(&container_name),
        "sweeper should remove stale test container, removed: {removed:?}"
    );
    assert!(
        docker_inspect_label(&container_name).is_none(),
        "stale container should no longer exist"
    );
}

fn docker_available() -> bool {
    Command::new("docker")
        .arg("info")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn docker_inspect_label(container_name: &str) -> Option<String> {
    let output = Command::new("docker")
        .args([
            "inspect",
            "-f",
            "{{ index .Config.Labels \"com.libragent.session_id\" }}",
            container_name,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn cleanup_container(container_name: &str) {
    let _ = Command::new("docker")
        .args(["rm", "-f", container_name])
        .output();
}
