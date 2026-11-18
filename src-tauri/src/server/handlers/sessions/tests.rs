//! Unit tests for the Session API session routes (#2888), extracted from
//! the inline `mod tests` of the original single-file module.

use super::super::helpers::open_gate;
use super::super::types::{ListSessionsQuery, PatchSessionRequest};
use super::crud::{
    classify_spawn_error, get_session_reply, list_sessions_reply, open_session_reply,
};
use super::lifecycle::patch_session_reply;
use crate::agent::state::AgentSession;
use crate::execution_mode::ExecutionMode;
use crate::repositories::SessionRepository;
use crate::server::session_ownership;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{http::StatusCode, Rejection, Reply};

use crate::repositories::{SessionMetadata, SessionStatus, SqliteSessionRepository};
use migration::{Migrator, MigratorTrait};

async fn sqlite_session_repo() -> Arc<dyn SessionRepository> {
    crate::lifecycle::database::register_sqlite_vec();
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should connect");
    Migrator::up(&db, None)
        .await
        .expect("migrations should run");
    Arc::new(SqliteSessionRepository::new(db))
}

#[test]
fn a_refused_docker_workspace_is_a_bad_request() {
    let (status, error) = classify_spawn_error(
            "Invalid Docker workspace config: The workspace '/home/user' is your home directory or contains it.",
        );
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error.contains("home directory"));
}

async fn get_session_json(
    id: &str,
    repo: &Arc<dyn SessionRepository>,
) -> (StatusCode, serde_json::Value) {
    let response = get_session_reply(id, None, repo).await.into_response();
    let status = response.status();
    let body = warp::hyper::body::to_bytes(response.into_body())
        .await
        .expect("reply body should be readable");
    let json = serde_json::from_slice(&body).expect("reply body should be JSON");
    (status, json)
}

async fn reply_to_json(
    reply: warp::reply::WithStatus<warp::reply::Json>,
) -> (StatusCode, serde_json::Value) {
    let response = reply.into_response();
    let status = response.status();
    let body = warp::hyper::body::to_bytes(response.into_body())
        .await
        .expect("reply body should be readable");
    let json = serde_json::from_slice(&body).expect("reply body should be JSON");
    (status, json)
}

/// #2700: a release build can list sessions, page by page.
#[tokio::test]
async fn list_sessions_pages_through_every_session_once() {
    let repo = sqlite_session_repo().await;
    for (index, id) in ["list-a", "list-b", "list-c"].into_iter().enumerate() {
        let mut session = SessionMetadata::test_fixture(id);
        session.updated_at = 1_000 + index as i64;
        repo.upsert_session(&session)
            .await
            .expect("session should be stored");
    }

    let (status, first) = reply_to_json(
        list_sessions_reply(
            ListSessionsQuery {
                limit: Some(2),
                ..Default::default()
            },
            &repo,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let first_ids: Vec<&str> = first["sessions"]
        .as_array()
        .expect("sessions array")
        .iter()
        .map(|session| session["id"].as_str().expect("id"))
        .collect();
    assert_eq!(first_ids.len(), 2, "{first}");
    let cursor = &first["nextCursor"];
    assert!(
        cursor.is_object(),
        "a full page must carry a cursor: {first}"
    );

    let (_, second) = reply_to_json(
        list_sessions_reply(
            ListSessionsQuery {
                limit: Some(2),
                cursor_updated_at: cursor["updatedAt"].as_i64(),
                cursor_id: cursor["id"].as_str().map(str::to_string),
                ..Default::default()
            },
            &repo,
        )
        .await,
    )
    .await;
    let second_ids: Vec<&str> = second["sessions"]
        .as_array()
        .expect("sessions array")
        .iter()
        .map(|session| session["id"].as_str().expect("id"))
        .collect();

    let mut all: Vec<&str> = first_ids.iter().chain(second_ids.iter()).copied().collect();
    all.sort_unstable();
    assert_eq!(all, vec!["list-a", "list-b", "list-c"]);
    assert_eq!(second["nextCursor"], serde_json::Value::Null);
}

#[tokio::test]
async fn list_sessions_rejects_a_bad_limit_and_half_a_cursor() {
    let repo = sqlite_session_repo().await;

    let (status, _) = reply_to_json(
        list_sessions_reply(
            ListSessionsQuery {
                limit: Some(0),
                ..Default::default()
            },
            &repo,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, json) = reply_to_json(
        list_sessions_reply(
            ListSessionsQuery {
                cursor_id: Some("list-a".to_string()),
                ..Default::default()
            },
            &repo,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"].as_str().unwrap().contains("together"));
}

#[tokio::test]
async fn get_session_returns_the_failure_reason_until_the_next_status_change() {
    let repo = sqlite_session_repo().await;
    let id = "status-reason-http-session";
    repo.upsert_session(&SessionMetadata::test_fixture(id))
        .await
        .expect("session should be stored");

    let (status, json) = get_session_json(id, &repo).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["statusReason"], serde_json::Value::Null);

    let reason = "Docker workspace setup failed: image 'missing:latest' was not found";
    repo.update_status(id, SessionStatus::Error, Some(reason))
        .await
        .expect("failure should be stored");
    let (status, json) = get_session_json(id, &repo).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "error");
    assert_eq!(json["statusReason"], reason);

    repo.update_status(id, SessionStatus::Idle, None)
        .await
        .expect("status change should be stored");
    let (_, json) = get_session_json(id, &repo).await;
    assert_eq!(json["status"], "idle");
    assert_eq!(json["statusReason"], serde_json::Value::Null);
}

#[tokio::test]
async fn a_full_upsert_with_a_new_status_clears_the_failure_reason() {
    let repo = sqlite_session_repo().await;
    let id = "status-reason-upsert-session";
    let mut session = SessionMetadata::test_fixture(id);
    repo.upsert_session(&session)
        .await
        .expect("session should be stored");
    repo.update_status(id, SessionStatus::Error, Some("setup failed"))
        .await
        .expect("failure should be stored");

    session.status = SessionStatus::Idle;
    session.status_reason = None;
    repo.upsert_session(&session)
        .await
        .expect("upsert should be stored");

    let (_, json) = get_session_json(id, &repo).await;
    assert_eq!(json["status"], "idle");
    assert_eq!(json["statusReason"], serde_json::Value::Null);

    session.status = SessionStatus::Error;
    session.status_reason = Some("restart failed".to_string());
    repo.upsert_session(&session)
        .await
        .expect("upsert should be stored");
    let (_, json) = get_session_json(id, &repo).await;
    assert_eq!(json["statusReason"], "restart failed");
}

#[tokio::test]
async fn get_session_shows_pending_approval_details_while_status_stays_busy() {
    use crate::agent::context::registry::ContextRegistry;
    use crate::agent::state::{PendingApprovalData, PendingApprovalKind};
    use crate::repositories::session_repository::SessionAttentionReason;
    use tokio::sync::oneshot;

    let repo = sqlite_session_repo().await;
    let id = "a1b2c3d4e5";
    let mut metadata = SessionMetadata::test_fixture(id);
    metadata.status = SessionStatus::Busy;
    metadata.last_attention_reason = Some(SessionAttentionReason::PendingApproval);
    repo.upsert_session(&metadata)
        .await
        .expect("session should be stored");

    let agent_session = AgentSession::new(metadata, Arc::new(ContextRegistry::new()), None);
    let (tx, _rx) = oneshot::channel();
    agent_session.pending_approvals.write().await.insert(
        "call-hard-1".to_string(),
        PendingApprovalData {
            sender: tx,
            tool_name: "workspace__runShell".to_string(),
            arguments: r#"{"command":"echo $(whoami)"}"#.to_string(),
            approval_kind: PendingApprovalKind::Hard,
            request_id: None,
            description: Some("Command substitution requires approval".to_string()),
            input_preview: Some("echo $(whoami)".to_string()),
        },
    );
    let active_sessions = RwLock::new(HashMap::from([(id.to_string(), agent_session)]));

    let (status, json) =
        reply_to_json(get_session_reply(id, Some(&active_sessions), &repo).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "busy");
    assert_eq!(json["lastAttentionReason"], "pendingApproval");
    let pending = json["pendingApprovals"]
        .as_array()
        .expect("pendingApprovals array");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["toolCallId"], "call-hard-1");
    assert_eq!(pending[0]["toolName"], "workspace__runShell");
    assert_eq!(
        pending[0]["reason"],
        "Command substitution requires approval"
    );
    assert_eq!(pending[0]["commandPreview"], "echo $(whoami)");
    assert_eq!(pending[0]["approvalKind"], "hard");
}

#[tokio::test]
async fn patch_session_raises_to_unsafe_releasing_hard_approval_and_allows_lowering_mode() {
    use crate::agent::context::registry::ContextRegistry;
    use crate::agent::state::{PendingApprovalData, PendingApprovalKind};
    use tokio::sync::oneshot;

    let repo = sqlite_session_repo().await;
    let id = "f6e5d4c3b2";
    let mut metadata = SessionMetadata::test_fixture(id);
    metadata.status = SessionStatus::Busy;
    metadata.execution_mode = ExecutionMode::Normal;
    repo.upsert_session(&metadata)
        .await
        .expect("session should be stored");

    let agent_session = AgentSession::new(metadata, Arc::new(ContextRegistry::new()), None);
    let (tx, mut rx) = oneshot::channel();
    agent_session.pending_approvals.write().await.insert(
        "call-hard-1".to_string(),
        PendingApprovalData {
            sender: tx,
            tool_name: "workspace__runShell".to_string(),
            arguments: r#"{"command":"echo $(whoami)"}"#.to_string(),
            approval_kind: PendingApprovalKind::Hard,
            request_id: None,
            description: Some("Command substitution requires approval".to_string()),
            input_preview: Some("echo $(whoami)".to_string()),
        },
    );
    let active_sessions = RwLock::new(HashMap::from([(id.to_string(), agent_session)]));

    // Raising to yolo keeps hard approval pending.
    let (status, yolo_json) = reply_to_json(
        patch_session_reply(
            id,
            PatchSessionRequest {
                execution_mode: Some("yolo".to_string()),
                name: None,
            },
            &active_sessions,
            &repo,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(yolo_json["executionMode"], "yolo");
    assert_eq!(yolo_json["resolvedApprovalIds"], serde_json::json!([]));
    assert!(rx.try_recv().is_err());

    // Raising to unsafe releases the pending hard approval.
    let (status, unsafe_json) = reply_to_json(
        patch_session_reply(
            id,
            PatchSessionRequest {
                execution_mode: Some("unsafe".to_string()),
                name: None,
            },
            &active_sessions,
            &repo,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(unsafe_json["executionMode"], "unsafe");
    assert_eq!(
        unsafe_json["resolvedApprovalIds"],
        serde_json::json!(["call-hard-1"])
    );
    assert_eq!(rx.await, Ok(true));
    assert_eq!(
        repo.get_session(id).await.unwrap().unwrap().execution_mode,
        ExecutionMode::Unsafe
    );

    // Lowering mode to normal is allowed and persisted.
    let (status, normal_json) = reply_to_json(
        patch_session_reply(
            id,
            PatchSessionRequest {
                execution_mode: Some("normal".to_string()),
                name: None,
            },
            &active_sessions,
            &repo,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(normal_json["executionMode"], "normal");
    assert_eq!(
        repo.get_session(id).await.unwrap().unwrap().execution_mode,
        ExecutionMode::Normal
    );
}

#[tokio::test]
async fn patch_session_renames_without_changing_execution_mode() {
    let repo = sqlite_session_repo().await;
    let id = "rename01abc";
    let mut metadata = SessionMetadata::test_fixture(id);
    metadata.name = Some("Old Name".to_string());
    metadata.execution_mode = ExecutionMode::Normal;
    repo.upsert_session(&metadata).await.expect("store");
    let active_sessions = RwLock::new(HashMap::new());

    let (status, json) = reply_to_json(
        patch_session_reply(
            id,
            PatchSessionRequest {
                execution_mode: None,
                name: Some("New Name".to_string()),
            },
            &active_sessions,
            &repo,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["name"], "New Name");
    assert!(json.get("executionMode").is_none() || json["executionMode"].is_null());
    assert_eq!(
        repo.get_session(id).await.unwrap().unwrap().name.as_deref(),
        Some("New Name")
    );
}

#[tokio::test]
async fn patch_session_rejects_empty_name_and_empty_body() {
    let repo = sqlite_session_repo().await;
    let id = "rename02def";
    let mut metadata = SessionMetadata::test_fixture(id);
    metadata.name = Some("Keep".to_string());
    repo.upsert_session(&metadata).await.expect("store");
    let active_sessions = RwLock::new(HashMap::new());

    let (status, _) = reply_to_json(
        patch_session_reply(
            id,
            PatchSessionRequest {
                execution_mode: None,
                name: Some("   ".to_string()),
            },
            &active_sessions,
            &repo,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        repo.get_session(id).await.unwrap().unwrap().name.as_deref(),
        Some("Keep")
    );

    let (status, _) = reply_to_json(
        patch_session_reply(
            id,
            PatchSessionRequest {
                execution_mode: None,
                name: None,
            },
            &active_sessions,
            &repo,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn patch_session_invalid_mode_does_not_apply_a_pending_rename() {
    let repo = sqlite_session_repo().await;
    let id = "rename03ghi";
    let mut metadata = SessionMetadata::test_fixture(id);
    metadata.name = Some("Original".to_string());
    metadata.execution_mode = ExecutionMode::Normal;
    repo.upsert_session(&metadata).await.expect("store");
    let active_sessions = RwLock::new(HashMap::new());

    let (status, _) = reply_to_json(
        patch_session_reply(
            id,
            PatchSessionRequest {
                execution_mode: Some("not-a-mode".to_string()),
                name: Some("Should Not Stick".to_string()),
            },
            &active_sessions,
            &repo,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let stored = repo.get_session(id).await.unwrap().unwrap();
    assert_eq!(stored.name.as_deref(), Some("Original"));
    assert_eq!(stored.execution_mode, ExecutionMode::Normal);
}

// ---- Session ownership over the Session API (#2856) ----
//
// HTTP callers never claim and are never refused for ownership: the
// Session API gate only resolves the session reference. Claim/refusal
// and liveness behavior are covered at the store level
// (`session_ownership` tests); here we pin the route-level contract.

use crate::server::routes::{frontend_attachment_header, FRONTEND_ATTACHMENT_HEADER};
use warp::Filter;

/// Repo and ownership store share one database, as in production where
/// sessions and ownership rows live in the same SQLite file.
async fn ownership_repo() -> (Arc<dyn SessionRepository>, sea_orm::DatabaseConnection) {
    let db = session_ownership::shared_test_store().await;
    (Arc::new(SqliteSessionRepository::new(db.clone())), db)
}

fn unique_session_id() -> String {
    format!("own-{}", uuid::Uuid::new_v4())
}

/// The ownership-relevant Session API routes, built from the real
/// `open_gate` and the real reply functions. Resume and send run exactly
/// the gate their handlers run first; the session work behind the gate
/// needs a full `AgentSessionManager`, so the stub answers 200 where the
/// handler would proceed.
fn ownership_routes(
    repo: Arc<dyn SessionRepository>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let with_repo = warp::any().map(move || repo.clone());

    let get = warp::get()
        .and(warp::path!("api" / "sessions" / String))
        .and(with_repo.clone())
        .and_then(|id: String, repo: Arc<dyn SessionRepository>| async move {
            Ok::<_, Rejection>(get_session_reply(&id, None, &repo).await)
        });
    let list = warp::get()
        .and(warp::path!("api" / "sessions"))
        .and(with_repo.clone())
        .and_then(|repo: Arc<dyn SessionRepository>| async move {
            Ok::<_, Rejection>(list_sessions_reply(ListSessionsQuery::default(), &repo).await)
        });
    let open = warp::post()
        .and(warp::path!("api" / "sessions" / String / "open"))
        .and(with_repo.clone())
        .and(frontend_attachment_header())
        .and_then(
            |id: String, repo: Arc<dyn SessionRepository>, attachment: Option<String>| async move {
                Ok::<_, Rejection>(open_session_reply(&id, attachment.as_deref(), &repo).await)
            },
        );
    let gated = |suffix: &'static str, ok: &'static str| {
        warp::post()
            .and(warp::path("api"))
            .and(warp::path("sessions"))
            .and(warp::path::param::<String>())
            .and(warp::path(suffix))
            .and(warp::path::end())
            .and(with_repo.clone())
            .and_then(
                move |id: String, repo: Arc<dyn SessionRepository>| async move {
                    Ok::<_, Rejection>(match open_gate(&id, None, repo.as_ref()).await {
                        Ok(_) => warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "status": ok })),
                            StatusCode::OK,
                        ),
                        Err(refusal) => refusal,
                    })
                },
            )
    };
    let resume = gated("resume", "resumed");
    let send = gated("messages", "sent");

    get.or(list).or(open).or(resume).or(send)
}

async fn call(
    api: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
    method: &str,
    path: &str,
) -> (StatusCode, serde_json::Value) {
    let response = warp::test::request()
        .method(method)
        .path(path)
        .reply(api)
        .await;
    let json = serde_json::from_slice(response.body()).expect("reply body should be JSON");
    (response.status(), json)
}

async fn stored_session(repo: &Arc<dyn SessionRepository>) -> String {
    let id = unique_session_id();
    repo.upsert_session(&SessionMetadata::test_fixture(&id))
        .await
        .expect("session should be stored");
    id
}

#[tokio::test]
async fn open_paths_never_refuse_and_never_claim_over_http() {
    let (repo, db) = ownership_repo().await;
    let api = ownership_routes(repo.clone());
    let id = stored_session(&repo).await;
    let foreign = session_ownership::live_foreign_identity();
    session_ownership::claim_by(&db, &foreign, &id)
        .await
        .expect("foreign claim");

    for path in [
        format!("/api/sessions/{id}/open"),
        format!("/api/sessions/{id}/resume"),
        format!("/api/sessions/{id}/messages"),
    ] {
        // Ownership is a GUI-process concern; the Session API neither
        // refuses nor claims, even against a live foreign owner (#2856).
        let (status, json) = call(&api, "POST", &path).await;
        assert_eq!(status, StatusCode::OK, "{path}: {json}");
    }
    assert_eq!(
        session_ownership::owner_of(&db, &id).await.as_deref(),
        Some(foreign.run_id.as_str()),
        "the HTTP caller claimed nothing"
    );
}

#[tokio::test]
async fn a_metadata_read_never_claims_and_is_never_refused() {
    let (repo, db) = ownership_repo().await;
    let api = ownership_routes(repo.clone());
    let id = stored_session(&repo).await;

    // Reading an unowned session claims nothing.
    let (status, json) = call(&api, "GET", &format!("/api/sessions/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(session_ownership::owner(&id).await, None);

    // A session owned by another process is still readable.
    let foreign = session_ownership::live_foreign_identity();
    session_ownership::claim_by(&db, &foreign, &id)
        .await
        .expect("foreign claim");
    let (status, json) = call(&api, "GET", &format!("/api/sessions/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["id"], id.as_str());
    assert_eq!(
        session_ownership::owner_of(&db, &id).await.as_deref(),
        Some(foreign.run_id.as_str()),
        "the read did not touch ownership"
    );
}

#[tokio::test]
async fn the_list_flags_every_live_owner_including_this_run() {
    let (repo, db) = ownership_repo().await;
    let api = ownership_routes(repo.clone());
    let foreign_owned = stored_session(&repo).await;
    let own_owned = stored_session(&repo).await;
    let free = stored_session(&repo).await;

    let foreign = session_ownership::live_foreign_identity();
    session_ownership::claim_by(&db, &foreign, &foreign_owned)
        .await
        .expect("foreign claim");
    session_ownership::claim(&own_owned)
        .await
        .expect("own-run claim");

    let (_, json) = call(&api, "GET", "/api/sessions").await;
    let find = |id: &str| {
        json["sessions"]
            .as_array()
            .expect("sessions array")
            .iter()
            .find(|session| session["id"] == id)
            .unwrap_or_else(|| panic!("{id} missing from {json}"))["openElsewhere"]
            .as_bool()
            .expect("openElsewhere flag")
    };
    // A headerless HTTP caller is outside the ownership model, so every
    // held session is flagged — this backend's own run included
    // (docs/api/http_api.md, #2856).
    assert!(find(&foreign_owned), "foreign live owner flagged");
    assert!(find(&own_owned), "own-run owner flagged for externals");
    assert!(!find(&free), "free session not flagged");

    session_ownership::release_session(&own_owned).await;
}

#[tokio::test]
async fn a_stale_foreign_owner_is_not_flagged_in_the_list() {
    let (repo, db) = ownership_repo().await;
    let api = ownership_routes(repo.clone());
    let id = stored_session(&repo).await;
    let foreign = session_ownership::live_foreign_identity();
    session_ownership::claim_by(&db, &foreign, &id)
        .await
        .expect("foreign claim");

    // The owner's heartbeat goes past the stale cutoff, as a hung
    // process's would after its ownership timeout elapsed: the lock is
    // up for grabs, so the list no longer flags it.
    session_ownership::backdate_row(&db, &id).await;

    let (_, json) = call(&api, "GET", "/api/sessions").await;
    let listed = json["sessions"]
        .as_array()
        .expect("sessions array")
        .iter()
        .find(|session| session["id"] == id.as_str())
        .expect("session listed");
    assert_eq!(listed["openElsewhere"], false);
}

#[tokio::test]
async fn an_attachment_header_is_refused_on_the_open_path() {
    let (repo, _db) = ownership_repo().await;
    let api = ownership_routes(repo.clone());
    let id = stored_session(&repo).await;

    // #2866 removed the attachment registry: any header value is a 400
    // on every route that reads it, and nothing is ever claimed.
    for header in ["host", "fabricated-id"] {
        let response = warp::test::request()
            .method("POST")
            .path(&format!("/api/sessions/{id}/open"))
            .header(FRONTEND_ATTACHMENT_HEADER, header)
            .reply(&api)
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{header}");
    }
    assert_eq!(session_ownership::owner(&id).await, None, "nothing claimed");
}

#[tokio::test]
async fn opening_a_missing_session_is_404_and_claims_nothing() {
    let (repo, _db) = ownership_repo().await;
    let api = ownership_routes(repo);
    let missing = unique_session_id();

    let (status, _) = call(&api, "POST", &format!("/api/sessions/{missing}/open")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(session_ownership::owner(&missing).await, None);
}
