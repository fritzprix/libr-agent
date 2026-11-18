//! Cross-process session ownership in the profile database (#2856).
//!
//! Every GUI process hosts its own backend (#2853, #2855), so the old
//! in-memory ownership map (#2759) was invisible across GUIs: two GUIs on
//! the same profile could open the same session at once. Ownership now
//! lives in the shared profile database (`session_ownership`, migration
//! `m20261116_000050`) and is **GUI-to-GUI only**: the claiming actor is
//! this process's backend run, and claims come from the GUI IPC paths.
//!
//! HTTP callers are outside the ownership model: a Session API caller
//! neither claims a session nor is refused for ownership (any
//! `x-frontend-attachment` header is rejected outright, #2854/#2866). The
//! list routes surface ownership instead — see [`open_elsewhere_ids`] and
//! [`live_owned_ids`].
//!
//! Headless `liblet` (#2703) never starts the backend HTTP server and
//! therefore never initializes this module, so every claim there is a
//! no-op: headless code paths skip ownership entirely.
//!
//! Liveness: an owned row is live while its heartbeat is fresh
//! ([`STALE_AFTER`]); each backend process refreshes the heartbeats of the
//! sessions it owns every [`HEARTBEAT_INTERVAL`]. A row whose pid is
//! definitively gone — or whose pid now belongs to a process with a
//! different OS start marker (pid reuse) — is dead at once, so a crashed
//! GUI releases its sessions without waiting for the heartbeat to lapse.
//! An owner whose heartbeat lapses loses the lock even while its process
//! exists: that is the hung-owner path. A row owned by this very run is
//! live without any probe (#2856 G1.2).
//!
//! When a lock is taken from under its owner (a takeover while the owner
//! was hung), the previous owner notices on its next heartbeat, forgets
//! the session, and stops its local run.

use crate::agent::AgentSessionManager;
use once_cell::sync::Lazy;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, Value};
use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Exact refusal wording the GUI shows when entry is refused (#2759).
pub const OPEN_ELSEWHERE_MESSAGE: &str = "is open in another process";

/// A ready-made HTTP refusal: status code and body message.
pub type HttpRefusal = (warp::http::StatusCode, String);

const ATTACHMENT_HEADER_REMOVED: &str =
    "x-frontend-attachment is not supported; attachment registry removed";

/// Refuse any HTTP caller that sends an `x-frontend-attachment` header (#2854).
///
/// - No header: external caller; the request continues and claims nothing.
/// - Any header value: `400` — cross-process attachment registration is gone;
///   only the in-process host may own sessions.
pub fn reject_attachment_header(header: Option<&str>) -> Result<(), HttpRefusal> {
    match header {
        None => Ok(()),
        Some(_) => Err((
            warp::http::StatusCode::BAD_REQUEST,
            ATTACHMENT_HEADER_REMOVED.to_string(),
        )),
    }
}

/// How often the backend refreshes the heartbeats of the sessions it owns.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// A claimed row whose heartbeat is older than this is stale: the next
/// claimer takes it over.
pub const STALE_AFTER: Duration = Duration::from_secs(15);

/// Identity of one backend process run: the actor that owns sessions in
/// the database. `started_marker` is the OS process start marker (see
/// [`crate::utils::process::process_start_marker`] — platform units, only
/// equality is meaningful), so a row whose pid the OS later reuses is
/// detected instead of trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunIdentity {
    pub run_id: String,
    pub pid: u32,
    pub started_marker: i64,
}

struct Store {
    db: DatabaseConnection,
    identity: RunIdentity,
}

static STORE: OnceLock<Store> = OnceLock::new();

/// Sessions this process believes it owns. The heartbeat compares this set
/// with the database to notice locks that were taken over.
static CLAIMED: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn current_identity() -> RunIdentity {
    let pid = std::process::id();
    RunIdentity {
        run_id: uuid::Uuid::new_v4().to_string(),
        pid,
        started_marker: crate::utils::process::process_start_marker(pid).unwrap_or(0),
    }
}

/// Milliseconds timestamp before which an ownership heartbeat counts as
/// stale, derived from [`STALE_AFTER`]. Single source of truth for the rule
/// so other readers (e.g. the headless write-back) cannot drift.
pub fn stale_before_ms() -> i64 {
    now_ms() - STALE_AFTER.as_millis() as i64
}

const SELECT_OWNER: &str = "SELECT owner_run, owner_pid, owner_started_marker, heartbeat_at FROM session_ownership WHERE session_id = ?";
const INSERT_OWNER: &str = "INSERT INTO session_ownership (session_id, owner_run, owner_pid, owner_started_marker, heartbeat_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(session_id) DO NOTHING";
const TAKEOVER_OWNER: &str = "UPDATE session_ownership SET owner_run = ?, owner_pid = ?, owner_started_marker = ?, heartbeat_at = ? WHERE session_id = ? AND owner_run = ?";
const REFRESH_OWNER: &str =
    "UPDATE session_ownership SET heartbeat_at = ? WHERE session_id = ? AND owner_run = ?";
const HEARTBEAT_RUN: &str = "UPDATE session_ownership SET heartbeat_at = ? WHERE owner_run = ?";
const SELECT_OWNED: &str = "SELECT session_id FROM session_ownership WHERE owner_run = ?";
const DELETE_OWNER: &str = "DELETE FROM session_ownership WHERE session_id = ? AND owner_run = ?";
const SELECT_OWNED_MANY_PREFIX: &str = "SELECT session_id, owner_run, owner_pid, owner_started_marker, heartbeat_at FROM session_ownership WHERE session_id IN (";

/// Stored ownership of one session.
#[derive(Debug)]
struct OwnerRow {
    owner_run: String,
    owner_pid: i64,
    owner_started_marker: i64,
    heartbeat_at: i64,
}

async fn read_owner(db: &DatabaseConnection, session_id: &str) -> Result<Option<OwnerRow>, String> {
    let backend = db.get_database_backend();
    let stmt = Statement::from_sql_and_values(backend, SELECT_OWNER, [Value::from(session_id)]);
    let row = db
        .query_one(stmt)
        .await
        .map_err(|e| format!("session ownership read failed: {e}"))?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some(OwnerRow {
        owner_run: row
            .try_get::<String>("", "owner_run")
            .map_err(|e| format!("session ownership row unreadable: {e}"))?,
        owner_pid: row
            .try_get::<i64>("", "owner_pid")
            .map_err(|e| format!("session ownership row unreadable: {e}"))?,
        owner_started_marker: row
            .try_get::<i64>("", "owner_started_marker")
            .map_err(|e| format!("session ownership row unreadable: {e}"))?,
        heartbeat_at: row
            .try_get::<i64>("", "heartbeat_at")
            .map_err(|e| format!("session ownership row unreadable: {e}"))?,
    }))
}

/// True when `pid` is definitively gone: it no longer exists, or it now
/// belongs to a process with a different OS start marker (pid reuse). An
/// unprovable pid is not proof of death (#2823 contract): only a positive
/// `Ok(false)` from the liveness probe kills the row, so a broken probe can
/// never destroy a live backend's lock (#2858 G2.2 rule).
fn owner_is_dead_with_marker(
    pid: u32,
    recorded_marker: i64,
    is_alive: fn(u32) -> io::Result<bool>,
) -> bool {
    match is_alive(pid) {
        Ok(false) => true,
        Ok(true) => match (
            recorded_marker,
            crate::utils::process::process_start_marker(pid),
        ) {
            (recorded, Some(current)) if recorded != 0 => recorded != current,
            _ => false,
        },
        Err(_) => false,
    }
}

fn owner_is_dead_with(row: &OwnerRow, is_alive: fn(u32) -> io::Result<bool>) -> bool {
    let Ok(pid) = u32::try_from(row.owner_pid) else {
        return true; // a pid that cannot name a process is a broken row
    };
    owner_is_dead_with_marker(pid, row.owner_started_marker, is_alive)
}

fn row_is_live(row: &OwnerRow) -> bool {
    row.heartbeat_at > stale_before_ms()
        && !owner_is_dead_with(row, crate::utils::process::process_is_alive)
}

fn note_claim(who: &RunIdentity, session_id: &str) {
    if STORE
        .get()
        .is_some_and(|store| store.identity.run_id == who.run_id)
    {
        CLAIMED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(session_id.to_string());
    }
}

/// Claim `session_id` for `who` with a conditional atomic update:
///
/// - unowned, or already owned by `who`: the row is inserted or refreshed;
/// - owned by a stale owner (heartbeat lapsed, pid gone, pid reused): the
///   takeover `UPDATE ... WHERE owner_run = <observed>` either lands on
///   the row we saw or loses to a concurrent claimer and retries;
/// - owned by a different live process: `Err` with
///   [`OPEN_ELSEWHERE_MESSAGE`].
pub async fn claim_by(
    db: &DatabaseConnection,
    who: &RunIdentity,
    session_id: &str,
) -> Result<(), String> {
    // Bounded so sustained contention ends with a refusal rather than a spin.
    for _ in 0..3 {
        let row = read_owner(db, session_id).await?;
        match row {
            None => {
                let stmt = Statement::from_sql_and_values(
                    db.get_database_backend(),
                    INSERT_OWNER,
                    [
                        Value::from(session_id),
                        Value::from(who.run_id.as_str()),
                        Value::from(who.pid as i64),
                        Value::from(who.started_marker),
                        Value::from(now_ms()),
                    ],
                );
                let result = db
                    .execute(stmt)
                    .await
                    .map_err(|e| format!("session ownership claim failed: {e}"))?;
                if result.rows_affected() == 1 {
                    note_claim(who, session_id);
                    return Ok(());
                }
                continue; // lost the insert race; re-read and re-decide
            }
            Some(row) if row.owner_run == who.run_id => {
                let stmt = Statement::from_sql_and_values(
                    db.get_database_backend(),
                    REFRESH_OWNER,
                    [
                        Value::from(now_ms()),
                        Value::from(session_id),
                        Value::from(who.run_id.as_str()),
                    ],
                );
                db.execute(stmt)
                    .await
                    .map_err(|e| format!("session ownership refresh failed: {e}"))?;
                note_claim(who, session_id);
                return Ok(());
            }
            Some(row)
                if row.heartbeat_at <= stale_before_ms()
                    || owner_is_dead_with(&row, crate::utils::process::process_is_alive) =>
            {
                let stmt = Statement::from_sql_and_values(
                    db.get_database_backend(),
                    TAKEOVER_OWNER,
                    [
                        Value::from(who.run_id.as_str()),
                        Value::from(who.pid as i64),
                        Value::from(who.started_marker),
                        Value::from(now_ms()),
                        Value::from(session_id),
                        Value::from(row.owner_run.as_str()),
                    ],
                );
                let result = db
                    .execute(stmt)
                    .await
                    .map_err(|e| format!("session ownership takeover failed: {e}"))?;
                if result.rows_affected() == 1 {
                    note_claim(who, session_id);
                    return Ok(());
                }
                continue; // the row changed under us; re-read and re-decide
            }
            Some(_) => {
                return Err(format!("Session {} {}", session_id, OPEN_ELSEWHERE_MESSAGE));
            }
        }
    }
    Err(format!("Session {} {}", session_id, OPEN_ELSEWHERE_MESSAGE))
}

/// Live owner run id of `session_id` in `db`, if any.
pub async fn owner_of(db: &DatabaseConnection, session_id: &str) -> Option<String> {
    read_owner(db, session_id)
        .await
        .ok()
        .flatten()
        .filter(row_is_live)
        .map(|row| row.owner_run)
}

/// Free `session_id` for `who` if it still owns the row.
pub async fn release_by(db: &DatabaseConnection, who: &RunIdentity, session_id: &str) {
    let stmt = Statement::from_sql_and_values(
        db.get_database_backend(),
        DELETE_OWNER,
        [Value::from(session_id), Value::from(who.run_id.as_str())],
    );
    if let Err(error) = db.execute(stmt).await {
        log::warn!("Session ownership release failed: {error}");
    }
    CLAIMED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(session_id);
}

/// Start the per-process ownership guard with `db` and a fresh run
/// identity. Called once when the backend HTTP server starts; a process
/// that never starts a server (headless `liblet`) never initializes
/// ownership and claims nothing.
pub fn init(db: DatabaseConnection) {
    init_with(db, current_identity());
}

fn init_with(db: DatabaseConnection, identity: RunIdentity) {
    let _ = STORE.set(Store { db, identity });
}

fn store() -> Option<&'static Store> {
    STORE.get()
}

/// Claim `session_id` for this process. Ok when this module is
/// uninitialized (headless, or before the backend server starts):
/// ownership does not apply there (#2856).
pub async fn claim(session_id: &str) -> Result<(), String> {
    let Some(store) = store() else {
        return Ok(());
    };
    claim_by(&store.db, &store.identity, session_id).await
}

/// Live owner run id of `session_id`, if any.
pub async fn owner(session_id: &str) -> Option<String> {
    let store = store()?;
    owner_of(&store.db, session_id).await
}

/// The sessions of `session_ids` that a live process **other than this
/// run** owns — the GUI list view (#2856). One query for the whole page;
/// rows owned by this run are live without a probe, and the liveness
/// probe for foreign rows runs on the blocking pool so a page of sessions
/// never runs its liveness probes on the async runtime (#2856 G1.2).
pub async fn open_elsewhere_ids(session_ids: &[String]) -> HashSet<String> {
    let Some(store) = store() else {
        return HashSet::new();
    };
    let my_run = store.identity.run_id.clone();
    live_owners(
        &store.db,
        &my_run,
        session_ids,
        crate::utils::process::process_is_alive,
    )
    .await
    .into_iter()
    .filter(|(_, owner_run)| owner_run != &my_run)
    .map(|(session_id, _)| session_id)
    .collect()
}

/// The sessions of `session_ids` with **any** live owner — the external
/// Session API caller's view: every session a process holds, including
/// this backend's own (`docs/api/http_api.md`, #2856). A headerless HTTP
/// caller is outside the ownership model, so "open elsewhere" means "held
/// by some live process".
pub async fn live_owned_ids(session_ids: &[String]) -> HashSet<String> {
    let Some(store) = store() else {
        return HashSet::new();
    };
    let my_run = store.identity.run_id.clone();
    live_owners(
        &store.db,
        &my_run,
        session_ids,
        crate::utils::process::process_is_alive,
    )
    .await
    .into_keys()
    .collect()
}

/// One `SELECT ... WHERE session_id IN (...)` for the whole page: returns
/// every session whose row is live. Stale rows are never live (the lock is
/// up for grabs regardless of liveness); rows owned by `my_run` are live
/// without a probe; foreign fresh rows are probed on the blocking pool,
/// and a probe that cannot prove death keeps the row live (#2823).
///
/// Rows whose owner is *positively proved* gone are deleted here (guarded
/// on the observed `owner_run`), so the same dead pid is not probed again
/// on every later render or start (#2882).
async fn live_owners(
    db: &DatabaseConnection,
    my_run: &str,
    session_ids: &[String],
    is_alive: fn(u32) -> io::Result<bool>,
) -> HashMap<String, String> {
    if session_ids.is_empty() {
        return HashMap::new();
    }
    let placeholders = vec!["?"; session_ids.len()].join(", ");
    let sql = format!("{SELECT_OWNED_MANY_PREFIX}{placeholders})");
    let values: Vec<Value> = session_ids
        .iter()
        .map(|session_id| Value::from(session_id.as_str()))
        .collect();
    let stmt = Statement::from_sql_and_values(db.get_database_backend(), sql, values);
    let rows = match db.query_all(stmt).await {
        Ok(rows) => rows,
        Err(e) => {
            log::warn!("session ownership list read failed: {e}");
            return HashMap::new();
        }
    };

    let cutoff = stale_before_ms();
    let mut live: HashMap<String, String> = HashMap::new();
    let mut probes: Vec<(String, String, u32, i64)> = Vec::new();
    for row in &rows {
        let parsed = (
            row.try_get::<String>("", "session_id"),
            row.try_get::<String>("", "owner_run"),
            row.try_get::<i64>("", "owner_pid"),
            row.try_get::<i64>("", "owner_started_marker"),
            row.try_get::<i64>("", "heartbeat_at"),
        );
        let (Ok(session_id), Ok(owner_run), Ok(owner_pid), Ok(started_marker), Ok(heartbeat_at)) =
            parsed
        else {
            log::warn!("session ownership list row failed to parse; treated as not live");
            continue;
        };
        if heartbeat_at <= cutoff {
            continue; // heartbeat lapsed: the lock is up for grabs regardless of liveness
        }
        if owner_run == my_run {
            live.insert(session_id, owner_run); // our own rows need no probe (#2856 G1.2)
            continue;
        }
        // Foreign rows start live; the probe below removes them only on
        // proven death (#2823). A pid that cannot name a process is a
        // broken row: not live, so it is simply never inserted.
        if let Ok(pid) = u32::try_from(owner_pid) {
            live.insert(session_id.clone(), owner_run.clone());
            probes.push((session_id, owner_run, pid, started_marker));
        }
    }

    if !probes.is_empty() {
        // The probe is a synchronous `kill(2)` syscall (Linux/macOS) or
        // process handle open (Windows): work that must stay off the async
        // runtime (#2856 G1.2). A join failure proves no death, so the
        // rows stay live (#2823).
        let dead: Vec<(String, String)> = tokio::task::spawn_blocking(move || {
            probes
                .into_iter()
                .filter(|(_, _, pid, marker)| owner_is_dead_with_marker(*pid, *marker, is_alive))
                .map(|(session_id, owner_run, _, _)| (session_id, owner_run))
                .collect()
        })
        .await
        .unwrap_or_default();
        for (session_id, owner_run) in dead {
            live.remove(&session_id);
            // #2882: a proven-dead row left in the database is probed again
            // on every later render and every start. Remove it once. The
            // delete is guarded on the `owner_run` observed above, so a
            // takeover that replaced the row since then is left untouched.
            let stmt = Statement::from_sql_and_values(
                db.get_database_backend(),
                DELETE_OWNER,
                [
                    Value::from(session_id.as_str()),
                    Value::from(owner_run.as_str()),
                ],
            );
            if let Err(error) = db.execute(stmt).await {
                log::warn!("session ownership dead-owner sweep failed: {error}");
            }
        }
    }
    live
}

/// Drop this process's ownership of one session. Used when opening a
/// brand-new session fails after its claim, so no row outlives a session
/// that was never created.
pub async fn release_session(session_id: &str) {
    let Some(store) = store() else { return };
    release_by(&store.db, &store.identity, session_id).await;
}

/// One heartbeat tick: refresh the heartbeats of every row this run owns,
/// then return the sessions from the local claimed set that the database
/// no longer credits to this run — those locks were taken over (or the
/// sessions were deleted), so the caller stops the local runs.
pub(crate) async fn heartbeat_once(
    db: &DatabaseConnection,
    run_id: &str,
) -> Result<Vec<String>, String> {
    let stmt = Statement::from_sql_and_values(
        db.get_database_backend(),
        HEARTBEAT_RUN,
        [Value::from(now_ms()), Value::from(run_id)],
    );
    db.execute(stmt)
        .await
        .map_err(|e| format!("session ownership heartbeat failed: {e}"))?;

    let stmt = Statement::from_sql_and_values(
        db.get_database_backend(),
        SELECT_OWNED,
        [Value::from(run_id)],
    );
    let rows = db
        .query_all(stmt)
        .await
        .map_err(|e| format!("session ownership heartbeat read failed: {e}"))?;
    let owned: HashSet<String> = rows
        .iter()
        .filter_map(|row| row.try_get::<String>("", "session_id").ok())
        .collect();

    let mut claimed = CLAIMED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let lost: Vec<String> = claimed.difference(&owned).cloned().collect();
    for session_id in &lost {
        claimed.remove(session_id);
    }
    Ok(lost)
}

/// Refresh owned heartbeats and stop sessions whose locks were taken over.
/// Spawns the loop for the initialized store; a no-op before [`init`].
pub fn spawn_heartbeat_loop(manager: Arc<AgentSessionManager>) {
    let Some(store) = store() else { return };
    let db = store.db.clone();
    let run_id = store.identity.run_id.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(HEARTBEAT_INTERVAL).await;
            match heartbeat_once(&db, &run_id).await {
                Ok(lost) => {
                    for session_id in lost {
                        log::warn!(
                            "Session ownership of {session_id} was taken over (#2856); stopping the local session"
                        );
                        if let Err(error) = manager.terminate_session(session_id.clone()).await {
                            log::debug!("Stopping taken-over session {session_id} failed: {error}");
                        }
                    }
                }
                Err(error) => log::warn!("Session ownership heartbeat failed: {error}"),
            }
        }
    });
}

/// One temp-file database, migrated once, shared by every ownership test
/// in this binary: the store is a process-wide `OnceLock`, so all tests
/// must agree on one database. Session ids are unique, so isolation holds.
#[cfg(test)]
pub(crate) async fn shared_test_store() -> DatabaseConnection {
    static DB: tokio::sync::OnceCell<DatabaseConnection> = tokio::sync::OnceCell::const_new();
    DB.get_or_init(|| async {
        crate::lifecycle::database::register_sqlite_vec();
        use std::str::FromStr;
        // A real temp file, not `:memory:`: every `#[tokio::test]` owns its
        // own runtime, and an in-memory database dies with the connections
        // opened on the runtime that created it — the next test would then
        // see an empty database. One file, named per process, survives all
        // of them. Foreign keys stay off so ownership rows may reference
        // synthetic session ids the store's contract does not require.
        let path = std::env::temp_dir().join(format!(
            "libragent_ownership_test_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let opts = sea_orm::sqlx::sqlite::SqliteConnectOptions::from_str(
            &crate::utils::sqlite::format_sqlite_url(&path.to_string_lossy()),
        )
        .expect("sqlite test url should parse")
        .foreign_keys(false)
        .create_if_missing(true);
        let sqlx_pool = sea_orm::sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("sqlite should connect for ownership tests");
        let db: DatabaseConnection =
            sea_orm::SqlxSqliteConnector::from_sqlx_sqlite_pool(sqlx_pool).into();
        use migration::MigratorTrait;
        migration::Migrator::up(&db, None)
            .await
            .expect("migrations should run for ownership tests");
        // Deterministic, not via connect options: the `foreign_keys(false)`
        // builder is not honored by this sqlx build, and the store's tests
        // insert ownership rows for synthetic session ids by design.
        db.execute(sea_orm::Statement::from_string(
            db.get_database_backend(),
            "PRAGMA foreign_keys = OFF".to_string(),
        ))
        .await
        .expect("disabling foreign keys should land");
        init_with(db.clone(), current_identity());
        db
    })
    .await
    .clone()
}

/// A run identity that reads as a live *other* process: a distinct run id
/// on this machine's pid with the current OS start marker.
#[cfg(test)]
pub(crate) fn live_foreign_identity() -> RunIdentity {
    let pid = std::process::id();
    RunIdentity {
        run_id: format!("foreign-{}", uuid::Uuid::new_v4()),
        pid,
        started_marker: crate::utils::process::process_start_marker(pid).unwrap_or(0),
    }
}

/// Move a row's heartbeat past the stale cutoff, as a hung owner's would
/// be after its ownership timeout elapsed.
#[cfg(test)]
pub(crate) async fn backdate_row(db: &DatabaseConnection, session_id: &str) {
    let stmt = Statement::from_sql_and_values(
        db.get_database_backend(),
        "UPDATE session_ownership SET heartbeat_at = ? WHERE session_id = ?",
        [
            Value::from(now_ms() - STALE_AFTER.as_millis() as i64 - 1_000),
            Value::from(session_id),
        ],
    );
    db.execute(stmt).await.expect("backdate should land");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_attachment_header_is_refused_and_none_passes() {
        assert!(reject_attachment_header(None).is_ok());
        for header in ["host", "fabricated-id"] {
            let (status, message) =
                reject_attachment_header(Some(header)).expect_err("any header is refused");
            assert_eq!(status, warp::http::StatusCode::BAD_REQUEST, "{header}");
            assert!(message.contains("not supported"), "{message}");
        }
    }

    fn unique(tag: &str) -> String {
        format!("{tag}-{}", uuid::Uuid::new_v4())
    }

    #[tokio::test]
    async fn a_second_run_is_refused_while_the_owner_is_live() {
        let db = shared_test_store().await;
        let session = unique("session");
        let first = live_foreign_identity();
        let second = live_foreign_identity();

        claim_by(&db, &first, &session).await.expect("first claim");
        let err = claim_by(&db, &second, &session)
            .await
            .expect_err("the second run must be refused");
        assert!(err.contains(&session));
        assert!(err.contains(OPEN_ELSEWHERE_MESSAGE));
        // The holder keeps entering freely.
        claim_by(&db, &first, &session).await.expect("re-enter");
        assert_eq!(
            owner_of(&db, &session).await.as_deref(),
            Some(first.run_id.as_str())
        );
    }

    #[tokio::test]
    async fn a_stale_heartbeat_loses_the_lock() {
        let db = shared_test_store().await;
        let session = unique("session");
        let holder = live_foreign_identity();
        let successor = live_foreign_identity();

        claim_by(&db, &holder, &session).await.expect("claim");
        backdate_row(&db, &session).await;
        claim_by(&db, &successor, &session)
            .await
            .expect("a stale owner loses the lock");
        assert_eq!(
            owner_of(&db, &session).await.as_deref(),
            Some(successor.run_id.as_str())
        );
    }

    #[tokio::test]
    async fn a_dead_owner_pid_loses_the_lock_immediately() {
        // A pid far above every real pid (and above `pid_t`'s range, which
        // the probe rejects without ever signalling): the probe reads the
        // row's owner as definitively gone, so the lock is taken over
        // without waiting for the heartbeat to lapse.
        let db = shared_test_store().await;
        let session = unique("session");
        let crashed = RunIdentity {
            run_id: unique("run"),
            pid: 1 << 31,
            started_marker: 0,
        };
        claim_by(&db, &crashed, &session)
            .await
            .expect("claim records the crashed owner");
        claim_by(&db, &live_foreign_identity(), &session)
            .await
            .expect("a definitively dead pid loses the lock at once");
    }

    #[tokio::test]
    async fn pid_reuse_is_refused_despite_a_fresh_heartbeat() {
        let own_start = crate::utils::process::process_start_marker(std::process::id());
        let Some(own_start) = own_start else {
            return; // platform without a start-marker probe
        };
        let db = shared_test_store().await;
        let session = unique("session");

        // A row that recorded this pid under an earlier lifetime: the OS
        // handed the pid to us, so the recorded owner is gone.
        let stmt = Statement::from_sql_and_values(
            db.get_database_backend(),
            INSERT_OWNER,
            [
                Value::from(session.as_str()),
                Value::from("reused-run"),
                Value::from(std::process::id() as i64),
                Value::from(own_start - 1_000_000),
                Value::from(now_ms()),
            ],
        );
        db.execute(stmt).await.expect("insert reused-owner row");

        claim_by(&db, &live_foreign_identity(), &session)
            .await
            .expect("pid reuse releases the stale row");
    }

    #[tokio::test]
    async fn release_by_frees_the_row_for_another_run() {
        let db = shared_test_store().await;
        let session = unique("session");
        let holder = live_foreign_identity();
        let successor = live_foreign_identity();

        claim_by(&db, &holder, &session).await.expect("claim");
        release_by(&db, &holder, &session).await;
        assert_eq!(owner_of(&db, &session).await, None);
        claim_by(&db, &successor, &session)
            .await
            .expect("claim after release");
    }

    #[tokio::test]
    async fn heartbeat_keeps_owned_locks_and_reports_takeovers() {
        let db = shared_test_store().await;
        let session = unique("session");

        // Claim through the store identity so the local claimed set is fed.
        claim(&session).await.expect("store claim");
        let run_id = store().expect("store").identity.run_id.clone();

        // Steal the row behind the owner's back, as a takeover would.
        let stmt = Statement::from_sql_and_values(
            db.get_database_backend(),
            "UPDATE session_ownership SET owner_run = ? WHERE session_id = ?",
            [Value::from("thief-run"), Value::from(session.as_str())],
        );
        db.execute(stmt).await.expect("steal row");

        let lost = heartbeat_once(&db, &run_id).await.expect("heartbeat");
        assert_eq!(lost, vec![session.clone()], "the takeover is reported");
        assert!(
            heartbeat_once(&db, &run_id).await.unwrap().is_empty(),
            "a reported loss is not reported twice"
        );
    }

    #[tokio::test]
    async fn the_list_views_separate_foreign_from_own_live_rows() {
        let db = shared_test_store().await;
        let ours = unique("session");
        let foreign = unique("session");
        let free = unique("session");

        claim(&ours).await.expect("store claim");
        claim_by(&db, &live_foreign_identity(), &foreign)
            .await
            .expect("foreign claim");

        let all = vec![ours.clone(), foreign.clone(), free.clone()];

        // GUI view: only foreign live owners are "open elsewhere".
        let open = open_elsewhere_ids(&all).await;
        assert_eq!(open, [foreign.clone()].into_iter().collect());

        // External caller view: any live owner, this backend's own included.
        let live = live_owned_ids(&all).await;
        assert_eq!(live, [ours, foreign].into_iter().collect());
    }

    #[tokio::test]
    async fn a_page_of_own_rows_never_calls_the_liveness_probe() {
        // #2856 G1.2: a list of sessions owned by this run must not probe
        // at all. The injected probe panics, so any probe call fails the
        // test through the panic, not a flaky assertion.
        let db = shared_test_store().await;
        let ours = unique("session");
        claim(&ours).await.expect("store claim");
        let run_id = store().expect("store").identity.run_id.clone();

        fn poisoned_probe(_pid: u32) -> io::Result<bool> {
            panic!("the liveness probe must not run for rows owned by this run");
        }

        let live = live_owners(&db, &run_id, &[ours.clone()], poisoned_probe).await;
        assert_eq!(
            live.get(&ours).map(String::as_str),
            Some(run_id.as_str()),
            "our own row is live without a probe"
        );
    }

    #[tokio::test]
    async fn a_stale_foreign_row_is_not_live_in_the_list_views() {
        let db = shared_test_store().await;
        let foreign = unique("session");
        claim_by(&db, &live_foreign_identity(), &foreign)
            .await
            .expect("foreign claim");
        backdate_row(&db, &foreign).await;

        assert!(open_elsewhere_ids(&[foreign.clone()]).await.is_empty());
        assert!(live_owned_ids(&[foreign]).await.is_empty());
    }

    #[tokio::test]
    async fn a_dead_owner_row_is_swept_after_the_list_view_probes_it() {
        // #2882: a proven-dead ownership row used to stay in the database
        // forever, so every later list render — and every app start —
        // probed the same dead pid again. The first list view that proves
        // the owner gone must remove the row.
        let db = shared_test_store().await;
        let session = unique("session");
        let dead_run = unique("run");
        // A pid far above every real pid: the probe proves it gone at once
        // (same stand-in as `a_dead_owner_pid_loses_the_lock_immediately`).
        let stmt = Statement::from_sql_and_values(
            db.get_database_backend(),
            INSERT_OWNER,
            [
                Value::from(session.as_str()),
                Value::from(dead_run.as_str()),
                Value::from(1i64 << 31),
                Value::from(0i64),
                Value::from(now_ms()),
            ],
        );
        db.execute(stmt).await.expect("insert dead-owner row");

        let live = live_owned_ids(&[session.clone()]).await;
        assert!(
            live.is_empty(),
            "the proven-dead owner must not count as live: {live:?}"
        );

        let row = db
            .query_one(Statement::from_sql_and_values(
                db.get_database_backend(),
                "SELECT COUNT(*) AS c FROM session_ownership WHERE session_id = ?",
                [Value::from(session.as_str())],
            ))
            .await
            .expect("count ownership rows")
            .expect("COUNT always returns a row");
        let remaining: i64 = row.try_get("", "c").expect("count readable");
        assert_eq!(
            remaining, 0,
            "the proven-dead row must be swept so later starts do not re-probe it (#2882)"
        );
    }
}
