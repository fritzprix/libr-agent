use crate::mcp::builtin::service_id::{BUILTIN_SERVICE_REGISTRY, CORE_BUILTIN_SERVICE_ALIASES};
use serde_json::Value;
use std::collections::HashSet;

/// Resolve any alias string (including legacy pre-0.6.0 names) to the current
/// canonical service name.
///
/// Delegates to [`crate::mcp::builtin::service_id::BuiltinServiceId::from_alias`]
/// which is the single source of truth for all alias mappings.
pub fn canonicalize_builtin_service_alias(alias: &str) -> Option<&'static str> {
    crate::mcp::builtin::service_id::BuiltinServiceId::from_alias(alias).map(|id| id.name())
}

pub fn runtime_allowed_builtin_service_aliases(
    agent_config: &crate::agent::AgentConfig,
) -> Vec<String> {
    let mut allowed: HashSet<String> = CORE_BUILTIN_SERVICE_ALIASES
        .iter()
        .map(|alias| alias.to_string())
        .collect();

    if let Some(configured_ids) = &agent_config.allowed_built_in_service_aliases {
        for id in configured_ids {
            allowed.insert(id.name().to_string());
        }
    } else {
        // No explicit list → all optional services are implicitly enabled
        for entry in BUILTIN_SERVICE_REGISTRY.iter().filter(|e| e.optional) {
            allowed.insert(entry.canonical.to_string());
        }
    }

    // Preserve canonical ordering from the registry
    BUILTIN_SERVICE_REGISTRY
        .iter()
        .filter(|entry| allowed.contains(entry.canonical))
        .map(|entry| entry.canonical.to_string())
        .collect()
}

/// Derive the runtime-enabled builtin aliases directly from a stored config JSON value.
///
/// This preserves the runtime contract even when legacy or partially invalid config payloads
/// fail full `AgentConfig` deserialization. Explicit builtin lists remain explicit; they do not
/// silently fall back to "all optional builtins enabled".
pub fn runtime_allowed_builtin_service_aliases_from_value(config: &Value) -> Vec<String> {
    if let Ok(agent_config) = serde_json::from_value::<crate::agent::AgentConfig>(config.clone()) {
        return runtime_allowed_builtin_service_aliases(&agent_config);
    }

    let mut allowed: HashSet<String> = CORE_BUILTIN_SERVICE_ALIASES
        .iter()
        .map(|alias| alias.to_string())
        .collect();

    match config.get("allowedBuiltInServiceAliases") {
        Some(Value::Array(configured_aliases)) => {
            for alias in configured_aliases.iter().filter_map(Value::as_str) {
                if let Some(canonical_alias) = canonicalize_builtin_service_alias(alias) {
                    allowed.insert(canonical_alias.to_string());
                }
            }
        }
        Some(_) => {}
        None => {
            for entry in BUILTIN_SERVICE_REGISTRY.iter().filter(|e| e.optional) {
                allowed.insert(entry.canonical.to_string());
            }
        }
    }

    BUILTIN_SERVICE_REGISTRY
        .iter()
        .filter(|entry| allowed.contains(entry.canonical))
        .map(|entry| entry.canonical.to_string())
        .collect()
}

pub fn is_builtin_service_alias_enabled(
    agent_config: &crate::agent::AgentConfig,
    alias: &str,
) -> bool {
    let Some(target_alias) = canonicalize_builtin_service_alias(alias) else {
        return false;
    };

    runtime_allowed_builtin_service_aliases(agent_config)
        .iter()
        .any(|current| current == target_alias)
}

/// Extract builtin tool IDs from agent configuration
pub fn extract_builtin_tool_ids(agent_config: &crate::agent::AgentConfig) -> Vec<String> {
    runtime_allowed_builtin_service_aliases(agent_config)
}
