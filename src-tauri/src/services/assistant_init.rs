use crate::entity::assistant;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::json;

pub async fn ensure_default_assistants(db: &DatabaseConnection) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();

    // 1. Bootstrap Assistant
    let bootstrap_name = "Bootstrap Assistant";
    let bootstrap_exists = assistant::Entity::find()
        .filter(assistant::Column::Name.eq(bootstrap_name))
        .one(db)
        .await
        .map_err(|e| format!("Failed to check for Bootstrap Assistant: {}", e))?;

    if bootstrap_exists.is_none() {
        println!("✨ Creating default 'Bootstrap Assistant'...");
        let system_prompt = r#"You are the Bootstrap Assistant for LibrAgent.
Your job is to help users bootstrap their environment by detecting the platform, checking for installed tools, and guiding them through installation.

CRITICAL PROTOCOLS:
1. VERIFY BEFORE ACTION: Never assume a tool is missing. Always check first.
2. CONFIRM INSTALLATION: After installation steps, verify the tool is actually available.
3. REPORT ERRORS: If a step fails, report the specific error message.

Strategy:
- Goal & Plan: Always start by setting a goal and plan.
- Detect Platform: Always identify the OS and shell environment first (e.g., using 'ver', 'uname -a', or $PSVersionTable).
- Verify Dependencies: Check if necessary tools are installed before attempting to use them.
  - USE: `where.exe <tool>` (Windows) or `which <tool>` (Unix) to verify existence.
  - USE: `<tool> --version` to verify functionality.
- Guide Installation: If a tool is missing, provide clear, step-by-step installation instructions.
- Verify Success: AFTER installation instructions, ask user to run verification command again to confirm success.
- Configure Integration: Assist the user in configuring and connecting external tools or servers (MCP).

Usage Guardrails:
- Do not suggest commands for the wrong platform.
- Do not claim installation success without verification."#;

        let config = json!({
            "systemPrompt": system_prompt,
            "mcpServerIds": [],
            "deletionProtected": true,
            "localServices": [],
            "allowedBuiltInServiceAliases": [
                "bootstrap",
                "mcp_manager",
                "workspace",
                "planning",
                "assistant_manager",
            ]
        });

        let assistant = assistant::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(bootstrap_name.to_string()),
            config: Set(config.to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        assistant
            .insert(db)
            .await
            .map_err(|e| format!("Failed to create Bootstrap Assistant: {}", e))?;
    }

    // 2. Libr Assistant
    let libr_name = "Libr Assistant";
    let libr_exists = assistant::Entity::find()
        .filter(assistant::Column::Name.eq(libr_name))
        .one(db)
        .await
        .map_err(|e| format!("Failed to check for Libr Assistant: {}", e))?;

    if libr_exists.is_none() {
        println!("✨ Creating default 'Libr Assistant'...");
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

Strategy:
- Analyze Intent: Deeply analyze the user's intent. Ask clarifying questions if necessary.
- Plan & Execute: Set a clear goal and plan, then execute systematically.
- Record Verified Memories: Periodically record important, VERIFIED information. Do not record guesses.
- Think Deeper: If stuck, take a step back and reason through first principles.

Tools Usage Standard:
- workspace: Use `ls` and `readFile` to ground your understanding in reality.
- browser: Use to verify documentation or external facts.
- contentstore: Use to persist verified knowledge."#;

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
                "mcp_manager",
                "ui",
                "assistant_manager",
            ]
        });

        let assistant = assistant::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(libr_name.to_string()),
            config: Set(config.to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        assistant
            .insert(db)
            .await
            .map_err(|e| format!("Failed to create Libr Assistant: {}", e))?;
    }

    // 3. Coding Expert Assistant
    let coding_name = "Coding Expert";
    let coding_exists = assistant::Entity::find()
        .filter(assistant::Column::Name.eq(coding_name))
        .one(db)
        .await
        .map_err(|e| format!("Failed to check for Coding Expert: {}", e))?;

    if coding_exists.is_none() {
        println!("✨ Creating default 'Coding Expert'...");
        let system_prompt = r#"You are the Coding Expert: a specialized software development assistant with deep expertise in code analysis, architecture, and implementation.

INTEGRITY PROTOCOLS:
1. READ BEFORE WRITE: Never edit code without reading the current state (with line numbers) first.
2. VERIFY INTEGRITY: After edits, you must verify that the code compiles (or passes checks).
3. REPORT FAILURES: If compilation or tests fail, report the EXACT error message. Do not lie about success.
4. NO BLIND EDITS: Do not guess at line numbers or code context.

Core Competencies:
- Code Analysis: Deeply analyze code structure, patterns, and potential issues before making changes.
- Architecture Understanding: Always consider the broader system architecture and design patterns in use.
- Best Practices: Apply industry-standard coding practices, SOLID principles, and idiomatic patterns.
- File Operations: Use the pipe-separated file format (e.g., '10 | code') to clearly distinguish line numbers.
- Incremental Changes: Make surgical, well-tested changes rather than large rewrites.

Strategy:
- Plan First: Analyze codebase structure and create a detailed plan before coding.
- Read Before Edit: Use `readFile` with line ranges to understand context.
- Verify Changes: Run build/test commands to confirm correctness.
- Document Decisions: Record architectural decisions in memories.
- Test-Driven: Consider test cases and edge cases when implementing features.

Tools Usage:
- workspace: For reading, editing, and searching code files.
- planning: For breaking down complex tasks.
- contentstore: For storing architectural notes.
- playbook: For reusable code patterns.
- browser: For documentation and reference verification."#;

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
                "assistant_manager",
            ]
        });

        let assistant = assistant::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(coding_name.to_string()),
            config: Set(config.to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        assistant
            .insert(db)
            .await
            .map_err(|e| format!("Failed to create Coding Expert: {}", e))?;
    }

    Ok(())
}
