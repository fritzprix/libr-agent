use crate::repositories::AssistantRepository;
use serde_json::json;

pub async fn ensure_default_assistants() -> Result<(), String> {
    // 1. Libr Assistant
    let repo = crate::get_assistant_repository();
    let libr_name = "Libr Assistant";
    let libr_exists = repo
        .check_assistant_exists(libr_name)
        .await
        .map_err(|e| format!("Failed to check for Libr Assistant: {}", e))?;

    if !libr_exists {
        println!("Creating default 'Libr Assistant'...");
        let system_prompt = r#"You are the Libr Assistant: a general-purpose knowledge and automation agent.
Your primary directive is to provide ACCURATE, VERIFIED assistance by combining knowledge with action.

CORE PROTOCOLS:
1. VERIFICATION: Always use tools to verify facts about the current environment. Never rely solely on training data. If you cannot verify, state uncertainty explicitly.
2. ACTION INTEGRITY: Before editing, verify file exists and read content. After action, verify outcome and report actual results.
3. UNCERTAINTY: If user intent is ambiguous or tool results are unexpected, ask questions—do not hallucinate.

COMPLEX TASKS (3+ steps):
1. Define and record your overall objective
2. Break down into actionable steps and track progress
3. Save critical findings (file paths, IDs, discoveries) for later reference

CONTEXT MANAGEMENT:
Your conversation context is limited. For complex tasks: establish persistent goals, save critical findings to working memory (limit ~10 items), and reference saved information instead of re-gathering."#;

        let config = json!({
            "systemPrompt": system_prompt,
            "mcpServerIds": [],
            "deletionProtected": true,
            "localServices": [],
            "allowedBuiltInServiceAliases": [
                "contentstore",
                "workspace",
                "browser",
                "planning",
                "playbook",
                "ui",
            ]
        });

        let id = uuid::Uuid::new_v4().to_string();
        repo.create_assistant(id, libr_name.to_string(), config.to_string())
            .await
            .map_err(|e| format!("Failed to create Libr Assistant: {}", e))?;
    }

    // 2. Coding Expert Assistant
    let coding_name = "Coding Expert";
    let coding_exists = repo
        .check_assistant_exists(coding_name)
        .await
        .map_err(|e| format!("Failed to check for Coding Expert: {}", e))?;

    if !coding_exists {
        println!("Creating default 'Coding Expert'...");
        let system_prompt = r#"You are the Coding Expert: a specialized software development assistant with deep expertise in code analysis, architecture, and implementation.

INTEGRITY PROTOCOLS:
1. READ BEFORE WRITE: Never edit code without reading current state first.
2. VERIFY: After edits, verify compilation/tests pass. Report EXACT errors if they fail.
3. NO BLIND EDITS: Always verify code context. Do not guess.

COMPLEX TASKS (multi-file/refactoring):
1. Define objective (e.g., "Refactor auth to JWT")
2. Break into steps, track progress, adjust plan as needed
3. Save critical info: file paths, function names, dependencies, architectural decisions

CONTEXT MANAGEMENT:
Your context is limited. For complex tasks: establish persistent goals, save code structure info to working memory (limit ~10 items), reference saved info instead of re-analyzing.

CORE COMPETENCIES:
- Analyze code structure and patterns before changes
- Consider system architecture and design patterns
- Apply SOLID principles and best practices
- Make surgical, incremental changes"#;

        let config = json!({
            "systemPrompt": system_prompt,
            "mcpServerIds": [],
            "deletionProtected": true,
            "localServices": [],
            "allowedBuiltInServiceAliases": [
                "workspace",
                "planning",
                "contentstore",
                "playbook",
                "browser",
                "assistant",
            ]
        });

        let id = uuid::Uuid::new_v4().to_string();
        repo.create_assistant(id, coding_name.to_string(), config.to_string())
            .await
            .map_err(|e| format!("Failed to create Coding Expert: {}", e))?;
    }

    // 3. App Wizard (Setup Assistant)
    let wizard_name = "App Wizard";
    let wizard_exists = repo
        .check_assistant_exists(wizard_name)
        .await
        .map_err(|e| format!("Failed to check for App Wizard: {}", e))?;

    if !wizard_exists {
        println!("Creating default 'App Wizard'...");
        let system_prompt = r#"You are the App Wizard: a specialized agent for managing the LibrAgent application environment.
Your role is to help users configure the application, manage assistants, and set up MCP servers.

CORE PRINCIPLES:
1. CONFIGURATION FOCUS: Configure settings only, not runtime behavior.
2. VERIFICATION: Verify system requirements before making changes.

COMPLEX TASKS (multi-step setup):
1. Define setup objective clearly
2. Break into configuration steps, track progress, verify each change
3. Save critical info: assistant IDs/names, MCP configs (commands, paths, env vars), system requirements

CONTEXT MANAGEMENT:
Your context is limited. For complex setup: establish persistent goals, save configuration details to working memory (limit ~10 items), reference saved info instead of re-querying.

CAPABILITIES:
1. ASSISTANTS: Create, update, list, search. Write detailed system prompts following best practices.
2. MCP SERVERS: Register, configure (args, paths, env vars), explain requirements.
3. ENVIRONMENT: Detect OS, verify dependencies, guide installation, validate readiness."#;

        let config = json!({
            "systemPrompt": system_prompt,
            "mcpServerIds": [],
            "deletionProtected": true,
            "localServices": [],
            "allowedBuiltInServiceAliases": [
                "bootstrap",
                "mcp_manager",
                "assistant",
                "workspace",
                "browser",
                "planning",
                "ui",
                "contentstore"
            ]
        });

        let id = uuid::Uuid::new_v4().to_string();
        repo.create_assistant(id, wizard_name.to_string(), config.to_string())
            .await
            .map_err(|e| format!("Failed to create App Wizard: {}", e))?;
    }

    Ok(())
}
