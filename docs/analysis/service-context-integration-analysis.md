# Service Context Integration Analysis

## Overview

This document analyzes how service contexts from builtin MCP tools are integrated into the system prompt in LibrAgent's Agent V2 architecture.

---

## Architecture Flow

### 1. **Service Context Collection** (`MCPServiceProxy`)

The `MCPServiceProxy` acts as the session-bound intermediary that manages builtin MCP servers and collects their service contexts.

**Location**: `src-tauri/src/mcp/service_proxy.rs`

```rust
/// Collect service contexts from all builtin servers
pub async fn get_service_contexts(&self) -> HashMap<String, ServiceContext> {
    let mut contexts = HashMap::new();

    for (tool_id, server) in &self.builtin_servers {
        let context = server.get_service_context(None).await;
        contexts.insert(tool_id.clone(), context);
    }

    log::debug!(
        "Collected {} service contexts for session '{}'",
        contexts.len(),
        self.session_id
    );

    contexts
}
```

**Key Points**:

- Each session has its own `MCPServiceProxy` instance
- The proxy iterates through all registered builtin servers
- Calls `get_service_context()` on each server
- Returns a map of `tool_id -> ServiceContext`

---

### 2. **System Prompt Construction** (Rust Backend)

The system prompt is built by the agent orchestrator before sending messages to the LLM.

**Location**: `src-tauri/src/agent/llm.rs`

```rust
/// Build complete system prompt (Pure logic)
pub async fn build_system_prompt(
    agent_config: &crate::agent::AgentConfig,
    proxy: Option<Arc<MCPServiceProxy>>,
) -> Result<String, String> {
    let mut parts = Vec::new();

    // 1. Add time and location context first
    parts.push(build_time_location_context());

    // 2. Add agent's custom system prompt
    if !agent_config.system_prompt.trim().is_empty() {
        parts.push(agent_config.system_prompt.clone());
    }

    // 3. Add service contexts from all tools
    if let Some(p) = proxy {
        let contexts = p.get_service_contexts().await;

        if !contexts.is_empty() {
            parts.push("\n\n## Available Tools & Current State\n".to_string());

            for (_tool_id, service_context) in contexts {
                if !service_context.context_prompt.trim().is_empty() {
                    parts.push(service_context.context_prompt);
                }
            }
        }
    }

    Ok(parts.join("\n"))
}
```

**System Prompt Structure**:

```text
# Current Context Information
[Date, Time, Timezone]

[Agent's Custom System Prompt]

## Available Tools & Current State
[Service Context 1: e.g., Content Store]
[Service Context 2: e.g., Planning]
[Service Context 3: e.g., Browser]
```

---

### 3. **Frontend Integration** (`AgentChatContext`)

The frontend can fetch and display service contexts for UI purposes.

**Location**: `src/context/AgentChatContext.tsx`

```typescript
// Fetch service contexts from backend
const updateServiceContexts = useCallback(async () => {
  const sessionId = session?.id;
  if (!sessionId) return;

  try {
    const contexts = await invoke<Record<string, ServiceContext>>(
      'agent_get_service_contexts',
      { sessionId },
    );
    setServiceContexts(contexts);
    logger.info('Service contexts updated', { contexts });
  } catch (error) {
    logger.error('Failed to update service contexts', error);
  }
}, [session?.id]);
```

**Note**: This is primarily for UI display. The actual system prompt construction happens in Rust backend during LLM calls.

---

## ServiceContext Data Structure

**Rust Definition** (`src-tauri/src/mcp/types.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceContext<T = serde_json::Value> {
    /// The context prompt describing the current state.
    pub context_prompt: String,
    /// Optional structured state data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_state: Option<T>,
}
```

**TypeScript Definition** (`src/context/AgentChatContext.tsx`):

```typescript
export interface ServiceContext {
  contextPrompt: string;
  structuredState?: Record<string, unknown>;
}
```

---

## Builtin Tool Service Context Implementations

### 1. **Browser** (`src-tauri/src/mcp/builtin/browser/mod.rs`)

**Status**: ❌ **NOT IMPLEMENTED**

```rust
async fn get_service_context(
    &self,
    _options: Option<&Value>,
) -> crate::mcp::types::ServiceContext {
    crate::mcp::types::ServiceContext {
        context_prompt: String::new(),  // Empty!
        structured_state: None,
    }
}
```

**Recommendation**:

- Should report current browser session ID (if active)
- Current URL and page title
- Number of open tabs/sessions
- Last navigation action

**Example Implementation**:

```rust
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    let session_id = self.browser_session_id.read().ok()
        .and_then(|guard| guard.clone());
    
    if let Some(sid) = session_id {
        let service = self.get_browser_service().ok();
        if let Some(browser) = service {
            // Get current URL and title from browser service
            let context_prompt = format!(
                "## Browser\n\
                **Session ID**: {}\n\
                **Status**: Active\n\
                *Use navigation tools to interact with the browser.*",
                sid
            );
            
            ServiceContext {
                context_prompt,
                structured_state: Some(json!({
                    "session_id": sid,
                    "active": true
                })),
            }
        } else {
            ServiceContext {
                context_prompt: "## Browser\n**Status**: Service unavailable".to_string(),
                structured_state: None,
            }
        }
    } else {
        ServiceContext {
            context_prompt: "## Browser\n**Status**: No active session\n*Use createSession to start browsing.*".to_string(),
            structured_state: None,
        }
    }
}
```

---

### 2. **Playbook** (`src-tauri/src/mcp/builtin/playbook/mod.rs`)

**Status**: ❌ **NOT IMPLEMENTED**

The `PlaybookServer` implementation **does not override** `get_service_context()`, so it falls back to the default implementation in the trait:

**Default Implementation** (`src-tauri/src/mcp/builtin/mod.rs`):

```rust
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    ServiceContext {
        context_prompt: format!(
            "## {}\n**Description**: {}",
            self.display_name(),
            self.description()
        ),
        structured_state: None,
    }
}
```

**Current Output**:

```text
## Playbook
**Description**: Playbook management for reusable workflows
```

**Recommendation**:

- Should query database for playbook count
- List recent playbooks
- Show currently selected playbook (if any)

**Example Implementation**:

```rust
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    // Query playbook count from database
    let count_result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM playbooks WHERE session_id = ?"
    )
    .bind(&self.session_id)
    .fetch_one(self.db_pool.as_ref())
    .await;
    
    match count_result {
        Ok(count) if count > 0 => {
            // Fetch recent playbooks
            let recent = sqlx::query_as::<_, (String, String)>(
                "SELECT id, goal FROM playbooks WHERE session_id = ? 
                 ORDER BY updated_at DESC LIMIT 3"
            )
            .bind(&self.session_id)
            .fetch_all(self.db_pool.as_ref())
            .await
            .unwrap_or_default();
            
            let mut parts = vec![
                format!("## Playbook\n**{} playbooks available**\n", count),
            ];
            
            if !recent.is_empty() {
                parts.push("**Recent:**".to_string());
                for (id, goal) in recent {
                    let goal_short = if goal.len() > 50 {
                        format!("{}...", &goal[..50])
                    } else {
                        goal.clone()
                    };
                    parts.push(format!("  - {} ({})", goal_short, id));
                }
            }
            
            ServiceContext {
                context_prompt: parts.join("\n"),
                structured_state: Some(json!({
                    "playbook_count": count,
                    "session_id": self.session_id
                })),
            }
        }
        Ok(_) => {
            ServiceContext {
                context_prompt: "## Playbook\n**No playbooks yet**\n*Use createPlaybook to save reusable workflows.*".to_string(),
                structured_state: Some(json!({
                    "playbook_count": 0
                })),
            }
        }
        Err(e) => {
            log::error!("Failed to get playbook context: {}", e);
            ServiceContext {
                context_prompt: "## Playbook\n**Status**: Error loading state".to_string(),
                structured_state: None,
            }
        }
    }
}
```

---

### 3. **Content Store** (`src-tauri/src/mcp/builtin/content_store/server.rs`)

**Status**: ✅ **FULLY IMPLEMENTED**

```rust
pub async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    // Get current session ID
    let session_id = match self.session_manager.get_current_session() {
        Some(sid) => sid,
        None => {
            return ServiceContext {
                context_prompt: "## Content Store\n**Status**: No active session".to_string(),
                structured_state: None,
            };
        }
    };

    // Get content information for this session
    let (count, summaries) = match self.storage.try_lock() {
        Ok(storage) => {
            let count = storage.get_content_count(&session_id);
            let summaries = storage.get_content_summary(&session_id, 5);
            (count, summaries)
        }
        Err(e) => {
            log::warn!("Failed to lock content storage: {}", e);
            return ServiceContext {
                context_prompt: "## Content Store\n**Status**: Error loading state".to_string(),
                structured_state: None,
            };
        }
    };

    // Build context prompt
    let mut parts = vec!["## Content Store".to_string()];

    if count == 0 {
        parts.push("\n**No content stored yet.**".to_string());
        parts.push("*Use addContent to store files, documents, or text.*".to_string());
    } else {
        let file_label = if count == 1 { "file" } else { "files" };
        parts.push(format!("\n**{} {} stored**", count, file_label));

        // List content items with previews
        for (idx, (filename, size, preview)) in summaries.iter().enumerate() {
            let size_str = if *size < 1024 {
                format!("{}B", size)
            } else if *size < 1024 * 1024 {
                format!("{}KB", size / 1024)
            } else {
                format!("{}MB", size / (1024 * 1024))
            };

            let preview_short = if preview.len() > 50 {
                format!("{}...", &preview[..50])
            } else {
                preview.clone()
            };

            parts.push(format!(
                "  {}. **{}** ({}) - {}",
                idx + 1, filename, size_str, preview_short
            ));
        }

        if count > 5 {
            parts.push(format!("  ...and {} more files.", count - 5));
        }
    }

    ServiceContext {
        context_prompt: parts.join("\n"),
        structured_state: Some(serde_json::json!({
            "session_id": session_id,
            "content_count": count
        })),
    }
}
```

**Example Output**:

```text
## Content Store
**5 files stored**
  1. **project_requirements.txt** (12KB) - The project should implement a content managemen...
  2. **api_documentation.md** (45KB) - # API Documentation\n\nThis document describes...
  3. **meeting_notes_2025.txt** (3KB) - Meeting with stakeholders on January 2, 2025...
  4. **error_logs.json** (156KB) - {"timestamp": "2025-01-02T10:30:00Z", "level...
  5. **design_mockup.png** (2MB) - [Binary content]
```

---

## Key Design Patterns

### 1. **Session Isolation**

- Each agent session gets its own `MCPServiceProxy`
- Service contexts are queried per-session
- Tools maintain session-specific state (e.g., todo lists, content stores)

### 2. **Lazy Context Loading**

- Contexts are fetched only when building the system prompt
- No periodic polling or background updates
- Fresh context on every LLM call

### 3. **Structured State**

- `context_prompt`: Human-readable markdown for LLM
- `structured_state`: Machine-readable JSON for frontend/debugging

### 4. **Fallback Pattern**

- Default trait implementation provides basic context
- Tools override to add rich state information
- Graceful degradation on errors

---

## Summary Table

| Tool              | Status             | Context Information                         | Structured State                    |
| ----------------- | ------------------ | ------------------------------------------- | ----------------------------------- |
| **Content Store** | ✅ Implemented     | File count, file list with previews         | `content_count`, `session_id`       |
| **Playbook**      | ❌ Not Implemented | Only shows description (default trait)      | None                                |
| **Browser**       | ❌ Not Implemented | Empty string                                | None                                |
| **Planning**      | ✅ Implemented     | Todo count, recent todos, completion stats  | `todo_count`, `completed_count`     |
| **Knowledge**     | ✅ Implemented     | Knowledge entry count, recent entries       | `entry_count`, `session_id`         |
| **Workspace**     | ✅ Implemented     | Current workspace path, file structure      | `workspace_path`, `file_count`      |

---

## Recommendations

### For Browser Tool

1. Track active browser session in service context
2. Report current URL, page title, tab count
3. Add structured state with session metadata

### For Playbook Tool

1. Query database for playbook count
2. List 3 most recent playbooks with goals
3. Highlight currently selected playbook (if any)
4. Add structured state with playbook statistics

### General

- All builtin tools should implement meaningful service contexts
- Contexts should be concise (< 500 chars) to avoid token bloat
- Use markdown formatting for readability
- Include actionable hints (e.g., "Use createSession to start")

---

## Testing Service Contexts

**Tauri Command** (Frontend):

```typescript
const contexts = await invoke<Record<string, ServiceContext>>(
  'agent_get_service_contexts',
  { sessionId: 'your-session-id' }
);
console.log(contexts);
```

**Rust Backend** (Direct):

```rust
let proxy = proxy_manager.get_proxy(session_id).await.unwrap();
let contexts = proxy.get_service_contexts().await;
for (tool_id, context) in contexts {
    println!("{}: {}", tool_id, context.context_prompt);
}
```

---

## References

- **Service Proxy**: `src-tauri/src/mcp/service_proxy.rs`
- **System Prompt Builder**: `src-tauri/src/agent/llm.rs`
- **Frontend Context**: `src/context/AgentChatContext.tsx`
- **Type Definitions**: `src-tauri/src/mcp/types.rs`
- **Builtin Trait**: `src-tauri/src/mcp/builtin/mod.rs`
