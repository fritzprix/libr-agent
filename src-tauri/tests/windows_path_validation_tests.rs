use tauri_mcp_agent_lib::mcp::builtin::utils::{normalize_user_path, SecurityValidator};
use tempfile::tempdir;

#[test]
fn test_normalize_user_path_relative_unchanged() {
    assert_eq!(
        normalize_user_path("src/mcp/builtin/utils.rs"),
        "src/mcp/builtin/utils.rs"
    );
    assert_eq!(
        normalize_user_path("./subdir/file.txt"),
        "./subdir/file.txt"
    );
    // Bare `~name` (no slash) is not a home shortcut.
    assert_eq!(normalize_user_path("~backup"), "~backup");
}

#[test]
fn test_normalize_user_path_expands_home_shortcuts() {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let home = home.to_string_lossy().replace('\\', "/");

    assert_eq!(normalize_user_path("~"), home);
    assert_eq!(
        normalize_user_path("~/my_works/project/docs/PLANNING.md"),
        format!("{home}/my_works/project/docs/PLANNING.md")
    );
    assert_eq!(
        normalize_user_path("$HOME/my_works/project"),
        format!("{home}/my_works/project")
    );
    assert_eq!(
        normalize_user_path("${HOME}/my_works/project"),
        format!("{home}/my_works/project")
    );
}

#[test]
fn test_scoped_validator_rejects_tilde_outside_workspace() {
    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_scoped_with_base_dir(temp_dir.path().to_path_buf());
    let Some(home) = dirs::home_dir() else {
        return;
    };
    // Ensure the expanded path is outside this temp workspace.
    if home.starts_with(temp_dir.path()) {
        return;
    }

    let result = validator.validate_path_for_write("~/outside_workspace_file.txt");
    assert!(
        result.is_err(),
        "tilde path outside workspace must be rejected (not create literal '~' dir); got {result:?}"
    );

    // Relative join under workspace must never produce workspace/~/...
    let literal_tilde = temp_dir.path().join("~").join("outside_workspace_file.txt");
    assert!(
        !literal_tilde.exists(),
        "must not create a literal '~' directory under the workspace"
    );
}

#[test]
fn test_scoped_validator_allows_tilde_when_workspace_is_home() {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let validator = SecurityValidator::new_scoped_with_base_dir(home.clone());
    let expected = home.join("libragent_tilde_path_test.txt");
    let resolved = validator
        .validate_path_for_write("~/libragent_tilde_path_test.txt")
        .expect("tilde under home workspace should resolve");
    assert_eq!(resolved, expected);
}

#[test]
#[cfg(windows)]
fn test_normalize_user_path_windows_formats() {
    assert_eq!(
        normalize_user_path("/C:/Users/example/project"),
        "C:/Users/example/project"
    );
    assert_eq!(
        normalize_user_path("\\C:\\Users\\example\\project"),
        "C:/Users/example/project"
    );
    assert_eq!(
        normalize_user_path("file:///C:/Users/example/project"),
        "C:/Users/example/project"
    );
    assert_eq!(
        normalize_user_path("file://localhost/C:/Users/example/project"),
        "C:/Users/example/project"
    );
    assert_eq!(
        normalize_user_path("/c/Users/example/project"),
        "c:/Users/example/project"
    );
}

#[test]
#[cfg(unix)]
fn test_normalize_user_path_unix_file_url_and_msys_passthrough() {
    assert_eq!(
        normalize_user_path("file:///home/example/project/readme.md"),
        "/home/example/project/readme.md"
    );
    // On Unix, `/c/...` is a real absolute path and must not be rewritten as a drive path.
    assert_eq!(
        normalize_user_path("/c/Users/example/project"),
        "/c/Users/example/project"
    );
}

#[test]
#[cfg(windows)]
fn test_scoped_validator_windows_leading_slash() {
    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_scoped_with_base_dir(temp_dir.path().to_path_buf());
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "hello").expect("write test file");

    let raw_path = test_file.to_string_lossy();
    let leading_slash_path = format!("/{}", raw_path.replace('\\', "/"));

    let validated = validator
        .validate_path_for_read(&leading_slash_path)
        .expect("absolute path with leading slash within base_dir must succeed");
    assert_eq!(
        validated.canonicalize().unwrap(),
        test_file.canonicalize().unwrap()
    );
}

#[test]
#[cfg(windows)]
fn test_scoped_validator_rejects_outside_base_leading_slash_drive_path() {
    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_scoped_with_base_dir(temp_dir.path().to_path_buf());

    let outside = normalize_user_path("/C:/Windows/System32/drivers/etc/hosts");
    let result = validator.validate_path_for_read(&outside);
    assert!(
        result.is_err(),
        "path outside base_dir must be rejected, got: {result:?}"
    );
}

#[test]
fn test_scoped_validator_file_url_within_base() {
    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_scoped_with_base_dir(temp_dir.path().to_path_buf());
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "hello").expect("write test file");

    // Use a non-canonical path in the file:// URL when the OS exposes one (macOS
    // `/var` vs `/private/var`) so we cover the early containment canonicalize path.
    let file_url = url::Url::from_file_path(&test_file)
        .expect("file url")
        .to_string();

    let validated = validator
        .validate_path_for_read(&file_url)
        .expect("file:// URL within base_dir must succeed");
    assert_eq!(
        validated.canonicalize().unwrap(),
        test_file.canonicalize().unwrap()
    );
}

#[cfg(windows)]
mod windows_workspace_glob {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
    use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
    use tauri_mcp_agent_lib::session::SessionManager;

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

    #[tokio::test]
    async fn test_glob_files_handles_windows_absolute_path_with_leading_slash() {
        let temp_dir = tempdir().expect("temp dir");
        let session_id = "glob-files-leading-slash";
        let server = build_workspace_server(temp_dir.path(), session_id);
        let workspace_dir = server.get_workspace_dir(session_id);

        std::fs::write(
            workspace_dir.join("message_bubble.rs"),
            "pub struct MessageBubble;",
        )
        .expect("write file");

        let raw_workspace_path = workspace_dir.to_string_lossy();
        let leading_slash_path = format!("/{}", raw_workspace_path.replace('\\', "/"));

        let glob_result = server
            .call_tool(
                "globFiles",
                json!({
                    "path": leading_slash_path,
                    "filePattern": "*message*bubble*",
                }),
                Some(session_id.to_string()),
            )
            .await
            .expect("globFiles dispatch should succeed");

        let glob_text = extract_text_content(&glob_result);
        assert!(
            glob_text.contains("message_bubble.rs"),
            "Expected glob result to contain message_bubble.rs but got: {glob_text}"
        );
    }
}

/// Integration tests for workspace override ~ expansion via `set_override`
/// and `hydrate_persisted_workspace_override`.
#[cfg(test)]
mod workspace_override_tilde_expansion {
    use super::*;
    use std::fs;
    use std::sync::Arc;
    use tauri_mcp_agent_lib::entity::prelude::*;
    use tauri_mcp_agent_lib::entity::session;
    use tauri_mcp_agent_lib::execution_mode::ExecutionMode;
    use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
    use tauri_mcp_agent_lib::repositories::session_repository::{
        SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
    };
    use tauri_mcp_agent_lib::services::workspace_service::WorkspaceService;
    use tauri_mcp_agent_lib::session::hydrate_persisted_workspace_override;
    use tauri_mcp_agent_lib::session::manager::SessionManager;
    use tokio::sync::Mutex;

    /// Global mutex to serialize access to global OnceLock state across async tests.
    static GLOBAL_STATE_LOCK: Mutex<()> = Mutex::const_new(());

    /// Shared setup: creates a temp base dir, in-memory SQLite DB with sessions table,
    /// a SessionManager, and a SessionRepository. Returns (temp_dir, session_manager, repo).
    /// Callers must hold the GLOBAL_STATE_LOCK to avoid race conditions with global state.
    async fn setup_workspace_test() -> (tempfile::TempDir, SessionManager, SqliteSessionRepository)
    {
        use sea_orm::{Database, DatabaseConnection, Statement};

        let temp_dir = tempfile::TempDir::expect("create temp dir for workspace override tests");
        let base_path = temp_dir.path().to_path_buf();

        // Create the session manager
        let session_manager =
            SessionManager::new_with_base_dir(base_path.clone()).expect("init session manager");

        // Create in-memory SQLite DB and create sessions table
        let db: DatabaseConnection = Database::connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");

        db.execute(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                name TEXT,
                status TEXT NOT NULL DEFAULT 'idle',
                model TEXT NOT NULL DEFAULT 'gpt-4',
                provider TEXT NOT NULL DEFAULT 'openai',
                assistant_id TEXT,
                parent_session_id TEXT,
                lineage_id TEXT,
                depth INTEGER,
                max_depth INTEGER,
                max_fanout INTEGER,
                org_id TEXT,
                org_name TEXT,
                org_root_session_id TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_viewed_at INTEGER,
                last_message_at INTEGER,
                last_attention_at INTEGER,
                last_attention_reason TEXT,
                is_bookmarked INTEGER NOT NULL DEFAULT 0,
                execution_mode TEXT NOT NULL DEFAULT 'normal',
                workspace_override TEXT,
                workspace_isolation TEXT NOT NULL DEFAULT 'none',
                docker_config_json TEXT,
                docker_container_name TEXT,
                docker_host_workspace_path TEXT
            )
            "#
            .to_string(),
        ))
        .await
        .expect("create sessions table");

        let repo = SqliteSessionRepository::new(db);
        (temp_dir, session_manager, repo)
    }

    /// Inserts a session row into the repository, then sets global state
    /// (DB connection, session repo, session manager).
    async fn init_global_state(
        session_id: &str,
        repo: &SqliteSessionRepository,
        session_manager: &SessionManager,
    ) {
        // Create workspace dirs so they exist
        let _ = session_manager.get_session_workspace_dir_by_id(session_id);

        let session_meta = SessionMetadata {
            id: session_id.to_string(),
            name: Some("tilde-test".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            assistant_id: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            org_id: None,
            org_name: None,
            org_root_session_id: None,
            created_at: 0,
            updated_at: 0,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            execution_mode: ExecutionMode::Normal,
            workspace_override: None,
            workspace_isolation: WorkspaceIsolationMode::default(),
            docker_config: None,
            docker_container_name: None,
            docker_host_workspace_path: None,
        };

        repo.upsert_session(&session_meta)
            .await
            .expect("upsert session");

        // Set global state
        use tauri_mcp_agent_lib::state::{
            get_database_connection, reset_state, set_database_connection, set_session_manager,
            set_session_repository,
        };

        reset_state();
        set_session_repository(repo.clone());
        set_database_connection(
            get_database_connection(), // already set via set_session_repository's repo field
        );
        // We need to set the DB connection separately — get it from repo
        // Actually, set_session_repository doesn't set DATABASE_CONNECTION.
        // Let's set it manually.
    }

    fn set_global_state_for_test(
        db: DatabaseConnection,
        repo: SqliteSessionRepository,
        session_manager: SessionManager,
    ) {
        use tauri_mcp_agent_lib::state::{
            set_database_connection, set_session_manager, set_session_repository,
        };

        set_database_connection(db);
        set_session_repository(repo);
        set_session_manager(session_manager);
    }

    fn cleanup_global_state() {
        use tauri_mcp_agent_lib::state::reset_state;
        reset_state();
    }

    #[tokio::test]
    async fn test_set_override_expands_tilde_to_absolute_in_db() {
        let _lock = GLOBAL_STATE_LOCK.lock().await;

        let (temp_dir, session_manager, repo) = setup_workspace_test().await;
        let session_id = "set-override-tilde-test";

        // Create a real directory to use as the override target
        let override_dir = temp_dir.path().join("real_workspace");
        fs::create_dir(&override_dir).expect("create override dir");

        let db = repo.clone().db.clone();

        // Initialize session in pool by resolving its workspace dir
        let _ = session_manager.get_session_workspace_dir_by_id(session_id);

        let session_meta = SessionMetadata {
            id: session_id.to_string(),
            name: Some("tilde-test".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            assistant_id: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            org_id: None,
            org_name: None,
            org_root_session_id: None,
            created_at: 0,
            updated_at: 0,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            execution_mode: ExecutionMode::Normal,
            workspace_override: None,
            workspace_isolation: WorkspaceIsolationMode::default(),
            docker_config: None,
            docker_container_name: None,
            docker_host_workspace_path: None,
        };
        repo.upsert_session(&session_meta)
            .await
            .expect("upsert session");

        set_global_state_for_test(db, repo.clone(), session_manager);

        // Call set_override with a tilde path
        let tilde_path = format!("~/{}", override_dir.file_name().unwrap().to_string_lossy());
        WorkspaceService::set_override(session_id, tilde_path)
            .await
            .expect("set_override should succeed");

        // Verify: DB should store the expanded absolute path, NOT the literal ~/...
        let retrieved = repo
            .get_session(session_id)
            .await
            .expect("get session")
            .expect("session exists");

        let stored = retrieved
            .workspace_override
            .expect("workspace_override should be set");

        // The stored path must be an absolute path starting with /home/ (home dir expansion)
        assert!(
            stored.starts_with("/home/"),
            "DB should store expanded absolute path (starts with /home/), got: {stored}"
        );

        // Must NOT contain a literal "~"
        assert!(
            !stored.contains('~'),
            "DB must NOT contain literal '~' in workspace_override, got: {stored}"
        );

        // The expanded path should match the override_dir
        assert!(
            stored == override_dir.to_string_lossy().as_ref()
                || stored.starts_with(&override_dir.to_string_lossy()),
            "Expanded path should match override_dir. Expected: {}, Got: {}",
            override_dir.display(),
            stored
        );

        // Verify in-memory pool also has the expanded path
        let pool_path = session_manager.get_session_workspace_dir_by_id(session_id);
        assert_eq!(
            pool_path, override_dir,
            "Session manager workspace override should be the expanded path"
        );

        cleanup_global_state();
    }

    #[tokio::test]
    async fn test_hydrate_legacy_tilde_override_expands_path() {
        let _lock = GLOBAL_STATE_LOCK.lock().await;

        let (temp_dir, session_manager, repo) = setup_workspace_test().await;
        let session_id = "hydrate-tilde-test";

        // Simulate a legacy DB row with a literal ~/... path.
        // We create a "home-like" directory under temp to represent where ~/legacy_ws would expand.
        let legacy_home_like = temp_dir.path().join("home_like");
        fs::create_dir(&legacy_home_like).expect("create legacy home-like dir");
        let legacy_ws = legacy_home_like.join("legacy_workspace");
        fs::create_dir(&legacy_ws).expect("create legacy workspace dir");

        // Insert session with literal ~/... path (simulating pre-fix DB row)
        let legacy_tilde_path = format!("~/{}", legacy_ws.file_name().unwrap().to_string_lossy());

        let session_meta = SessionMetadata {
            id: session_id.to_string(),
            name: Some("hydrate-test".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            assistant_id: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            org_id: None,
            org_name: None,
            org_root_session_id: None,
            created_at: 0,
            updated_at: 0,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            execution_mode: ExecutionMode::Normal,
            workspace_override: Some(legacy_tilde_path.clone()),
            workspace_isolation: WorkspaceIsolationMode::default(),
            docker_config: None,
            docker_container_name: None,
            docker_host_workspace_path: None,
        };
        repo.upsert_session(&session_meta)
            .await
            .expect("upsert legacy session");

        set_global_state_for_test(repo.db.clone(), repo.clone(), session_manager);

        // Call hydrate — it should expand ~/legacy_workspace to the real path
        // and register it in the session manager's workspace pool.
        let result =
            hydrate_persisted_workspace_override(&repo, &session_manager, session_id).await;

        assert!(
            result.is_ok(),
            "hydrate should succeed for legacy tilde path; got: {:?}",
            result
        );

        // The session manager should now have the expanded path registered
        let pool_path = session_manager.get_session_workspace_dir_by_id(session_id);
        assert_eq!(
            pool_path,
            legacy_ws,
            "Session manager should have expanded path after hydration. Expected: {}, Got: {}",
            legacy_ws.display(),
            pool_path.display()
        );

        cleanup_global_state();
    }
}
