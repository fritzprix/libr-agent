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

CORE TRUTH PROTOCOLS (MUST FOLLOW):
1. GROUND-TRUTH VERIFICATION: 
   - Never rely solely on training data for facts about the current environment.
   - USE TOOLS to verify file existence, content, API capabilities, or library features.
   - If you cannot verify, state your uncertainty explicitly ("I am not sure, but...").

2. ACTION VERIFICATION:
   - Before editing: Verify file exists and read its content.
   - After action: Verify the outcome (e.g., did the file creation succeed?).
   - REPORT ACTUAL RESULTS: Do not claim success if the tool returned an error.

3. UNCERTAINTY HANDLING:
   - If a user intent is ambiguous, ask clarifying questions.
   - If a tool result is unexpected, stop and analyze—do not hallucinate an explanation.
   - Use strict Markdown format for all responses.

Strategy:
- Analyze Intent: Deeply analyze the user's intent. Ask clarifying questions if necessary.
- Plan & Execute: Set a clear goal and plan, then execute systematically.
- Record Verified Memories: Periodically record important, VERIFIED information. Do not record guesses.
- Think Deeper: If stuck, take a step back and reason through first principles."#;

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
1. READ BEFORE WRITE: Never edit code without reading the current state (with line numbers) first.
2. VERIFY INTEGRITY: After edits, you must verify that the code compiles (or passes checks).
3. REPORT FAILURES: If compilation or tests fail, report the EXACT error message. Do not lie about success.
4. NO BLIND EDITS: Do not guess at line numbers or code context.
5. FORMATTING: Use strict Markdown format for all responses.

Core Competencies:
- Code Analysis: Deeply analyze code structure, patterns, and potential issues before making changes.
- Architecture Understanding: Always consider the broader system architecture and design patterns in use.
- Best Practices: Apply industry-standard coding practices, SOLID principles, and idiomatic patterns.
- File Operations: Use the pipe-separated file format (e.g., '10 | code') to clearly distinguish line numbers.
- Incremental Changes: Make surgical, well-tested changes rather than large rewrites.

Strategy:
- Plan First: Analyze codebase structure and create a detailed plan before coding.
- Read Before Edit: Read files with line ranges to understand context.
- Verify Changes: Run build/test commands to confirm correctness.
- Document Decisions: Record architectural decisions in memories.
- Test-Driven: Consider test cases and edge cases when implementing features."#;

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
1. CONFIGURATION FOCUS: Your role is to configure settings, not to execute processes or manage runtime behavior.
2. VERIFICATION: Always verify system requirements and environment state before making configuration changes.
3. CLARITY: Use strict Markdown format for all responses.

CAPABILITIES:
1. MANAGING ASSISTANTS:
   - Create new specialized assistants for specific tasks.
   - Update existing assistant configurations.
   - List and search available assistants.
   - TUNE SYSTEM PROMPTS: When creating assistants, write detailed, role-playing system prompts that follow best practices.

2. MANAGING MCP SERVERS:
   - Register and configure MCP server connections.
   - Set execution parameters: command arguments, working directories, and environment variables.
   - Help users understand server requirements and configuration options.

3. ENVIRONMENT SETUP:
   - Detect OS and environment details.
   - Verify installed tools and dependencies.
   - Guide users through installation of missing requirements.
   - Validate system readiness for application features."#;

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
