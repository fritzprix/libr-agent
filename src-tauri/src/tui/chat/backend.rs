//! Async `SharedBackendClient` calls for the chat front (#2886).

use super::render::pick_pending_approval;
use super::state::{AskDraft, ChatState, Focus, TranscriptLine};
use crate::agent::session_manager::PendingApprovalSummary;
use crate::server::shared_backend::SharedBackendClient;
use crate::tui::sessions::{
    create_new_session, delete_session, load_resume_picker, open_session_or_refuse, rename_session,
    SessionCommand,
};
use serde_json::{json, Value};

pub(super) async fn poll_session(api: &SharedBackendClient, state: &mut ChatState) {
    let Some(session_id) = state.session_id.clone() else {
        return;
    };
    match api.get_session(&session_id).await {
        Ok(session) => {
            if let Some(name) = session.get("name").and_then(Value::as_str) {
                if !name.is_empty() {
                    state.session_title = name.to_string();
                }
            }
            state.pending_approval = session
                .get("pendingApprovals")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .and_then(pick_pending_approval);
            state.pending_prompt = session
                .get("pendingUserPrompt")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            state.sync_ask_with_prompt();
            if !state.has_s5() {
                state.focus = Focus::Composer;
            }
            let status = session
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            state.status = format!("{status} · session {session_id}");
        }
        Err(err) => state.notice = Some(err),
    }

    match api.get_messages(&session_id, Some(100)).await {
        Ok(payload) => {
            if let Some(messages) = payload.get("messages").and_then(Value::as_array) {
                state.apply_messages(messages);
            }
        }
        Err(err) => state.notice = Some(err),
    }
}

pub(super) async fn send_user_text(api: &SharedBackendClient, state: &mut ChatState, text: &str) {
    state.notice = None;
    if state.session_id.is_none() {
        match ensure_session(api, text).await {
            Ok(id) => {
                state.session_id = Some(id);
                state.transcript.push(TranscriptLine {
                    role: "You".to_string(),
                    text: text.to_string(),
                });
            }
            Err(err) => state.notice = Some(err),
        }
        return;
    }
    let session_id = state.session_id.clone().expect("checked above");
    match api
        .send_message(&session_id, &json!({ "content": text }))
        .await
    {
        Ok(_) => {
            state.transcript.push(TranscriptLine {
                role: "You".to_string(),
                text: text.to_string(),
            });
        }
        Err(err) => state.notice = Some(err),
    }
}

pub(super) async fn run_session_command(
    api: &SharedBackendClient,
    state: &mut ChatState,
    command: SessionCommand,
) {
    state.notice = None;
    match command {
        SessionCommand::New { assistant_id } => {
            match create_new_session(api, assistant_id.as_deref()).await {
                Ok((id, name)) => {
                    state.session_id = Some(id.clone());
                    state.session_title = name;
                    state.transcript.clear();
                    state.pending_approval = None;
                    state.pending_prompt = None;
                    state.scroll_from_bottom = 0;
                    state.status = format!("idle · session {id}");
                    state.notice = Some(format!("Started session {id}"));
                }
                Err(err) => state.notice = Some(err),
            }
        }
        SessionCommand::Resume {
            session_id: Some(id),
        } => {
            switch_to_session(api, state, &id).await;
        }
        SessionCommand::Resume { session_id: None } => match load_resume_picker(api).await {
            Ok(picker) => {
                if picker.rows.is_empty() {
                    state.notice = Some("No sessions to resume".to_string());
                } else {
                    state.picker = Some(picker);
                }
            }
            Err(err) => state.notice = Some(err),
        },
        SessionCommand::Rename { name } => {
            let Some(session_id) = state.session_id.clone() else {
                state.notice = Some("No open session to rename".to_string());
                return;
            };
            match rename_session(api, &session_id, &name).await {
                Ok(()) => {
                    state.session_title = name.clone();
                    state.notice = Some(format!("Renamed to {name}"));
                }
                Err(err) => state.notice = Some(err),
            }
        }
        SessionCommand::Delete { session_id } => {
            let Some(id) = session_id.or_else(|| state.session_id.clone()) else {
                state.notice = Some("No session to delete".to_string());
                return;
            };
            match delete_session(api, &id).await {
                Ok(()) => {
                    if state.session_id.as_deref() == Some(id.as_str()) {
                        state.session_id = None;
                        state.session_title = "New Session".to_string();
                        state.transcript.clear();
                        state.pending_approval = None;
                        state.pending_prompt = None;
                        state.status = "Session deleted".to_string();
                    }
                    state.notice = Some(format!("Deleted {id}"));
                }
                Err(err) => state.notice = Some(err),
            }
        }
    }
}

pub(super) async fn switch_to_session(
    api: &SharedBackendClient,
    state: &mut ChatState,
    session_id: &str,
) {
    if let Err(err) = open_session_or_refuse(api, session_id).await {
        state.notice = Some(err);
        return;
    }
    // Resume a paused workflow when the session is not already driven.
    if let Ok(session) = api.get_session(session_id).await {
        let status = session
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if matches!(status, "paused" | "error") {
            if let Err(err) = api.resume_session(session_id).await {
                state.notice = Some(err);
                return;
            }
        }
        if let Some(name) = session.get("name").and_then(Value::as_str) {
            if !name.is_empty() {
                state.session_title = name.to_string();
            }
        }
    }
    state.session_id = Some(session_id.to_string());
    state.transcript.clear();
    state.pending_approval = None;
    state.pending_prompt = None;
    state.scroll_from_bottom = 0;
    state.notice = Some(format!("Opened {session_id}"));
    poll_session(api, state).await;
}

pub(super) async fn ensure_session(
    api: &SharedBackendClient,
    first_message: &str,
) -> Result<String, String> {
    let assistants = api.list_assistants().await?;
    let list = assistants
        .as_array()
        .or_else(|| assistants.get("assistants").and_then(Value::as_array))
        .ok_or_else(|| "Unexpected /api/assistants response".to_string())?;
    let assistant_id = list
        .first()
        .and_then(|row| row.get("id").or_else(|| row.get("assistantId")))
        .and_then(Value::as_str)
        .ok_or_else(|| "No assistant configured — open the GUI once to create one".to_string())?
        .to_string();

    let created = api
        .create_session(&json!({
            "assistantId": assistant_id,
            "request": first_message,
            "name": "TUI chat",
        }))
        .await?;
    created
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "create_session response missing id".to_string())
}

pub(super) async fn respond_approval(
    api: &SharedBackendClient,
    state: &mut ChatState,
    approval: &PendingApprovalSummary,
    approved: bool,
) {
    let Some(session_id) = state.session_id.clone() else {
        return;
    };
    let behavior = if approved { "allow" } else { "deny" };
    let body = json!({
        "toolCallId": approval.tool_call_id,
        "requestId": approval.request_id,
        "behavior": behavior,
    });
    match api.respond_permission(&session_id, &body).await {
        Ok(_) => {
            state.pending_approval = None;
            state.focus = Focus::Composer;
            state.s5_scroll = 0;
        }
        Err(err) => state.notice = Some(err),
    }
}

pub(super) async fn respond_user_prompt(
    api: &SharedBackendClient,
    state: &mut ChatState,
    request_id: &str,
    action: &str,
    value: Option<String>,
) {
    let Some(session_id) = state.session_id.clone() else {
        return;
    };
    let mut body = json!({
        "requestId": request_id,
        "action": action,
    });
    if let Some(value) = value {
        body["value"] = Value::String(value);
    }
    match api.respond_prompt(&session_id, &body).await {
        Ok(_) => {
            state.pending_prompt = None;
            state.focus = Focus::Composer;
            state.ask = AskDraft::default();
        }
        Err(err) => state.notice = Some(err),
    }
}
