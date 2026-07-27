use crate::agent::state::AgentSession;
use crate::mcp::service_proxy::MCPServiceProxy;
use crate::mcp::MCPServiceProxyManager;
use crate::session::get_session_manager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Filenames checked (in order) for workspace-level behavior instructions.
/// Only the FIRST file found with content is injected.
const WORKSPACE_INSTRUCTION_FILES: &[&str] = &["agents.md", "AGENTS.md", "CLAUDE.md", "GEMINI.md"];

/// Filenames checked (in order) for persona / tone instructions.
/// Only the FIRST file found with content is injected.
const SOUL_INSTRUCTION_FILES: &[&str] =
    &[".github/SOUL.md", "SOUL.md", ".github/soul.md", "soul.md"];

const STABLE_PROMPT_KEY_PREFIX: &str = "<!-- stable-key:";

fn encode_cached_stable_prompt(source_key: &str, prompt: &str) -> String {
    format!("{STABLE_PROMPT_KEY_PREFIX}{source_key} -->\n{prompt}")
}

fn decode_cached_stable_prompt(cached: &str) -> Option<(&str, &str)> {
    let rest = cached.strip_prefix(STABLE_PROMPT_KEY_PREFIX)?;
    let (source_key, prompt) = rest.split_once(" -->\n")?;
    Some((source_key, prompt))
}

fn get_session_workspace_dir(session_id: &str) -> Option<std::path::PathBuf> {
    match get_session_manager() {
        Ok(mgr) => Some(mgr.get_session_workspace_dir_by_id(session_id)),
        Err(_) => None,
    }
}

async fn load_first_instruction_file(
    workspace: &std::path::Path,
    candidates: &[&str],
) -> Option<(String, String)> {
    for &filename in candidates {
        let path = workspace.join(filename);
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Some((filename.to_string(), trimmed));
            }
        }
    }

    None
}

/// Reads the first workspace behavior instruction file that exists for the session.
async fn load_workspace_agent_instructions(session_id: &str) -> Vec<(String, String)> {
    let Some(workspace) = get_session_workspace_dir(session_id) else {
        return vec![];
    };

    load_first_instruction_file(&workspace, WORKSPACE_INSTRUCTION_FILES)
        .await
        .into_iter()
        .collect()
}

/// Reads the first persona / tone instruction file that exists for the session.
async fn load_soul_instruction(session_id: &str) -> Option<(String, String)> {
    let workspace = get_session_workspace_dir(session_id)?;
    load_first_instruction_file(&workspace, SOUL_INSTRUCTION_FILES).await
}

/// Build the stable prefix and volatile sections separately for a session.
///
/// Returns `(stable_prompt, session_context)` where:
/// - `stable_prompt` — sections 1–4 plus service-context blocks explicitly marked
///   `ContextVolatility::Stable`. This forms the most reusable per-turn prefix.
/// - `session_context` — rebuilt fresh on every LLM call: context providers plus
///   non-stable service tool state.
///
/// Rust owns the final request layout. Callers pass both parts into the backend
/// request-layout builder, which decides whether volatile context becomes part
/// of the stable prompt or a synthetic tail message for the target provider.
pub(crate) async fn build_session_system_prompt_split(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    session_id: &str,
) -> Result<(String, Option<String>), String> {
    // --- Read session state under a short-lived read lock ---
    let (session_metadata, context_registry, cached_stable_prompt_arc) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        (
            session.metadata.clone(),
            session.context_registry.clone(),
            session.cached_stable_prompt.clone(),
        )
    };

    let agent_config = crate::agent::resolve_agent_config(&session_metadata).await?;
    let source_key = crate::agent::stable_prompt_source_key(&agent_config);

    // --- Build (or reuse) stable prefix ---
    let stable_prefix = {
        let cached = cached_stable_prompt_arc.read().await;
        if let Some(ref existing) = *cached {
            if let Some((cached_key, cached_prompt)) = decode_cached_stable_prompt(existing) {
                if cached_key == source_key {
                    cached_prompt.to_string()
                } else {
                    drop(cached);
                    build_and_cache_stable_prefix(
                        &cached_stable_prompt_arc,
                        &agent_config,
                        session_id,
                        &source_key,
                    )
                    .await
                }
            } else {
                drop(cached);
                build_and_cache_stable_prefix(
                    &cached_stable_prompt_arc,
                    &agent_config,
                    session_id,
                    &source_key,
                )
                .await
            }
        } else {
            drop(cached);
            build_and_cache_stable_prefix(
                &cached_stable_prompt_arc,
                &agent_config,
                session_id,
                &source_key,
            )
            .await
        }
    };

    // --- Build per-turn sections fresh each call ---
    let proxy = proxy_manager.get_proxy(session_id).await;
    let (cacheable_context, volatile) =
        build_volatile_sections_split(Some(context_registry), proxy, agent_config.id.as_deref())
            .await;

    let stable_prompt = if cacheable_context.trim().is_empty() {
        stable_prefix
    } else {
        format!("{}\n{}", stable_prefix, cacheable_context)
    };

    let session_context = if volatile.trim().is_empty() {
        None
    } else {
        Some(volatile)
    };

    Ok((stable_prompt, session_context))
}

async fn build_and_cache_stable_prefix(
    cached_stable_prompt_arc: &Arc<RwLock<Option<String>>>,
    agent_config: &crate::agent::AgentConfig,
    session_id: &str,
    source_key: &str,
) -> String {
    let mut write_guard = cached_stable_prompt_arc.write().await;
    if let Some(ref existing) = *write_guard {
        if let Some((cached_key, cached_prompt)) = decode_cached_stable_prompt(existing) {
            if cached_key == source_key {
                return cached_prompt.to_string();
            }
        }
    }

    let soul_instruction = load_soul_instruction(session_id).await;
    let workspace_instructions = load_workspace_agent_instructions(session_id).await;
    let stable = build_stable_prefix(
        agent_config,
        session_id,
        soul_instruction,
        workspace_instructions,
    );
    *write_guard = Some(encode_cached_stable_prompt(source_key, &stable));
    stable
}

/// Build complete system prompt for session (wrapper)
///
/// The stable prefix (sections 1–3: agent identity, persona, workspace instructions)
/// is computed once and cached in the session. Only the volatile
/// sections (4: context providers, 5: service contexts) are rebuilt on every LLM call.
pub async fn build_session_system_prompt(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    session_id: &str,
) -> Result<String, String> {
    let (stable_prompt, session_context) =
        build_session_system_prompt_split(active_sessions, proxy_manager, session_id).await?;

    if let Some(volatile) = session_context {
        Ok(format!("{}\n{}", stable_prompt, volatile))
    } else {
        Ok(stable_prompt)
    }
}

/// Build complete system prompt (Pure logic)
///
/// Structure:
/// 1. Agent Identity & Strategy (who am I, how do I work)
/// 2. Persona / Voice Template (SOUL.md found in workspace)
/// 3. Workspace Instructions (agents.md / CLAUDE.md found in workspace)
/// 4. Read-only Context Providers (time, skills, documentation)
/// 5. Service Contexts (tools & current state - immediately actionable)
pub async fn build_system_prompt(
    agent_config: &crate::agent::AgentConfig,
    proxy: Option<Arc<MCPServiceProxy>>,
    context_registry: Option<Arc<crate::agent::context::registry::ContextRegistry>>,
    soul_instruction: Option<(String, String)>,
    workspace_instructions: Vec<(String, String)>,
) -> Result<String, String> {
    let stable = build_stable_prefix(
        agent_config,
        "(unknown-session)",
        soul_instruction,
        workspace_instructions,
    );
    let (cacheable_context, volatile) =
        build_volatile_sections_split(context_registry, proxy, agent_config.id.as_deref()).await;

    let stable = if cacheable_context.trim().is_empty() {
        stable
    } else {
        format!("{}\n{}", stable, cacheable_context)
    };

    if volatile.trim().is_empty() {
        Ok(stable)
    } else {
        Ok(format!("{}\n{}", stable, volatile))
    }
}

/// Build the stable, session-immutable prefix (sections 1–3).
///
/// These sections never change within a session so callers may cache the result.
fn build_stable_prefix(
    agent_config: &crate::agent::AgentConfig,
    session_id: &str,
    soul_instruction: Option<(String, String)>,
    workspace_instructions: Vec<(String, String)>,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    // 1. Agent Identity & Strategy (first priority)
    if !agent_config.system_prompt.trim().is_empty() {
        parts.push(agent_config.system_prompt.clone());
    }

    if agent_config.id.is_some() || !agent_config.name.trim().is_empty() {
        let agent_id = agent_config.id.as_deref().unwrap_or("(unknown)");
        let mut identity = format!(
            "\n\n## Agent Runtime Identity\n\
            - Agent Name: {}\n\
            - Agent ID: {}\n\
            - Session ID: {}",
            agent_config.name.trim(),
            agent_id,
            session_id
        );

        if let Some(ref parent_id) = agent_config.parent_session_id {
            let short_parent: String = parent_id.chars().take(8).collect();
            let depth = agent_config.depth.unwrap_or(1);
            identity.push_str(&format!(
                "\n- Session Type: Sub-Agent (Depth: {})\n\
                - Parent Session: {}\n\n\
                ⚠️ You are running as a SUB-AGENT delegated by Parent Session `{}`. Focus strictly on your assigned task and return your results concisely to your parent agent.",
                depth, short_parent, short_parent
            ));
        }

        parts.push(identity);
    }

    // 2. Persona / Voice Template — injected from SOUL.md and kept distinct from
    //    workspace instructions because it defines character, not task guidance.
    if let Some((filename, content)) = &soul_instruction {
        parts.push(format!(
            "\n\n## Persona Template ({})\n\n{}",
            filename, content
        ));
    }

    // 3. Workspace Instructions — injected from agents.md / CLAUDE.md etc.
    //    These are workspace-scoped operating constraints, not persona.
    for (filename, content) in &workspace_instructions {
        parts.push(format!(
            "\n\n## Workspace Instructions ({})\n\n{}",
            filename, content
        ));
    }

    parts.join("\n")
}

fn split_service_context_prompts(
    contexts: std::collections::HashMap<String, crate::mcp::types::ServiceContext>,
) -> (Vec<String>, Vec<String>) {
    let mut sorted_contexts: Vec<(String, _)> = contexts.into_iter().collect();
    sorted_contexts.sort_by(|(left_id, left), (right_id, right)| {
        left.volatility
            .cmp(&right.volatility)
            .then_with(|| left_id.cmp(right_id))
    });

    let mut cacheable_parts = Vec::new();
    let mut volatile_parts = Vec::new();

    for (_tool_id, service_context) in sorted_contexts {
        if service_context.context_prompt.trim().is_empty() {
            continue;
        }

        match service_context.volatility {
            crate::mcp::types::ContextVolatility::Stable => {
                cacheable_parts.push(service_context.context_prompt)
            }
            crate::mcp::types::ContextVolatility::Medium
            | crate::mcp::types::ContextVolatility::Volatile => {
                volatile_parts.push(service_context.context_prompt)
            }
        }
    }

    (cacheable_parts, volatile_parts)
}

/// Build the per-turn prompt sections, split into a cacheable stable-service
/// fragment and a volatile fragment.
async fn build_volatile_sections_split(
    context_registry: Option<Arc<crate::agent::context::registry::ContextRegistry>>,
    proxy: Option<Arc<MCPServiceProxy>>,
    assistant_id: Option<&str>,
) -> (String, String) {
    let mut cacheable_parts: Vec<String> = Vec::new();
    let mut volatile_parts: Vec<String> = Vec::new();

    // 5. Read-only Context Providers (time, skills, documentation, etc.)
    // Context providers currently emit one merged block without per-provider
    // volatility metadata, so keep them on the per-turn channel.
    if let Some(registry) = context_registry {
        let context = registry.build_context(assistant_id).await;
        if !context.trim().is_empty() {
            volatile_parts.push(context);
        }
    }

    // 6. Service Contexts - immediately actionable information
    if let Some(p) = proxy {
        let contexts = p.get_service_contexts(assistant_id).await;

        if !contexts.is_empty() {
            let (stable_service_parts, volatile_service_parts) =
                split_service_context_prompts(contexts);

            if !stable_service_parts.is_empty() {
                cacheable_parts.push("\n\n## Available Tools & Stable Reference\n".to_string());
                cacheable_parts.extend(stable_service_parts);
            }

            if !volatile_service_parts.is_empty() {
                volatile_parts.push("\n\n## Available Tools & Current State\n".to_string());
                volatile_parts.extend(volatile_service_parts);
            }
        }
    }

    (cacheable_parts.join("\n"), volatile_parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::context::registry::ContextRegistry;
    use crate::mcp::service_proxy::MCPServiceProxy;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_build_system_prompt_all_sections() {
        // 1. Agent Identity
        let agent_config = crate::agent::AgentConfig {
            id: Some("test-assistant".to_string()),
            name: "Test Assistant".to_string(),
            system_prompt: "You are a test assistant.".to_string(),
            ..Default::default()
        };

        // 2. Persona Template
        let soul_instruction = Some((".github/SOUL.md".to_string(), "Soul rule".to_string()));

        // 3. Workspace Instructions
        let workspace_instructions =
            vec![("agents.md".to_string(), "Custom agents.md rule".to_string())];

        // 4. Read-only Context Providers (Simulate empty for unit test simplicty, or mock)
        let context_registry = Some(Arc::new(ContextRegistry::new()));

        // 5. Service Contexts (Simulate None representing no MCPs for now)
        let proxy: Option<Arc<MCPServiceProxy>> = None;

        let prompt = build_system_prompt(
            &agent_config,
            proxy,
            context_registry,
            soul_instruction,
            workspace_instructions,
        )
        .await
        .unwrap();

        // Assert 1: Agent Identity
        assert!(prompt.contains("You are a test assistant."));

        // Assert 2: Persona Template
        assert!(prompt.contains("## Persona Template (.github/SOUL.md)"));
        assert!(prompt.contains("Soul rule"));

        // Assert 3: Workspace Instructions
        assert!(prompt.contains("## Workspace Instructions (agents.md)"));
        assert!(prompt.contains("Custom agents.md rule"));

        let persona_pos = prompt.find("## Persona Template").unwrap();
        let workspace_pos = prompt.find("## Workspace Instructions").unwrap();
        assert!(persona_pos < workspace_pos);
    }

    #[tokio::test]
    async fn test_build_system_prompt_missing_optional_sections() {
        let agent_config = crate::agent::AgentConfig {
            system_prompt: "Base prompt only.".to_string(),
            ..Default::default()
        };

        let prompt = build_system_prompt(
            &agent_config,
            None,   // No proxy
            None,   // No context registry
            None,   // No soul instruction
            vec![], // No workspace instructions
        )
        .await
        .unwrap();

        assert_eq!(
            prompt,
            "Base prompt only.\n\n\n## Agent Runtime Identity\n\
            - Agent Name: Default Assistant\n\
            - Agent ID: (unknown)\n\
            - Session ID: (unknown-session)"
        );
        assert!(!prompt.contains("## Session Context"));
        assert!(!prompt.contains("## Workspace Instructions"));
        assert!(!prompt.contains("## Available Tools & Current State"));
    }

    #[test]
    fn test_split_service_context_prompts_deterministic_order() {
        // Create mock service contexts out of order with mixed volatility.
        let mut contexts = std::collections::HashMap::new();

        contexts.insert(
            "tool_c".to_string(),
            crate::mcp::types::ServiceContext::<serde_json::Value> {
                context_prompt: "Context for tool C.".to_string(),
                structured_state: None,
                volatility: crate::mcp::types::ContextVolatility::Volatile,
            },
        );
        contexts.insert(
            "tool_a".to_string(),
            crate::mcp::types::ServiceContext::<serde_json::Value> {
                context_prompt: "Context for tool A.".to_string(),
                structured_state: None,
                volatility: crate::mcp::types::ContextVolatility::Stable,
            },
        );
        contexts.insert(
            "tool_b".to_string(),
            crate::mcp::types::ServiceContext::<serde_json::Value> {
                context_prompt: "Context for tool B.".to_string(),
                structured_state: None,
                volatility: crate::mcp::types::ContextVolatility::Medium,
            },
        );

        let (cacheable_parts, volatile_parts) = split_service_context_prompts(contexts);

        assert_eq!(cacheable_parts, vec!["Context for tool A.".to_string()]);
        assert_eq!(
            volatile_parts,
            vec![
                "Context for tool B.".to_string(),
                "Context for tool C.".to_string()
            ]
        );
    }
}
