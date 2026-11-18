//! S8 (#2860): write a finished headless run's history back into the profile
//! database before its ephemeral snapshot is removed.
//!
//! Rules (from the issue):
//! - A fresh run inserts exactly one new `sessions` row plus its messages.
//! - A `--session <id>` run **merges** into the original session when the
//!   profile's message history is unchanged since the run started (same
//!   `sessions.version` counter as the captured [`SessionForkBase`], bumped by
//!   the message-change triggers (#2878)) and no GUI process currently owns
//!   the session; otherwise it **forks**: a brand-new session row whose
//!   `forked_from` names the original id.
//! - The version check and the writes share one `BEGIN IMMEDIATE`
//!   transaction, so a concurrent GUI write cannot slip between them.
//! - Secrets that reached the ephemeral copy (provider configs, MCP env,
//!   process environment) are redacted from every row before it is inserted.

use crate::headless::runtime::EphemeralHeadlessRuntime;
use crate::mcp::secret_templates::mask_values;
use rusqlite::types::Value;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

/// Which session the profile database now holds for this run, and — when the
/// run was written as a fork — the session it was forked from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteBackOutcome {
    pub session_id: String,
    pub forked_from: Option<String>,
}

/// Secrets shorter than this are too ambiguous to mask: the same short text
/// occurs as ordinary content (`maxTokens = "4096"`) and masking it would
/// silently corrupt history.
const MIN_SECRET_LEN: usize = 8;

/// `sessions` columns written by the write-back, in fixed order. `forked_from`
/// is appended separately because older profile databases predate it.
const SESSION_COLUMNS: &str = "id, name, status, model, provider, assistant_id, \
    parent_session_id, lineage_id, depth, max_depth, max_fanout, org_id, org_name, \
    org_root_session_id, created_at, updated_at, last_viewed_at, last_message_at, \
    last_attention_at, last_attention_reason, is_bookmarked, execution_mode, \
    workspace_override, workspace_isolation, docker_config_json, docker_container_name, \
    docker_host_workspace_path, status_reason";

/// `messages` columns copied verbatim (after redaction), in fixed order.
const MESSAGE_COLUMNS: &str = "id, session_id, role, content, tool_calls, tool_call_id, \
    is_streaming, thinking, thinking_signature, assistant_id, attachments, tool_use, \
    created_at, updated_at, source, error, usage, prompt_tokens";

/// Free-text `sessions` fields that can echo user/tool content (and thus
/// secrets): indices into a row read with [`SESSION_COLUMNS`].
const SESSION_TEXT_FIELDS: &[usize] = &[1, 19, 27]; // name, last_attention_reason, status_reason

/// Free-text `messages` fields: indices into a row read with
/// [`MESSAGE_COLUMNS`].
const MESSAGE_TEXT_FIELDS: &[usize] = &[3, 4, 7, 10, 11, 15]; // content, tool_calls, thinking, attachments, tool_use, error

/// Write `runtime`'s session and messages into the profile database.
pub fn write_back_run(runtime: &EphemeralHeadlessRuntime) -> Result<WriteBackOutcome, String> {
    let profile_db_path = runtime.env.profile_db_path.as_deref().ok_or_else(|| {
        "headless write-back skipped: no profile database path was recorded".to_string()
    })?;

    let ephemeral = open_read_only(&runtime.env.db_path)?;
    let secrets = collect_secret_keys(&ephemeral)?;
    let session_row = read_row(
        &ephemeral,
        &format!("SELECT {SESSION_COLUMNS} FROM sessions WHERE id = ?1"),
        &runtime.session_id,
    )?
    .ok_or_else(|| {
        format!(
            "headless write-back: ephemeral session row {} is gone",
            runtime.session_id
        )
    })?;
    let message_rows = read_rows(
        &ephemeral,
        &format!(
            "SELECT {MESSAGE_COLUMNS} FROM messages WHERE session_id = ?1 ORDER BY created_at, id"
        ),
        &runtime.session_id,
    )?;
    drop(ephemeral);

    let session_row = redact_row(&session_row, SESSION_TEXT_FIELDS, &secrets);
    let message_rows: Vec<Vec<Value>> = message_rows
        .iter()
        .map(|row| redact_row(row, MESSAGE_TEXT_FIELDS, &secrets))
        .collect();

    if !profile_db_path.exists() {
        return write_back_fresh_profile(&runtime.env.db_path, profile_db_path, runtime);
    }

    let mut profile = open_read_write(profile_db_path)?;
    let _ = profile.busy_timeout(Duration::from_secs(5));
    ensure_forked_from_column(&profile)?;
    ensure_version_column_and_triggers(&profile)?;

    if !runtime.resumed {
        let session_id = runtime.session_id.clone();
        with_immediate_tx(&mut profile, |conn| {
            insert_session_row(conn, &session_row, &session_id, None)?;
            for row in &message_rows {
                insert_message_row(conn, row, &session_id, None)?;
            }
            Ok(())
        })?;
        return Ok(WriteBackOutcome {
            session_id: runtime.session_id.clone(),
            forked_from: None,
        });
    }

    let original_id = runtime.session_id.clone();
    let fork_base = runtime.fork_base.clone();
    with_immediate_tx(&mut profile, |conn| {
        let merge = session_row_exists(conn, &original_id)?
            && !has_live_owner(conn, &original_id)?
            && fork_base_matches(conn, &original_id, &fork_base)?;
        if merge {
            let existing = existing_message_ids(conn, &original_id)?;
            for row in &message_rows {
                if let Some(id) = row_text(&row[0]) {
                    if !existing.contains(id) {
                        insert_message_row(conn, row, &original_id, None)?;
                    }
                }
            }
            refresh_merged_session(conn, &session_row, &original_id)?;
            Ok(WriteBackOutcome {
                session_id: original_id,
                forked_from: None,
            })
        } else {
            let fork_id = format!("headless-{}", uuid::Uuid::new_v4());
            insert_session_row(conn, &session_row, &fork_id, Some(&original_id))?;
            for row in &message_rows {
                // Fresh message ids: the originals already live in the profile
                // under the forked-from session.
                let fresh_id = uuid::Uuid::new_v4().to_string();
                insert_message_row(conn, row, &fork_id, Some(&fresh_id))?;
            }
            Ok(WriteBackOutcome {
                session_id: fork_id,
                forked_from: Some(original_id),
            })
        }
    })
}

/// A profile with no database yet gets a consistent copy of the ephemeral
/// snapshot via `VACUUM INTO` — the ephemeral DB was built from the same
/// migrations, and its WAL contents are folded into the copy.
fn write_back_fresh_profile(
    ephemeral_db: &Path,
    profile_db: &Path,
    runtime: &EphemeralHeadlessRuntime,
) -> Result<WriteBackOutcome, String> {
    if let Some(parent) = profile_db.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create profile directory {parent:?}: {e}"))?;
    }
    let conn = open_read_only(ephemeral_db)?;
    conn.execute("VACUUM INTO ?1", [profile_db.to_string_lossy().as_ref()])
        .map_err(|e| format!("Failed to create profile DB {profile_db:?}: {e}"))?;
    drop(conn);
    Ok(WriteBackOutcome {
        session_id: runtime.session_id.clone(),
        forked_from: None,
    })
}

fn open_read_only(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Failed to open database {path:?}: {e}"))
}

fn open_read_write(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Failed to open profile database {path:?}: {e}"))
}

/// Add `forked_from` to an older profile database that predates the migration;
/// the GUI's next start runs migrations anyway, so this only keeps the
/// write-back's own SQL valid.
fn ensure_forked_from_column(conn: &Connection) -> Result<(), String> {
    let has_column = column_names(conn, "sessions")?
        .iter()
        .any(|name| name == "forked_from");
    if has_column {
        return Ok(());
    }
    conn.execute_batch("ALTER TABLE sessions ADD COLUMN forked_from TEXT;")
        .map_err(|e| format!("Failed to add forked_from column to profile DB: {e}"))
}

/// Install the `version` column and the message-change triggers (#2878) on an
/// older profile database that predates the migration; the GUI's next start
/// runs migrations anyway, so this only keeps the write-back's own SQL valid
/// and keeps the counter live for writes that happen before that start. When
/// the schema is already installed (the common case), no DDL runs, so the
/// write-back never takes a schema write lock against a running GUI.
fn ensure_version_column_and_triggers(conn: &Connection) -> Result<(), String> {
    let has_column = column_names(conn, "sessions")?
        .iter()
        .any(|name| name == "version");
    let names = migration::session_version::SESSION_VERSION_TRIGGER_NAMES;
    let placeholders = names.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let installed_triggers: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name IN ({placeholders})"
            ),
            rusqlite::params_from_iter(names.iter()),
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to inspect session version triggers on profile DB: {e}"))?;
    if has_column && installed_triggers == names.len() as i64 {
        return Ok(());
    }

    if !has_column {
        conn.execute_batch("ALTER TABLE sessions ADD COLUMN version INTEGER NOT NULL DEFAULT 0;")
            .map_err(|e| format!("Failed to add version column to profile DB: {e}"))?;
    }
    for sql in migration::session_version::SESSION_VERSION_TRIGGERS {
        conn.execute_batch(sql).map_err(|e| {
            format!("Failed to install session version triggers on profile DB: {e}")
        })?;
    }
    Ok(())
}

fn column_names(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| format!("Failed to read schema of {table}: {e}"))?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| format!("Failed to read schema of {table}: {e}"))?
        .filter_map(|name| name.ok())
        .collect();
    Ok(names)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .map_err(|e| format!("Failed to inspect profile DB schema: {e}"))
}

/// Run `body` inside one `BEGIN IMMEDIATE` transaction: the reserved write
/// lock makes the merge-or-fork decision and its writes atomic against a
/// concurrent GUI backend writer.
fn with_immediate_tx<T>(
    conn: &mut Connection,
    body: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| format!("BEGIN IMMEDIATE failed: {e}"))?;
    match body(conn) {
        Ok(value) => match conn.execute_batch("COMMIT;") {
            Ok(()) => Ok(value),
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                Err(format!("COMMIT failed: {e}"))
            }
        },
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(err)
        }
    }
}

fn session_row_exists(conn: &Connection, id: &str) -> Result<bool, String> {
    conn.query_row("SELECT COUNT(*) FROM sessions WHERE id = ?1", [id], |row| {
        row.get::<_, i64>(0)
    })
    .map(|count| count > 0)
    .map_err(|e| format!("Failed to check session {id} in profile DB: {e}"))
}

fn has_live_owner(conn: &Connection, id: &str) -> Result<bool, String> {
    if !table_exists(conn, "session_ownership")? {
        return Ok(false);
    }
    // Headless never claims, so any live row means a GUI has the session
    // open. The stale rule comes from session_ownership so both sides agree.
    let stale_before = crate::server::session_ownership::stale_before_ms();
    conn.query_row(
        "SELECT COUNT(*) FROM session_ownership WHERE session_id = ?1 AND heartbeat_at >= ?2",
        params![id, stale_before],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .map_err(|e| format!("Failed to check ownership of session {id}: {e}"))
}

/// The profile is still at the snapshot the run started from: the session's
/// message-change counter (#2878) has not moved since [`SessionForkBase`] was
/// captured.
fn fork_base_matches(
    conn: &Connection,
    id: &str,
    base: &crate::headless::runtime::SessionForkBase,
) -> Result<bool, String> {
    let version: Option<i64> = conn
        .query_row("SELECT version FROM sessions WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| format!("Failed to read version of session {id} in profile DB: {e}"))?;
    Ok(version == Some(base.version))
}

fn existing_message_ids(conn: &Connection, id: &str) -> Result<HashSet<String>, String> {
    let mut stmt = conn
        .prepare("SELECT id FROM messages WHERE session_id = ?1")
        .map_err(|e| format!("Failed to list message ids of session {id}: {e}"))?;
    let ids = stmt
        .query_map([id], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to list message ids of session {id}: {e}"))?
        .filter_map(|id| id.ok())
        .collect();
    Ok(ids)
}

/// After a merge, surface the run's activity in the session list without
/// touching GUI-owned fields (status, model, …).
fn refresh_merged_session(
    conn: &Connection,
    session_row: &[Value],
    id: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET updated_at = ?1, last_message_at = ?2 WHERE id = ?3",
        params![&session_row[15], &session_row[17], id],
    )
    .map_err(|e| format!("Failed to refresh merged session {id}: {e}"))?;
    Ok(())
}

fn insert_session_row(
    conn: &Connection,
    row: &[Value],
    new_id: &str,
    forked_from: Option<&str>,
) -> Result<(), String> {
    let placeholders = (1..=SESSION_COLUMNS.split(',').count() + 1)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql =
        format!("INSERT INTO sessions ({SESSION_COLUMNS}, forked_from) VALUES ({placeholders})");
    let mut values: Vec<Value> = row.to_vec();
    values[0] = Value::Text(new_id.to_string());
    values.push(match forked_from {
        Some(original) => Value::Text(original.to_string()),
        None => Value::Null,
    });
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare session insert: {e}"))?;
    stmt.execute(rusqlite::params_from_iter(values.iter()))
        .map_err(|e| format!("Failed to insert session row {new_id}: {e}"))?;
    Ok(())
}

fn insert_message_row(
    conn: &Connection,
    row: &[Value],
    new_session_id: &str,
    new_id: Option<&str>,
) -> Result<(), String> {
    let sql = format!("INSERT INTO messages ({MESSAGE_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)");
    let mut values: Vec<Value> = row.to_vec();
    if let Some(id) = new_id {
        // `messages.id` is a global primary key: a fork must carry fresh ids
        // for its copy of the original history.
        values[0] = Value::Text(id.to_string());
    }
    values[1] = Value::Text(new_session_id.to_string());
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare message insert: {e}"))?;
    stmt.execute(rusqlite::params_from_iter(values.iter()))
        .map_err(|e| format!("Failed to insert message row: {e}"))?;
    Ok(())
}

fn read_row(conn: &Connection, sql: &str, id: &str) -> Result<Option<Vec<Value>>, String> {
    Ok(read_rows(conn, sql, id)?.into_iter().next())
}

fn read_rows(conn: &Connection, sql: &str, id: &str) -> Result<Vec<Vec<Value>>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("Failed to prepare read: {e}"))?;
    let width = stmt.column_count();
    let rows = stmt
        .query_map([id], |row| {
            (0..width).map(|i| row.get::<_, Value>(i)).collect()
        })
        .map_err(|e| format!("Failed to read rows: {e}"))?
        .collect::<Result<Vec<Vec<Value>>, _>>()
        .map_err(|e| format!("Failed to read rows: {e}"))?;
    Ok(rows)
}

fn row_text(value: &Value) -> Option<&str> {
    match value {
        Value::Text(text) => Some(text),
        _ => None,
    }
}

fn redact_row(row: &[Value], text_fields: &[usize], secrets: &[String]) -> Vec<Value> {
    row.iter()
        .enumerate()
        .map(|(index, value)| {
            if text_fields.contains(&index) {
                match value {
                    Value::Text(text) if !text.is_empty() => {
                        Value::Text(mask_values(text, secrets.iter().map(String::as_str)))
                    }
                    other => other.clone(),
                }
            } else {
                value.clone()
            }
        })
        .collect()
}

/// Values that must never reach the profile database in clear text: provider
/// and MCP credentials found in the ephemeral snapshot, plus this process's
/// credential environment variables. Keychain-backed and already-encrypted
/// stores are safe by construction and are not read here.
///
/// Fails closed: if a secret source cannot be read, the caller aborts the
/// write-back instead of persisting rows that might be unredacted.
fn collect_secret_keys(ephemeral: &Connection) -> Result<Vec<String>, String> {
    let mut secrets: Vec<String> = Vec::new();

    let mut stmt = ephemeral
        .prepare("SELECT key, value FROM settings")
        .map_err(|e| format!("Failed to read settings for redaction: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|e| format!("Failed to read settings rows for redaction: {e}"))?;
    for row in rows {
        let (key, value) =
            row.map_err(|e| format!("Failed to decode settings row for redaction: {e}"))?;
        if let Some(value) = value {
            if is_secret_key_name(&key) && value.trim().len() >= MIN_SECRET_LEN {
                secrets.push(value.clone());
            }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&value) {
                collect_json_secrets(&parsed, Some(&key), &mut secrets);
            }
        }
    }

    for column in column_names(ephemeral, "mcp_servers")? {
        let sql = format!("SELECT \"{}\" FROM mcp_servers", column.replace('"', ""));
        let mut stmt = ephemeral
            .prepare(&sql)
            .map_err(|e| format!("Failed to read mcp_servers.{column} for redaction: {e}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, Option<Value>>(0))
            .map_err(|e| format!("Failed to read mcp_servers.{column} rows for redaction: {e}"))?;
        for row in rows {
            let value = row.map_err(|e| {
                format!("Failed to decode mcp_servers.{column} row for redaction: {e}")
            })?;
            // Only TEXT can carry a secret; integers (`tool_count`) and NULLs
            // are skipped without failing the read.
            let Value::Text(text) = value.unwrap_or(Value::Null) else {
                continue;
            };
            if is_secret_key_name(&column) && text.trim().len() >= MIN_SECRET_LEN {
                secrets.push(text.clone());
            }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                collect_json_secrets(&parsed, None, &mut secrets);
            }
        }
    }

    for (name, value) in std::env::vars() {
        let upper = name.to_uppercase();
        let credential_var = upper.ends_with("_API_KEY")
            || upper.ends_with("_APIKEY")
            || upper.ends_with("_TOKEN")
            || upper.ends_with("_SECRET");
        if credential_var && value.trim().len() >= MIN_SECRET_LEN {
            secrets.push(value);
        }
    }

    secrets.sort();
    secrets.dedup();
    Ok(secrets)
}

fn is_secret_key_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["key", "token", "secret", "password", "credential"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn collect_json_secrets(value: &serde_json::Value, parent: Option<&str>, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                collect_json_secrets(child, Some(key.as_str()), out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_json_secrets(item, parent, out);
            }
        }
        serde_json::Value::String(text)
            if parent.is_some_and(is_secret_key_name) && text.trim().len() >= MIN_SECRET_LEN =>
        {
            out.push(text.clone());
        }
        _ => {}
    }
}
