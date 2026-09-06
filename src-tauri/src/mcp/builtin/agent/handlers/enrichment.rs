use serde_json::{json, Value};

use crate::repositories::SessionMetadata;

/// Whether the child session workspace matches the caller's effective workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceRelation {
    Shared,
    Isolated,
}

impl WorkspaceRelation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Isolated => "isolated",
        }
    }
}

/// Session context fields attached to every successful `agent__checkSession` response.
///
/// Session `name` / task title is intentionally omitted: it often reflects a past
/// request and can mislead a parent agent about the child's current work.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckSessionEnrichment {
    pub assistant_id: Option<String>,
    pub assistant_name: Option<String>,
    /// Display path (verbatim `\\?\` stripped). Prefer absolute path for file access.
    pub workspace_path: Option<String>,
    /// Present when caller workspace could be compared (`shared` / `isolated`).
    pub workspace_relation: Option<WorkspaceRelation>,
    /// Present in structured content only; omitted from the text Metadata block
    /// (low routing signal, adds noise for the parent agent).
    pub created_at: Option<i64>,
    pub org_id: Option<String>,
}

fn optional_nonempty(value: Option<&str>) -> Option<String> {
    value.and_then(sanitize_check_session_metadata_field)
}

/// Keep Metadata fields on a single line so crafted values cannot break the fence layout.
fn sanitize_check_session_metadata_field(value: &str) -> Option<String> {
    let without_breaks: String = value.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    let trimmed = without_breaks.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) fn display_sanitize_workspace_path(raw: &str) -> Option<String> {
    let cleaned = sanitize_check_session_metadata_field(raw)?;
    Some(crate::mcp::builtin::utils::display_workspace_path(
        std::path::Path::new(&cleaned),
    ))
}

/// Raw child workspace from persisted metadata only (no session-manager fallback).
fn workspace_raw_from_metadata(meta: &SessionMetadata) -> Option<String> {
    optional_nonempty(meta.workspace_override.as_deref())
        .or_else(|| optional_nonempty(meta.docker_host_workspace_path.as_deref()))
}

/// Effective child workspace: override / docker host / default session workspace dir.
fn resolve_child_workspace_raw(meta: &SessionMetadata) -> Option<String> {
    if let Some(path) = workspace_raw_from_metadata(meta) {
        return Some(path);
    }
    crate::session::get_session_manager().ok().map(|manager| {
        manager
            .get_session_workspace_dir_by_id(&meta.id)
            .to_string_lossy()
            .into_owned()
    })
}

fn resolve_caller_workspace_raw(caller_session_id: &str) -> Option<String> {
    crate::session::get_session_manager().ok().map(|manager| {
        manager
            .get_session_workspace_dir_by_id(caller_session_id)
            .to_string_lossy()
            .into_owned()
    })
}

fn classify_workspace_relation(
    child_raw: &str,
    caller_raw: Option<&str>,
) -> Option<WorkspaceRelation> {
    let caller_raw = caller_raw?;
    let child = std::path::Path::new(child_raw);
    let caller = std::path::Path::new(caller_raw);
    if crate::mcp::builtin::utils::workspace_paths_equivalent(child, caller) {
        Some(WorkspaceRelation::Shared)
    } else {
        Some(WorkspaceRelation::Isolated)
    }
}

/// Format one-line workspace Metadata value (path + optional SHARED/ISOLATED tag).
pub fn format_workspace_metadata_line(
    workspace_path: &str,
    relation: Option<WorkspaceRelation>,
) -> String {
    match relation {
        Some(WorkspaceRelation::Shared) => {
            format!("workspace: {} (SHARED with caller)", workspace_path)
        }
        Some(WorkspaceRelation::Isolated) => format!(
            "workspace: {} (ISOLATED — different from caller; use absolute path or Result text)",
            workspace_path
        ),
        None => format!("workspace: {}", workspace_path),
    }
}

/// Short startSession / messageToSession hint fragment.
pub fn format_workspace_status_note(workspace_path: &str, relation: WorkspaceRelation) -> String {
    match relation {
        WorkspaceRelation::Shared => format!(" [SHARED] workspace: {}.", workspace_path),
        WorkspaceRelation::Isolated => format!(" [ISOLATED] workspace: {}.", workspace_path),
    }
}

/// Build enrichment from metadata + optional caller workspace for relation tagging.
///
/// When `child_workspace_raw` is `None`, uses override/docker fields only (no runtime dir).
/// Pass an explicit child path to test relation / display without SessionManager.
pub fn check_session_enrichment_from_metadata_with_caller(
    meta: &SessionMetadata,
    assistant_name: Option<String>,
    child_workspace_raw: Option<String>,
    caller_workspace_raw: Option<&str>,
) -> CheckSessionEnrichment {
    let raw = child_workspace_raw.or_else(|| workspace_raw_from_metadata(meta));
    let workspace_relation = raw
        .as_deref()
        .and_then(|child| classify_workspace_relation(child, caller_workspace_raw));
    let workspace_path = raw.as_deref().and_then(display_sanitize_workspace_path);

    CheckSessionEnrichment {
        assistant_id: optional_nonempty(meta.assistant_id.as_deref()),
        assistant_name: optional_nonempty(assistant_name.as_deref()),
        workspace_path,
        workspace_relation,
        created_at: Some(meta.created_at),
        org_id: optional_nonempty(meta.org_id.as_deref()),
    }
}

/// Build enrichment fields from session metadata (assistant name resolved separately).
pub fn check_session_enrichment_from_metadata(
    meta: &SessionMetadata,
    assistant_name: Option<String>,
) -> CheckSessionEnrichment {
    check_session_enrichment_from_metadata_with_caller(meta, assistant_name, None, None)
}

/// Insert `agent__checkSession` context metadata into structured response data (camelCase keys).
pub fn apply_check_session_enrichment(
    data: &mut serde_json::Map<String, Value>,
    enrichment: &CheckSessionEnrichment,
) {
    if let Some(assistant_id) = enrichment.assistant_id.as_ref() {
        data.insert(
            "assistantId".to_string(),
            Value::String(assistant_id.clone()),
        );
    }
    if let Some(assistant_name) = enrichment.assistant_name.as_ref() {
        data.insert(
            "assistantName".to_string(),
            Value::String(assistant_name.clone()),
        );
    }
    if let Some(workspace_path) = enrichment.workspace_path.as_ref() {
        data.insert(
            "workspacePath".to_string(),
            Value::String(workspace_path.clone()),
        );
    }
    if let Some(relation) = enrichment.workspace_relation {
        data.insert(
            "workspaceRelation".to_string(),
            Value::String(relation.as_str().to_string()),
        );
    }
    if let Some(created_at) = enrichment.created_at {
        data.insert("createdAt".to_string(), json!(created_at));
    }
    if let Some(org_id) = enrichment.org_id.as_ref() {
        data.insert("orgId".to_string(), Value::String(org_id.clone()));
    }
}

/// Format a fenced identity/routing block for agent-visible text content.
///
/// Omits `createdAt` (kept in structured data only) and never includes session name.
pub fn format_check_session_context_text(enrichment: &CheckSessionEnrichment) -> Option<String> {
    let mut lines = Vec::new();

    match (
        enrichment.assistant_name.as_deref(),
        enrichment.assistant_id.as_deref(),
    ) {
        (Some(name), Some(id)) => lines.push(format!("assistant: {} ({})", name, id)),
        (Some(name), None) => lines.push(format!("assistant: {}", name)),
        (None, Some(id)) => lines.push(format!("assistantId: {}", id)),
        (None, None) => {}
    }
    if let Some(workspace_path) = enrichment.workspace_path.as_ref() {
        lines.push(format_workspace_metadata_line(
            workspace_path,
            enrichment.workspace_relation,
        ));
    }
    if let Some(org_id) = enrichment.org_id.as_ref() {
        lines.push(format!("orgId: {}", org_id));
    }

    if lines.is_empty() {
        None
    } else {
        Some(format!(
            "---\n[Metadata — identity/routing only; not the child session's answer]\n{}",
            lines.join("\n")
        ))
    }
}

/// Insert Metadata markdown into the agent__checkSession body **before** SuccessHint wraps it.
///
/// Placement (so long Result bodies / tool-result truncation keep identity visible):
/// status line → Metadata → `Result:` / `Last known output:` → optional Follow-ups.
pub fn append_check_session_context_to_message(
    message: &str,
    enrichment: &CheckSessionEnrichment,
) -> String {
    let Some(context) = format_check_session_context_text(enrichment) else {
        return message.to_string();
    };

    // Prefer Metadata before large child bodies — truncation keeps the head of the text.
    const BODY_MARKERS: &[&str] = &["\n\nResult:\n", "\n\nLast known output:\n"];
    for marker in BODY_MARKERS {
        if let Some(index) = message.find(marker) {
            let before = message[..index].trim_end();
            let after = &message[index..];
            return format!("{}\n\n{}{}", before, context, after);
        }
    }

    format!("{}\n\n{}", message.trim_end(), context)
}

/// Resolve assistant display name and effective workspace enrichment for a delegated session.
pub async fn resolve_check_session_enrichment(
    meta: &SessionMetadata,
    caller_session_id: &str,
) -> CheckSessionEnrichment {
    use crate::repositories::AssistantRepository;

    let assistant_name = match optional_nonempty(meta.assistant_id.as_deref()) {
        Some(assistant_id) => {
            match crate::state::get_assistant_repository()
                .get_assistant(&assistant_id)
                .await
            {
                Ok(Some(assistant)) => Some(assistant.name),
                Ok(None) => None,
                Err(error) => {
                    log::warn!(
                        "agent__checkSession: failed to resolve assistantName for {}: {}",
                        assistant_id,
                        error
                    );
                    None
                }
            }
        }
        None => None,
    };

    let child_raw = resolve_child_workspace_raw(meta);
    let caller_raw = resolve_caller_workspace_raw(caller_session_id);
    check_session_enrichment_from_metadata_with_caller(
        meta,
        assistant_name,
        child_raw,
        caller_raw.as_deref(),
    )
}
