//! Durable last-N external user requests for post-compaction intent tracking.
//!
//! Appended into the compact summary as a deterministic SSOT block so resume
//! always sees recent user intent — not only the summarizer's Active Request.

use crate::agent::compaction_text::sanitize_compaction_semantic_text;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;

use super::instruction::truncate_instruction_text;

/// How many distinct external user turns to keep in the ledger.
pub const RECENT_USER_REQUEST_LIMIT: usize = 5;
/// Character cap per request bullet in the ledger / Working Intent fallback.
pub const RECENT_USER_REQUEST_TEXT_LIMIT: usize = 200;

const RECENT_USER_REQUESTS_HEADING: &str = "Recent User Requests";
const WORKING_INTENT_HEADING: &str = "Working Intent";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentUserRequest {
    pub message_id: String,
    pub text: String,
}

fn extract_external_request_text(message: &Message) -> Option<String> {
    let mut fragments = Vec::new();
    for content in &message.content {
        if let MCPContent::Text { text, .. } = content {
            let sanitized = sanitize_compaction_semantic_text(text);
            let trimmed = sanitized.trim();
            if !trimmed.is_empty() {
                fragments.push(trimmed.to_string());
            }
        }
    }

    if fragments.is_empty() {
        None
    } else {
        Some(fragments.join(" "))
    }
}

/// Collect up to `limit` most recent external user request turns (oldest → newest).
pub fn collect_recent_external_user_requests(
    messages: &[Message],
    limit: usize,
) -> Vec<RecentUserRequest> {
    if limit == 0 {
        return Vec::new();
    }

    let mut collected = Vec::new();
    for message in messages.iter().rev() {
        if !message.is_external_request_message() {
            continue;
        }
        let Some(text) = extract_external_request_text(message) else {
            continue;
        };
        collected.push(RecentUserRequest {
            message_id: message.id.clone(),
            text: truncate_instruction_text(&text, RECENT_USER_REQUEST_TEXT_LIMIT),
        });
        if collected.len() >= limit {
            break;
        }
    }

    collected.reverse();
    collected
}

pub fn format_recent_user_requests_section(requests: &[RecentUserRequest]) -> String {
    let mut lines = vec![format!("### {}", RECENT_USER_REQUESTS_HEADING)];
    if requests.is_empty() {
        lines.push("- None".to_string());
    } else {
        for request in requests {
            lines.push(format!("- [`{}`] {}", request.message_id, request.text));
        }
    }
    lines.join("\n")
}

fn format_working_intent_section(requests: &[RecentUserRequest]) -> String {
    let mut lines = vec![format!("### {}", WORKING_INTENT_HEADING)];
    if let Some(latest) = requests.last() {
        lines.push(format!("- {}", latest.text));
    } else {
        lines.push("- None".to_string());
    }
    lines.join("\n")
}

fn normalize_heading_title(candidate: &str) -> &str {
    candidate.trim_end_matches(':').trim()
}

fn is_thematic_break(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    // Common markdown horizontal rules; treat as section boundaries so
    // strip_section does not swallow the --- before Context recovery.
    (trimmed.chars().all(|c| c == '-')
        || trimmed.chars().all(|c| c == '*')
        || trimmed.chars().all(|c| c == '_'))
        && trimmed.len() >= 3
}

fn section_heading_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix('#') {
        let candidate = normalize_heading_title(rest.trim_start_matches('#').trim());
        if candidate.is_empty() {
            None
        } else {
            Some(candidate)
        }
    } else if !trimmed.starts_with("- ") && !trimmed.starts_with("* ") && trimmed.ends_with(':') {
        let candidate = normalize_heading_title(trimmed.trim_end_matches(':').trim());
        if candidate.is_empty() {
            None
        } else {
            Some(candidate)
        }
    } else {
        None
    }
}

/// Remove a markdown section by heading title (keeps surrounding content).
fn strip_section(summary: &str, section_heading: &str) -> String {
    let mut kept = Vec::new();
    let mut skipping = false;

    for line in summary.lines() {
        if let Some(heading) = section_heading_line(line) {
            if heading == section_heading {
                skipping = true;
                continue;
            }
            skipping = false;
        } else if skipping && is_thematic_break(line) {
            // Keep `---` / `***` dividers; they bound Context recovery.
            skipping = false;
        }

        if skipping {
            continue;
        }
        kept.push(line);
    }

    kept.join("\n").trim().to_string()
}

fn has_section(summary: &str, section_heading: &str) -> bool {
    summary
        .lines()
        .any(|line| section_heading_line(line).is_some_and(|heading| heading == section_heading))
}

/// Returns `(split_before_divider_or_heading, index_of_###_Context_recovery)`.
/// When a `---` divider precedes Context recovery, `split` is before that divider
/// and the divider is re-emitted in canonical LF form.
fn find_context_recovery_split(summary: &str) -> Option<(usize, usize)> {
    const HEADING: &str = "### Context recovery";
    for prefix in ["\r\n---\r\n", "\n---\n", "\r\n---\n", "\n---\r\n"] {
        let marker = format!("{}{}", prefix, HEADING);
        if let Some(idx) = summary.find(&marker) {
            return Some((idx, idx + prefix.len()));
        }
    }
    summary.find(HEADING).map(|idx| (idx, idx))
}

fn insert_before_context_recovery(summary: &str, block: &str) -> String {
    let block = block.trim();
    if block.is_empty() {
        return summary.trim().to_string();
    }

    if let Some((split_idx, recovery_heading_idx)) = find_context_recovery_split(summary) {
        let head = summary[..split_idx].trim_end();
        let tail = &summary[recovery_heading_idx..];
        if split_idx < recovery_heading_idx {
            // Divider was present (possibly CRLF); re-emit canonical LF ---.
            return format!("{}\n\n{}\n\n---\n{}", head, block, tail);
        }
        return format!("{}\n\n{}\n\n{}", head, block, tail.trim_start());
    }
    if summary.trim().is_empty() {
        block.to_string()
    } else {
        format!("{}\n\n{}", summary.trim_end(), block)
    }
}

/// Ensure compact summary includes deterministic Recent User Requests and a
/// Working Intent section (LLM-authored if present; otherwise derived from last-N).
pub fn ensure_intent_sections_in_summary(summary: &str, messages: &[Message]) -> String {
    let requests = collect_recent_external_user_requests(messages, RECENT_USER_REQUEST_LIMIT);
    let recent_block = format_recent_user_requests_section(&requests);

    let body = strip_section(summary, RECENT_USER_REQUESTS_HEADING);

    let working_intent_block = if has_section(&body, WORKING_INTENT_HEADING) {
        None
    } else {
        Some(format_working_intent_section(&requests))
    };

    let mut injected = recent_block;
    if let Some(working_intent) = working_intent_block {
        injected = format!("{}\n\n{}", working_intent, injected);
    }

    insert_before_context_recovery(&body, &injected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chat::MessageSource;

    fn external_user(id: &str, text: &str) -> Message {
        let mut message =
            Message::new_user_message("session".to_string(), text.to_string(), None, None);
        message.id = id.to_string();
        message.source = Some(MessageSource::Ui);
        message
    }

    #[test]
    fn collect_recent_external_user_requests_keeps_last_n_oldest_to_newest() {
        let messages = vec![
            external_user("u1", "First ask"),
            Message::new_user_message(
                "session".to_string(),
                "assistant reply".to_string(),
                None,
                None,
            ),
            external_user("u2", "Second ask"),
            external_user("u3", "Third ask"),
            external_user("u4", "Fourth ask"),
        ];
        // Make the middle "assistant" role so it is skipped.
        let mut messages = messages;
        messages[1].role = "assistant".to_string();

        let collected = collect_recent_external_user_requests(&messages, 3);
        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0].message_id, "u2");
        assert_eq!(collected[1].message_id, "u3");
        assert_eq!(collected[2].message_id, "u4");
        assert_eq!(collected[2].text, "Fourth ask");
    }

    #[test]
    fn ensure_intent_sections_replaces_recent_requests_and_adds_working_intent() {
        let messages = vec![
            external_user("u1", "Ship the release notes"),
            external_user("u2", "Also fix the changelog link"),
        ];
        let summary = "### Active Request\n- None\n\n### Next Actions\n- Stop\n\n### Recent User Requests\n- stale";

        let ensured = ensure_intent_sections_in_summary(summary, &messages);

        assert!(ensured.contains("### Working Intent"));
        assert!(ensured.contains("Also fix the changelog link"));
        assert!(ensured.contains("### Recent User Requests"));
        assert!(ensured.contains("[`u1`] Ship the release notes"));
        assert!(ensured.contains("[`u2`] Also fix the changelog link"));
        assert!(!ensured.contains("- stale"));
        assert!(ensured.contains("### Active Request"));
    }

    #[test]
    fn ensure_intent_sections_preserves_llm_working_intent() {
        let messages = vec![external_user("u1", "Write the report")];
        let summary = "### Working Intent\n- Deliver the final report with sources\n\n### Active Request\n- Write the report";

        let ensured = ensure_intent_sections_in_summary(summary, &messages);

        assert!(ensured.contains("Deliver the final report with sources"));
        assert_eq!(
            ensured.matches("### Working Intent").count(),
            1,
            "must not duplicate Working Intent"
        );
    }

    #[test]
    fn ensure_intent_sections_inserts_before_context_recovery() {
        let messages = vec![external_user("u1", "Recover me")];
        let summary = "### Active Request\n- Keep going\n\n---\n### Context recovery\nread `.libragent/pre_compaction_epoch_1.md`";

        let ensured = ensure_intent_sections_in_summary(summary, &messages);

        let recent_idx = ensured.find("### Recent User Requests").expect("recent");
        let recovery_idx = ensured.find("### Context recovery").expect("recovery");
        assert!(recent_idx < recovery_idx);
        assert!(ensured.contains("[`u1`] Recover me"));
        assert!(
            ensured.contains("---\n### Context recovery"),
            "must keep thematic-break divider before Context recovery"
        );
    }

    #[test]
    fn ensure_intent_sections_recognizes_working_intent_with_trailing_colon() {
        let messages = vec![external_user("u1", "Write the report")];
        let summary = "### Working Intent:\n- Deliver the final report with sources\n\n### Active Request\n- Write the report";

        let ensured = ensure_intent_sections_in_summary(summary, &messages);

        assert_eq!(
            ensured.matches("### Working Intent").count(),
            1,
            "colon-suffixed Working Intent heading must not cause duplicate injection"
        );
        assert!(ensured.contains("Deliver the final report with sources"));
    }

    #[test]
    fn ensure_intent_sections_preserves_divider_when_replacing_stale_recent_requests() {
        let messages = vec![external_user("u1", "Recover me")];
        let summary = "### Active Request\n- Keep going\n\n### Recent User Requests\n- stale\n\n---\n### Context recovery\nepoch";

        let ensured = ensure_intent_sections_in_summary(summary, &messages);

        assert!(!ensured.contains("- stale"));
        assert!(ensured.contains("[`u1`] Recover me"));
        let recent_idx = ensured.find("### Recent User Requests").expect("recent");
        let divider_idx = ensured
            .find("\n---\n### Context recovery")
            .expect("divider");
        assert!(recent_idx < divider_idx);
    }

    #[test]
    fn ensure_intent_sections_handles_crlf_context_recovery_divider() {
        let messages = vec![external_user("u1", "Recover me")];
        let summary =
            "### Active Request\r\n- Keep going\r\n\r\n---\r\n### Context recovery\r\nepoch";

        let ensured = ensure_intent_sections_in_summary(summary, &messages);

        let recent_idx = ensured.find("### Recent User Requests").expect("recent");
        let recovery_idx = ensured.find("### Context recovery").expect("recovery");
        assert!(recent_idx < recovery_idx);
        assert!(ensured.contains("---\n### Context recovery"));
    }
}
