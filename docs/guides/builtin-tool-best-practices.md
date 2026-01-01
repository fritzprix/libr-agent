# Built-in Tool Implementation Best Practices

**Last Updated:** December 30, 2025  
**Target Audience:** Developers implementing new MCP built-in tools  
**Applies To:** Rust backend (`src-tauri/src/mcp/builtin/`) and TypeScript Web MCP (`src/lib/web-mcp/modules/`)

---

## Table of Contents

1. [Overview](#overview)
2. [Input Validation](#input-validation)
3. [Error Response Design](#error-response-design)
4. [Success Response Design](#success-response-design)
5. [Error Guidance System](#error-guidance-system)
6. [Tool Chaining & Next Actions](#tool-chaining--next-actions)
7. [Implementation Checklist](#implementation-checklist)
8. [Code Examples](#code-examples)
9. [Anti-Patterns](#anti-patterns)
10. [Testing Guidelines](#testing-guidelines)

---

## Overview

LibrAgent built-in tools follow the Model Context Protocol (MCP) specification and provide AI agents with capabilities for task automation. Well-designed tools ensure:

- **Robustness**: Comprehensive input validation prevents runtime errors
- **Usability**: Clear error messages with actionable guidance
- **Chainability**: Success responses suggest logical next steps
- **Security**: No internal state leakage to LLM in error responses
- **Consistency**: Uniform response formats across all tools

**Core Principle:** Every tool response (success or error) must include human-readable text content with actionable next steps.

---

## Input Validation

### Multi-Layer Validation Pattern

Validate in order from **cheapest to most expensive** operations:

```
Layer 1: Parameter Existence & Type
         ↓
Layer 2: Value Constraints (format, length, range)
         ↓
Layer 3: Business Logic (duplicates, conflicts)
         ↓
Layer 4: Relational Integrity (foreign keys, dependencies)
```

### TypeScript Example

```typescript
async function addTodo(params: unknown): Promise<MCPResult> {
  // Layer 1: Parameter existence
  const { title, priority, parentId } = params as AddTodoParams;
  if (!title || typeof title !== 'string') {
    return createError(
      'MISSING_REQUIRED_PARAM',
      'title is required and must be a string',
    );
  }

  // Layer 2: Value constraints
  if (title.trim().length === 0) {
    return createError(
      'INVALID_INPUT',
      'title cannot be empty or whitespace-only',
    );
  }
  if (priority && (priority < 1 || priority > 5)) {
    return createError('INVALID_INPUT', 'priority must be between 1 and 5');
  }

  // Layer 3: Business logic
  const existing = await db.query(
    'SELECT id FROM todos WHERE LOWER(title) = ?',
    [title.toLowerCase()],
  );
  if (existing.length > 0) {
    return createError(
      'DUPLICATE_TITLE',
      `Todo "${title}" already exists. Use update_todo to modify it.`,
    );
  }

  // Layer 4: Relational integrity
  if (parentId) {
    const parent = await db.query(
      'SELECT id, parent_id FROM todos WHERE id = ?',
      [parentId],
    );
    if (parent.length === 0) {
      return createError(
        'INVALID_PARENT',
        `Parent todo ${parentId} not found. Use list_todos to see available todos.`,
      );
    }
    if (parent[0].parent_id) {
      return createError(
        'NESTING_TOO_DEEP',
        'Cannot nest todos more than 2 levels deep. Create as top-level todo instead.',
      );
    }
  }

  // Validation passed - proceed with operation
  const result = await db.insert('todos', {
    title,
    priority,
    parent_id: parentId,
  });
  return createSuccess(result);
}
```

### Rust Example

```rust
async fn add_todo(&self, args: Value) -> Result<MCPResult, String> {
    // Layer 1: Parameter existence & type
    let title = args.get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "title is required and must be a string".to_string())?;

    let priority = args.get("priority")
        .and_then(|v| v.as_i64())
        .unwrap_or(3) as i32;

    // Layer 2: Value constraints
    if title.trim().is_empty() {
        return Err("title cannot be empty or whitespace-only".to_string());
    }
    if !(1..=5).contains(&priority) {
        return Err(format!("priority must be between 1 and 5. Received: {}", priority));
    }

    // Layer 3: Business logic - duplicate detection
    let duplicate_check = sqlx::query!(
        "SELECT id FROM todos WHERE LOWER(title) = LOWER(?) AND session_id = ?",
        title,
        session_id
    )
    .fetch_optional(&self.db_pool)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    if duplicate_check.is_some() {
        return Err(format!(
            "Todo \"{}\" already exists in this session. Use update_todo to modify it.",
            title
        ));
    }

    // Layer 4: Relational integrity
    if let Some(parent_id) = args.get("parentId").and_then(|v| v.as_str()) {
        let parent = sqlx::query!(
            "SELECT id, parent_id FROM todos WHERE id = ? AND session_id = ?",
            parent_id,
            session_id
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        match parent {
            None => return Err(format!(
                "Parent todo {} not found. Use list_todos to see available todos.",
                parent_id
            )),
            Some(p) if p.parent_id.is_some() => return Err(
                "Cannot nest todos more than 2 levels deep. Create as top-level todo instead.".to_string()
            ),
            _ => {}
        }
    }

    // Validation passed - proceed with operation
    Ok(self.create_success_result(result))
}
```

### Key Validation Principles

✅ **DO:**

- Provide **specific error messages** with parameter names and values
- Include **examples** of valid input in error messages
- Use **case-insensitive** comparison for duplicate detection (`LOWER()`)
- Validate **array elements with indices** for clarity (e.g., "Subtask #3 has invalid priority")
- Return early on validation failures (fail fast)
- Include **recovery guidance** in validation errors

❌ **DON'T:**

- Silently coerce invalid input (e.g., `Math.max(1, Math.min(5, priority))`)
- Return generic errors (e.g., "Invalid input")
- Skip validation assuming upstream validation
- Use 0-based indexing in user-facing messages (use 1-based)

---

## Error Response Design

### Standard Error Structure

All errors **MUST** follow this structure:

```typescript
interface ErrorResponse {
  content: [
    {
      type: 'text';
      text: string; // Human-readable error + guidance
    },
  ];
  structured_content: undefined | null; // ⚠️ NEVER include in errors
  is_error: true;
}
```

### Error Message Template

```
✗ [Clear description of what went wrong]

[Context: relevant values, IDs, or state that caused the error]

💡 Next Steps:
1. [First recovery action - most common solution]
2. [Alternative action]
3. [Fallback or diagnostic action]

[Optional: Link to related tools or documentation]
```

### TypeScript Error Handler Example

```typescript
function handleToolError(error: ToolError, toolName: string): MCPResult {
  const guidance = getGuidanceForError(error.code, toolName);

  return {
    content: [
      {
        type: 'text',
        text: `
✗ ${error.message}

${error.context ? `Context: ${error.context}\n` : ''}
💡 Next Steps:
${guidance.map((step, i) => `${i + 1}. ${step}`).join('\n')}

${getToolChainHints(error.code, toolName)}
      `.trim(),
      },
    ],
    structured_content: undefined, // ⚠️ Critical: Never expose internals
    is_error: true,
  };
}
```

### Rust Error Handler Example

```rust
fn create_error_result(message: &str, guidance: Vec<&str>) -> MCPResult {
    let guidance_text = guidance.iter()
        .enumerate()
        .map(|(i, step)| format!("{}. {}", i + 1, step))
        .collect::<Vec<_>>()
        .join("\n");

    MCPResult {
        content: Some(vec![crate::mcp::types::MCPContent::Text {
            text: format!(
                "✗ {}\n\n💡 Next Steps:\n{}",
                message,
                guidance_text
            ),
        }]),
        structured_content: None,  // ⚠️ Critical: Never expose internals
        is_error: Some(true),
    }
}
```

### Error Code Taxonomy

Organize errors into categories for consistent handling:

```typescript
enum ErrorCategory {
  // Input validation errors (user-fixable)
  MISSING_REQUIRED_PARAM = 'MISSING_REQUIRED_PARAM',
  INVALID_INPUT = 'INVALID_INPUT',
  INVALID_FORMAT = 'INVALID_FORMAT',

  // State/resource errors (context-dependent)
  RESOURCE_NOT_FOUND = 'RESOURCE_NOT_FOUND',
  DUPLICATE_RESOURCE = 'DUPLICATE_RESOURCE',
  INVALID_STATE = 'INVALID_STATE',
  NESTING_TOO_DEEP = 'NESTING_TOO_DEEP',

  // Operation failures (may be transient)
  OPERATION_FAILED = 'OPERATION_FAILED',
  TIMEOUT = 'TIMEOUT',
  NETWORK_ERROR = 'NETWORK_ERROR',

  // System errors (escalation needed)
  INTERNAL_ERROR = 'INTERNAL_ERROR',
  DATABASE_ERROR = 'DATABASE_ERROR',
  PERMISSION_DENIED = 'PERMISSION_DENIED',
}
```

---

## Success Response Design

### Dual-Format Response Pattern

Success responses should include **both human-readable text and structured data**:

```typescript
interface SuccessResponse {
  content: [
    {
      type: 'text';
      text: string; // Confirmation + next action hints
    },
  ];
  structured_content: object; // ✅ OK in success responses
  is_error: false;
}
```

### Success Message Template

```
✓ [Action completed successfully]

[Key result details: IDs, names, status]

💡 Next: [Suggested next action with tool name]
```

### TypeScript Success Handler Example

```typescript
function createSuccessResult(data: TodoCreated): MCPResult {
  return {
    content: [
      {
        type: 'text',
        text: `
✓ Todo created successfully

ID: ${data.id}
Title: ${data.title}
Priority: ${data.priority}/5
Status: pending

💡 Next: Use update_todo to modify or complete_todo when done
      `.trim(),
      },
    ],
    structured_content: {
      id: data.id,
      title: data.title,
      priority: data.priority,
      status: 'pending',
      created_at: data.createdAt,
      parent_id: data.parentId || null,
    },
    is_error: false,
  };
}
```

### Rust Success Handler Example

```rust
fn create_success_result(&self, todo: Todo) -> MCPResult {
    let text = format!(
        "✓ Todo created successfully\n\n\
         ID: {}\n\
         Title: {}\n\
         Priority: {}/5\n\
         Status: {}\n\n\
         💡 Next: Use update_todo to modify or complete_todo when done",
        todo.id,
        todo.title,
        todo.priority,
        todo.status
    );

    MCPResult {
        content: Some(vec![crate::mcp::types::MCPContent::Text { text }]),
        structured_content: Some(json!({
            "id": todo.id,
            "title": todo.title,
            "priority": todo.priority,
            "status": todo.status,
            "created_at": todo.created_at,
            "parent_id": todo.parent_id
        })),
        is_error: Some(false),
    }
}
```

---

## Error Guidance System

### Guidance Mapping Pattern

Create a centralized mapping of error codes to recovery steps:

> **⚠️ CRITICAL DESIGN PRINCIPLE: Tool Group Isolation**
>
> Error guidance MUST stay within the same tool group. Browser tool errors should only suggest browser tools, planning tool errors should only suggest planning tools, etc.
>
> **❌ DON'T:** Mix tool groups in guidance
>
> ```typescript
> // BAD: Browser tool suggesting planning tools
> ELEMENT_NOT_FOUND: [
>   'Use listInteractable to see elements',
>   'Use add_todo to track this task', // ❌ Wrong tool group!
> ];
> ```
>
> **✅ DO:** Keep guidance within the same tool ecosystem
>
> ```typescript
> // GOOD: Browser tool suggesting only browser tools
> ELEMENT_NOT_FOUND: [
>   'Use listInteractable to see available elements',
>   'Try extractWebContent to view page structure',
>   'Use navigateToUrl to try a different page',
> ];
> ```
>
> This maintains clear separation of concerns and prevents confusing the LLM with unrelated tool suggestions.

```typescript
const ERROR_GUIDANCE: Record<string, (toolName: string) => string[]> = {
  // Browser Tools Group - Only suggest browser tools
  SESSION_NOT_FOUND: (tool) => [
    'Run createSession to start a new browser session',
    'Use listSessions to see available sessions',
    `Verify the session_id parameter in ${tool}`,
  ],

  ELEMENT_NOT_FOUND: (tool) => [
    'Use listInteractable to see clickable elements',
    'Try extractWebContent to view page structure',
    'Verify the selector syntax is correct',
    'Wait for page to fully load before interacting',
  ],

  NAVIGATION_FAILED: (tool) => [
    'Verify the URL is valid and accessible',
    'Check network connectivity',
    'Try extractWebContent after navigation completes',
    'Use a simpler URL without query parameters first',
  ],

  // Planning Tools Group - Only suggest planning tools
  DUPLICATE_TITLE: (tool) => [
    'Use a different title for the new item',
    'Use update_todo to modify the existing item',
    'Use list_todos to see all existing items',
  ],

  SESSION_NOT_FOUND_PLANNING: (tool) => [
    'Use create_planning_session to start a new session',
    'Use list_planning_sessions to see available sessions',
    'Verify the session_id is correct',
  ],

  // Generic Errors - Tool-agnostic guidance
  INVALID_INPUT: (tool) => [
    'Check parameter types match the tool schema',
    'Review the tool documentation for valid values',
    'Use describe_tool to see parameter requirements',
  ],
};

function getGuidanceForError(errorCode: string, toolName: string): string[] {
  const guidanceFn = ERROR_GUIDANCE[errorCode];
  return guidanceFn
    ? guidanceFn(toolName)
    : [
        'Review the error message for specific details',
        'Check tool documentation for correct usage',
        'Try a simpler operation to isolate the issue',
      ];
}
```

### Rust Guidance System

```rust
struct ErrorGuidance {
    code: &'static str,
    guidance: Vec<&'static str>,
}

impl ErrorGuidance {
    fn get_guidance(error_code: &str, tool_name: &str) -> Vec<String> {
        match error_code {
            "SESSION_NOT_FOUND" => vec![
                "Run createSession to start a new browser session".to_string(),
                "Use listSessions to see available sessions".to_string(),
                format!("Verify the session_id parameter in {}", tool_name),
            ],
            "ELEMENT_NOT_FOUND" => vec![
                "Use listInteractable to see clickable elements".to_string(),
                "Try extractWebContent to view page structure".to_string(),
                "Verify the selector syntax is correct".to_string(),
            ],
            "DUPLICATE_TITLE" => vec![
                "Use a different title for the new item".to_string(),
                "Use update_todo to modify the existing item".to_string(),
                "Use list_todos to see all existing items".to_string(),
            ],
            _ => vec![
                "Review the error message for specific details".to_string(),
                "Check tool documentation for correct usage".to_string(),
            ],
        }
    }
}
```

### Failure Tracking for Escalated Guidance

Track repeated failures to provide more detailed help:

```typescript
class FailureTracker {
  private failures = new Map<string, number>();

  recordFailure(toolName: string, errorCode: string): void {
    const key = `${toolName}:${errorCode}`;
    this.failures.set(key, (this.failures.get(key) || 0) + 1);
  }

  shouldEscalateGuidance(toolName: string, errorCode: string): boolean {
    const count = this.failures.get(`${toolName}:${errorCode}`) || 0;
    return count >= 3; // After 3 failures, escalate
  }

  getEscalatedGuidance(errorCode: string): string {
    const detailed = DETAILED_TROUBLESHOOTING[errorCode];
    return (
      detailed ||
      'This error has occurred multiple times. Consider:\n' +
        '- Reviewing the complete tool documentation\n' +
        '- Trying an alternative approach\n' +
        '- Checking for system-level issues'
    );
  }

  reset(toolName: string, errorCode: string): void {
    this.failures.delete(`${toolName}:${errorCode}`);
  }
}
```

---

## Tool Chaining & Next Actions

### Success Path Hints

Always suggest logical next steps after successful operations:

> **📌 Note:** Each tool group has its own recovery paths. Never cross-reference tools from different groups.
> **📌 Tool Group Boundary:** Success hints should primarily suggest tools within the same group. Cross-group suggestions are acceptable only for high-level workflow transitions (e.g., "planning complete → browser automation").

```typescript
// Browser Tools - Suggest browser workflow
const BROWSER_TOOL_CHAIN_HINTS: Record<string, string[]> = {
  createSession: [
    'Use navigateToUrl to load a webpage',
    'Or use extractWebContent to see the initial page'
  ],
  navigateToUrl: [
    'Use extractWebContent to see page content',
    'Or listInteractable to see clickable elements'
  ],
  extractWebContent: [
    'Use listInteractable to see interactive elements',
    'Or clickElement to interact with the page'
  ]
};

// Planning Tools - Suggest planning workflow
const PLANNING_TOOL_CHAIN_HINTS: Record<string, string[]> = {
  addTodo: [
    'Use list_todos to see all todos',
    'Use update_todo to modify details',
    'Use complete_todo to mark as done'
  ],
  createPlan: [
    'Use list_plans to see all plans',
    'Use add_todo to create tasks for this plan'
  ],
  completeTodo: [
    'Use list_todos to see remaining tasks',
    'Use add_todo to create follow-up tasks'
  ]
};

function appendNextActionHints(toolName: string, toolGroup: string): string {
  const hints = toolGroup === 'browser'
    ? BROWSER_TOOL_CHAIN_HINTS[toolName]
    : PLANNING_TOOL_CHAIN_HINTS[toolName];

    'list_todos - View all items',
    'delete_todo - Remove duplicate'
  ],
  TODO_NOT_FOUND: [
    'list_todos - See all available todos',
    'add_todo - Create a new todo'
  ],
  INVALID_PARENT: [
    'list_todos - Find valid parent todos',
    'add_todo - Create as top-level todoone'
  ],
  createPlan: [
    'Use list_plans to see all plans',
    'Use add_todo to create tasks for this plan'
  ]
};

function appendNextActionHints(toolName: string): string {
  const hints = TOOL_CHAIN_HINTS[toolName];
  if (!hints || hints.length === 0) return '';

  return '\n\n💡 Next: ' + hints.join(' or ');
}
```

### Error Recovery Paths

Suggest alternative tools when operations fail:

```typescript
const ERROR_RECOVERY_TOOLS: Record<string, string[]> = {
  ELEMENT_NOT_FOUND: [
    'listInteractable - See all clickable elements',
    'extractWebContent - View page structure',
    'navigateToUrl - Try a different page',
  ],
  SESSION_NOT_FOUND: [
    'createSession - Start a new browser session',
    'listSessions - See active sessions',
  ],
  DUPLICATE_TITLE: [
    'update_todo - Modify existing item',
    'list_todos - View all items',
    'delete_todo - Remove duplicate',
  ],
};
```

---

## Implementation Checklist

Use this checklist when implementing a new built-in tool:

### Input Validation

- [ ] All required parameters validated for existence and type
- [ ] Value constraints checked (range, length, format) with specific errors
- [ ] Business logic validated (duplicates, conflicts)
- [ ] Relational integrity validated (foreign keys, dependencies)
- [ ] Array elements validated with 1-based indices in errors
- [ ] Case-insensitive comparison used where appropriate

### Error Handling

- [ ] All errors include human-readable text content
- [ ] Error messages follow template: `✗ [description]\n\n💡 Next Steps:\n[guidance]`
- [ ] Each error code has 2-3 actionable recovery steps
- [ ] `structured_content` is `null` or `undefined` in all error responses
- [ ] `is_error` flag is `true` for error responses
- [ ] Error codes follow consistent taxonomy

### Success Responses

- [ ] Success messages include confirmation text
- [ ] Key result details included (IDs, names, status)
- [ ] Suggested next actions included (`💡 Next: ...`)
- [ ] Both text and structured content provided
- [ ] `is_error` flag is `false`
- [ ] Structured content matches documented schema

### Documentation

- [ ] Tool registered in schema with clear description
- [ ] Input parameters documented with types and constraints
- [ ] Output schema documented
- [ ] Error codes documented with recovery guidance
- [ ] Usage examples provided

### Testing

- [ ] Unit tests cover all validation paths
- [ ] Error cases tested with expected guidance
- [ ] Success cases tested with expected output format
- [ ] Edge cases tested (empty strings, boundary values)
- [ ] Tool chaining tested with related tools

---

## Code Examples

### Complete Tool Implementation (TypeScript)

```typescript
import type { MCPResult, MCPTool } from '../mcp-types';

interface AddTodoParams {
  sessionId: string;
  title: string;
  priority?: number;
  parentId?: string;
}

export const ADD_TODO_TOOL: MCPTool = {
  name: 'add_todo',
  description: 'Create a new todo item in the planning session',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: { type: 'string', description: 'Planning session ID' },
      title: { type: 'string', description: 'Todo title' },
      priority: { type: 'number', description: 'Priority 1-5 (default: 3)' },
      parentId: {
        type: 'string',
        description: 'Optional parent todo ID for subtasks',
      },
    },
    required: ['sessionId', 'title'],
  },
};

export async function handleAddTodo(
  params: unknown,
  db: Database,
): Promise<MCPResult> {
  try {
    // Layer 1: Parameter validation
    const args = params as AddTodoParams;
    if (!args.sessionId || typeof args.sessionId !== 'string') {
      return createError(
        'MISSING_REQUIRED_PARAM',
        'sessionId is required and must be a string',
      );
    }
    if (!args.title || typeof args.title !== 'string') {
      return createError(
        'MISSING_REQUIRED_PARAM',
        'title is required and must be a string',
      );
    }

    // Layer 2: Value constraints
    const title = args.title.trim();
    if (title.length === 0) {
      return createError(
        'INVALID_INPUT',
        'title cannot be empty or whitespace-only.\n\nExample: "Implement user authentication"',
      );
    }

    const priority = args.priority ?? 3;
    if (priority < 1 || priority > 5) {
      return createError(
        'INVALID_INPUT',
        `priority must be between 1 and 5. Received: ${args.priority}`,
      );
    }

    // Layer 3: Business logic validation
    const duplicate = await db.query(
      'SELECT id FROM todos WHERE session_id = ? AND LOWER(title) = LOWER(?)',
      [args.sessionId, title],
    );
    if (duplicate.length > 0) {
      return createError(
        'DUPLICATE_TITLE',
        `Todo "${title}" already exists in this session.`,
        [
          'Use update_todo to modify the existing todo',
          'Use list_todos to see all todos',
        ],
      );
    }

    // Layer 4: Relational validation
    if (args.parentId) {
      const parent = await db.query(
        'SELECT id, parent_id FROM todos WHERE id = ? AND session_id = ?',
        [args.parentId, args.sessionId],
      );

      if (parent.length === 0) {
        return createError(
          'INVALID_PARENT',
          `Parent todo ${args.parentId} not found.`,
          [
            'Use list_todos to see available todos',
            'Create as top-level todo by omitting parentId',
          ],
        );
      }

      if (parent[0].parent_id) {
        return createError(
          'NESTING_TOO_DEEP',
          'Cannot nest todos more than 2 levels deep (parent → child → grandchild).',
          [
            'Create as top-level todo',
            'Attach to a different parent that has no parent',
          ],
        );
      }
    }

    // Execute operation
    const result = await db.insert('todos', {
      session_id: args.sessionId,
      title,
      priority,
      parent_id: args.parentId || null,
      status: 'pending',
      created_at: new Date().toISOString(),
    });

    // Return success response
    return {
      content: [
        {
          type: 'text',
          text: `
✓ Todo created successfully

ID: ${result.id}
Title: ${title}
Priority: ${priority}/5
Status: pending

💡 Next: Use update_todo to modify or complete_todo when done
        `.trim(),
        },
      ],
      structured_content: {
        id: result.id,
        title,
        priority,
        status: 'pending',
        created_at: result.created_at,
        parent_id: args.parentId || null,
      },
      is_error: false,
    };
  } catch (error) {
    return createError(
      'INTERNAL_ERROR',
      `Failed to create todo: ${error instanceof Error ? error.message : String(error)}`,
      ['Check database connectivity', 'Verify session exists', 'Try again'],
    );
  }
}

function createError(
  code: string,
  message: string,
  guidance?: string[],
): MCPResult {
  const defaultGuidance = [
    'Review the error message for specific details',
    'Check tool documentation for correct usage',
  ];

  const steps = guidance || defaultGuidance;
  const guidanceText = steps.map((step, i) => `${i + 1}. ${step}`).join('\n');

  return {
    content: [
      {
        type: 'text',
        text: `✗ ${message}\n\n💡 Next Steps:\n${guidanceText}`,
      },
    ],
    structured_content: undefined,
    is_error: true,
  };
}
```

### Complete Tool Implementation (Rust)

```rust
use serde_json::{json, Value};
use sqlx::SqlitePool;
use crate::mcp::types::{MCPResult, MCPContent};

pub struct TodoServer {
    db_pool: SqlitePool,
}

impl TodoServer {
    pub async fn add_todo(&self, args: Value) -> Result<MCPResult, String> {
        // Layer 1: Parameter validation
        let session_id = args.get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "sessionId is required and must be a string".to_string())?;

        let title = args.get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "title is required and must be a string".to_string())?;

        let priority = args.get("priority")
            .and_then(|v| v.as_i64())
            .unwrap_or(3) as i32;

        let parent_id = args.get("parentId").and_then(|v| v.as_str());

        // Layer 2: Value constraints
        let title_trimmed = title.trim();
        if title_trimmed.is_empty() {
            return Err(
                "title cannot be empty or whitespace-only.\n\n\
                 Example: \"Implement user authentication\"".to_string()
            );
        }

        if !(1..=5).contains(&priority) {
            return Err(format!(
                "priority must be between 1 and 5. Received: {}",
                priority
            ));
        }

        // Layer 3: Business logic validation
        let duplicate = sqlx::query!(
            "SELECT id FROM todos WHERE session_id = ? AND LOWER(title) = LOWER(?)",
            session_id,
            title_trimmed
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if duplicate.is_some() {
            return Err(format!(
                "✗ Todo \"{}\" already exists in this session.\n\n\
                 💡 Next Steps:\n\
                 1. Use update_todo to modify the existing todo\n\
                 2. Use list_todos to see all todos",
                title_trimmed
            ));
        }

        // Layer 4: Relational validation
        if let Some(pid) = parent_id {
            let parent = sqlx::query!(
                "SELECT id, parent_id FROM todos WHERE id = ? AND session_id = ?",
                pid,
                session_id
            )
            .fetch_optional(&self.db_pool)
            .await
            .map_err(|e| format!("Database error: {}", e))?;

            match parent {
                None => return Err(format!(
                    "✗ Parent todo {} not found.\n\n\
                     💡 Next Steps:\n\
                     1. Use list_todos to see available todos\n\
                     2. Create as top-level todo by omitting parentId",
                    pid
                )),
                Some(p) if p.parent_id.is_some() => return Err(
                    "✗ Cannot nest todos more than 2 levels deep.\n\n\
                     💡 Next Steps:\n\
                     1. Create as top-level todo\n\
                     2. Attach to a different parent that has no parent".to_string()
                ),
                _ => {}
            }
        }

        // Execute operation
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query!(
            "INSERT INTO todos (session_id, title, priority, parent_id, status, created_at) \
             VALUES (?, ?, ?, ?, 'pending', ?) RETURNING id",
            session_id,
            title_trimmed,
            priority,
            parent_id,
            now
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| format!("Failed to insert todo: {}", e))?;

        // Return success response
        Ok(MCPResult {
            content: Some(vec![MCPContent::Text {
                text: format!(
                    "✓ Todo created successfully\n\n\
                     ID: {}\n\
                     Title: {}\n\
                     Priority: {}/5\n\
                     Status: pending\n\n\
                     💡 Next: Use update_todo to modify or complete_todo when done",
                    result.id,
                    title_trimmed,
                    priority
                ),
            }]),
            structured_content: Some(json!({
                "id": result.id,
                "title": title_trimmed,
                "priority": priority,
                "status": "pending",
                "created_at": now,
                "parent_id": parent_id
            })),
            is_error: Some(false),
        })
    }
}
```

---

## Anti-Patterns

### ❌ Leaking Internal State in Errors

```typescript
// BAD: Exposing internals to LLM
return {
  content: [{ type: 'text', text: 'Operation failed' }],
  structured_content: {
    stack_trace: error.stack, // ❌ Security risk
    internal_state: debugData, // ❌ Information leak
    database_query: sqlQuery, // ❌ Exposes implementation
  },
  is_error: true,
};

// GOOD: Clean error with guidance
return {
  content: [{ type: 'text', text: '✗ Operation failed\n\n💡 Try: ...' }],
  structured_content: undefined, // ✅ No internal data
  is_error: true,
};
```

### ❌ Generic Error Messages

```typescript
// BAD: No actionable information
return createError('Error', 'Operation failed');

// GOOD: Specific with guidance
return createError(
  'NAVIGATION_FAILED',
  `Navigation to ${url} failed: ${reason}\n\n` +
    `💡 Next Steps:\n` +
    `1. Verify URL is valid and accessible\n` +
    `2. Check network connectivity\n` +
    `3. Try extractWebContent after navigation`,
);
```

### ❌ Silent Input Coercion

```typescript
// BAD: Silently fixes invalid input
const priority = Math.max(1, Math.min(5, input.priority || 3));

// GOOD: Explicit validation
if (!input.priority || input.priority < 1 || input.priority > 5) {
  throw new Error(
    `priority must be between 1 and 5. Received: ${input.priority}\n\n` +
      `Example: { "priority": 3 }`,
  );
}
```

### ❌ Missing Success Guidance

```typescript
// BAD: No next action hints
return { content: [{ type: 'text', text: 'Todo created' }], is_error: false };

// GOOD: Includes tool chaining hints
return {
  content: [
    {
      type: 'text',
      text: `
✓ Todo created successfully

ID: ${id}

💡 Next: Use update_todo to modify or complete_todo when done
    `.trim(),
    },
  ],
  is_error: false,
};
```

### ❌ Inconsistent Response Formats

```typescript
// BAD: Mixed visual markers
return { content: [{ type: 'text', text: 'ERROR: Failed' }] };
return { content: [{ type: 'text', text: '[Success] Created' }] };

// GOOD: Consistent markers
return { content: [{ type: 'text', text: '✗ Operation failed' }] };
return { content: [{ type: 'text', text: '✓ Todo created' }] };
```

---

## Testing Guidelines

### Unit Test Structure

```typescript
describe('add_todo', () => {
  describe('input validation', () => {
    it('should reject missing title', async () => {
      const result = await addTodo({ sessionId: 'test' });
      expect(result.is_error).toBe(true);
      expect(result.content[0].text).toContain('title is required');
      expect(result.content[0].text).toContain('💡 Next Steps:');
    });

    it('should reject empty title', async () => {
      const result = await addTodo({ sessionId: 'test', title: '   ' });
      expect(result.is_error).toBe(true);
      expect(result.content[0].text).toContain('cannot be empty');
    });

    it('should reject invalid priority range', async () => {
      const result = await addTodo({
        sessionId: 'test',
        title: 'Test',
        priority: 10,
      });
      expect(result.is_error).toBe(true);
      expect(result.content[0].text).toContain(
        'priority must be between 1 and 5',
      );
    });
  });

  describe('business logic validation', () => {
    it('should reject duplicate title (case-insensitive)', async () => {
      await addTodo({ sessionId: 'test', title: 'Test Todo' });
      const result = await addTodo({ sessionId: 'test', title: 'test todo' });

      expect(result.is_error).toBe(true);
      expect(result.content[0].text).toContain('already exists');
      expect(result.content[0].text).toContain('update_todo');
    });

    it('should reject invalid parent', async () => {
      const result = await addTodo({
        sessionId: 'test',
        title: 'Subtask',
        parentId: 'nonexistent',
      });

      expect(result.is_error).toBe(true);
      expect(result.content[0].text).toContain(
        'Parent todo nonexistent not found',
      );
      expect(result.content[0].text).toContain('list_todos');
    });

    it('should reject nesting beyond 2 levels', async () => {
      const parent = await addTodo({ sessionId: 'test', title: 'Parent' });
      const child = await addTodo({
        sessionId: 'test',
        title: 'Child',
        parentId: parent.structured_content.id,
      });
      const grandchild = await addTodo({
        sessionId: 'test',
        title: 'Grandchild',
        parentId: child.structured_content.id,
      });

      expect(grandchild.is_error).toBe(true);
      expect(grandchild.content[0].text).toContain('more than 2 levels');
    });
  });

  describe('success response', () => {
    it('should return formatted success message', async () => {
      const result = await addTodo({
        sessionId: 'test',
        title: 'New Todo',
        priority: 4,
      });

      expect(result.is_error).toBe(false);
      expect(result.content[0].text).toContain('✓');
      expect(result.content[0].text).toContain('New Todo');
      expect(result.content[0].text).toContain('Priority: 4/5');
      expect(result.content[0].text).toContain('💡 Next:');
    });

    it('should include structured data in success', async () => {
      const result = await addTodo({
        sessionId: 'test',
        title: 'Test',
        priority: 3,
      });

      expect(result.structured_content).toBeDefined();
      expect(result.structured_content.title).toBe('Test');
      expect(result.structured_content.priority).toBe(3);
      expect(result.structured_content.status).toBe('pending');
    });
  });

  describe('error response format', () => {
    it('should never include structured_content in errors', async () => {
      const result = await addTodo({ sessionId: 'test' });

      expect(result.is_error).toBe(true);
      expect(result.structured_content).toBeUndefined();
    });

    it('should include guidance in all errors', async () => {
      const result = await addTodo({ sessionId: 'test', title: '' });

      expect(result.content[0].text).toContain('💡 Next Steps:');
      expect(result.content[0].text).toMatch(/\d+\./); // Numbered list
    });
  });
});
```

### Integration Test Examples

```typescript
describe('tool chaining', () => {
  it('should support create → update → complete flow', async () => {
    const created = await addTodo({ sessionId: 'test', title: 'Test' });
    expect(created.is_error).toBe(false);

    const updated = await updateTodo({
      sessionId: 'test',
      todoId: created.structured_content.id,
      priority: 5,
    });
    expect(updated.is_error).toBe(false);

    const completed = await completeTodo({
      sessionId: 'test',
      todoId: created.structured_content.id,
    });
    expect(completed.is_error).toBe(false);
  });
});
```

---

## Reference Implementations

Refer to these implementations as examples:

### TypeScript Web MCP

- **Planning Server**: [`src/lib/web-mcp/modules/planning-server/`](../../src/lib/web-mcp/modules/planning-server/)
  - Comprehensive validation patterns
  - State-dependent validation
  - Duplicate detection
- **Browser Tools**: [`src/features/tools/browser-tools/`](../../src/features/tools/browser-tools/)
  - Centralized error handling
  - Error guidance system
  - Failure tracking

### Rust Built-in Tools

- **Planning**: [`src-tauri/src/mcp/builtin/planning/mod.rs`](../../src-tauri/src/mcp/builtin/planning/mod.rs)
  - 100% validation parity with legacy
  - SQLite integration
  - Session isolation
- **Browser**: [`src-tauri/src/mcp/builtin/browser.rs`](../../src-tauri/src/mcp/builtin/browser.rs)
  - Tauri integration
  - Resource management
  - Mixed error patterns (reference for improvement)

---

## Related Documentation

- [Chat Feature Architecture](../architecture/chat-feature-architecture.md) - Tool integration patterns
- [UI Resource Implementation Guide](./ui-resource-implementation.md) - Interactive HTML responses
- [MCP Protocol Specification](../mcp/) - Protocol details
- [LibrAgent Coding Standards](../../.github/copilot-instructions.md) - Project-wide coding style

---

## Version History

| Version | Date       | Changes                                              |
| ------- | ---------- | ---------------------------------------------------- |
| 1.0.0   | 2025-12-30 | Initial comprehensive guide based on legacy analysis |

---

**Questions or Suggestions?** Open an issue or submit a PR to improve this guide.
