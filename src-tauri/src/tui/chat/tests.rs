//! Unit tests for the TUI chat front (#2886).

use super::render::S5_MAX_ROWS;
use super::*;
use crate::agent::session_manager::PendingApprovalSummary;
use crate::agent::state::{
    PendingApprovalKind, PendingUserPromptSummary, UserPromptKind, UserPromptPayload, UserQuestion,
};
use crate::tui::sessions::SessionCommand;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use serde_json::{json, Value};

fn approval() -> PendingApprovalSummary {
    PendingApprovalSummary {
        tool_call_id: "call_1".to_string(),
        tool_name: "workspace__runPowerShell".to_string(),
        reason: "run".to_string(),
        command_preview: "pnpm test".to_string(),
        approval_kind: PendingApprovalKind::Standard,
        request_id: Some("req_1".to_string()),
    }
}

fn hard_approval() -> PendingApprovalSummary {
    PendingApprovalSummary {
        tool_call_id: "call_hard".to_string(),
        tool_name: "workspace__runDangerous".to_string(),
        reason: "hard".to_string(),
        command_preview: "rm -rf /".to_string(),
        approval_kind: PendingApprovalKind::Hard,
        request_id: Some("req_hard".to_string()),
    }
}

fn question_prompt(questions: Vec<UserQuestion>) -> PendingUserPromptSummary {
    PendingUserPromptSummary {
        request_id: "prompt_1".to_string(),
        kind: UserPromptKind::Question,
        payload: UserPromptPayload::Question { questions },
        created_at: chrono::Utc::now(),
    }
}

#[test]
fn plain_enter_sends_composer_text() {
    let mut state = ChatState::new();
    state.composer = "hello world".to_string();
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Enter)),
        ChatAction::Send("hello world".to_string())
    );
    assert!(state.composer.is_empty());
}

#[test]
fn slash_slash_sends_a_literal_leading_slash() {
    let mut state = ChatState::new();
    state.composer = "//not-a-command".to_string();
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Enter)),
        ChatAction::Send("/not-a-command".to_string())
    );
}

#[test]
fn unknown_slash_command_is_not_sent() {
    let mut state = ChatState::new();
    state.composer = "/foo".to_string();
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Enter)),
        ChatAction::None
    );
    assert!(state.notice.as_deref().unwrap_or("").contains("/foo"));
}

#[test]
fn slash_new_emits_session_command() {
    let mut state = ChatState::new();
    state.composer = "/new".to_string();
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Enter)),
        ChatAction::Session(SessionCommand::New { assistant_id: None })
    );
    assert!(state.composer.is_empty());
}

#[test]
fn slash_delete_asks_for_confirm_on_open_session() {
    let mut state = ChatState::new();
    state.session_id = Some("sess_1".into());
    state.composer = "/delete".to_string();
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Enter)),
        ChatAction::None
    );
    assert_eq!(state.confirm_delete.as_deref(), Some("sess_1"));
    assert!(state.notice.as_deref().unwrap_or("").contains("children"));
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Char('y'))),
        ChatAction::Session(SessionCommand::Delete {
            session_id: Some("sess_1".into())
        })
    );
}

#[test]
fn slash_delete_with_id_also_asks_for_confirm_and_n_cancels() {
    let mut state = ChatState::new();
    state.composer = "/delete other_sess".to_string();
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Enter)),
        ChatAction::None
    );
    assert_eq!(state.confirm_delete.as_deref(), Some("other_sess"));
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Char('n'))),
        ChatAction::None
    );
    assert!(state.confirm_delete.is_none());
    assert!(state.notice.as_deref().unwrap_or("").contains("cancelled"));
}

#[test]
fn slash_rename_without_name_shows_usage() {
    let mut state = ChatState::new();
    state.composer = "/rename".to_string();
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Enter)),
        ChatAction::None
    );
    assert!(state
        .notice
        .as_deref()
        .unwrap_or("")
        .contains("/rename <name>"));
}

#[test]
fn alt_y_approves_without_prompt_focus() {
    let mut state = ChatState::new();
    state.pending_approval = Some(approval());
    state.focus = Focus::Composer;
    let mut key = KeyEvent::from(KeyCode::Char('y'));
    key.modifiers = KeyModifiers::ALT;
    assert_eq!(state.handle_key(key), ChatAction::Approve);
}

#[test]
fn bare_a_does_not_approve_while_composer_focused() {
    let mut state = ChatState::new();
    state.pending_approval = Some(approval());
    state.focus = Focus::Composer;
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Char('a'))),
        ChatAction::None
    );
    assert_eq!(state.composer, "a");
}

#[test]
fn tab_then_a_approves() {
    let mut state = ChatState::new();
    state.pending_approval = Some(approval());
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Tab)),
        ChatAction::None
    );
    assert_eq!(state.focus, Focus::Prompt);
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Char('a'))),
        ChatAction::Approve
    );
}

#[test]
fn apply_messages_reverses_api_newest_first_order() {
    let mut state = ChatState::new();
    let messages = vec![
        json!({"role": "assistant", "content": "second"}),
        json!({"role": "user", "content": "first"}),
    ];
    state.apply_messages(&messages);
    assert_eq!(
        state.transcript,
        vec![
            TranscriptLine {
                role: "You".to_string(),
                text: "first".to_string(),
            },
            TranscriptLine {
                role: "AI".to_string(),
                text: "second".to_string(),
            },
        ]
    );
}

#[test]
fn pick_pending_approval_prefers_hard() {
    let picked = pick_pending_approval(vec![approval(), hard_approval()]).unwrap();
    assert_eq!(picked.tool_call_id, "call_hard");
    assert_eq!(picked.approval_kind, PendingApprovalKind::Hard);
}

#[test]
fn transcript_scroll_follows_tail_and_up_moves_to_older() {
    // 10 content rows in a 4-row view → max scroll 6; from_bottom 0 pins to end.
    assert_eq!(transcript_scroll_offset(10, 4, 0), 6);
    assert_eq!(transcript_scroll_offset(10, 4, 2), 4);
    assert_eq!(transcript_scroll_offset(10, 4, 100), 0);

    let mut state = ChatState::new();
    state.handle_key(KeyEvent::from(KeyCode::Up));
    assert_eq!(state.scroll_from_bottom, 1);
    state.handle_key(KeyEvent::from(KeyCode::Up));
    assert_eq!(state.scroll_from_bottom, 2);
    state.handle_key(KeyEvent::from(KeyCode::Down));
    assert_eq!(state.scroll_from_bottom, 1);
    state.handle_key(KeyEvent::from(KeyCode::End));
    assert_eq!(state.scroll_from_bottom, 0);
}

#[test]
fn decide_front_exit_uses_process_without_app_handle() {
    assert_eq!(decide_front_exit(false), FrontExitKind::Process);
    assert!(!app_handle_exit_waits(FrontExitKind::Process));
    assert_eq!(decide_front_exit(true), FrontExitKind::AppHandle);
    assert!(app_handle_exit_waits(FrontExitKind::AppHandle));
    assert!(APP_HANDLE_EXIT_WAIT > Duration::ZERO);
}

#[test]
fn focus_defaults_to_composer() {
    assert_eq!(Focus::default(), Focus::Composer);
}

#[test]
fn paragraph_row_count_counts_display_width_like_the_renderer() {
    // Double-width text: 204 Korean chars are 408 columns, so 80 columns
    // fit 40 per row — six rows, where a Unicode scalar count saw three.
    assert_eq!(paragraph_row_count(&"가".repeat(204), 80), 6);
    // ASCII keeps the previous counts.
    assert_eq!(paragraph_row_count(&"x".repeat(120), 80), 2);
    assert_eq!(paragraph_row_count(&"x".repeat(400), 80), 5);
    assert_eq!(paragraph_row_count("", 80), 1);
}

#[test]
fn s5_layout_keeps_clip_marker_for_long_approval() {
    // Header + footer + ~10 wrapped command rows at width 80 → over S5_MAX_ROWS.
    let long = "x".repeat(800);
    let mut state = ChatState::new();
    state.pending_approval = Some(PendingApprovalSummary {
        command_preview: long,
        ..approval()
    });
    let (height, text) = s5_layout(&state, 80);
    assert_eq!(height, S5_MAX_ROWS);
    assert!(
        text.contains("more rows (Tab"),
        "clipped S5 must show a scroll hint, got: {text}"
    );
    assert!(
        text.contains("Approve workspace__runPowerShell?"),
        "header stays in the body"
    );
}

#[test]
fn draw_chat_shows_full_long_approval_preview() {
    let long = "x".repeat(120);
    let mut state = ChatState::new();
    state.pending_approval = Some(PendingApprovalSummary {
        command_preview: long.clone(),
        ..approval()
    });
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_chat(frame, &state))
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let rendered: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(
        rendered.contains(&long),
        "full command preview must appear without truncation"
    );
}

#[test]
fn draw_chat_keeps_marker_and_newest_reply_visible() {
    let long = "x".repeat(800);
    let mut state = ChatState::new();
    state.pending_approval = Some(PendingApprovalSummary {
        command_preview: long,
        ..approval()
    });
    // One logical line that wraps to many rows — follow-tail must still
    // land on the newest content.
    state.transcript = vec![TranscriptLine {
        role: "AI".to_string(),
        text: format!("{}\nTAIL", "paragraph ".repeat(40)),
    }];
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_chat(frame, &state))
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let rendered: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(
        rendered.contains("more rows (Tab"),
        "S5 clip marker must stay on screen"
    );
    assert!(
        rendered.contains("TAIL"),
        "follow-tail must keep the newest transcript row visible"
    );
}

#[test]
fn draw_chat_keeps_newest_korean_reply_and_s5_footer_visible() {
    // Double-width text without spaces (the shipped ko locale): scalar
    // counting would miss the clip marker entirely and follow-tail would
    // stop short of the newest rows.
    let mut state = ChatState::new();
    state.pending_approval = Some(PendingApprovalSummary {
        command_preview: "한국어".repeat(100),
        ..approval()
    });
    state.transcript = vec![TranscriptLine {
        role: "AI".to_string(),
        text: format!("{}\nTAIL", "한국어 ".repeat(600)),
    }];
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_chat(frame, &state))
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let rendered: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(
        rendered.contains("more rows (Tab"),
        "double-width approval body must be recognized as clipped"
    );
    assert!(
        rendered.contains("TAIL"),
        "follow-tail must keep the newest Korean row visible"
    );
}

#[test]
fn ask_user_single_choice_submits_json_object() {
    let mut state = ChatState::new();
    state.pending_prompt = Some(question_prompt(vec![UserQuestion {
        id: "lang".to_string(),
        question: "Language?".to_string(),
        description: None,
        options: Some(vec!["Rust".to_string(), "TS".to_string()]),
        multi_select: false,
        allow_custom: false,
    }]));
    state.sync_ask_with_prompt();
    state.focus = Focus::Prompt;
    state.handle_key(KeyEvent::from(KeyCode::Char(' ')));
    let action = state.handle_key(KeyEvent::from(KeyCode::Enter));
    match action {
        ChatAction::AnswerPrompt(raw) => {
            let parsed: Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(parsed["lang"], json!("Rust"));
        }
        other => panic!("expected AnswerPrompt, got {other:?}"),
    }
}

#[test]
fn ask_user_multi_select_toggles_and_submits_array() {
    let mut state = ChatState::new();
    state.pending_prompt = Some(question_prompt(vec![UserQuestion {
        id: "targets".to_string(),
        question: "Targets?".to_string(),
        description: None,
        options: Some(vec![
            "linux".to_string(),
            "mac".to_string(),
            "win".to_string(),
        ]),
        multi_select: true,
        allow_custom: false,
    }]));
    state.sync_ask_with_prompt();
    state.focus = Focus::Prompt;
    state.handle_key(KeyEvent::from(KeyCode::Char(' '))); // linux
    state.handle_key(KeyEvent::from(KeyCode::Down));
    state.handle_key(KeyEvent::from(KeyCode::Char(' '))); // mac
    let action = state.handle_key(KeyEvent::from(KeyCode::Enter));
    match action {
        ChatAction::AnswerPrompt(raw) => {
            let parsed: Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(parsed["targets"], json!(["linux", "mac"]));
        }
        other => panic!("expected AnswerPrompt, got {other:?}"),
    }
}

#[test]
fn ask_user_two_questions_submit_together() {
    let mut state = ChatState::new();
    state.pending_prompt = Some(question_prompt(vec![
        UserQuestion {
            id: "name".to_string(),
            question: "Name?".to_string(),
            description: None,
            options: None,
            multi_select: false,
            allow_custom: false,
        },
        UserQuestion {
            id: "lang".to_string(),
            question: "Language?".to_string(),
            description: None,
            options: Some(vec!["Rust".to_string()]),
            multi_select: false,
            allow_custom: false,
        },
    ]));
    state.sync_ask_with_prompt();
    state.focus = Focus::Prompt;
    for ch in "demo".chars() {
        state.handle_key(KeyEvent::from(KeyCode::Char(ch)));
    }
    assert_eq!(
        state.handle_key(KeyEvent::from(KeyCode::Enter)),
        ChatAction::None
    );
    assert_eq!(state.ask.index, 1);
    state.handle_key(KeyEvent::from(KeyCode::Char(' ')));
    let action = state.handle_key(KeyEvent::from(KeyCode::Enter));
    match action {
        ChatAction::AnswerPrompt(raw) => {
            let parsed: Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(parsed["name"], json!("demo"));
            assert_eq!(parsed["lang"], json!("Rust"));
        }
        other => panic!("expected AnswerPrompt, got {other:?}"),
    }
}
