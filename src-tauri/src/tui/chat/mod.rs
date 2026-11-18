//! TUI chat core: transcript, composer, streaming poll, and S5 prompts (#2762),
//! plus session slash commands (#2763).
//!
//! Drives the shared Session API from the TUI thread once the backend is live.
//! Layout and key rules follow `docs/specs/tui-parity.md` §3 (S1 + S2 + S5).
//!
//! Split by responsibility (#2886): `state` holds the transcript/composer state and
//! its transitions, `render` the pure layout and S5 rendering, `backend` the async
//! Session API calls, and this module the front loop that ties them together.
//! `run_chat_front` is the only entry point other modules call; everything else
//! stays inside `chat/`, so the submodules need no crate-wide re-exports.

use self::backend::{
    poll_session, respond_approval, respond_user_prompt, run_session_command, send_user_text,
    switch_to_session,
};
use self::render::draw_chat;
use self::state::{ChatAction, ChatState, POLL_EVERY};
use crate::server::shared_backend::SharedBackendClient;
use ratatui::prelude::*;
use std::io::Stdout;
use std::time::{Duration, Instant};

mod backend;
mod render;
mod state;

#[cfg(test)]
mod tests;

/// How the chat front should leave when it is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontExitKind {
    /// Backend already exposed an app handle — ask Tauri to exit (Exit restores).
    AppHandle,
    /// No handle yet — restore the terminal and exit the process.
    Process,
}

/// How long to wait for Tauri's graceful `RunEvent::Exit` before force-exiting.
pub const APP_HANDLE_EXIT_WAIT: Duration = Duration::from_secs(3);

/// Pick the exit path for the front thread. Pure so tests can cover G1.4 / G2.2.
pub fn decide_front_exit(has_app_handle: bool) -> FrontExitKind {
    if has_app_handle {
        FrontExitKind::AppHandle
    } else {
        FrontExitKind::Process
    }
}

/// True when the AppHandle path must wait for graceful Exit before `process::exit`.
pub fn app_handle_exit_waits(kind: FrontExitKind) -> bool {
    matches!(kind, FrontExitKind::AppHandle)
}

/// Request a clean exit. With an app handle, wait for `RunEvent::Exit` (restore +
/// unpublish) before falling back to `process::exit`.
fn leave_front() -> ! {
    let kind = decide_front_exit(crate::state::get_app_handle().is_some());
    if app_handle_exit_waits(kind) {
        if let Some(handle) = crate::state::get_app_handle() {
            handle.exit(0);
            // Let RunEvent::Exit restore the terminal and drop the backend
            // descriptor; only force-exit if that never arrives.
            std::thread::sleep(APP_HANDLE_EXIT_WAIT);
        }
    }
    crate::tui::restore_terminal();
    std::process::exit(0);
}

/// Run the chat UI on an already-prepared alternate screen.
pub fn run_chat_front(stdout: &mut Stdout) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("error: tokio runtime: {err}");
            leave_front();
        }
    };

    let mut terminal = match Terminal::new(CrosstermBackend::new(stdout)) {
        Ok(terminal) => terminal,
        Err(err) => {
            eprintln!("error: ratatui terminal: {err}");
            leave_front();
        }
    };

    let mut state = ChatState::new();
    let mut client: Option<SharedBackendClient> = None;
    let mut last_poll = Instant::now() - POLL_EVERY;

    loop {
        if client.is_none() {
            if let Some(descriptor) = crate::server::backend_descriptor::find_chat_backend() {
                match SharedBackendClient::new(descriptor.port) {
                    Ok(c) => {
                        state.status = format!("Backend on port {}", descriptor.port);
                        client = Some(c);
                    }
                    Err(err) => state.status = err,
                }
            }
        }

        if last_poll.elapsed() >= POLL_EVERY {
            last_poll = Instant::now();
            if let Some(api) = client.as_ref() {
                runtime.block_on(poll_session(api, &mut state));
            }
        }

        if terminal.draw(|frame| draw_chat(frame, &state)).is_err() {
            leave_front();
        }

        if crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() {
                if key.kind != crossterm::event::KeyEventKind::Press
                    && key.kind != crossterm::event::KeyEventKind::Repeat
                {
                    continue;
                }
                match state.handle_key(key) {
                    ChatAction::Quit => leave_front(),
                    ChatAction::Send(text) => {
                        if let Some(api) = client.as_ref() {
                            runtime.block_on(send_user_text(api, &mut state, &text));
                        } else {
                            state.notice = Some("Backend is not ready yet".to_string());
                        }
                    }
                    ChatAction::Session(command) => {
                        if let Some(api) = client.as_ref() {
                            runtime.block_on(run_session_command(api, &mut state, command));
                        } else {
                            state.notice = Some("Backend is not ready yet".to_string());
                        }
                    }
                    ChatAction::OpenSession(id) => {
                        if let Some(api) = client.as_ref() {
                            runtime.block_on(switch_to_session(api, &mut state, &id));
                        } else {
                            state.notice = Some("Backend is not ready yet".to_string());
                        }
                    }
                    ChatAction::Approve => {
                        if let (Some(api), Some(approval)) =
                            (client.as_ref(), state.pending_approval.clone())
                        {
                            runtime.block_on(respond_approval(api, &mut state, &approval, true));
                        }
                    }
                    ChatAction::Reject => {
                        if let (Some(api), Some(approval)) =
                            (client.as_ref(), state.pending_approval.clone())
                        {
                            runtime.block_on(respond_approval(api, &mut state, &approval, false));
                        }
                    }
                    ChatAction::AnswerPrompt(value) => {
                        if let (Some(api), Some(prompt)) =
                            (client.as_ref(), state.pending_prompt.clone())
                        {
                            runtime.block_on(respond_user_prompt(
                                api,
                                &mut state,
                                &prompt.request_id,
                                "answer",
                                Some(value),
                            ));
                        }
                    }
                    ChatAction::CancelPrompt => {
                        if let (Some(api), Some(prompt)) =
                            (client.as_ref(), state.pending_prompt.clone())
                        {
                            runtime.block_on(respond_user_prompt(
                                api,
                                &mut state,
                                &prompt.request_id,
                                "cancel",
                                None,
                            ));
                        }
                    }
                    ChatAction::None => {}
                }
            }
        }
    }
}
