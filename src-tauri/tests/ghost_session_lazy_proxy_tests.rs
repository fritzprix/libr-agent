/// Regression tests for the ghost session lazy proxy fix.
///
/// ## Background
///
/// Sessions that exist in the DB but have no active proxy (because they have never run a
/// workflow in the current app session) used to fail all builtin tool calls immediately
/// with "No proxy found for session: …".  This caused `AgentResourceAttachmentContext`
/// to log errors on mount for every dormant session visible in the sidebar.
///
/// ## Fix (commit 5d72d24a)
///
/// `call_tool()` in `management.rs` detects builtin tools and, when no proxy exists,
/// calls `ensure_builtin_proxy()` in `creation.rs` to lazily create a builtin-only proxy
/// before forwarding the call.  `ensure_builtin_proxy` reads the session's `agent_config`
/// from the DB to determine which builtins to enable, falling back to
/// `CORE_BUILTIN_SERVICE_ALIASES` on any error.
///
/// ## What these tests cover
///
/// 1. `attachments__list` — the exact tool that triggered the original bug — is
///    correctly classified as a builtin tool by the routing guard in `call_tool`.
/// 2. HTTP MCP tools such as `exa__web_search_exa` are NOT misidentified as builtins,
///    which would incorrectly send them into the lazy-proxy path.
/// 3. All entries in `CORE_BUILTIN_SERVICE_ALIASES` resolve via `BuiltinServiceId::from_alias`,
///    confirming the fallback always produces valid tool IDs.
/// 4. `extract_builtin_tool_ids` honours `allowed_built_in_service_aliases`:
///    - `None` → all non-optional builtins (including `attachments`) are enabled.
///    - `Some([…])` → only the specified subset is enabled.
/// 5. `attachments` appears in the core fallback list so the lazy proxy path can always
///    serve `attachments__list` even when the DB is unavailable.
use tauri_mcp_agent_lib::agent::tools::extract_builtin_tool_ids;
use tauri_mcp_agent_lib::agent::AgentConfig;
use tauri_mcp_agent_lib::mcp::builtin::service_id::{
    BuiltinServiceId, BUILTIN_SERVICE_REGISTRY, CORE_BUILTIN_SERVICE_ALIASES,
};

// ─── helpers ────────────────────────────────────────────────────────────────────────────

/// Mirrors the `is_builtin` check in `management.rs::call_tool`.
fn is_builtin_tool(tool_name: &str) -> bool {
    tool_name
        .split_once("__")
        .map(|(server, _)| BuiltinServiceId::from_alias(server).is_some())
        .unwrap_or(false)
}

/// Build a minimal `AgentConfig` from a JSON string (panics on parse error).
fn config_from_json(json: &str) -> AgentConfig {
    AgentConfig::from_json(json).expect("AgentConfig::from_json should not fail")
}

// ─── test 1: exact repro tool is classified as builtin ─────────────────────────────────

/// Regression: `attachments__list` (the exact tool call that produced the original
/// "No proxy found" error) must be identified as a builtin so `call_tool` enters the
/// lazy-proxy path instead of the external-MCP path (which would immediately fail).
#[test]
fn attachments_list_content_is_classified_as_builtin() {
    assert!(
        is_builtin_tool("attachments__list"),
        "`attachments__list` must be detected as a builtin tool; \
         if this fails the lazy-proxy path is never reached"
    );
}

// ─── test 2: other builtin prefixes are also classified correctly ───────────────────────

#[test]
fn all_registry_services_classified_as_builtin() {
    for entry in BUILTIN_SERVICE_REGISTRY.iter() {
        let tool_name = format!("{}__{}", entry.canonical, "dummyAction");
        assert!(
            is_builtin_tool(&tool_name),
            "Registry service `{}` should be detected as builtin via `{}`",
            entry.canonical,
            tool_name
        );
    }
}

// ─── test 3: external HTTP MCP tools are NOT misidentified ─────────────────────────────

#[test]
fn external_tools_are_not_misidentified_as_builtin() {
    let external_tools = [
        "exa__web_search_exa",
        "filesystem__read_file",
        "github__search_repos",
        "custom_server__some_action",
    ];
    for tool in &external_tools {
        assert!(
            !is_builtin_tool(tool),
            "External tool `{tool}` must NOT be detected as builtin"
        );
    }
}

// ─── test 4: tools without the __ separator are not misidentified ──────────────────────

#[test]
fn tools_without_separator_are_not_builtin() {
    let bare_names = ["list", "search", "", "attachments"];
    for name in &bare_names {
        assert!(
            !is_builtin_tool(name),
            "Bare name `{name}` must not be detected as builtin (requires `__` separator)"
        );
    }
}

// ─── test 5: CORE fallback aliases all resolve via BuiltinServiceId ────────────────────

/// The fallback in `resolve_tool_ids_for_session` emits `CORE_BUILTIN_SERVICE_ALIASES`
/// as-is.  Every entry must be resolvable through `BuiltinServiceId::from_alias`;
/// otherwise the lazy proxy would be built with unrecognised tool IDs.
#[test]
fn core_builtin_fallback_aliases_all_resolve() {
    for alias in CORE_BUILTIN_SERVICE_ALIASES {
        assert!(
            BuiltinServiceId::from_alias(alias).is_some(),
            "CORE_BUILTIN_SERVICE_ALIASES entry `{alias}` must resolve via BuiltinServiceId::from_alias"
        );
    }
}

// ─── test 6: attachments is in the core fallback ────────────────────────────────────────

/// The lazy-proxy fallback must include `attachments` so that `attachments__list`
/// can always be served even when the DB lookup fails.
#[test]
fn core_builtin_fallback_includes_attachments() {
    assert!(
        CORE_BUILTIN_SERVICE_ALIASES.contains(&"attachments"),
        "`attachments` must be in CORE_BUILTIN_SERVICE_ALIASES for the fallback path to work"
    );
}

// ─── test 7: extract_builtin_tool_ids with no explicit alias list ───────────────────────

/// When `allowed_built_in_service_aliases` is `None` (default config), all non-optional
/// builtin services must be enabled.  This is the normal case for most agents and the
/// path exercised by `resolve_tool_ids_for_session` for a typical session.
#[test]
fn extract_builtin_tool_ids_default_includes_attachments() {
    let config = config_from_json(
        r#"{
            "name": "Test Agent",
            "systemPrompt": "You are a test agent.",
            "mcp_server_ids": [],
            "local_services": []
        }"#,
    );

    let tool_ids = extract_builtin_tool_ids(&config);

    assert!(
        tool_ids.contains(&"attachments".to_string()),
        "`attachments` must be in the default tool ID list; got: {tool_ids:?}"
    );
    // Sanity: non-optional builtins are all present
    for alias in CORE_BUILTIN_SERVICE_ALIASES {
        let alias_str: &str = alias;
        assert!(
            tool_ids.contains(&alias_str.to_string()),
            "Core alias `{alias}` must be in default tool IDs; got: {tool_ids:?}"
        );
    }
}

// ─── test 8: extract_builtin_tool_ids respects explicit allowlist ───────────────────────

/// When `allowed_built_in_service_aliases` is an explicit list, only those services
/// (plus the always-enabled core builtins) should appear.  This ensures the DB-read path
/// in `resolve_tool_ids_for_session` narrows the proxy down correctly.
#[test]
fn extract_builtin_tool_ids_respects_explicit_allowlist() {
    let config = config_from_json(
        r#"{
            "name": "Restricted Agent",
            "systemPrompt": "You are a restricted agent.",
            "mcp_server_ids": [],
            "allowedBuiltInServiceAliases": ["planning", "workspace"]
        }"#,
    );

    let tool_ids = extract_builtin_tool_ids(&config);

    // Core non-optional builtins are always present
    assert!(tool_ids.contains(&"planning".to_string()));
    assert!(tool_ids.contains(&"workspace".to_string()));
    // Optional builtins not in the explicit list must be absent
    assert!(
        !tool_ids.contains(&"browser".to_string()),
        "Optional `browser` must be excluded when not in allowlist"
    );
    assert!(
        !tool_ids.contains(&"bootstrap".to_string()),
        "Optional `bootstrap` must be excluded when not in allowlist"
    );
}

// ─── test 9: idempotency guarantee — is_builtin is deterministic ────────────────────────

/// Repeated calls with the same tool name must always return the same result.
/// This documents that `is_builtin` is a pure function with no side-effects.
#[test]
fn is_builtin_check_is_idempotent() {
    let cases = [
        ("attachments__list", true),
        ("exa__web_search_exa", false),
        ("planning__addTask", true),
    ];
    for (tool, expected) in &cases {
        for _ in 0..3 {
            assert_eq!(
                is_builtin_tool(tool),
                *expected,
                "is_builtin({tool}) must be deterministic"
            );
        }
    }
}
