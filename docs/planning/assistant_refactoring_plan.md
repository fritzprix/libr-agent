# Assistant Module Refactoring Plan

**Date**: January 15, 2026  
**Status**: Planning  
**Priority**: HIGH  
**Estimated Effort**: 8-12 hours

---

## Executive Summary

The Assistant module requires refactoring to align with builtin tool best practices. While the module has a solid foundation with proper error handling and comprehensive tests, it lacks AI-compatible tool descriptions, dynamic service context, and thorough input validation.

**Current Score**: 6.3/10  
**Target Score**: 9.0/10

---

## Table of Contents

1. [Phase 1: High Priority Fixes](#phase-1-high-priority-fixes)
2. [Phase 2: Medium Priority Improvements](#phase-2-medium-priority-improvements)
3. [Phase 3: Low Priority Enhancements](#phase-3-low-priority-enhancements)
4. [Testing Strategy](#testing-strategy)
5. [Rollout Plan](#rollout-plan)

---

## Phase 1: High Priority Fixes

### 1.1 Implement Dynamic Service Context

**Current Issue**: Static context provides no runtime information about available assistants.

**Goal**: AI agents should see current assistant count and recent assistants in system prompt.

**Implementation**:

```rust
// Add cache field to AssistantServer struct
pub struct AssistantServer {
    db: Arc<DatabaseConnection>,
    context_cache: Arc<RwLock<Option<(String, Instant)>>>,
}

impl AssistantServer {
    pub async fn new(db: Arc<DatabaseConnection>) -> Result<Self, String> {
        Ok(Self {
            db,
            context_cache: Arc::new(RwLock::new(None)),
        })
    }

    fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.context_cache.write() {
            *cache = None;
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for AssistantServer {
    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        const CACHE_TTL_SECS: u64 = 10;

        // Check cache
        {
            let cache = self.context_cache.read().unwrap();
            if let Some((text, timestamp)) = cache.as_ref() {
                if timestamp.elapsed().as_secs() < CACHE_TTL_SECS {
                    return ServiceContext {
                        context_prompt: text.clone(),
                        structured_state: None,
                    };
                }
            }
        }

        // Fetch fresh data
        let db = self.get_db();

        let total_count = AssistantEntity::find()
            .count(db)
            .await
            .unwrap_or(0);

        let recent_assistants = AssistantEntity::find()
            .order_by_desc(assistant::Column::UpdatedAt)
            .limit(5)
            .all(db)
            .await
            .unwrap_or_default();

        let context = if total_count == 0 {
            "## Assistant Configuration\n\n\
            **Status**: No assistants configured\n\
            **Available Actions**:\n\
            • Use builtin_assistant__createAssistant to create your first assistant\n\
            • Assistants are global and reusable across all sessions\n".to_string()
        } else {
            let assistant_list = recent_assistants
                .iter()
                .map(|a| {
                    let config = serde_json::from_str::<Value>(&a.config)
                        .unwrap_or(json!({}));
                    let model = config.get("modelName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("default");
                    format!("  • {} (ID: {}, Model: {})", a.name, a.id, model)
                })
                .collect::<Vec<_>>()
                .join("\n");

            let showing_text = if total_count > 5 {
                format!("(showing 5 most recent of {})", total_count)
            } else {
                String::new()
            };

            format!(
                "## Assistant Configuration\n\n\
                **Total Assistants**: {}\n\
                **Recent Assistants** {}:\n\
                {}\n\n\
                💡 Use builtin_assistant__listAssistants to see all assistants\n\
                💡 Use builtin_assistant__getAssistant(id) to view full configuration",
                total_count,
                showing_text,
                assistant_list
            )
        };

        // Update cache
        {
            let mut cache = self.context_cache.write().unwrap();
            *cache = Some((context.clone(), Instant::now()));
        }

        ServiceContext {
            context_prompt: context,
            structured_state: Some(json!({
                "total_count": total_count,
                "recent_count": recent_assistants.len()
            })),
        }
    }
}
```

**Cache Invalidation Points**:

- After `create_assistant` succeeds
- After `update_assistant` succeeds
- After `delete_assistant` succeeds

**Add to each operation**:

```rust
// At the end of successful operations
self.invalidate_cache();
```

**Files to Modify**:

- `src-tauri/src/mcp/builtin/assistant/mod.rs`

**Estimated Time**: 2 hours

---

### 1.2 Add AI-Compatible Tool Descriptions

**Current Issue**: Minimal descriptions without workflow guidance.

**Goal**: Each tool description should include CRITICAL WORKFLOW, common errors, and next steps.

**Template to Follow**:

```rust
description: "[Brief description of what tool does]

⚠️ CRITICAL WORKFLOW:
1. Step-by-step process
2. Include prerequisites
3. Explain parameter relationships

💡 COMMON USE CASES:
• Use case 1
• Use case 2

❌ COMMON ERRORS:
• Error pattern 1: How to fix
• Error pattern 2: How to fix

✓ SUCCESS: What happens after successful execution".to_string(),
```

**Specific Tool Descriptions**:

#### createAssistant

```rust
fn create_create_assistant_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "id".to_string(),
        string_prop_with_desc(
            "Unique assistant identifier (alphanumeric, hyphens, underscores allowed).\n\
            If omitted, a CUID will be auto-generated.\n\
            Examples: 'code-reviewer-v2', 'customer-support-bot', 'data-analyst'",
            false,
        ),
    );

    props.insert(
        "name".to_string(),
        string_prop_with_desc(
            "Human-readable assistant name (1-100 characters).\n\
            Examples: 'Code Reviewer', 'Customer Support Bot', 'Data Analyst'",
            true,
        ),
    );

    props.insert(
        "systemPrompt".to_string(),
        string_prop_with_desc(
            "System prompt that defines assistant behavior and personality.\n\
            This text is prepended to every conversation.\n\
            Example: 'You are a helpful code reviewer focusing on best practices.'",
            false,
        ),
    );

    props.insert(
        "modelProvider".to_string(),
        string_prop_with_desc(
            "AI model provider name.\n\
            Supported: 'openai', 'anthropic', 'google', 'openrouter', 'ollama', 'deepseek', 'xai'\n\
            Example: 'openai'",
            false,
        ),
    );

    props.insert(
        "modelName".to_string(),
        string_prop_with_desc(
            "Specific model identifier.\n\
            Examples: 'gpt-4', 'claude-3-5-sonnet-20241022', 'gemini-2.0-flash-exp'",
            false,
        ),
    );

    props.insert(
        "temperature".to_string(),
        number_prop_with_desc(
            "Model temperature controlling randomness (0.0 = deterministic, 1.0 = creative).\n\
            Range: 0.0-1.0\n\
            Recommended: 0.7 for balanced, 0.0 for factual, 1.0 for creative",
            false,
        ),
    );

    props.insert(
        "maxTokens".to_string(),
        integer_prop_with_desc(
            "Maximum tokens for model responses.\n\
            Typical ranges: 1000-4000 for short responses, 8000-16000 for detailed analysis",
            false,
        ),
    );

    props.insert(
        "allowedBuiltInServiceAliases".to_string(),
        array_prop_with_desc(
            "List of built-in tool aliases this assistant can use.\n\
            Examples: ['browser', 'planning', 'workspace', 'knowledge']\n\
            Leave empty or omit to allow all tools",
            false,
        ),
    );

    props.insert(
        "mcpServerIds".to_string(),
        array_prop_with_desc(
            "List of external MCP server IDs this assistant can access.\n\
            Must match server IDs from MCP configuration.\n\
            Example: ['filesystem', 'github', 'database']",
            false,
        ),
    );

    MCPTool {
        name: "builtin_assistant__createAssistant".to_string(),
        title: Some("Create Assistant".to_string()),
        description: "Create a new global assistant configuration that can be reused across all agent sessions.

⚠️ CRITICAL WORKFLOW:
1. Choose a descriptive name (required, 1-100 characters)
2. Optionally provide a unique ID (auto-generated if omitted)
3. Configure model settings (provider, model name, temperature, max tokens)
4. Define system prompt to set assistant personality and behavior
5. Specify allowed tools and MCP servers (optional, defaults to all)
6. Use builtin_assistant__getAssistant to verify creation

💡 COMMON USE CASES:
• Create specialized assistant for code review: Set systemPrompt to code review guidelines
• Create customer support bot: Configure lower temperature (0.3) for consistent responses
• Create research assistant: Enable browser and knowledge tools, higher max tokens

❌ COMMON ERRORS:
• Duplicate ID: Use builtin_assistant__updateAssistant instead to modify existing assistant
• Empty name: Provide a non-empty name (whitespace-only names are rejected)
• Name too long: Keep names under 100 characters
• Invalid config JSON: Ensure all JSON fields are properly formatted

✓ SUCCESS: Assistant is created and immediately available for use across all sessions.
The assistant ID can be referenced in session creation or multi-agent configurations.

💡 NEXT STEPS:
• Use builtin_assistant__listAssistants to see all configured assistants
• Use builtin_assistant__updateAssistant to modify configuration later
• Reference this assistant ID when creating new agent sessions".to_string(),
        input_schema: object_schema(props, vec!["name".to_string()]),
        annotations: None,
        output_schema: None,
    }
}
```

#### updateAssistant

```rust
fn create_update_assistant_tool() -> MCPTool {
    // Similar structure with props

    MCPTool {
        name: "builtin_assistant__updateAssistant".to_string(),
        title: Some("Update Assistant".to_string()),
        description: "Update an existing assistant configuration. Supports partial updates - only modified fields need to be provided.

⚠️ CRITICAL WORKFLOW:
1. ALWAYS call builtin_assistant__getAssistant FIRST to see current configuration
2. Extract the assistant ID from the response
3. Provide the ID and fields to update (name, systemPrompt, model settings, tools)
4. Omitted fields retain their current values (partial update)
5. Use builtin_assistant__getAssistant again to verify changes

💡 COMMON USE CASES:
• Update system prompt: Modify only systemPrompt field
• Change model: Update modelProvider and modelName together
• Adjust temperature: Fine-tune temperature without changing other settings
• Enable new tools: Add to allowedBuiltInServiceAliases array

❌ COMMON ERRORS:
• Assistant not found: Verify ID is correct using builtin_assistant__listAssistants
• Invalid JSON in config: Ensure proper JSON formatting
• Forgetting to verify: Always check updated values with getAssistant

✓ SUCCESS: Assistant configuration is updated and takes effect immediately for new sessions.
Existing sessions using this assistant are NOT affected (session configs are snapshots).

💡 NEXT STEPS:
• Use builtin_assistant__getAssistant to verify changes
• Create new agent session to test updated configuration
• Use builtin_assistant__listAssistants to see updated timestamp".to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
        annotations: None,
        output_schema: None,
    }
}
```

#### deleteAssistant

```rust
fn create_delete_assistant_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "id".to_string(),
        string_prop_with_desc(
            "⚠️ CRITICAL: Exact assistant identifier to delete.\n\n\
            MANDATORY WORKFLOW:\n\
            1. Call builtin_assistant__listAssistants FIRST to see available assistants\n\
            2. Extract the exact ID from the list (not the name)\n\
            3. Use the extracted ID as this parameter\n\n\
            ❌ NEVER use ID reconstructed from memory or assumptions\n\
            ✅ ALWAYS use ID exactly as shown in listAssistants response",
            true,
        ),
    );

    MCPTool {
        name: "builtin_assistant__deleteAssistant".to_string(),
        title: Some("Delete Assistant".to_string()),
        description: "Permanently delete an assistant configuration.

⚠️ CRITICAL WORKFLOW:
1. ALWAYS call builtin_assistant__listAssistants FIRST to confirm assistant exists
2. Extract the exact assistant ID from the response
3. Call deleteAssistant with the extracted ID
4. Deletion is PERMANENT and cannot be undone

💡 WHEN TO USE:
• Removing obsolete or test assistants
• Cleaning up unused configurations
• Before recreating an assistant with the same ID

❌ COMMON ERRORS:
• Assistant not found: ID doesn't exist or was already deleted
• Using name instead of ID: Must use the unique ID, not the display name
• Deleting active assistant: Existing sessions continue using cached config (new sessions cannot use it)

⚠️ WARNING: This operation is IRREVERSIBLE. The assistant configuration is permanently removed.
Existing agent sessions using this assistant will continue working (they use a snapshot),
but new sessions cannot reference this assistant ID.

✓ SUCCESS: Assistant is permanently deleted from the database.

💡 NEXT STEPS:
• Use builtin_assistant__listAssistants to verify deletion
• Create new assistant with builtin_assistant__createAssistant if needed".to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
        annotations: None,
        output_schema: None,
    }
}
```

#### listAssistants

```rust
fn create_list_assistants_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "page".to_string(),
        integer_prop_with_desc(
            "Page number (1-based). Default: 1\n\
            Example: page=2 shows the second page of results",
            false,
        ),
    );
    props.insert(
        "pageSize".to_string(),
        integer_prop_with_desc(
            "Number of assistants per page. Range: 1-100. Default: 20\n\
            Example: pageSize=50 shows 50 assistants per page",
            false,
        ),
    );
    props.insert(
        "limit".to_string(),
        integer_prop_with_desc(
            "Alternative to pageSize: maximum results to return. Range: 1-100\n\
            Example: limit=10 returns at most 10 assistants",
            false,
        ),
    );
    props.insert(
        "offset".to_string(),
        integer_prop_with_desc(
            "Alternative to page: number of results to skip\n\
            Example: offset=20 skips first 20 assistants",
            false,
        ),
    );

    MCPTool {
        name: "builtin_assistant__listAssistants".to_string(),
        title: Some("List Assistants".to_string()),
        description: "List all configured assistants with pagination support. Returns assistants ordered by most recently updated.

⚠️ PAGINATION OPTIONS (choose one):
• Legacy: Use page (1-based) and pageSize
• Modern: Use limit and offset
• Default: No parameters = first 20 assistants

💡 COMMON USE CASES:
• View all assistants: Call with no parameters (returns first 20)
• Get specific page: listAssistants(page=2, pageSize=20)
• Get next batch: Use offset from previous response (offset=20, limit=20)
• See all assistants: Check 'has_more' field, increment page/offset until false

❌ COMMON ERRORS:
• Page 0 or negative: Pages are 1-based (page=1 is the first page)
• pageSize > 100: Maximum page size is 100 (automatically capped)
• Mixing pagination styles: Don't mix page/pageSize with limit/offset in same call

✓ SUCCESS: Returns list of assistants with metadata:
• assistants: Array of assistant objects (id, name, config, timestamps)
• total: Total count across all pages
• limit: Items per page (from pageSize or limit parameter)
• offset: Number of items skipped
• returned: Actual number of items in this response
• has_more: Boolean indicating if more pages exist

💡 NEXT STEPS:
• Use builtin_assistant__getAssistant(id) to view full details of specific assistant
• Use builtin_assistant__updateAssistant to modify configuration
• Use builtin_assistant__deleteAssistant to remove assistants
• If has_more=true, call again with incremented page or offset".to_string(),
        input_schema: object_schema(props, vec![]),
        annotations: None,
        output_schema: None,
    }
}
```

#### getAssistant

```rust
fn create_get_assistant_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "id".to_string(),
        string_prop_with_desc(
            "⚠️ CRITICAL: Exact assistant identifier.\n\n\
            RECOMMENDED WORKFLOW:\n\
            1. Call builtin_assistant__listAssistants to see available assistants\n\
            2. Extract the exact ID from the list\n\
            3. Use the extracted ID as this parameter\n\n\
            ✅ ALWAYS use ID exactly as shown in listAssistants response",
            true,
        ),
    );

    MCPTool {
        name: "builtin_assistant__getAssistant".to_string(),
        title: Some("Get Assistant".to_string()),
        description: "Retrieve complete configuration for a specific assistant by ID.

⚠️ CRITICAL WORKFLOW:
1. Call builtin_assistant__listAssistants to find available assistants (if ID unknown)
2. Extract the exact assistant ID from the list
3. Call getAssistant with the extracted ID
4. Review the returned configuration (name, systemPrompt, model settings, tools)

💡 COMMON USE CASES:
• View full configuration before updating
• Verify assistant creation/update succeeded
• Export assistant configuration for backup or sharing
• Check which tools and MCP servers are enabled

❌ COMMON ERRORS:
• Assistant not found: ID doesn't exist or was deleted
• Using name instead of ID: Must use the unique ID
• Typo in ID: IDs are case-sensitive, ensure exact match

✓ SUCCESS: Returns complete assistant configuration including:
• id: Unique identifier
• name: Display name
• config: Full configuration object (systemPrompt, model settings, tools, etc.)
• created_at: Timestamp of creation
• updated_at: Timestamp of last modification

💡 NEXT STEPS:
• Use builtin_assistant__updateAssistant to modify configuration
• Use builtin_assistant__deleteAssistant to remove assistant
• Reference this ID when creating new agent sessions".to_string(),
        input_schema: object_schema(props, vec!["id".to_string()]),
        annotations: None,
        output_schema: None,
    }
}
```

#### searchAssistant

```rust
fn create_search_assistant_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "query".to_string(),
        string_prop_with_desc(
            "Search term to match against assistant names and configuration content.\n\
            Supports partial matching (case-insensitive).\n\
            Examples: 'code', 'support', 'gpt-4', 'browser'",
            true,
        ),
    );
    props.insert(
        "limit".to_string(),
        integer_prop_with_desc(
            "Maximum number of results to return. Range: 1-100. Default: 10\n\
            Example: limit=5 returns at most 5 matching assistants",
            false,
        ),
    );

    MCPTool {
        name: "builtin_assistant__searchAssistant".to_string(),
        title: Some("Search Assistant".to_string()),
        description: "Search assistants by name or configuration content. Uses case-insensitive partial matching.

⚠️ SEARCH BEHAVIOR:
• Searches in: assistant name, system prompt, model settings, tool lists
• Matching: Case-insensitive substring match
• Ordering: Most recently updated first
• Limit: Returns up to 'limit' results (default 10, max 100)

💡 COMMON USE CASES:
• Find by name: searchAssistant(query='code reviewer')
• Find by model: searchAssistant(query='gpt-4')
• Find by capability: searchAssistant(query='browser') finds assistants with browser tools
• Find by keyword in prompt: searchAssistant(query='customer support')

❌ COMMON ERRORS:
• No results: Query doesn't match any assistant name or config
• Too many results: Use more specific query or reduce limit
• Expecting exact match: Search uses partial/substring matching

✓ SUCCESS: Returns matching assistants with:
• assistants: Array of matching assistant objects
• count: Number of results returned

💡 NEXT STEPS:
• Use builtin_assistant__getAssistant to view full details
• Use builtin_assistant__listAssistants if no matches (to see all available)
• Refine query if too many or too few results".to_string(),
        input_schema: object_schema(props, vec!["query".to_string()]),
        annotations: None,
        output_schema: None,
    }
}
```

**Files to Modify**:

- `src-tauri/src/mcp/builtin/assistant/mod.rs` (tool definition functions)
- `src-tauri/src/mcp/utils/schema_builder.rs` (add helper functions if needed)

**Estimated Time**: 3-4 hours

---

## Phase 2: Medium Priority Improvements

### 2.1 Add Input Validation

**Current Issue**: Missing validation for name length, format, and other constraints.

**Implementation**:

```rust
// Add validation helper functions
impl AssistantServer {
    fn validate_name(name: &str) -> Result<String, MCPResult> {
        let trimmed = name.trim();

        if trimmed.is_empty() {
            return Err(invalid_input_error(
                "Assistant name cannot be empty or whitespace only",
                ToolGroup::Assistant,
            ));
        }

        if trimmed.len() > 100 {
            return Err(invalid_input_error(
                "Assistant name cannot exceed 100 characters",
                ToolGroup::Assistant,
            ));
        }

        Ok(trimmed.to_string())
    }

    fn validate_id(id: &str) -> Result<String, MCPResult> {
        let trimmed = id.trim();

        if trimmed.is_empty() {
            return Err(invalid_input_error(
                "Assistant ID cannot be empty or whitespace only",
                ToolGroup::Assistant,
            ));
        }

        // Allow alphanumeric, hyphens, underscores
        let valid_chars = trimmed.chars().all(|c| {
            c.is_alphanumeric() || c == '-' || c == '_'
        });

        if !valid_chars {
            return Err(invalid_input_error(
                "Assistant ID can only contain alphanumeric characters, hyphens, and underscores",
                ToolGroup::Assistant,
            ));
        }

        if trimmed.len() > 64 {
            return Err(invalid_input_error(
                "Assistant ID cannot exceed 64 characters",
                ToolGroup::Assistant,
            ));
        }

        Ok(trimmed.to_string())
    }

    fn validate_temperature(temp: f64) -> Result<(), MCPResult> {
        if temp < 0.0 || temp > 2.0 {
            return Err(invalid_input_error(
                "Temperature must be between 0.0 and 2.0",
                ToolGroup::Assistant,
            ));
        }
        Ok(())
    }

    fn validate_max_tokens(tokens: i64) -> Result<(), MCPResult> {
        if tokens < 1 || tokens > 200000 {
            return Err(invalid_input_error(
                "Max tokens must be between 1 and 200000",
                ToolGroup::Assistant,
            ));
        }
        Ok(())
    }
}
```

**Update create_assistant**:

```rust
async fn create_assistant(&self, args: Value) -> Result<MCPResult, String> {
    let db = self.get_db();

    // Validate and extract ID
    let id = if let Some(id_str) = args.get("id").and_then(|v| v.as_str()) {
        match Self::validate_id(id_str) {
            Ok(valid_id) => valid_id,
            Err(err) => return Ok(err),
        }
    } else {
        cuid2::create_id()
    };

    // Validate and extract name
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) => match Self::validate_name(n) {
            Ok(valid_name) => valid_name,
            Err(err) => return Ok(err),
        },
        None => return Ok(missing_param_error("name", ToolGroup::Assistant)),
    };

    // Validate temperature if provided
    if let Some(temp) = args.get("temperature").and_then(|v| v.as_f64()) {
        if let Err(err) = Self::validate_temperature(temp) {
            return Ok(err);
        }
    }

    // Validate maxTokens if provided
    if let Some(tokens) = args.get("maxTokens").and_then(|v| v.as_i64()) {
        if let Err(err) = Self::validate_max_tokens(tokens) {
            return Ok(err);
        }
    }

    // Rest of implementation...
}
```

**Files to Modify**:

- `src-tauri/src/mcp/builtin/assistant/mod.rs`

**Estimated Time**: 2 hours

---

### 2.2 Improve Success Message Clarity

**Current Issue**: Success messages don't always clearly show both ID and name.

**Implementation**:

```rust
// Update create_assistant success message
let hint = SuccessHint::new(
    format!(
        "Assistant created successfully:\n\
        \n\
        **ID**: {}\n\
        **Name**: {}\n\
        \n\
        The assistant is now available globally across all sessions.",
        id, name
    ),
    vec![
        format!("Use builtin_assistant__getAssistant(id=\"{}\") to view full configuration", id),
        "Use builtin_assistant__listAssistants to see all configured assistants".to_string(),
    ],
);

// Update update_assistant success message
let hint = SuccessHint::new(
    format!(
        "Assistant updated successfully:\n\
        \n\
        **ID**: {}\n\
        **Name**: {}\n\
        \n\
        Changes take effect immediately for new sessions.\n\
        Existing sessions using this assistant are not affected.",
        id, name
    ),
    vec![
        format!("Use builtin_assistant__getAssistant(id=\"{}\") to verify changes", id),
    ],
);

// Update delete_assistant success message
let hint = SuccessHint::new(
    format!(
        "Assistant deleted successfully:\n\
        \n\
        **ID**: {}\n\
        \n\
        The assistant configuration has been permanently removed.\n\
        New sessions cannot use this ID, but existing sessions are unaffected.",
        id
    ),
    vec![
        "Use builtin_assistant__listAssistants to see remaining assistants".to_string(),
        "Use builtin_assistant__createAssistant to create a new assistant if needed".to_string(),
    ],
);
```

**Files to Modify**:

- `src-tauri/src/mcp/builtin/assistant/mod.rs`

**Estimated Time**: 1 hour

---

### 2.3 Enhance Parameter Descriptions

**Current Issue**: Parameter descriptions lack examples and constraints.

**Implementation**: See detailed parameter descriptions in Phase 1.2 (tool descriptions above).

**Additional Helper Functions Needed** in `schema_builder.rs`:

```rust
pub fn string_prop_with_desc(description: &str, required: bool) -> Value {
    json!({
        "type": "string",
        "description": description
    })
}

pub fn integer_prop_with_desc(description: &str, required: bool) -> Value {
    json!({
        "type": "integer",
        "description": description
    })
}

pub fn number_prop_with_desc(description: &str, required: bool) -> Value {
    json!({
        "type": "number",
        "description": description
    })
}

pub fn array_prop_with_desc(description: &str, required: bool) -> Value {
    json!({
        "type": "array",
        "description": description,
        "items": { "type": "string" }
    })
}
```

**Files to Modify**:

- `src-tauri/src/mcp/utils/schema_builder.rs` (add helpers)
- `src-tauri/src/mcp/builtin/assistant/mod.rs` (use helpers)

**Estimated Time**: 1.5 hours

---

## Phase 3: Low Priority Enhancements

### 3.1 Refactor into Multiple Files

**Current Issue**: 1014 lines in single `mod.rs` file.

**Goal**: Organize into feature-based files for better maintainability.

**New Structure**:

```
assistant/
├── mod.rs           # Server struct, trait impl, tool routing, cache management
├── operations.rs    # create_assistant, update_assistant, delete_assistant
├── queries.rs       # list_assistants, get_assistant, search_assistant
├── schema.rs        # Tool schema definitions (all create_*_tool functions)
└── validation.rs    # Input validation helpers
```

**mod.rs (core)**:

```rust
mod operations;
mod queries;
mod schema;
mod validation;

use operations::*;
use queries::*;
use schema::*;
use validation::*;

// Keep only: AssistantServer struct, trait impl, cache management
```

**operations.rs**:

```rust
use super::*;

pub async fn create_assistant(
    server: &AssistantServer,
    args: Value,
) -> Result<MCPResult, String> {
    // Implementation
}

pub async fn update_assistant(
    server: &AssistantServer,
    args: Value,
) -> Result<MCPResult, String> {
    // Implementation
}

pub async fn delete_assistant(
    server: &AssistantServer,
    args: Value,
) -> Result<MCPResult, String> {
    // Implementation
}
```

**queries.rs**:

```rust
use super::*;

pub async fn list_assistants(
    server: &AssistantServer,
    args: Value,
) -> Result<MCPResult, String> {
    // Implementation
}

pub async fn get_assistant(
    server: &AssistantServer,
    args: Value,
) -> Result<MCPResult, String> {
    // Implementation
}

pub async fn search_assistant(
    server: &AssistantServer,
    args: Value,
) -> Result<MCPResult, String> {
    // Implementation
}
```

**schema.rs**:

```rust
use super::*;

pub fn create_create_assistant_tool() -> MCPTool { /* ... */ }
pub fn create_update_assistant_tool() -> MCPTool { /* ... */ }
pub fn create_delete_assistant_tool() -> MCPTool { /* ... */ }
pub fn create_list_assistants_tool() -> MCPTool { /* ... */ }
pub fn create_get_assistant_tool() -> MCPTool { /* ... */ }
pub fn create_search_assistant_tool() -> MCPTool { /* ... */ }
```

**validation.rs**:

```rust
use super::*;

pub fn validate_name(name: &str) -> Result<String, MCPResult> { /* ... */ }
pub fn validate_id(id: &str) -> Result<String, MCPResult> { /* ... */ }
pub fn validate_temperature(temp: f64) -> Result<(), MCPResult> { /* ... */ }
pub fn validate_max_tokens(tokens: i64) -> Result<(), MCPResult> { /* ... */ }
```

**Files to Create**:

- `src-tauri/src/mcp/builtin/assistant/operations.rs`
- `src-tauri/src/mcp/builtin/assistant/queries.rs`
- `src-tauri/src/mcp/builtin/assistant/schema.rs`
- `src-tauri/src/mcp/builtin/assistant/validation.rs`

**Files to Modify**:

- `src-tauri/src/mcp/builtin/assistant/mod.rs` (refactor into coordinator)

**Estimated Time**: 3 hours

---

### 3.2 Standardize Schema Definition

**Current Issue**: Mix of manual JSON and helper functions.

**Goal**: Use helper functions consistently across all tool definitions.

**Implementation**: Convert all manual `json!({...})` schema definitions to use helper functions from `schema_builder.rs`.

**Before**:

```rust
input_schema: serde_json::from_value(json!({
    "type": "object",
    "properties": {
        "id": { "type": "string", "description": "..." },
        "name": { "type": "string", "description": "..." }
    },
    "required": ["name"]
})).unwrap(),
```

**After**:

```rust
let mut props = HashMap::new();
props.insert("id".to_string(), string_prop_optional("..."));
props.insert("name".to_string(), string_prop_required("..."));

input_schema: object_schema(props, vec!["name".to_string()]),
```

**Files to Modify**:

- `src-tauri/src/mcp/builtin/assistant/schema.rs` (or mod.rs if Phase 3.1 not done)

**Estimated Time**: 1 hour

---

### 3.3 Add Import at Top of File

**Current Issue**: Using inline imports in success messages.

**Goal**: Add `std::time::Instant` import.

**Implementation**:

```rust
use std::sync::{Arc, RwLock};
use std::time::Instant;  // Add this import
use std::collections::HashMap;
// ... rest of imports
```

**Files to Modify**:

- `src-tauri/src/mcp/builtin/assistant/mod.rs`

**Estimated Time**: 5 minutes

---

## Testing Strategy

### Unit Tests to Add/Update

1. **Service Context Tests**:

```rust
#[tokio::test]
async fn test_service_context_no_assistants() {
    let db = create_test_db().await;
    let server = AssistantServer::new(db).await.unwrap();

    let context = server.get_service_context(None).await;
    assert!(context.context_prompt.contains("No assistants configured"));
}

#[tokio::test]
async fn test_service_context_with_assistants() {
    let db = create_test_db().await;
    let server = AssistantServer::new(db).await.unwrap();

    // Create test assistants
    for i in 1..=3 {
        server.create_assistant(json!({
            "id": format!("test-{}", i),
            "name": format!("Test Assistant {}", i),
            "config": {}
        })).await.unwrap();
    }

    let context = server.get_service_context(None).await;
    assert!(context.context_prompt.contains("Total Assistants: 3"));
    assert!(context.context_prompt.contains("Test Assistant"));
}

#[tokio::test]
async fn test_service_context_caching() {
    let db = create_test_db().await;
    let server = AssistantServer::new(db).await.unwrap();

    server.create_assistant(json!({
        "id": "test",
        "name": "Test",
        "config": {}
    })).await.unwrap();

    let context1 = server.get_service_context(None).await;
    let context2 = server.get_service_context(None).await;

    // Should be from cache (same text)
    assert_eq!(context1.context_prompt, context2.context_prompt);
}

#[tokio::test]
async fn test_cache_invalidation_on_create() {
    let db = create_test_db().await;
    let server = AssistantServer::new(db).await.unwrap();

    let context_before = server.get_service_context(None).await;
    assert!(context_before.context_prompt.contains("No assistants"));

    server.create_assistant(json!({
        "id": "test",
        "name": "Test",
        "config": {}
    })).await.unwrap();

    let context_after = server.get_service_context(None).await;
    assert!(context_after.context_prompt.contains("Total Assistants: 1"));
}
```

2. **Validation Tests**:

```rust
#[tokio::test]
async fn test_create_assistant_empty_name() {
    let db = create_test_db().await;
    let server = AssistantServer::new(db).await.unwrap();

    let result = server.create_assistant(json!({
        "id": "test",
        "name": "   ", // Whitespace only
        "config": {}
    })).await.unwrap();

    assert_eq!(result.is_error, Some(true));
    assert!(result.content.unwrap()[0].text.as_ref().unwrap()
        .contains("cannot be empty or whitespace"));
}

#[tokio::test]
async fn test_create_assistant_name_too_long() {
    let db = create_test_db().await;
    let server = AssistantServer::new(db).await.unwrap();

    let long_name = "a".repeat(101);
    let result = server.create_assistant(json!({
        "id": "test",
        "name": long_name,
        "config": {}
    })).await.unwrap();

    assert_eq!(result.is_error, Some(true));
    assert!(result.content.unwrap()[0].text.as_ref().unwrap()
        .contains("cannot exceed 100 characters"));
}

#[tokio::test]
async fn test_create_assistant_invalid_id() {
    let db = create_test_db().await;
    let server = AssistantServer::new(db).await.unwrap();

    let result = server.create_assistant(json!({
        "id": "test@invalid!",
        "name": "Test",
        "config": {}
    })).await.unwrap();

    assert_eq!(result.is_error, Some(true));
    assert!(result.content.unwrap()[0].text.as_ref().unwrap()
        .contains("alphanumeric characters, hyphens, and underscores"));
}

#[tokio::test]
async fn test_create_assistant_invalid_temperature() {
    let db = create_test_db().await;
    let server = AssistantServer::new(db).await.unwrap();

    let result = server.create_assistant(json!({
        "id": "test",
        "name": "Test",
        "temperature": 3.0, // Out of range
        "config": {}
    })).await.unwrap();

    assert_eq!(result.is_error, Some(true));
    assert!(result.content.unwrap()[0].text.as_ref().unwrap()
        .contains("between 0.0 and 2.0"));
}
```

3. **Success Message Tests**:

```rust
#[tokio::test]
async fn test_create_assistant_success_message() {
    let db = create_test_db().await;
    let server = AssistantServer::new(db).await.unwrap();

    let result = server.create_assistant(json!({
        "id": "my-assistant",
        "name": "My Assistant",
        "config": {}
    })).await.unwrap();

    assert_eq!(result.is_error, Some(false));
    let text = result.content.unwrap()[0].text.as_ref().unwrap();

    // Should show both ID and name
    assert!(text.contains("ID: my-assistant"));
    assert!(text.contains("Name: My Assistant"));

    // Should have next steps
    assert!(text.contains("builtin_assistant__getAssistant"));
}
```

### Integration Tests

Create `src-tauri/tests/assistant_integration_test.rs`:

```rust
#[tokio::test]
async fn test_assistant_full_lifecycle() {
    // 1. Create assistant
    // 2. Verify service context shows it
    // 3. Update assistant
    // 4. Verify changes
    // 5. Delete assistant
    // 6. Verify service context updated
}

#[tokio::test]
async fn test_assistant_pagination_edge_cases() {
    // Test with 0, 1, 20, 21, 100, 101 assistants
}
```

---

## Rollout Plan

### Phase 1 (High Priority) - Week 1

**Days 1-2**: Implement dynamic service context with caching
**Days 3-5**: Add comprehensive AI-compatible tool descriptions

**Validation**:

- Manual testing: Create/update/delete assistants, check service context
- Unit tests: Service context caching, cache invalidation
- AI agent testing: Verify tool descriptions guide agent behavior

**Deliverables**:

- Service context shows current assistants
- All tool descriptions follow best practice template
- 10+ new unit tests

---

### Phase 2 (Medium Priority) - Week 2

**Days 1-2**: Add input validation (name, ID, temperature, maxTokens)
**Day 3**: Improve success message clarity
**Days 4-5**: Enhance parameter descriptions

**Validation**:

- Validation tests: Invalid inputs rejected with clear errors
- Success message tests: Verify ID and name shown
- Manual testing: All error cases produce helpful messages

**Deliverables**:

- All inputs validated before database operations
- Success messages clearly show ID and name
- Parameter descriptions include examples and constraints
- 8+ new validation tests

---

### Phase 3 (Low Priority) - Week 3

**Days 1-2**: Refactor into multiple files (operations, queries, schema, validation)
**Day 3**: Standardize schema definitions (all use helpers)
**Day 4**: Add missing imports, final cleanup
**Day 5**: Documentation and integration tests

**Validation**:

- Build succeeds without warnings
- All tests pass (unit + integration)
- Run `pnpm refactor:validate` successfully
- Code review: Verify file organization

**Deliverables**:

- Assistant module split into 5 files
- All schemas use helper functions
- No inline imports
- Integration test suite
- Updated documentation

---

## Success Criteria

### Phase 1 (HIGH)

- [ ] Service context dynamically shows current assistants
- [ ] Service context cached for 10 seconds
- [ ] Cache invalidated after create/update/delete
- [ ] All tool descriptions include CRITICAL WORKFLOW section
- [ ] All tool descriptions include COMMON ERRORS section
- [ ] All tool descriptions include SUCCESS section
- [ ] Tool descriptions use AI-compatible language (no "copy", "paste", "from memory")

### Phase 2 (MEDIUM)

- [ ] Name validation (non-empty, ≤100 chars)
- [ ] ID validation (alphanumeric + hyphens/underscores, ≤64 chars)
- [ ] Temperature validation (0.0-2.0)
- [ ] MaxTokens validation (1-200000)
- [ ] Success messages show both ID and name
- [ ] Parameter descriptions include examples
- [ ] Parameter descriptions explain constraints

### Phase 3 (LOW)

- [ ] Code split into 5 files (mod, operations, queries, schema, validation)
- [ ] All schemas use helper functions
- [ ] No manual `json!({...})` schemas
- [ ] All imports at top of file
- [ ] `pnpm refactor:validate` passes

---

## Risk Mitigation

### Breaking Changes

**Risk**: Refactoring could break existing functionality  
**Mitigation**:

- Comprehensive test coverage before refactoring
- Run full test suite after each phase
- Manual testing with real assistant creation/update/delete flows

### Performance Impact

**Risk**: Service context fetches on every call could slow down  
**Mitigation**:

- Implement 10-second cache (tested with Browser module)
- Cache invalidation only on state changes
- Monitor query performance with large assistant counts

### AI Agent Confusion

**Risk**: New tool descriptions might still confuse AI agents  
**Mitigation**:

- Test with actual AI agents (GPT-4, Claude)
- Follow proven patterns from Browser/Planning modules
- Iterate based on real agent behavior

---

## Post-Implementation

### Documentation Updates

- [ ] Update `docs/builtin-tools.md` with Assistant examples
- [ ] Add Assistant to architecture diagrams
- [ ] Create user guide for assistant configuration

### Monitoring

- [ ] Track service context cache hit rate
- [ ] Monitor query performance (list, search)
- [ ] Log validation errors for pattern analysis

### Future Enhancements

- [ ] Assistant templates (pre-configured assistants)
- [ ] Assistant versioning (track config changes)
- [ ] Assistant analytics (usage tracking)
- [ ] Bulk operations (import/export multiple assistants)

---

## Appendix

### Reference Implementations

- **Service Context with Caching**: `src-tauri/src/mcp/builtin/browser/mod.rs`
- **AI-Compatible Tool Descriptions**: `src-tauri/src/mcp/builtin/planning/mod.rs`
- **Input Validation**: `src-tauri/src/mcp/builtin/workspace/mod.rs`
- **File Organization**: `src-tauri/src/mcp/builtin/browser/` (session.rs, navigation.rs, etc.)

### Tools and Commands

- **Run tests**: `cd src-tauri && cargo test assistant`
- **Format code**: `pnpm rust:fmt`
- **Lint code**: `pnpm rust:clippy`
- **Full validation**: `pnpm refactor:validate`
- **Find dead code**: `pnpm dead-code`

### Estimated Total Time

- **Phase 1 (HIGH)**: 5-6 hours
- **Phase 2 (MEDIUM)**: 4-5 hours
- **Phase 3 (LOW)**: 4-5 hours
- **Testing & Documentation**: 2-3 hours
- **Total**: 15-19 hours (~2-3 days of focused work)

---

**End of Refactoring Plan**
