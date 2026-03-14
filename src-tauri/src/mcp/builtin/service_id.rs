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
//! - `name()` returns the *current* public name.  **Important**: `name()` currently
//!   returns the same values as the serde representation, so both must stay in sync.
//!   To decouple them in the future, add explicit `#[serde(rename = "...")]`
//!   attributes to each variant.
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
/// The serde representation (`snake_case` of the variant name) is the **stable DB key**
/// — it must never change.  The `name()` method currently returns the same values,
/// so **both must be kept in sync**.  To truly decouple them, add explicit
/// `#[serde(rename = "...")]` attributes to each variant and then `name()` can
/// diverge freely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinServiceId {
    Planning,
    Memory,
    Workspace,
    Knowledge,
    Assistant,
    Skills,
    Playbook,
    /// Formerly known as `content_store`.  DB value: `"attachments"`.
    ///
    /// The serde aliases ensure that legacy DB records containing `"content_store"`
    /// or `"contentstore"` (no underscore, written by early versions of
    /// `assistant_init.rs`) deserialise correctly without requiring a DB migration.
    #[serde(alias = "content_store", alias = "contentstore")]
    Attachments,
    /// `"session_api"` was the pre-0.5 name for this service; keep as serde alias
    /// so that DB records written by older binaries still deserialise correctly.
    #[serde(alias = "session_api")]
    Swarm,
    Ui,
    Browser,
    Bootstrap,
    McpManager,
    Media,
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
            // "scratchpad" is the legacy alias for memory — keep for backward compat.
            "memory" | "scratchpad" => Some(Self::Memory),
            "workspace" => Some(Self::Workspace),
            "knowledge" => Some(Self::Knowledge),
            "assistant" | "assistant_manager" => Some(Self::Assistant),
            "skills" => Some(Self::Skills),
            "playbook" => Some(Self::Playbook),
            // "content_store" is the pre-0.6.0 legacy name; "contentstore" (no
            // underscore) was written by early assistant_init.rs; keep both forever.
            "attachments" | "content_store" | "contentstore" => Some(Self::Attachments),
            // "session_api" was the pre-0.5 internal name; keep forever for DB compat.
            "swarm" | "session_api" => Some(Self::Swarm),
            "ui" => Some(Self::Ui),
            "browser" => Some(Self::Browser),
            "bootstrap" => Some(Self::Bootstrap),
            "mcp_manager" => Some(Self::McpManager),
            "media" => Some(Self::Media),
            _ => None,
        }
    }

    /// Current canonical alias for this service.
    ///
    /// This is the name used in tool IDs (`<name>__<tool>`) and what
    /// new `allowedBuiltInServiceAliases` records should contain.
    ///
    /// **Safe to rename**: changing this only affects new records and display;
    /// the DB stable ID (serde value) is the enum variant, not this string.
    pub fn name(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Memory => "memory",
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
            Self::Media => "media",
        }
    }
}

impl fmt::Display for BuiltinServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── from_alias coverage ──────────────────────────────────────────────────

    #[test]
    fn from_alias_canonical_names_resolve() {
        let cases = [
            ("planning", BuiltinServiceId::Planning),
            ("memory", BuiltinServiceId::Memory),
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
            ("media", BuiltinServiceId::Media),
        ];
        for (alias, expected) in &cases {
            assert_eq!(
                BuiltinServiceId::from_alias(alias),
                Some(*expected),
                "from_alias({alias:?}) should be {expected:?}"
            );
        }
    }

    /// Regression: legacy aliases written to DB before 0.6.0 must still resolve.
    #[test]
    fn from_alias_legacy_content_store_resolves_to_attachments() {
        assert_eq!(
            BuiltinServiceId::from_alias("content_store"),
            Some(BuiltinServiceId::Attachments)
        );
        // "contentstore" (no underscore) was written by early assistant_init.rs
        assert_eq!(
            BuiltinServiceId::from_alias("contentstore"),
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

    /// Regression: `"content_store"` and `"contentstore"` must deserialise via serde
    /// alias without error.  Both were written to the DB by earlier versions.
    /// `"content_store"` was the pre-0.6.0 canonical name; `"contentstore"` (no
    /// underscore) was written by early `assistant_init.rs`.
    #[test]
    fn serde_deserialize_legacy_content_store_alias() {
        let id: BuiltinServiceId = serde_json::from_str("\"content_store\"").unwrap();
        assert_eq!(id, BuiltinServiceId::Attachments);

        // "contentstore" (no underscore) must also be accepted
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
            BuiltinServiceId::Memory,
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
            BuiltinServiceId::Media,
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
            BuiltinServiceId::Memory,
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
            BuiltinServiceId::Media,
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
