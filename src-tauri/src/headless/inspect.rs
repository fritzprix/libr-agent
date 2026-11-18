//! `--list-sessions` and `--inspect-session` output (#2887).
//!
//! Startup error formatting plus the SQLite queries and message preview helpers
//! those two commands render.

use crate::execution_mode::ExecutionMode;
use crate::headless::cli::{
    exit_codes, HeadlessArgs, InspectSessionArgs, ListSessionsArgs, OutputFormat,
};
use crate::headless::ephemeral_db::resolve_workspace_path;
use crate::headless::output::{
    inspect_digest_text, inspect_turn_text, write_final_output, HeadlessResultPayload,
    InspectMessageDetail, InspectSessionDigest, InspectSessionMeta, InspectSessionTurn,
    InspectToolCallDetail, InspectToolCallSummary, InspectTurnSummary,
};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

/// Format a startup failure before a session could be created and return exit code `1`.
pub fn emit_startup_error<Out: Write, ErrOut: Write>(
    args: &HeadlessArgs,
    error_message: String,
    stdout: &mut Out,
    stderr: &mut ErrOut,
) -> HeadlessResultPayload {
    let workspace_path = args
        .cd
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| ".".to_string());
    let execution_mode = args.execution_mode.unwrap_or(ExecutionMode::Normal);
    let payload = HeadlessResultPayload::error(error_message, workspace_path, execution_mode);
    let _ = write_final_output(args.output_format, &payload, stdout, stderr);
    payload
}

/// Execute `--list-sessions` against the profile database, read-only.
///
/// Prints one tab-separated line per session of the target directory:
/// `id  status  model  updated_at  title`. A profile with no database yet has no
/// sessions, which is a success. A database that cannot be read (it cannot be
/// opened, has no `sessions` table, or has a bad row) is an error on stderr and
/// exit `1`, never an empty list.
pub fn run_list_sessions<Out: Write, ErrOut: Write>(
    list_args: &ListSessionsArgs,
    source_db_path: &Path,
    stdout: &mut Out,
    stderr: &mut ErrOut,
) -> i32 {
    let target_dir = match resolve_workspace_path(list_args.cd.as_deref()) {
        Ok(dir) => dir,
        Err(err) => {
            let _ = writeln!(stderr, "error: {err}");
            return exit_codes::ERROR;
        }
    };

    if !source_db_path.exists() {
        return exit_codes::SUCCESS;
    }

    match query_sessions_of(source_db_path, &target_dir) {
        Ok(rows) => {
            for (id, name, status, model, updated_at) in rows {
                let title = if name.trim().is_empty() {
                    "(untitled)"
                } else {
                    name.as_str()
                };
                let _ = writeln!(stdout, "{id}\t{status}\t{model}\t{updated_at}\t{title}");
            }
            exit_codes::SUCCESS
        }
        Err(err) => {
            let _ = writeln!(stderr, "error: failed to list sessions: {err}");
            exit_codes::ERROR
        }
    }
}

type SessionRow = (String, String, String, String, i64);

/// Select the sessions whose `workspace_override` designates `workspace`.
///
/// The app stores `workspace_override` exactly as the user gave it (only `~`
/// expanded), while `workspace` arrives canonicalized from
/// [`resolve_workspace_path`]. Raw string equality therefore misses real
/// sessions on Windows (`\\?\C:\...`) and macOS (`/private/var/...`) or behind
/// symlinks, so the stored value is canonicalized in Rust and compared with
/// `workspace`. A stored value that cannot be canonicalized does not designate
/// an existing directory, so its row is not a match.
fn query_sessions_of(db_path: &Path, workspace: &Path) -> Result<Vec<SessionRow>, rusqlite::Error> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(name, ''), status, model, updated_at, workspace_override \
         FROM sessions \
         WHERE workspace_override IS NOT NULL \
         ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;
    rows.filter_map(|row| match row {
        Ok((id, name, status, model, updated_at, stored_override)) => {
            let matches_workspace = match std::fs::canonicalize(&stored_override) {
                Ok(canonical_override) => canonical_override == workspace,
                Err(_) => false,
            };
            matches_workspace.then_some(Ok((id, name, status, model, updated_at)))
        }
        Err(err) => Some(Err(err)),
    })
    .collect()
}

/// Character cap for `--inspect-session` digest previews (#2874).
const INSPECT_PREVIEW_CHARS: usize = 120;

/// One message row as read for `--inspect-session`.
struct InspectMessageRow {
    id: String,
    role: String,
    content: String,
    tool_calls: String,
    tool_call_id: Option<String>,
    error: Option<String>,
    created_at: i64,
}

/// Truncate to [`INSPECT_PREVIEW_CHARS`] characters on a char boundary.
fn inspect_preview(raw: &str) -> String {
    let trimmed = raw.trim();
    let mut preview: String = trimmed.chars().take(INSPECT_PREVIEW_CHARS).collect();
    if trimmed.chars().count() > INSPECT_PREVIEW_CHARS {
        preview.push('…');
    }
    preview
}

/// Decode the persisted `content` column (JSON of `Vec<MCPContent>`, see
/// `repositories/message_repository/persist.rs`) into the concatenated text
/// parts a postmortem reader wants. This is the only place inspect tolerates
/// non-production formats: a malformed column falls back to the trimmed raw
/// value so inspect never loses data to a decode error.
fn persisted_content_text(content_json: &str) -> String {
    match serde_json::from_str::<Vec<crate::mcp::types::MCPContent>>(content_json) {
        Ok(parts) => parts
            .iter()
            .filter_map(|part| match part {
                crate::mcp::types::MCPContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Err(_) => content_json.trim().to_string(),
    }
}

/// Decode the persisted `error` column and content into
/// `(tool_failed, service_error_text)` via the production envelope decoder:
/// `toolError == true` (column envelope or legacy content item) marks tool
/// failure; the UI envelope itself is never surfaced as error text.
fn persisted_tool_error(content_json: &str, error_json: Option<&str>) -> (bool, Option<String>) {
    let raw_error: Option<serde_json::Value> =
        error_json.and_then(|raw| serde_json::from_str(raw).ok());
    let (error, metadata) = crate::repositories::message_repository::decode_persisted_tool_error(
        content_json,
        raw_error,
    );
    let is_error = metadata
        .as_ref()
        .and_then(|meta| meta.get("toolError"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let service_error = error.map(|value| match value {
        serde_json::Value::String(text) => text,
        other => other.to_string(),
    });
    (is_error, service_error)
}

/// Parse the persisted `tool_calls` JSON column into
/// `(tool_call_id, tool_name, raw_arguments)` triples. A missing, malformed,
/// or non-array value yields no calls.
fn parse_persisted_tool_calls(raw: &str) -> Vec<(String, String, String)> {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Vec::new();
    };
    let Some(items) = parsed.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let tool_call_id = item.get("id")?.as_str()?.to_string();
            let function = item.get("function")?;
            let tool_name = function.get("name")?.as_str()?.to_string();
            let arguments = match function.get("arguments")? {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            Some((tool_call_id, tool_name, arguments))
        })
        .collect()
}

/// Index tool-result messages by the tool call id they answer, as
/// `(decoded text, tool failed)`. Decoding happens here once, so the digest
/// and the detail view never read the raw persisted columns.
fn tool_results_by_call(messages: &[InspectMessageRow]) -> HashMap<String, (String, bool)> {
    messages
        .iter()
        .filter(|message| message.role == "tool")
        .filter_map(|message| {
            let call_id = message.tool_call_id.clone()?;
            let (is_error, _) = persisted_tool_error(&message.content, message.error.as_deref());
            Some((
                call_id,
                (persisted_content_text(&message.content), is_error),
            ))
        })
        .collect()
}

/// Read one session's metadata and its ordered messages, read-only.
/// `Ok(None)` means the session id is not in the profile database.
fn query_inspect_session(
    db_path: &Path,
    id: &str,
) -> Result<Option<(InspectSessionMeta, Vec<InspectMessageRow>)>, rusqlite::Error> {
    use rusqlite::OptionalExtension;
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    // A profile DB that predates the forked_from migration must still be
    // inspectable: select `NULL AS forked_from` when the column is absent so
    // the row maps once either way (S1.1, #2877 G2.1). A failed pragma read
    // propagates — a broken profile must not look like an old one (S2.1).
    let has_forked_from = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name = 'forked_from'",
        [],
        |row| row.get::<_, i64>(0),
    )? > 0;
    let forked_from_column = if has_forked_from {
        "forked_from"
    } else {
        "NULL AS forked_from"
    };
    let meta = conn
        .query_row(
            &format!(
                "SELECT id, status, status_reason, model, provider, workspace_override, \
                 created_at, updated_at, {forked_from_column} \
                 FROM sessions WHERE id = ?1"
            ),
            [id],
            |row| {
                Ok(InspectSessionMeta {
                    id: row.get(0)?,
                    forked_from: row.get(8)?,
                    status: row.get(1)?,
                    status_reason: row.get(2)?,
                    workspace_path: row.get(5)?,
                    model: row.get(3)?,
                    provider: row.get(4)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .optional()?;
    let Some(meta) = meta else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT id, role, COALESCE(content, ''), COALESCE(tool_calls, ''), \
         tool_call_id, error, created_at \
         FROM messages WHERE session_id = ?1 ORDER BY created_at, id",
    )?;
    let rows = stmt.query_map([id], |row| {
        Ok(InspectMessageRow {
            id: row.get(0)?,
            role: row.get(1)?,
            content: row.get(2)?,
            tool_calls: row.get(3)?,
            tool_call_id: row.get(4)?,
            error: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }
    Ok(Some((meta, messages)))
}

/// Execute `--inspect-session` against the profile database, read-only:
/// without `--turn` / `--message` a compact digest (session metadata plus
/// per-turn summaries with bounded content previews and no full tool
/// bodies); with one, the full persisted detail of that single message.
/// No backend, scheduler, or MCP host starts, so it works while a GUI owns
/// the same profile. Unknown sessions, unknown message ids, and
/// out-of-range turns exit `1` with a diagnostic on stderr (#2874).
pub fn run_inspect_session<Out: Write, ErrOut: Write>(
    inspect_args: &InspectSessionArgs,
    source_db_path: &Path,
    stdout: &mut Out,
    stderr: &mut ErrOut,
) -> i32 {
    if !source_db_path.exists() {
        let _ = writeln!(
            stderr,
            "error: session '{}' not found (no profile database yet)",
            inspect_args.session_id
        );
        return exit_codes::ERROR;
    }

    let (meta, messages) = match query_inspect_session(source_db_path, &inspect_args.session_id) {
        Ok(Some(pair)) => pair,
        Ok(None) => {
            let _ = writeln!(
                stderr,
                "error: session '{}' not found",
                inspect_args.session_id
            );
            return exit_codes::ERROR;
        }
        Err(err) => {
            let _ = writeln!(
                stderr,
                "error: failed to inspect session {}: {err}",
                inspect_args.session_id
            );
            return exit_codes::ERROR;
        }
    };

    let selected: Option<usize> = if let Some(turn) = inspect_args.turn {
        if turn >= messages.len() {
            let _ = writeln!(
                stderr,
                "error: turn {turn} out of range: session '{}' has {} turns",
                meta.id,
                messages.len()
            );
            return exit_codes::ERROR;
        }
        Some(turn)
    } else if let Some(message_id) = &inspect_args.message {
        match messages.iter().position(|m| &m.id == message_id) {
            Some(index) => Some(index),
            None => {
                let _ = writeln!(
                    stderr,
                    "error: message '{message_id}' not found in session '{}'",
                    meta.id
                );
                return exit_codes::ERROR;
            }
        }
    } else {
        None
    };

    let results = tool_results_by_call(&messages);
    let json_output = inspect_args.output_format == OutputFormat::Json;

    match selected {
        None => {
            let turns = messages
                .iter()
                .enumerate()
                .map(|(turn_index, message)| InspectTurnSummary {
                    turn_index,
                    message_id: message.id.clone(),
                    role: message.role.clone(),
                    content_preview: inspect_preview(&persisted_content_text(&message.content)),
                    tool_calls: parse_persisted_tool_calls(&message.tool_calls)
                        .into_iter()
                        .map(|(tool_call_id, tool_name, _)| {
                            let result = results.get(&tool_call_id);
                            InspectToolCallSummary {
                                tool_call_id,
                                tool_name,
                                is_error: result.map(|(_, is_error)| *is_error).unwrap_or(false),
                                error_preview: result
                                    .filter(|(_, is_error)| *is_error)
                                    .map(|(content, _)| inspect_preview(content)),
                            }
                        })
                        .collect(),
                })
                .collect();
            let digest = InspectSessionDigest {
                event_type: "session-digest".to_string(),
                session: meta,
                turns,
            };
            if json_output {
                match serde_json::to_string_pretty(&digest) {
                    Ok(json) => {
                        let _ = writeln!(stdout, "{json}");
                    }
                    Err(err) => {
                        let _ = writeln!(stderr, "error: failed to render digest: {err}");
                        return exit_codes::ERROR;
                    }
                }
            } else {
                let _ = write!(stdout, "{}", inspect_digest_text(&digest));
            }
            exit_codes::SUCCESS
        }
        Some(index) => {
            let message = &messages[index];
            let tool_calls = parse_persisted_tool_calls(&message.tool_calls)
                .into_iter()
                .map(|(tool_call_id, tool_name, arguments)| {
                    let result = results.get(&tool_call_id);
                    InspectToolCallDetail {
                        tool_call_id,
                        tool_name,
                        arguments,
                        result: result.map(|(content, _)| content.clone()),
                        is_error: result.map(|(_, is_error)| *is_error).unwrap_or(false),
                    }
                })
                .collect();
            let detail = InspectSessionTurn {
                event_type: "session-turn".to_string(),
                session: meta,
                turn_index: index,
                message: InspectMessageDetail {
                    message_id: message.id.clone(),
                    role: message.role.clone(),
                    content: persisted_content_text(&message.content),
                    tool_calls,
                    error: persisted_tool_error(&message.content, message.error.as_deref()).1,
                    created_at: message.created_at,
                },
            };
            if json_output {
                match serde_json::to_string_pretty(&detail) {
                    Ok(json) => {
                        let _ = writeln!(stdout, "{json}");
                    }
                    Err(err) => {
                        let _ = writeln!(stderr, "error: failed to render turn detail: {err}");
                        return exit_codes::ERROR;
                    }
                }
            } else {
                let _ = write!(stdout, "{}", inspect_turn_text(&detail));
            }
            exit_codes::SUCCESS
        }
    }
}
