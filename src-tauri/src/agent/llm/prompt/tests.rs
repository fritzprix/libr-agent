use super::*;
use crate::agent::context::registry::ContextRegistry;
use crate::mcp::service_proxy::MCPServiceProxy;
use std::sync::Arc;

#[tokio::test]
async fn first_prompt_waits_for_inflight_hook_context() {
    // #2812 G3.1: the prompt path must wait for an in-flight SessionStart
    // hook and carry its context on the very first LLM request.
    use crate::services::hooks::context::{
        begin_hook_run, clear_hook_context, end_hook_run, set_hook_context,
    };
    use std::time::Duration;

    let sid = "test-prompt-hook-wait-1";
    clear_hook_context(sid);
    begin_hook_run(sid);
    let sid_task = sid.to_string();
    let task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        set_hook_context(&sid_task, "superpowers bootstrap".to_string());
        end_hook_run(&sid_task);
    });

    let (cacheable, _volatile) = build_volatile_sections_split(None, None, None, Some(sid)).await;
    assert!(cacheable.contains("## Plugin Hook Context"));
    assert!(
        cacheable.contains("superpowers bootstrap"),
        "first prompt must carry the hook output: {cacheable}"
    );
    task.await.unwrap();
    clear_hook_context(sid);
}

#[tokio::test]
async fn turn_hook_context_is_injected_once_into_volatile_prompt() {
    use crate::services::hooks::context::{clear_hook_context, push_turn_hook_context};

    let sid = "prompt-turn-hook-context-once";
    clear_hook_context(sid);
    push_turn_hook_context(sid, "user-prompt-submit extra context");

    let (cacheable_1, volatile_1) =
        build_volatile_sections_split(None, None, None, Some(sid)).await;
    assert!(!cacheable_1.contains("user-prompt-submit extra context"));
    assert!(volatile_1.contains("## Plugin Hook Turn Context"));
    assert!(volatile_1.contains("user-prompt-submit extra context"));

    let (_cacheable_2, volatile_2) =
        build_volatile_sections_split(None, None, None, Some(sid)).await;
    assert!(
        !volatile_2.contains("user-prompt-submit extra context"),
        "turn hook context must be consumed on the first LLM request of the turn"
    );
    clear_hook_context(sid);
}

#[tokio::test]
async fn first_prompt_waits_through_session_start_runner() {
    // #2812 G4.1: `run_session_start_in` arms a blank placeholder before
    // the command finishes. The prompt path must still wait and carry
    // the hook output. Observing the blank entry first proves the call
    // happens inside that window, not before the runner starts.
    use crate::services::hooks::context::{
        begin_hook_run, clear_hook_context, end_hook_run, get_hook_context,
    };
    use crate::services::hooks::envelope::SessionStartSource;
    use crate::services::hooks::execute::run_session_start_in;
    use crate::services::hooks::registry::{
        compute_handler_approval_hashes, save_source_in, PluginSourceManifest,
    };
    use crate::services::hooks::schema::load_hooks_from_root;
    use std::fs;
    use std::time::{Duration, Instant};

    #[cfg(target_os = "windows")]
    let hooks_json = r#"{ "hooks": { "SessionStart": [ { "command": "ping -n 3 127.0.0.1 >nul && echo slow hook body" } ] } }"#;
    #[cfg(not(target_os = "windows"))]
    let hooks_json =
        r#"{ "hooks": { "SessionStart": [ { "command": "sleep 0.8 && echo slow hook body" } ] } }"#;

    let temp = tempfile::tempdir().expect("tempdir");
    let source_id = "plugin-0123456789abcdef".to_string();
    let root = temp.path().join(&source_id);
    fs::create_dir_all(root.join("hooks")).unwrap();
    fs::write(root.join("hooks").join("hooks.json"), hooks_json).unwrap();
    let cfg = load_hooks_from_root(&root).unwrap();
    let hashes = compute_handler_approval_hashes(&root, &cfg);
    save_source_in(
        temp.path(),
        PluginSourceManifest {
            mcp_servers: Vec::new(),
            mcp_approved: false,
            source_url: "test://local".to_string(),
            source_id,
            skill_names: vec![],
            hooks: None,
            hooks_approved: true,
            approval_hashes: hashes,
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![],
            enabled: true,
            installed_commit: None,
        },
    )
    .unwrap();

    let sid = format!("prompt-runner-{}", uuid::Uuid::new_v4());
    clear_hook_context(&sid);
    begin_hook_run(&sid);
    let dir = temp.path().to_path_buf();
    let run_sid = sid.clone();
    let run_root = root.clone();
    let run = tokio::spawn(async move {
        run_session_start_in(&dir, &run_sid, SessionStartSource::Startup, Some(run_root)).await;
        end_hook_run(&run_sid);
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let blank = get_hook_context(&sid).is_some_and(|text| text.trim().is_empty());
        if blank {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "runner never stored the blank placeholder"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let (cacheable, _volatile) = build_volatile_sections_split(None, None, None, Some(&sid)).await;
    assert!(
        cacheable.contains("slow hook body"),
        "first prompt must carry the hook output: {cacheable}"
    );
    assert!(cacheable.contains("## Plugin Hook Context"));
    run.await.unwrap();
    clear_hook_context(&sid);
}

#[tokio::test]
async fn prompt_without_hook_run_does_not_wait() {
    // No marker, no context: the prompt path proceeds immediately.
    let sid = "test-prompt-hook-wait-none";
    crate::services::hooks::clear_hook_context(sid);
    let (cacheable, _volatile) = build_volatile_sections_split(None, None, None, Some(sid)).await;
    assert!(!cacheable.contains("## Plugin Hook Context"));
}

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
    assert!(prompt.contains("External wake (this session): hooks, webhooks"));
    assert!(prompt.contains("must POST to $LIBRAGENT_WAKE_URL"));
    assert!(
        !prompt.contains("/api/sessions/(unknown-session)/messages"),
        "external hooks must not be pointed at the main Session API"
    );
    assert!(prompt.contains(r#"curl -s -X POST "$LIBRAGENT_WAKE_URL""#));
    assert!(
        !prompt.contains("/wake/"),
        "wake token URL must never be placed in the prompt"
    );

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

    assert!(prompt.starts_with(
        "Base prompt only.\n\n\n## Agent Runtime Identity\n\
        - Agent Name: Default Assistant\n\
        - Agent ID (Config ID): (unknown)\n\
        - Session ID: (unknown-session)"
    ));
    assert!(prompt.contains("External wake (this session): hooks, webhooks"));
    assert!(!prompt.contains("/api/sessions/(unknown-session)/messages"));
    assert!(prompt.contains("## Session Context"));
    assert!(prompt.contains("<session-context>"));
    assert!(!prompt.contains("passive environment reference only"));
    assert!(!prompt.contains("## Session Context Handling"));
    assert!(!prompt.contains("## Workspace Instructions"));
    assert!(!prompt.contains("## Available Tools & Current State"));
}

fn identity_with_workspace(workspace_root: Option<&str>) -> String {
    let agent_config = crate::agent::AgentConfig {
        id: Some("a".to_string()),
        name: "Agent".to_string(),
        ..Default::default()
    };
    build_stable_prefix(&agent_config, "s-1", workspace_root, None, vec![])
}

#[test]
fn docker_session_shows_the_container_workdir_as_workspace_root() {
    let host = std::path::Path::new("/home/user/.libragent/workspaces/s-1");
    let root = workspace_root_for_prompt(true, host, "/app/");
    assert_eq!(root, "/app");

    let prompt = identity_with_workspace(Some(&root));
    assert!(prompt.contains("- Workspace root (absolute): /app\n"));
    assert!(!prompt.contains("/home/user/.libragent"));
    assert!(prompt.contains("workspaceOverride=\"current\""));
    assert_eq!(workspace_root_for_prompt(true, host, "/"), "/");
}

#[test]
fn host_session_keeps_the_host_workspace_root() {
    let host = std::path::Path::new("/home/user/.libragent/workspaces/s-1");
    let root = workspace_root_for_prompt(false, host, "/workspace");
    assert_eq!(root, host.display().to_string());
    assert!(identity_with_workspace(Some(&root))
        .contains("- Workspace root (absolute): /home/user/.libragent/workspaces/s-1\n"));
    assert!(!identity_with_workspace(None).contains("Workspace root"));
}

#[test]
fn test_active_http_port_prefers_in_process_bound_port() {
    crate::server::set_active_http_port(34567);
    assert_eq!(crate::server::active_http_port(), 34567);
    // Reset so other tests keep the default fallback behavior.
    crate::server::set_active_http_port(0);
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
