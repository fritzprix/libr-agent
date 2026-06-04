use crate::common;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::services::skill_service::invalidate_skill_scan_cache;
use tauri_mcp_agent_lib::session::{get_session_manager, SessionManager};
use tauri_mcp_agent_lib::set_session_repository;
use tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url;
use tokio::sync::{Mutex, MutexGuard, OnceCell};

struct TestContext {
    _temp_dir: tempfile::TempDir,
    db: sea_orm::DatabaseConnection,
}

static TEST_CONTEXT: OnceCell<TestContext> = OnceCell::const_new();
static TEST_MUTEX: Mutex<()> = Mutex::const_new(());

#[derive(Debug)]
struct SkillScopeFixture {
    label: String,
    directory: PathBuf,
    skill_file: PathBuf,
    token: String,
}

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

fn build_workspace_server(base_dir: &Path, session_id: &str) -> WorkspaceServer {
    let session_manager =
        SessionManager::new_with_base_dir(base_dir.to_path_buf()).expect("session manager");
    WorkspaceServer::new(session_id.to_string(), Arc::new(session_manager))
}

async fn test_guard() -> MutexGuard<'static, ()> {
    TEST_MUTEX.lock().await
}

async fn session_repo() -> SqliteSessionRepository {
    let db = TEST_CONTEXT
        .get_or_init(|| async {
            common::register_sqlite_vec();
            let temp_dir = tempfile::tempdir().expect("temp dir should be created");
            let db_path = temp_dir
                .path()
                .join("workspace-skill-access-regression-tests.db");
            let url = format_sqlite_url(&db_path.to_string_lossy());
            let options = SqliteConnectOptions::from_str(&url)
                .expect("sqlite url should be valid")
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .expect("sqlite pool should connect");
            let db = sea_orm::SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
            tauri_mcp_agent_lib::migration::Migrator::up(&db, None)
                .await
                .expect("migrations should run");
            set_session_repository(SqliteSessionRepository::new(db.clone()));
            TestContext {
                _temp_dir: temp_dir,
                db,
            }
        })
        .await
        .db
        .clone();

    SqliteSessionRepository::new(db)
}

fn make_session(session_id: &str, assistant_id: &str) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Workspace Skill Access Regression".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        agent_config: Some(
            json!({
                "id": assistant_id,
                "name": "Regression Assistant",
                "systemPrompt": "Protect skill access regression coverage."
            })
            .to_string(),
        ),
        parent_session_id: None,
        lineage_id: None,
        depth: None,
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
        yolo_mode: false,
        unsafe_mode: false,
        workspace_override: None,
    }
}

fn write_skill(scope_root: &Path, skill_dir_name: &str, token: &str) -> SkillScopeFixture {
    write_skill_with_name(scope_root, skill_dir_name, skill_dir_name, token)
}

fn write_skill_with_name(
    scope_root: &Path,
    skill_dir_name: &str,
    skill_name: &str,
    token: &str,
) -> SkillScopeFixture {
    let directory = scope_root.join(skill_dir_name);
    std::fs::create_dir_all(&directory).expect("create skill directory");
    let skill_file = directory.join("SKILL.md");
    let content = format!(
        "---\nname: {skill_name}\ndescription: {token}\n---\n# {skill_dir_name}\n{token}\n"
    );
    std::fs::write(&skill_file, content).expect("write skill file");

    SkillScopeFixture {
        label: skill_dir_name.to_string(),
        directory,
        skill_file,
        token: token.to_string(),
    }
}

fn global_base_data_dir() -> PathBuf {
    get_session_manager()
        .expect("global session manager")
        .get_base_data_dir()
        .clone()
}

fn seed_skill_scopes(
    base_data_dir: &Path,
    server: &WorkspaceServer,
    session_id: &str,
    assistant_id: &str,
) -> Vec<SkillScopeFixture> {
    let system_dir_name = format!("system-skill-{session_id}");
    let user_dir_name = format!("user-skill-{session_id}");
    let assistant_dir_name = format!("assistant-skill-{session_id}");
    let workspace_dir_name = format!("workspace-skill-{session_id}");

    vec![
        write_skill(
            &base_data_dir.join("system_skills"),
            &system_dir_name,
            "SYSTEM_SCOPE_TOKEN",
        ),
        write_skill(
            &base_data_dir.join("user_skills"),
            &user_dir_name,
            "USER_SCOPE_TOKEN",
        ),
        write_skill(
            &base_data_dir
                .join("assistants")
                .join(assistant_id)
                .join("skills"),
            &assistant_dir_name,
            "ASSISTANT_SCOPE_TOKEN",
        ),
        write_skill(
            &server
                .get_workspace_dir(session_id)
                .join(".libragent")
                .join("skills"),
            &workspace_dir_name,
            "WORKSPACE_SCOPE_TOKEN",
        ),
    ]
}

fn skill_scope_root_paths(
    base_data_dir: &Path,
    workspace_dir: &Path,
    assistant_id: &str,
) -> Vec<SkillScopeFixture> {
    vec![
        SkillScopeFixture {
            label: "system-scope-root".to_string(),
            directory: base_data_dir.join("system_skills"),
            skill_file: PathBuf::new(),
            token: "system-skill".to_string(),
        },
        SkillScopeFixture {
            label: "user-scope-root".to_string(),
            directory: base_data_dir.join("user_skills"),
            skill_file: PathBuf::new(),
            token: "user-skill".to_string(),
        },
        SkillScopeFixture {
            label: "assistant-scope-root".to_string(),
            directory: base_data_dir
                .join("assistants")
                .join(assistant_id)
                .join("skills"),
            skill_file: PathBuf::new(),
            token: "assistant-skill".to_string(),
        },
        SkillScopeFixture {
            label: "workspace-scope-root".to_string(),
            directory: workspace_dir.join(".libragent").join("skills"),
            skill_file: PathBuf::new(),
            token: "workspace-skill".to_string(),
        },
    ]
}

fn assert_success(result: &MCPResult, label: &str) {
    let text = extract_text_content(result);
    assert_ne!(
        result.is_error,
        Some(true),
        "{label} should remain accessible via workspace MCP: {text}"
    );
}

#[tokio::test]
async fn read_file_allows_absolute_skill_paths_across_all_scopes() {
    let _guard = test_guard().await;
    let repo = session_repo().await;
    let base_data_dir = global_base_data_dir();
    let session_id = "workspace-read-skill-scopes";
    let assistant_id = "assistant-skill-owner";
    repo.upsert_session(&make_session(session_id, assistant_id))
        .await
        .expect("upsert session");

    let server = build_workspace_server(&base_data_dir, session_id);
    let scopes = seed_skill_scopes(&base_data_dir, &server, session_id, assistant_id);
    invalidate_skill_scan_cache();

    for scope in scopes {
        let result = server
            .handle_read_file(
                json!({ "path": scope.skill_file.to_string_lossy() }),
                Some(session_id.to_string()),
            )
            .await
            .expect("readFile should return MCP result");
        assert_success(&result, &scope.label);

        let text = extract_text_content(&result);
        assert!(
            text.contains(&scope.token),
            "{} token should be readable through readFile: {text}",
            scope.label
        );
    }
}

#[tokio::test]
async fn list_directory_allows_absolute_skill_directories_across_all_scopes() {
    let _guard = test_guard().await;
    let repo = session_repo().await;
    let base_data_dir = global_base_data_dir();
    let session_id = "workspace-list-skill-scopes";
    let assistant_id = "assistant-skill-owner-list";
    repo.upsert_session(&make_session(session_id, assistant_id))
        .await
        .expect("upsert session");

    let server = build_workspace_server(&base_data_dir, session_id);
    let scopes = seed_skill_scopes(&base_data_dir, &server, session_id, assistant_id);
    invalidate_skill_scan_cache();

    for scope in scopes {
        let result = server
            .handle_list_directory(
                json!({ "path": scope.directory.to_string_lossy() }),
                Some(session_id.to_string()),
            )
            .await
            .expect("listDirectory should return MCP result");
        assert_success(&result, &scope.label);

        let text = extract_text_content(&result);
        assert!(
            text.contains("SKILL.md"),
            "{} directory listing should expose SKILL.md: {text}",
            scope.label
        );
    }
}

#[tokio::test]
async fn search_allows_absolute_skill_directories_across_all_scopes() {
    let _guard = test_guard().await;
    let repo = session_repo().await;
    let base_data_dir = global_base_data_dir();
    let session_id = "workspace-search-skill-scopes";
    let assistant_id = "assistant-skill-owner-search";
    repo.upsert_session(&make_session(session_id, assistant_id))
        .await
        .expect("upsert session");

    let server = build_workspace_server(&base_data_dir, session_id);
    let scopes = seed_skill_scopes(&base_data_dir, &server, session_id, assistant_id);
    invalidate_skill_scan_cache();

    for scope in scopes {
        let result = server
            .handle_search(
                json!({
                    "path": scope.directory.to_string_lossy(),
                    "query": scope.token,
                    "limit": 20
                }),
                Some(session_id.to_string()),
            )
            .await
            .expect("search should return MCP result");
        assert_success(&result, &scope.label);

        let text = extract_text_content(&result);
        assert!(
            text.contains(&scope.token),
            "{} search should find the unique token: {text}",
            scope.label
        );
        assert!(
            text.contains("SKILL.md"),
            "{} search should still point at the skill file: {text}",
            scope.label
        );
    }
}

#[tokio::test]
async fn list_directory_allows_skill_scope_roots_across_all_scopes() {
    let _guard = test_guard().await;
    let repo = session_repo().await;
    let base_data_dir = global_base_data_dir();
    let session_id = "workspace-list-skill-scope-roots";
    let assistant_id = "assistant-skill-owner-roots";
    repo.upsert_session(&make_session(session_id, assistant_id))
        .await
        .expect("upsert session");

    let server = build_workspace_server(&base_data_dir, session_id);
    let _scopes = seed_skill_scopes(&base_data_dir, &server, session_id, assistant_id);
    invalidate_skill_scan_cache();

    for scope_root in skill_scope_root_paths(
        &base_data_dir,
        &server.get_workspace_dir(session_id),
        assistant_id,
    ) {
        let result = server
            .handle_list_directory(
                json!({ "path": scope_root.directory.to_string_lossy() }),
                Some(session_id.to_string()),
            )
            .await
            .expect("listDirectory should return MCP result");
        assert_success(&result, &scope_root.label);

        let text = extract_text_content(&result);
        assert!(
            text.contains(&scope_root.token),
            "{} should list the child skill directory: {text}",
            scope_root.label
        );
    }
}

#[tokio::test]
async fn read_file_allows_shadowed_lower_precedence_skill_by_absolute_path() {
    let _guard = test_guard().await;
    let repo = session_repo().await;
    let base_data_dir = global_base_data_dir();
    let session_id = "workspace-read-shadowed-skill";
    let assistant_id = "assistant-skill-owner-shadowed";
    repo.upsert_session(&make_session(session_id, assistant_id))
        .await
        .expect("upsert session");

    let server = build_workspace_server(&base_data_dir, session_id);
    let system_shadowed = write_skill_with_name(
        &base_data_dir.join("system_skills"),
        "system-shadowed-skill",
        "shared-skill",
        "SYSTEM_SHADOWED_TOKEN",
    );
    let workspace_winner = write_skill_with_name(
        &server
            .get_workspace_dir(session_id)
            .join(".libragent")
            .join("skills"),
        "workspace-shadowing-skill",
        "shared-skill",
        "WORKSPACE_WINNER_TOKEN",
    );
    invalidate_skill_scan_cache();

    for scope in [system_shadowed, workspace_winner] {
        let result = server
            .handle_read_file(
                json!({ "path": scope.skill_file.to_string_lossy() }),
                Some(session_id.to_string()),
            )
            .await
            .expect("readFile should return MCP result");
        assert_success(&result, &scope.label);

        let text = extract_text_content(&result);
        assert!(
            text.contains(&scope.token),
            "{} should remain readable even when shadowed: {text}",
            scope.label
        );
    }
}
