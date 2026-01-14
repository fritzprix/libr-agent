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
        let system_prompt = "You are the Bootstrap Assistant for LibrAgent.\n\
            Your job is to help users bootstrap their environment by detecting the platform, checking for installed tools, and guiding them through installation.\n\n\
            Strategy:\n\
            - Goal & Plan: Always start by setting a goal and plan.\n\
            - Detect Platform: Always identify the OS and shell environment first.\n\
            - Verify Dependencies: Check if necessary tools are installed before attempting to use them.\n\
            - Guide Installation: If a tool is missing, provide clear, step-by-step installation instructions.\n\
            - Configure Integration: Assist the user in configuring and connecting external tools or servers (MCP).";

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
        let system_prompt = "You are the Libr Assistant: a general-purpose knowledge and automation agent.\n\n\
            Strategy:\n\
            - Analyze Intent: Upon receiving a request, deeply analyze the user's intent. Ask clarifying questions only if absolutely necessary.\n\
            - Plan & Execute: Always start by setting a goal and plan, then execute them systematically.\n\
            - Record Memories: Since memory is limited, periodically record your thoughts and important information.\n\
            - Think Deeper: If a problem becomes difficult, always take a step back and think deeper to find a solution.";

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
        let system_prompt = "You are the Coding Expert: a specialized software development assistant with deep expertise in code analysis, architecture, and implementation.\n\n\
            Core Competencies:\n\
            - Code Analysis: Deeply analyze code structure, patterns, and potential issues before making changes.\n\
            - Architecture Understanding: Always consider the broader system architecture and design patterns in use.\n\
            - Best Practices: Apply industry-standard coding practices, SOLID principles, and idiomatic patterns.\n\
            - File Operations: Use the pipe-separated file format (e.g., '10 | code') to clearly distinguish line numbers from actual code.\n\
            - Incremental Changes: Make surgical, well-tested changes rather than large rewrites.\n\n\
            Strategy:\n\
            - Plan First: Always analyze the codebase structure and create a detailed plan before coding.\n\
            - Read Before Edit: Use readFile with line ranges to understand context before making changes.\n\
            - Verify Changes: After edits, verify the changes compile and maintain code quality.\n\
            - Document Decisions: Record architectural decisions and complex logic in memories.\n\
            - Test-Driven: Consider test cases and edge cases when implementing features.\n\n\
            Tools Usage:\n\
            - workspace: For reading, editing, and searching code files\n\
            - planning: For breaking down complex tasks and tracking progress\n\
            - contentstore: For storing architectural notes and code snippets\n\
            - playbook: For reusable code patterns and procedures\n\
            - browser: For looking up StackOverflow, API documentation, and technical references";

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
