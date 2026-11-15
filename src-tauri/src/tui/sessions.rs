//! TUI session commands: `/new`, `/resume`, list, `/rename`, `/delete` (#2763),
//! plus the inline P0 verbs `/permission`, `/continue`, `/retry`, `/clear`,
//! `/help`, `/history`, `/reload` (#2764, parity §3.3).
//!
//! Ownership badges follow #2759: a live foreign owner is **open elsewhere**,
//! or **running** when the workflow is still driven (`busy` / `queued` /
//! `provisioning`). Both refuse entry — no read-only mirror, no takeover.

use crate::server::shared_backend::SharedBackendClient;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use serde_json::{json, Value};

const DRIVEN_STATUSES: &[&str] = &["busy", "queued", "provisioning"];

/// Badge for a list row owned by another frontend.
pub fn ownership_label(open_elsewhere: bool, status: &str) -> Option<&'static str> {
    if !open_elsewhere {
        return None;
    }
    if DRIVEN_STATUSES.contains(&status) {
        Some("running")
    } else {
        Some("open elsewhere")
    }
}

/// Slash commands this module owns (parity §3.3 P0/P1 session verbs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    New {
        assistant_id: Option<String>,
    },
    Resume {
        session_id: Option<String>,
    },
    Rename {
        name: String,
    },
    Delete {
        session_id: Option<String>,
    },
    /// `/permission normal|yolo|unsafe` — execution mode of the open session (§2.5).
    Permission {
        mode: String,
    },
    /// `/continue` — resume the open session's paused workflow (§2.6).
    Continue,
    /// `/retry` — re-drive the open session after an error (§2.5/§2.7).
    Retry,
    /// `/clear` — clear the transcript view (§2.6, view-local).
    Clear,
    /// `/help` — command list (§3.3 rule 10).
    Help,
    /// `/history [search <text>]` — session list picker (§2.11 list rows).
    /// Search filters rows client-side; the §2.11 sort/status/lineage flags
    /// are not wired yet and parse as unknown commands.
    History {
        query: Option<String>,
    },
    /// `/reload` — refetch the open session's messages into the transcript.
    Reload,
}

/// Parse a leading `/…` composer line into a session command.
pub fn parse_session_command(text: &str) -> Option<SessionCommand> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return None;
    }
    let mut parts = trimmed[1..].split_whitespace();
    let verb = parts.next()?.to_ascii_lowercase();
    match verb.as_str() {
        "new" => Some(SessionCommand::New {
            assistant_id: parts.next().map(str::to_string),
        }),
        "resume" => Some(SessionCommand::Resume {
            session_id: parts.next().map(str::to_string),
        }),
        "rename" => Some(SessionCommand::Rename {
            name: parts.collect::<Vec<_>>().join(" "),
        }),
        "delete" => Some(SessionCommand::Delete {
            session_id: parts.next().map(str::to_string),
        }),
        "permission" => Some(SessionCommand::Permission {
            mode: parts.next().unwrap_or("").to_ascii_lowercase(),
        }),
        "continue" => Some(SessionCommand::Continue),
        "retry" => Some(SessionCommand::Retry),
        "clear" => Some(SessionCommand::Clear),
        "help" => Some(SessionCommand::Help),
        "reload" => Some(SessionCommand::Reload),
        "history" => {
            let tokens: Vec<&str> = parts.collect();
            match tokens.as_slice() {
                [] => Some(SessionCommand::History { query: None }),
                [word, rest @ ..] if *word == "search" && !rest.is_empty() => {
                    Some(SessionCommand::History {
                        query: Some(rest.join(" ")),
                    })
                }
                // Sort/status/lineage flags (§2.11) are not wired yet.
                _ => None,
            }
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct PickerRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub open_elsewhere: bool,
    pub badge: Option<&'static str>,
}

impl PickerRow {
    pub fn from_json(row: &Value) -> Option<Self> {
        let id = row.get("id")?.as_str()?.to_string();
        let name = row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let status = row
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let open_elsewhere = row
            .get("openElsewhere")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let badge = ownership_label(open_elsewhere, &status);
        Some(Self {
            id,
            name,
            status,
            open_elsewhere,
            badge,
        })
    }

    pub fn label(&self) -> String {
        let title = if self.name.is_empty() {
            self.id.as_str()
        } else {
            self.name.as_str()
        };
        match self.badge {
            Some(badge) => format!("{title}  [{badge}]  {}", self.status),
            None => format!("{title}  {}", self.status),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionPicker {
    pub title: String,
    pub rows: Vec<PickerRow>,
    pub selected: usize,
}

impl SessionPicker {
    pub fn from_list_payload(title: impl Into<String>, payload: &Value) -> Result<Self, String> {
        let sessions = payload
            .get("sessions")
            .and_then(Value::as_array)
            .ok_or_else(|| "list_sessions response missing sessions".to_string())?;
        let rows: Vec<PickerRow> = sessions.iter().filter_map(PickerRow::from_json).collect();
        Ok(Self {
            title: title.into(),
            rows,
            selected: 0,
        })
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> PickerAction {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                PickerAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.rows.is_empty() {
                    self.selected = (self.selected + 1).min(self.rows.len() - 1);
                }
                PickerAction::None
            }
            KeyCode::Enter => {
                let Some(row) = self.rows.get(self.selected).cloned() else {
                    return PickerAction::Cancel;
                };
                if row.open_elsewhere {
                    let badge = row.badge.unwrap_or("open elsewhere");
                    return PickerAction::Refused {
                        id: row.id.clone(),
                        reason: format!("Session {} is {badge} — entry refused", row.id),
                    };
                }
                PickerAction::Open(row.id)
            }
            KeyCode::Esc | KeyCode::Char('q') => PickerAction::Cancel,
            _ => PickerAction::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerAction {
    None,
    Cancel,
    Open(String),
    Refused { id: String, reason: String },
}

pub fn draw_picker(frame: &mut Frame<'_>, picker: &SessionPicker) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    frame.render_widget(Paragraph::new(format!(" {} ", picker.title)), chunks[0]);

    let items: Vec<ListItem> = picker
        .rows
        .iter()
        .map(|row| ListItem::new(row.label()))
        .collect();
    let mut state = ListState::default();
    if !picker.rows.is_empty() {
        state.select(Some(picker.selected));
    }
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" sessions "))
            .highlight_symbol("> "),
        chunks[1],
        &mut state,
    );

    frame.render_widget(
        Paragraph::new(" ↑/↓ select · Enter open · Esc cancel "),
        chunks[2],
    );
}

/// Create a draft session (no first message) for `/new`.
pub async fn create_new_session(
    api: &SharedBackendClient,
    assistant_id: Option<&str>,
) -> Result<(String, String), String> {
    let assistant = match assistant_id {
        Some(id) => id.to_string(),
        None => first_assistant_id(api).await?,
    };
    let created = api
        .create_session(&json!({
            "assistantId": assistant,
            "name": "TUI session",
        }))
        .await?;
    let id = created
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| "create_session response missing id".to_string())?
        .to_string();
    let name = created
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("TUI session")
        .to_string();
    Ok((id, name))
}

/// Map a Session API open error to a TUI notice when ownership refuses entry.
pub fn map_open_refusal(session_id: &str, err: &str) -> Result<(), String> {
    if err.contains(crate::server::session_ownership::OPEN_ELSEWHERE_MESSAGE) {
        Err(format!(
            "Session {session_id} is open elsewhere or running — entry refused"
        ))
    } else {
        Err(err.to_string())
    }
}

/// Open a session for this frontend; map ownership refusal to a clear notice.
pub async fn open_session_or_refuse(
    api: &SharedBackendClient,
    session_id: &str,
) -> Result<(), String> {
    match api.open_session(session_id).await {
        Ok(_) => Ok(()),
        Err(err) => map_open_refusal(session_id, &err),
    }
}

pub async fn rename_session(
    api: &SharedBackendClient,
    session_id: &str,
    name: &str,
) -> Result<(), String> {
    api.patch_session(session_id, &json!({ "name": name }))
        .await?;
    Ok(())
}

pub async fn delete_session(api: &SharedBackendClient, session_id: &str) -> Result<(), String> {
    api.delete_session(session_id).await?;
    Ok(())
}

pub async fn load_resume_picker(api: &SharedBackendClient) -> Result<SessionPicker, String> {
    let payload = api.list_sessions(Some(50)).await?;
    SessionPicker::from_list_payload("/resume", &payload)
}

/// `/history [search <text>]` — session list, search filtered client-side
/// against row name and id (§2.11 list rows).
pub async fn load_history_picker(
    api: &SharedBackendClient,
    query: Option<&str>,
) -> Result<SessionPicker, String> {
    let mut picker = {
        let payload = api.list_sessions(Some(50)).await?;
        SessionPicker::from_list_payload("/history", &payload)?
    };
    if let Some(query) = query {
        let needle = query.to_lowercase();
        picker.rows.retain(|row| {
            row.name.to_lowercase().contains(&needle) || row.id.to_lowercase().contains(&needle)
        });
    }
    Ok(picker)
}

async fn first_assistant_id(api: &SharedBackendClient) -> Result<String, String> {
    let assistants = api.list_assistants().await?;
    let list = assistants
        .as_array()
        .or_else(|| assistants.get("assistants").and_then(Value::as_array))
        .ok_or_else(|| "Unexpected /api/assistants response".to_string())?;
    list.first()
        .and_then(|row| row.get("id").or_else(|| row.get("assistantId")))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "No assistant configured — open the GUI once to create one".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ownership_label_distinguishes_running_from_open_elsewhere() {
        assert_eq!(ownership_label(false, "busy"), None);
        assert_eq!(ownership_label(true, "busy"), Some("running"));
        assert_eq!(ownership_label(true, "queued"), Some("running"));
        assert_eq!(ownership_label(true, "provisioning"), Some("running"));
        assert_eq!(ownership_label(true, "paused"), Some("open elsewhere"));
        assert_eq!(ownership_label(true, "idle"), Some("open elsewhere"));
    }

    #[test]
    fn parse_session_commands() {
        assert_eq!(
            parse_session_command("/new"),
            Some(SessionCommand::New { assistant_id: None })
        );
        assert_eq!(
            parse_session_command("/new asst_1"),
            Some(SessionCommand::New {
                assistant_id: Some("asst_1".into())
            })
        );
        assert_eq!(
            parse_session_command("/resume"),
            Some(SessionCommand::Resume { session_id: None })
        );
        assert_eq!(
            parse_session_command("/history"),
            Some(SessionCommand::History { query: None })
        );
        assert_eq!(
            parse_session_command("/resume abc"),
            Some(SessionCommand::Resume {
                session_id: Some("abc".into())
            })
        );
        assert_eq!(
            parse_session_command("/rename Hello World"),
            Some(SessionCommand::Rename {
                name: "Hello World".into()
            })
        );
        assert_eq!(
            parse_session_command("/rename"),
            Some(SessionCommand::Rename {
                name: String::new()
            })
        );
        assert_eq!(
            parse_session_command("/delete"),
            Some(SessionCommand::Delete { session_id: None })
        );
        assert_eq!(parse_session_command("/foo"), None);
        assert_eq!(parse_session_command("//new"), None);
    }

    #[test]
    fn parse_parity_inline_commands() {
        assert_eq!(
            parse_session_command("/permission YOLO"),
            Some(SessionCommand::Permission {
                mode: "yolo".into()
            })
        );
        assert_eq!(
            parse_session_command("/permission"),
            Some(SessionCommand::Permission {
                mode: String::new()
            })
        );
        assert_eq!(
            parse_session_command("/continue"),
            Some(SessionCommand::Continue)
        );
        assert_eq!(parse_session_command("/retry"), Some(SessionCommand::Retry));
        assert_eq!(parse_session_command("/clear"), Some(SessionCommand::Clear));
        assert_eq!(parse_session_command("/help"), Some(SessionCommand::Help));
        // Trailing junk on a no-arg verb is ignored, not an error.
        assert_eq!(
            parse_session_command("/continue now"),
            Some(SessionCommand::Continue)
        );
    }

    #[test]
    fn parse_history_and_reload() {
        assert_eq!(
            parse_session_command("/history"),
            Some(SessionCommand::History { query: None })
        );
        assert_eq!(
            parse_session_command("/history search korean report"),
            Some(SessionCommand::History {
                query: Some("korean report".into())
            })
        );
        // §2.11 sort/status/lineage flags are not wired yet — parse as
        // unknown so the composer shows "Unknown command", not a silent miss.
        assert_eq!(parse_session_command("/history --sort recent"), None);
        assert_eq!(parse_session_command("/history search"), None);
        assert_eq!(
            parse_session_command("/reload"),
            Some(SessionCommand::Reload)
        );
    }

    #[test]
    fn picker_refuses_open_elsewhere_rows() {
        let payload = json!({
            "sessions": [
                {
                    "id": "s1",
                    "name": "Mine",
                    "status": "idle",
                    "openElsewhere": false
                },
                {
                    "id": "s2",
                    "name": "Theirs",
                    "status": "busy",
                    "openElsewhere": true
                }
            ],
            "nextCursor": null
        });
        let mut picker = SessionPicker::from_list_payload("/resume", &payload).unwrap();
        assert_eq!(picker.rows[1].badge, Some("running"));
        picker.selected = 1;
        match picker.handle_key(KeyEvent::from(KeyCode::Enter)) {
            PickerAction::Refused { id, reason } => {
                assert_eq!(id, "s2");
                assert!(reason.contains("running"));
            }
            other => panic!("expected refuse, got {other:?}"),
        }
        picker.selected = 0;
        assert_eq!(
            picker.handle_key(KeyEvent::from(KeyCode::Enter)),
            PickerAction::Open("s1".into())
        );
    }

    #[test]
    fn map_open_refusal_uses_the_real_ownership_message() {
        let id = "sess_owned";
        let server_err = format!(
            "Session {id} {}",
            crate::server::session_ownership::OPEN_ELSEWHERE_MESSAGE
        );
        let refused = map_open_refusal(id, &server_err).unwrap_err();
        assert!(refused.contains("open elsewhere or running"));
        assert!(refused.contains(id));

        let other = map_open_refusal(id, "boom").unwrap_err();
        assert_eq!(other, "boom");
    }
}
