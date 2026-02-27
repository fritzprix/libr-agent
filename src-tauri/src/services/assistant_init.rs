use crate::repositories::AssistantRepository;
use serde_json::{json, Value};

fn mastermind_system_prompt() -> &'static str {
    r#"You are Master Mind: the command orchestrator for complex, high-impact missions.
You coordinate strategy, delegate execution to specialist assistants, and keep shared knowledge coherent under pressure.

PRIME DIRECTIVE:
Deliver reliable outcomes by combining planning discipline, evidence-based execution, and continuous situational awareness.

AUTONOMY CHARTER:
1. AI AGENCY: Respect specialist autonomy and decision quality. Do not micromanage execution details that can be handled by capable agents.
2. DELEGATION DEFAULT: Prefer delegation for efficiency and throughput.
3. DIRECT ACTION ALLOWED: Direct execution is always allowed when speed, clarity, or risk control justifies it.
4. RISK-AWARE CHOICE: Choose delegation vs direct action by expected reliability, latency, and token cost.
5. RECOVERY DUTY: If a path fails, provide immediate fallback and continue mission flow.
6. NO ZOMBIE MODE: Avoid rigid hard bans except explicit security/safety constraints.

COMMAND PROTOCOL:
1. MISSION CONTROL: Define objective, constraints, and success criteria before action.
2. ORCHESTRATION: Break work into tracked steps, assign priorities, and route work to the right specialist.
3. EVIDENCE FIRST: Never claim completion without verification (files, commands, tool outputs, current state).
4. KNOWLEDGE OPERATIONS: Capture critical findings (IDs, paths, decisions, risks) and reuse them deliberately.
5. REAL-TIME INTELLIGENCE: Pull fresh information via available tools when uncertainty exists; avoid stale assumptions.

ATTENTION ECONOMY:
- Keep active tool families minimal for each phase.
- Prefer delegation over direct multi-tool thrashing.
- Require explicit reason before switching tool domains.

KNOWLEDGE LOOP:
- Persist critical discoveries to shared memory.
- Retrieve and reconcile prior knowledge before major decisions.
- Prefer reusable knowledge over repeating expensive investigation.

SPECIALIST COORDINATION MODEL:
- Libr Assistant: general field operations and cross-domain execution.
- Coding Expert: implementation/refactor/debug execution.
- App Wizard: environment, MCP, assistant configuration execution.
- Master Mind: strategy, delegation, quality gates, conflict resolution.

OPERATING STYLE:
- Strategic, decisive, and explicit
- No vague status reports
- No hidden assumptions
- Clear next action at every step

FAILSAFE RULES:
- If required data is missing, ask precise questions.
- If a command or tool fails, report exact failure and recovery path.
- If risk escalates, recommend controlled rollback or containment."#
}

async fn ensure_assistant_description(
    repo: &crate::repositories::SqliteAssistantRepository,
    assistant_name: &str,
    description: &str,
) -> Result<(), String> {
    let assistants = repo
        .list_assistants()
        .await
        .map_err(|e| format!("Failed to list assistants for description backfill: {}", e))?;

    let Some(target) = assistants
        .into_iter()
        .find(|assistant| assistant.name == assistant_name)
    else {
        return Ok(());
    };

    let mut config_value =
        serde_json::from_str::<Value>(&target.config).unwrap_or_else(|_| json!({}));
    let has_description = config_value
        .get("description")
        .and_then(|value| value.as_str())
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);

    if has_description {
        return Ok(());
    }

    config_value["description"] = Value::String(description.to_string());

    repo.update_assistant(&target.id, None, Some(config_value.to_string()))
        .await
        .map_err(|e| {
            format!(
                "Failed to backfill description for {}: {}",
                assistant_name, e
            )
        })?;

    Ok(())
}

async fn ensure_assistant_system_prompt(
    repo: &crate::repositories::SqliteAssistantRepository,
    assistant_name: &str,
    system_prompt: &str,
) -> Result<(), String> {
    let assistants = repo
        .list_assistants()
        .await
        .map_err(|e| format!("Failed to list assistants for prompt update: {}", e))?;

    let Some(target) = assistants
        .into_iter()
        .find(|assistant| assistant.name == assistant_name)
    else {
        return Ok(());
    };

    let mut config_value =
        serde_json::from_str::<Value>(&target.config).unwrap_or_else(|_| json!({}));
    let current_prompt = config_value
        .get("systemPrompt")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();

    if current_prompt == system_prompt.trim() {
        return Ok(());
    }

    config_value["systemPrompt"] = Value::String(system_prompt.to_string());

    repo.update_assistant(&target.id, None, Some(config_value.to_string()))
        .await
        .map_err(|e| {
            format!(
                "Failed to update systemPrompt for {}: {}",
                assistant_name, e
            )
        })?;

    Ok(())
}

pub async fn ensure_default_assistants() -> Result<(), String> {
    // 1. Libr Assistant
    let repo = crate::get_assistant_repository();
    let libr_name = "Libr Assistant";
    let libr_description =
        "Field operations specialist for verified research, execution, and cross-domain task delivery.";
    let libr_exists = repo
        .check_assistant_exists(libr_name)
        .await
        .map_err(|e| format!("Failed to check for Libr Assistant: {}", e))?;

    if !libr_exists {
        println!("Creating default 'Libr Assistant'...");
        let system_prompt = r#"You are the Libr Assistant: a field operations specialist in the Master Mind command structure.
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
Your conversation context is limited. For complex tasks: establish persistent goals, save critical findings to working memory (limit ~10 items), and reference saved information instead of re-gathering.

TEAM DOCTRINE:
- When a higher-level plan exists, execute your assigned part with discipline.
- Report concrete outcomes, blockers, and evidence for command-level decisions.

ATTENTION ECONOMY:
- Prefer the smallest viable toolset per step.
- Do not hop tools unless current evidence requires it.
- Finish one investigative thread before opening another."#;

        let config = json!({
            "description": libr_description,
            "systemPrompt": system_prompt,
            "mcpServerIds": [],
            "deletionProtected": true,
            "localServices": [],
            "allowedBuiltInServiceAliases": [
                "knowledge",
                "attachments",
                "workspace",
                "browser",
                "planning",
                "playbook"
            ]
        });

        let id = uuid::Uuid::new_v4().to_string();
        repo.create_assistant(id, libr_name.to_string(), config.to_string())
            .await
            .map_err(|e| format!("Failed to create Libr Assistant: {}", e))?;
    }

    ensure_assistant_description(repo, libr_name, libr_description).await?;

    // 2. Coding Expert Assistant
    let coding_name = "Coding Expert";
    let coding_description =
        "Engineering execution specialist for implementation, refactoring, debugging, and verification.";
    let coding_exists = repo
        .check_assistant_exists(coding_name)
        .await
        .map_err(|e| format!("Failed to check for Coding Expert: {}", e))?;

    if !coding_exists {
        println!("Creating default 'Coding Expert'...");
        let system_prompt = r#"You are the Coding Expert: an engineering execution specialist under Master Mind command.
    You handle implementation-heavy software tasks with precision and evidence.

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
- Make surgical, incremental changes

TEAM DOCTRINE:
- When strategy is provided, translate it into safe, verifiable code changes.
- Return exact results, diffs, and technical risks for command-level review.

ATTENTION ECONOMY:
- Stay in code-analysis/edit/verification loop unless mission scope changes.
- Avoid unnecessary tool switching during implementation."#;

        let config = json!({
            "description": coding_description,
            "systemPrompt": system_prompt,
            "mcpServerIds": [],
            "deletionProtected": true,
            "localServices": [],
            "allowedBuiltInServiceAliases": [
                "workspace",
                "planning",
                "knowledge",
                "attachments",
                "playbook"
            ]
        });

        let id = uuid::Uuid::new_v4().to_string();
        repo.create_assistant(id, coding_name.to_string(), config.to_string())
            .await
            .map_err(|e| format!("Failed to create Coding Expert: {}", e))?;
    }

    ensure_assistant_description(repo, coding_name, coding_description).await?;

    // 3. App Wizard (Setup Assistant)
    let wizard_name = "App Wizard";
    let wizard_description =
        "Environment and configuration specialist for MCP setup, assistant management, and system readiness.";
    let wizard_exists = repo
        .check_assistant_exists(wizard_name)
        .await
        .map_err(|e| format!("Failed to check for App Wizard: {}", e))?;

    if !wizard_exists {
        println!("Creating default 'App Wizard'...");
        let system_prompt = r#"You are the App Wizard: an environment and systems setup specialist under Master Mind command.
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
3. ENVIRONMENT: Detect OS, verify dependencies, guide installation, validate readiness.

TEAM DOCTRINE:
- Execute setup plans reliably and surface operational risks early.
- Provide exact verification checkpoints for command-level go/no-go decisions.

ATTENTION ECONOMY:
- Stay focused on environment/configuration operations.
- Only escalate to broader tools when setup verification demands it."#;

        let config = json!({
            "description": wizard_description,
            "systemPrompt": system_prompt,
            "mcpServerIds": [],
            "deletionProtected": true,
            "localServices": [],
            "allowedBuiltInServiceAliases": [
                "bootstrap",
                "mcp_manager",
                "assistant",
                "workspace",
                "planning",
                "knowledge",
                "attachments"
            ]
        });

        let id = uuid::Uuid::new_v4().to_string();
        repo.create_assistant(id, wizard_name.to_string(), config.to_string())
            .await
            .map_err(|e| format!("Failed to create App Wizard: {}", e))?;
    }

    ensure_assistant_description(repo, wizard_name, wizard_description).await?;

    // 4. Master Mind (Orchestrator)
    let mastermind_name = "Master Mind";
    let mastermind_description =
        "Command orchestrator that plans strategy, delegates to specialists, and enforces quality gates.";
    let mastermind_exists = repo
        .check_assistant_exists(mastermind_name)
        .await
        .map_err(|e| format!("Failed to check for Master Mind: {}", e))?;

    if !mastermind_exists {
        println!("Creating default 'Master Mind'...");
        let system_prompt = mastermind_system_prompt();

        let config = json!({
            "description": mastermind_description,
            "systemPrompt": system_prompt,
            "mcpServerIds": [],
            "deletionProtected": true,
            "localServices": [],
            "allowedBuiltInServiceAliases": [
                "planning",
                "knowledge",
                "attachments",
                "playbook",
                "assistant",
                "swarm"
            ]
        });

        let id = uuid::Uuid::new_v4().to_string();
        repo.create_assistant(id, mastermind_name.to_string(), config.to_string())
            .await
            .map_err(|e| format!("Failed to create Master Mind: {}", e))?;
    }

    ensure_assistant_description(repo, mastermind_name, mastermind_description).await?;
    ensure_assistant_system_prompt(repo, mastermind_name, mastermind_system_prompt()).await?;

    Ok(())
}
