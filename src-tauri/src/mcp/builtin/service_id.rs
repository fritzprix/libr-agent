//! Stable identifier for every builtin service.
//!
//! ## Why this exists
//!
//! Historically the routing code matched raw strings such as `"content_store"` or
//! `"attachments"`.  String matching has two problems:
//!
//! 1. **No exhaustiveness**: a new variant is added to the registry but the
//!    developer forgets a match arm — the compiler stays silent and the service
//!    silently returns `Ok(None)` at runtime.
//! 2. **Name coupling**: the DB stores the public name (`allowedBuiltInServiceAliases`),
//!    so renaming a service requires a DB migration.
//!
//! `BuiltinServiceId` solves both:
//!
//! - `from_alias()` centralises all string → ID resolution (including legacy names).
//! - Exhaustive `match` in `factory.rs` / `server/tools.rs` means the compiler
//!   catches every missing arm if a new variant is added.
//! - `name()` returns the *current* public name; changing it in the future requires
//!   only updating this one function — no DB change.
//!
//! ## Serde
//!
//! Serializes to `snake_case` which matches the strings currently stored in the
//! `allowedBuiltInServiceAliases` JSON field of the `assistants` table.  No DB
//! migration is required for existing records (they already use the canonical forms
//! such as `"planning"`, `"attachments"`, etc.).

use std::fmt;

/// Type-safe, stable identifier for a builtin MCP service.
///
/// The serde representation (snake_case of the variant name) is the **stable DB key**
/// — it must never change.  The public-facing name returned by [`BuiltinServiceId::name`]
/// may be updated freely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinServiceId {
    Planning,
    Workspace,
    Knowledge,
    Assistant,
    Skills,
    Playbook,
    /// Formerly known as `content_store`.  DB value: `"attachments"`.
    ///
    /// The serde aliases ensure that legacy DB records containing `"content_store"`
    /// or `"contentstore"` deserialise correctly without requiring a DB migration.
    #[serde(alias = "content_store", alias = "contentstore")]
    Attachments,
    Swarm,
    Ui,
    Browser,
    Bootstrap,
    McpManager,
}

impl BuiltinServiceId {
    /// Resolve any alias string (including legacy names) to a [`BuiltinServiceId`].
    ///
    /// Recognised inputs:
    /// - Current canonical forms: `"planning"`, `"attachments"`, …
    /// - Pre-0.6.0 legacy names: `"content_store"`
    /// - Variant aliases: `"assistant_manager"`
    ///
    /// Returns `None` for completely unknown strings (e.g. external server names).
    pub fn from_alias(alias: &str) -> Option<Self> {
        match alias.trim().to_lowercase().as_str() {
            "planning" => Some(Self::Planning),
            "workspace" => Some(Self::Workspace),
            "knowledge" => Some(Self::Knowledge),
            "assistant" | "assistant_manager" => Some(Self::Assistant),
            "skills" => Some(Self::Skills),
            "playbook" => Some(Self::Playbook),
            // "content_store" is the pre-0.6.0 legacy name; keep forever for DB compat.
            "attachments" | "content_store" => Some(Self::Attachments),
            "swarm" => Some(Self::Swarm),
            "ui" => Some(Self::Ui),
            "browser" => Some(Self::Browser),
            "bootstrap" => Some(Self::Bootstrap),
            "mcp_manager" => Some(Self::McpManager),
            _ => None,
        }
    }

    /// Current canonical alias for this service.
    ///
    /// This is the name used in tool IDs (`builtin_<name>__<tool>`) and what
    /// new `allowedBuiltInServiceAliases` records should contain.
    ///
    /// **Safe to rename**: changing this only affects new records and display;
    /// the DB stable ID (serde value) is the enum variant, not this string.
    pub fn name(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Workspace => "workspace",
            Self::Knowledge => "knowledge",
            Self::Assistant => "assistant",
            Self::Skills => "skills",
            Self::Playbook => "playbook",
            Self::Attachments => "attachments",
            Self::Swarm => "swarm",
            Self::Ui => "ui",
            Self::Browser => "browser",
            Self::Bootstrap => "bootstrap",
            Self::McpManager => "mcp_manager",
        }
    }
}

impl fmt::Display for BuiltinServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ── Tool-name helpers ────────────────────────────────────────────────────────
//
// All builtin tool names follow the pattern:  builtin_<group>__<tool>
//
// These helpers are the SINGLE change-point for the naming convention.
// Routing code, proxy assembly, and external-API translation all go through
// these functions so that renaming the prefix is a one-line change here.

/// The prefix that distinguishes builtin tool names from external MCP tool names.
pub const BUILTIN_PREFIX: &str = "builtin_";

/// Build the canonical internal tool name for a builtin tool.
///
/// ```text
/// builtin_tool_name("planning", "addScratchpad") → "builtin_planning__addScratchpad"
/// ```
pub fn builtin_tool_name(group: &str, tool: &str) -> String {
    format!("{}{}__{}", BUILTIN_PREFIX, group, tool)
}

/// Return `true` if `tool_name` is an internal builtin tool name.
pub fn is_builtin_tool_name(tool_name: &str) -> bool {
    tool_name.starts_with(BUILTIN_PREFIX)
}

/// Strip the builtin prefix, returning `(group, tool)` or `None` for non-builtin names.
///
/// ```text
/// parse_builtin_tool_name("builtin_planning__addScratchpad") → Some(("planning", "addScratchpad"))
/// parse_builtin_tool_name("github__search_code")             → None
/// ```
pub fn parse_builtin_tool_name(tool_name: &str) -> Option<(&str, &str)> {
    let suffix = tool_name.strip_prefix(BUILTIN_PREFIX)?;
    suffix.split_once("__")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── from_alias coverage ──────────────────────────────────────────────────

    #[test]
    fn from_alias_canonical_names_resolve() {
        let cases = [
            ("planning", BuiltinServiceId::Planning),
            ("workspace", BuiltinServiceId::Workspace),
            ("knowledge", BuiltinServiceId::Knowledge),
            ("assistant", BuiltinServiceId::Assistant),
            ("assistant_manager", BuiltinServiceId::Assistant),
            ("skills", BuiltinServiceId::Skills),
            ("playbook", BuiltinServiceId::Playbook),
            ("attachments", BuiltinServiceId::Attachments),
            ("swarm", BuiltinServiceId::Swarm),
            ("ui", BuiltinServiceId::Ui),
            ("browser", BuiltinServiceId::Browser),
            ("bootstrap", BuiltinServiceId::Bootstrap),
            ("mcp_manager", BuiltinServiceId::McpManager),
        ];
        for (alias, expected) in &cases {
            assert_eq!(
                BuiltinServiceId::from_alias(alias),
                Some(*expected),
                "from_alias({alias:?}) should be {expected:?}"
            );
        }
    }

    /// Regression: legacy alias written to DB before 0.6.0 must still resolve.
    #[test]
    fn from_alias_legacy_content_store_resolves_to_attachments() {
        assert_eq!(
            BuiltinServiceId::from_alias("content_store"),
            Some(BuiltinServiceId::Attachments)
        );
    }

    #[test]
    fn from_alias_unknown_string_returns_none() {
        assert_eq!(BuiltinServiceId::from_alias("foobar"), None);
        assert_eq!(BuiltinServiceId::from_alias(""), None);
        assert_eq!(BuiltinServiceId::from_alias("external_server"), None);
    }

    // ── serde roundtrip ──────────────────────────────────────────────────────

    #[test]
    fn serde_serialize_produces_canonical_snake_case() {
        let id = BuiltinServiceId::Attachments;
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"attachments\"");

        let id = BuiltinServiceId::McpManager;
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"mcp_manager\"");
    }

    #[test]
    fn serde_deserialize_canonical_name() {
        let id: BuiltinServiceId = serde_json::from_str("\"attachments\"").unwrap();
        assert_eq!(id, BuiltinServiceId::Attachments);
    }

    /// Regression: `"content_store"` must deserialise via serde alias without error.
    /// This covers DB records written before the 0.6.0 rename.
    #[test]
    fn serde_deserialize_legacy_content_store_alias() {
        let id: BuiltinServiceId = serde_json::from_str("\"content_store\"").unwrap();
        assert_eq!(id, BuiltinServiceId::Attachments);

        let id: BuiltinServiceId = serde_json::from_str("\"contentstore\"").unwrap();
        assert_eq!(id, BuiltinServiceId::Attachments);
    }

    #[test]
    fn serde_deserialize_unknown_string_errors() {
        let result: Result<BuiltinServiceId, _> = serde_json::from_str("\"foobar\"");
        assert!(result.is_err(), "unknown variant must be rejected");
    }

    /// Roundtrip: serialize then deserialize must be identity.
    #[test]
    fn serde_roundtrip_all_variants() {
        let variants = [
            BuiltinServiceId::Planning,
            BuiltinServiceId::Workspace,
            BuiltinServiceId::Knowledge,
            BuiltinServiceId::Assistant,
            BuiltinServiceId::Skills,
            BuiltinServiceId::Playbook,
            BuiltinServiceId::Attachments,
            BuiltinServiceId::Swarm,
            BuiltinServiceId::Ui,
            BuiltinServiceId::Browser,
            BuiltinServiceId::Bootstrap,
            BuiltinServiceId::McpManager,
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant).unwrap();
            let back: BuiltinServiceId = serde_json::from_str(&json).unwrap();
            assert_eq!(*variant, back, "roundtrip failed for {variant:?}");
        }
    }

    // ── name() consistency ───────────────────────────────────────────────────

    /// name() must stay in sync with from_alias() — name(id) must resolve back to the same id.
    #[test]
    fn name_resolves_back_via_from_alias() {
        let variants = [
            BuiltinServiceId::Planning,
            BuiltinServiceId::Workspace,
            BuiltinServiceId::Knowledge,
            BuiltinServiceId::Assistant,
            BuiltinServiceId::Skills,
            BuiltinServiceId::Playbook,
            BuiltinServiceId::Attachments,
            BuiltinServiceId::Swarm,
            BuiltinServiceId::Ui,
            BuiltinServiceId::Browser,
            BuiltinServiceId::Bootstrap,
            BuiltinServiceId::McpManager,
        ];
        for variant in &variants {
            let name = variant.name();
            assert_eq!(
                BuiltinServiceId::from_alias(name),
                Some(*variant),
                "from_alias(name()) should round-trip for {variant:?}"
            );
        }
    }
}
