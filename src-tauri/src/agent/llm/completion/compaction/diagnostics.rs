use super::payload::{inspect_compaction_payload, CompactionPayloadDiagnostics};
use crate::agent::state::CompactionRecoveryPhase;
use crate::models::chat::Message;

fn format_compaction_payload_messages(diagnostics: &CompactionPayloadDiagnostics) -> String {
    if diagnostics.messages.is_empty() {
        return "  - <none>".to_string();
    }

    diagnostics
        .messages
        .iter()
        .map(|message| {
            format!(
                "  - {} | role={} | source={} | flags={} | preview={}",
                message.id,
                message.role,
                message.source,
                if message.flags.is_empty() {
                    "-".to_string()
                } else {
                    message.flags.join("+")
                },
                message.preview
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn log_compaction_input_diagnostics(
    session_id: &str,
    provider_id: &str,
    recovery_phase: CompactionRecoveryPhase,
    retry_attempt: u32,
    base_payload_message_count: usize,
    request_layout_message_count: usize,
    selected_messages: &[Message],
) {
    let diagnostics = inspect_compaction_payload(selected_messages);

    log::info!(
        "🧪 Compaction input diagnostics: session={}, provider={}, recovery_phase={:?}, retry_attempt={}, base_payload_message_count={}, request_layout_message_count={}, emitted_message_count={}, body_message_count={}, raw_delta_message_count={}, compact_summary_count={}, compaction_instruction_count={}, scaffolding_count={}, external_request_count={}, assistant_message_count={}, tool_message_count={}, latest_external_request_ids={:?}",
        session_id,
        provider_id,
        recovery_phase,
        retry_attempt,
        base_payload_message_count,
        request_layout_message_count,
        diagnostics.total_messages,
        diagnostics.body_message_count,
        diagnostics.raw_delta_message_count,
        diagnostics.compact_summary_count,
        diagnostics.compaction_instruction_count,
        diagnostics.scaffolding_count,
        diagnostics.external_request_count,
        diagnostics.assistant_message_count,
        diagnostics.tool_message_count,
        diagnostics.latest_external_request_message_ids
    );

    if diagnostics.external_request_count == 0 {
        log::warn!(
            "⚠️ Compaction input emitted without any external request messages: session={}, provider={}, recovery_phase={:?}, retry_attempt={}",
            session_id,
            provider_id,
            recovery_phase,
            retry_attempt
        );
    }

    if diagnostics.raw_delta_message_count == 1 && diagnostics.external_request_count == 1 {
        log::warn!(
            "⚠️ Compaction input collapsed to a single raw delta message around the latest request: session={}, provider={}, recovery_phase={:?}, retry_attempt={}",
            session_id,
            provider_id,
            recovery_phase,
            retry_attempt
        );
    }

    log::debug!(
        "🧾 Compaction input messages: session={}, provider={}, recovery_phase={:?}, retry_attempt={}, messages=\n{}",
        session_id,
        provider_id,
        recovery_phase,
        retry_attempt,
        format_compaction_payload_messages(&diagnostics)
    );
}

pub(super) fn log_preflight_split_boundary(
    session_id: &str,
    messages: &[Message],
    split_idx: usize,
    reason: &str,
) {
    let diagnostics =
        crate::agent::llm::context_selector::inspect_compaction_split_boundary(messages);
    let first_message_id = messages
        .first()
        .map(|message| message.id.as_str())
        .unwrap_or("?");
    let last_message_id = messages
        .last()
        .map(|message| message.id.as_str())
        .unwrap_or("?");
    let first_message_role = messages
        .first()
        .map(|message| message.role.as_str())
        .unwrap_or("?");
    let last_message_role = messages
        .last()
        .map(|message| message.role.as_str())
        .unwrap_or("?");

    log::warn!(
        "🧭 Preflight compaction split diagnostics: session={}, reason={}, message_count={}, split_idx={}, first_unresolved_owner_index={:?}, first_unresolved_owner_id={:?}, first_unresolved_tool_call_count={}, first_message_id={}, first_message_role={}, last_message_id={}, last_message_role={}",
        session_id,
        reason,
        messages.len(),
        split_idx,
        diagnostics.first_unresolved_owner_index,
        diagnostics.first_unresolved_owner_id,
        diagnostics.first_unresolved_tool_call_count,
        first_message_id,
        first_message_role,
        last_message_id,
        last_message_role
    );
}
