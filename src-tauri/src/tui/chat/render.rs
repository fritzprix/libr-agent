//! Pure layout, transcript text, and the S5 strip renderer (#2886).

use super::state::{AskDraft, ChatState, Focus};
use crate::agent::session_manager::PendingApprovalSummary;
use crate::agent::state::{UserPromptPayload, UserQuestion};
use crate::tui::sessions::draw_picker;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde_json::Value;

/// Max rows the S5 strip may grow to (approval / askUser bodies scroll inside).
pub(super) const S5_MAX_ROWS: u16 = 8;

pub(super) fn current_ask_answer(
    question: &UserQuestion,
    draft: &AskDraft,
) -> Result<Value, String> {
    let text = draft.text.trim();
    match question.options.as_ref() {
        None => {
            if text.is_empty() {
                return Err("Enter an answer".to_string());
            }
            Ok(Value::String(text.to_string()))
        }
        Some(_) if question.multi_select => {
            let mut values = draft.selected.clone();
            if question.allow_custom && draft.other_on {
                if text.is_empty() {
                    return Err("Enter text for Other".to_string());
                }
                values.push(text.to_string());
            }
            if values.is_empty() {
                return Err("Select at least one option (Space)".to_string());
            }
            Ok(Value::Array(
                values.into_iter().map(Value::String).collect(),
            ))
        }
        Some(_) => {
            if question.allow_custom && draft.other_on {
                if text.is_empty() {
                    return Err("Enter text for Other".to_string());
                }
                return Ok(Value::String(text.to_string()));
            }
            draft
                .selected
                .first()
                .cloned()
                .map(Value::String)
                .ok_or_else(|| "Select an option (Space)".to_string())
        }
    }
}

pub(super) fn message_text(message: &Value) -> String {
    if let Some(text) = message.get("content").and_then(Value::as_str) {
        return text.to_string();
    }
    let Some(parts) = message.get("content").and_then(Value::as_array) else {
        return String::new();
    };
    parts
        .iter()
        .filter_map(|part| match part.get("type").and_then(Value::as_str) {
            Some("text") => part.get("text").and_then(Value::as_str).map(str::to_string),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Prefer a hard approval when several wait (parity §3.1 S5).
pub fn pick_pending_approval(list: Vec<PendingApprovalSummary>) -> Option<PendingApprovalSummary> {
    let hard = list.iter().find(|a| {
        matches!(
            a.approval_kind,
            crate::agent::state::PendingApprovalKind::Hard
        )
    });
    hard.cloned().or_else(|| list.into_iter().next())
}

/// Paragraph scroll offset that pins to the bottom when `from_bottom` is 0.
pub fn transcript_scroll_offset(content_rows: u16, view_rows: u16, from_bottom: u16) -> u16 {
    let max_scroll = content_rows.saturating_sub(view_rows);
    max_scroll.saturating_sub(from_bottom.min(max_scroll))
}

/// How many terminal rows `text` needs at `width`, counted by the very widget
/// that draws it (`Paragraph` + `Wrap { trim: false }`), so the count cannot
/// drift from the renderer on double-width text or word boundaries.
pub fn paragraph_row_count(text: &str, width: u16) -> u16 {
    Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .line_count(width.max(1))
        .try_into()
        .unwrap_or(u16::MAX)
}

pub(super) fn s5_body_lines(state: &ChatState) -> Vec<String> {
    if let Some(approval) = &state.pending_approval {
        let focus = if state.focus == Focus::Prompt {
            "focused · a/r or Alt+Y/N · ↑↓ scroll"
        } else {
            "Alt+Y approve · Alt+N reject · Tab to focus"
        };
        let kind = match approval.approval_kind {
            crate::agent::state::PendingApprovalKind::Hard => "Hard approve",
            crate::agent::state::PendingApprovalKind::Standard => "Approve",
        };
        let mut lines = vec![format!("{kind} {}?", approval.tool_name)];
        for line in approval.command_preview.lines() {
            lines.push(line.to_string());
        }
        if approval.command_preview.is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("[{focus}]"));
        return lines;
    }
    if let Some(prompt) = &state.pending_prompt {
        return match &prompt.payload {
            UserPromptPayload::Secret { label, .. } => {
                let masked: String = std::iter::repeat_n('*', state.prompt_input.len()).collect();
                vec![format!(
                    "Secret: {label}  [{masked}]  Enter submit · Esc cancel"
                )]
            }
            UserPromptPayload::Question { questions } => ask_user_lines(state, questions),
        };
    }
    Vec::new()
}

pub(super) fn ask_user_lines(state: &ChatState, questions: &[UserQuestion]) -> Vec<String> {
    if questions.is_empty() {
        return vec!["Ask: (no questions)".to_string()];
    }
    let index = state.ask.index.min(questions.len() - 1);
    let question = &questions[index];
    let mut lines = vec![format!(
        "Ask ({}/{}): {}",
        index + 1,
        questions.len(),
        question.question
    )];
    if let Some(desc) = question.description.as_deref() {
        lines.push(desc.to_string());
    }
    if let Some(options) = question.options.as_ref() {
        for (i, opt) in options.iter().enumerate() {
            let marked = state.ask.selected.iter().any(|v| v == opt);
            let cursor = if state.ask.cursor == i { ">" } else { " " };
            let box_mark = if question.multi_select {
                if marked {
                    "[x]"
                } else {
                    "[ ]"
                }
            } else if marked {
                "(•)"
            } else {
                "( )"
            };
            lines.push(format!("{cursor} {box_mark} {opt}"));
        }
        if question.allow_custom {
            let cursor = if state.ask.cursor == options.len() {
                ">"
            } else {
                " "
            };
            let mark = if state.ask.other_on { "[x]" } else { "[ ]" };
            lines.push(format!("{cursor} {mark} Other: {}", state.ask.text));
        }
    } else {
        lines.push(format!("> {}", state.ask.text));
    }
    let hint = if index + 1 < questions.len() {
        "Space toggle · Enter next · ↑↓"
    } else {
        "Space toggle · Enter submit · ↑↓"
    };
    lines.push(format!("[{hint}]"));
    lines
}

/// S5 strip height and body text for `width`, with a clip marker when needed.
pub fn s5_layout(state: &ChatState, width: u16) -> (u16, String) {
    if !state.has_s5() {
        return (0, String::new());
    }
    let body = s5_body_lines(state).join("\n");
    let total = paragraph_row_count(&body, width);
    if total <= S5_MAX_ROWS {
        return (total.clamp(1, S5_MAX_ROWS), body);
    }
    // Reserve the last row for the marker; content scrolls in the rest.
    let content_rows = S5_MAX_ROWS.saturating_sub(1).max(1);
    let hidden = total.saturating_sub(content_rows);
    let marker = format!("… {hidden} more rows (Tab, ↑↓)");
    (S5_MAX_ROWS, format!("{body}\n{marker}"))
}

/// Draw the S1 chat frame (and S5 strip when a prompt is open).
pub fn draw_chat(frame: &mut Frame<'_>, state: &ChatState) {
    if let Some(picker) = &state.picker {
        draw_picker(frame, picker);
        return;
    }

    let width = frame.area().width.max(1);
    let (s5_rows, s5_text) = s5_layout(state, width);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(s5_rows),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let header = format!(" {} ", state.session_title);
    frame.render_widget(
        Paragraph::new(header).block(Block::default().borders(Borders::BOTTOM)),
        chunks[0],
    );

    let body: String = state
        .transcript
        .iter()
        .map(|line| format!("{}  {}", line.role, line.text))
        .collect::<Vec<_>>()
        .join("\n");
    let content_rows = paragraph_row_count(&body, chunks[1].width.max(1));
    let view_rows = chunks[1].height.max(1);
    let scroll_y = transcript_scroll_offset(content_rows, view_rows, state.scroll_from_bottom);
    frame.render_widget(
        Paragraph::new(body)
            .wrap(Wrap { trim: false })
            .scroll((scroll_y, 0))
            .block(Block::default().borders(Borders::NONE)),
        chunks[1],
    );

    if s5_rows > 0 {
        let clipped = s5_text.contains("more rows (Tab");
        if clipped {
            let (content, marker) = s5_text.rsplit_once('\n').unwrap_or((s5_text.as_str(), ""));
            let content_height = s5_rows.saturating_sub(1).max(1);
            let s5_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(content_height), Constraint::Length(1)])
                .split(chunks[2]);
            frame.render_widget(
                Paragraph::new(content)
                    .wrap(Wrap { trim: false })
                    .scroll((state.s5_scroll, 0)),
                s5_chunks[0],
            );
            frame.render_widget(Paragraph::new(marker), s5_chunks[1]);
        } else {
            frame.render_widget(
                Paragraph::new(s5_text)
                    .wrap(Wrap { trim: false })
                    .scroll((state.s5_scroll, 0)),
                chunks[2],
            );
        }
    }

    let composer_title = if state.focus == Focus::Composer {
        " composer "
    } else {
        " composer (Tab: prompt) "
    };
    frame.render_widget(
        Paragraph::new(format!("> {}", state.composer))
            .block(Block::default().borders(Borders::ALL).title(composer_title)),
        chunks[3],
    );

    let status = state.notice.as_deref().unwrap_or(state.status.as_str());
    frame.render_widget(Paragraph::new(format!(" {status}")), chunks[4]);
}
