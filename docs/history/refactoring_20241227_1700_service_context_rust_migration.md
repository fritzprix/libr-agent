# Refactoring Plan: Service Context Rust Migration

**Date**: 2024-12-27 17:00  
**Author**: AI Assistant  
**Status**: Proposed

---

## 1. 작업의 목적

Built-in MCP Server의 `getServiceContext()` 메서드를 TypeScript Web MCP 구현과 동일한 수준의 상세 정보를 제공하도록 개선하여, LLM이 현재 세션의 작업 상태를 정확히 인지하고 컨텍스트에 맞는 응답을 생성할 수 있도록 한다.

**핵심 목표:**

- Planning Server: Goal, Todos, Scratchpad 상태를 실시간으로 LLM에 전달
- Knowledge Server: Knowledge 항목 개수와 사용 안내 제공 (도구 설명은 제외)
- Content Store Server: 저장된 Content 목록을 Planning Scratchpad와 유사한 형식으로 표시

---

## 2. 현재 상태 및 문제점

### 2.1. TypeScript Web MCP vs Rust Built-in 구현 불일치

**Planning Server:**

- ❌ **Rust**: 기본 서버 정보만 제공 (session_id, 기능 설명)
- ✅ **TS**: Goal 내용, Todos 목록 (unchecked/checked 구분), Scratchpad 항목 (최대 5개)
- **문제**: LLM이 현재 작업 상태를 알 수 없어 컨텍스트에 맞지 않는 응답 생성

**Knowledge Server:**

- ❌ **Rust**: 최소 정보 (session_id, 서버 설명)
- ✅ **TS**: Knowledge 항목 개수, Empty state 안내, 사용 가능한 Operation 설명
- **문제**: LLM이 Knowledge base가 비어있는지 알 수 없음

**Content Store Server:**

- 🟡 **Rust**: 부분 구현 (파일 개수만 표시)
- ❌ **TS**: Web MCP에 구현 없음
- **개선 필요**: Scratchpad와 유사하게 content 제목, 내용 미리보기, 글자 수 표시

### 2.2. Frontend에서 System Prompt 구성 문제

현재 구조:

```typescript
// Frontend (TypeScript)
BuiltInToolProvider.buildToolPrompt()
  └─ service.getServiceContext() 호출 (각 서비스)
      └─ SystemPromptProvider.getSystemPrompt()
          └─ ChatContext.buildSystemPrompt()
              └─ useAIService.submit()
```

**문제점:**

- Frontend 상태 변경 시 Agentic Workflow 중단
- Session 전환 시 context 손실
- Multi-agent 지원 불가

---

## 3. 관련 코드 구조 및 동작 방식 Summary

### 3.1. Built-in MCP Server Architecture

```
AgentSessionManager
  ├─ MCPServiceProxyManager
  │   └─ MCPServiceProxy (per session)
  │       ├─ KnowledgeServer
  │       ├─ PlanningServer
  │       ├─ ContentStoreServer
  │       └─ WorkspaceServer
  └─ request_llm_completion()
      ├─ build_system_prompt(session_id) ← 여기서 완성
      └─ emit("agent://llm-completion-request", {
           messages,
           systemPrompt,  // ← Rust에서 완성하여 전달
         })
```

**Data Flow (Option B - System Prompt 전체 Rust 이관):**

1. AgentSessionManager가 LLM completion 요청 (`request_llm_completion()`)
2. **Backend에서 System Prompt 구성** (`build_system_prompt()`):
   - **Agent base prompt** (assistant.system_prompt from DB/config)
   - **Built-in Tool Context** (MCPServiceProxy.get_service_contexts())
     - PlanningServer.get_service_context() [IMPROVED]
     - KnowledgeServer.get_service_context() [IMPROVED]
     - ContentStoreServer.get_service_context() [IMPROVED]
     - WorkspaceServer.get_service_context() [COMPLETE]
   - **(Optional) Extension prompts**: Time/Location, etc. (향후 확장)
3. Frontend로 event emit: `agent://llm-completion-request` (완성된 systemPrompt 포함)
4. Frontend는 systemPrompt를 그대로 사용하여 LLM API 호출 (`useAIService.submit()`)
5. 응답을 Rust backend로 전달 (`agent_handle_llm_response()`)

**핵심 변경사항:**

- **Before (Option A)**: Frontend가 `invoke('get_builtin_service_contexts')` 호출 후 조합
- **After (Option B)**: Rust Backend가 완성된 `systemPrompt` 제공
- **장점**: Zero round-trip, Session isolation, Multi-Agent ready

### 3.2. Current getServiceContext() Implementations

**Planning Server** (`src-tauri/src/mcp/builtin/planning/mod.rs:538`):

```rust
fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    ServiceContext {
        context_prompt: format!(
            "# Planning Server Status\n\
            **Session**: {}\n\
            **Status**: Active\n\
            **Features**: Goal tracking, Todo management, Scratchpad notes",
            self.session_id
        ),
        structured_state: None,
    }
}
```

**Knowledge Server** (`src-tauri/src/mcp/builtin/knowledge/mod.rs:344`):

```rust
fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    ServiceContext {
        context_prompt: format!(
            "Knowledge Server (Session: {}): Save and search knowledge with full-text search",
            self.session_id
        ),
        structured_state: Some(json!({
            "session_id": self.session_id,
            "server": "knowledge"
        })),
    }
}
```

**Content Store Server** (`src-tauri/src/mcp/builtin/content_store/server.rs:124`):

```rust
pub fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
    let mut context = format!("## Content Store\n\nActive, {tools_count} tools");

    if let Some(session_id) = session_id {
        if let Ok(storage) = self.storage.try_lock() {
            let count = storage.get_content_count(session_id);
            context.push_str(&format!(", {count} files"));
        }
    }

    ServiceContext { context_prompt: context, structured_state: None }
}
```

### 3.3. TypeScript Web MCP Reference Implementations

**Planning Server** (`src/lib/web-mcp/modules/planning-server/server.ts:273`):

```typescript
async getServiceContext(options?: ServiceContextOptions) {
  const state = await stateManager.getStateForSession(sessionId, threadId);
  const { goal, todos, scratchpad } = state;
  const uncheckedTodos = todos.filter((t) => !t.checked);
  const checkedTodos = todos.filter((t) => t.checked);

  const contextParts = ['## Planning'];

  // Goal section
  if (goal) {
    contextParts.push(`\n**Current Goal:** "${goal}"`);
    contextParts.push('*Goal is active. Track progress with todos below.*');
  } else {
    contextParts.push('\n**No Goal Set**');
    contextParts.push('*Consider using createGoal to establish a clear objective...*');
  }

  // Todos section
  if (todos.length > 0) {
    contextParts.push(
      `\n**Todos:** ${uncheckedTodos.length} unchecked / ${checkedTodos.length} checked (${todos.length} total)`
    );

    if (uncheckedTodos.length > 0) {
      contextParts.push('\n**Unchecked Items:**');
      uncheckedTodos.slice(0, 5).forEach((t, idx) => {
        const priority = t.priority ? `Priority:${t.priority}` : 'Priority:none';
        const description = t.description
          ? `\n     ${t.description.slice(0, 80)}${t.description.length > 80 ? '...' : ''}`
          : '';

        contextParts.push(
          `  [${idx}] ID:${t.id} | ${t.title} | ${priority}${description}`
        );
      });

      if (uncheckedTodos.length > 5) {
        contextParts.push(`  ...and ${uncheckedTodos.length - 5} more (use listTodos to see all)`);
      }
    }

    // Show completed todos for work trace awareness
    if (checkedTodos.length > 0) {
      contextParts.push('\n**Checked Items (Completed):**');
      const recentCompleted = checkedTodos.slice(-3).reverse();
      recentCompleted.forEach((t) => {
        const priority = t.priority ? `[${t.priority}]` : '';
        const summary = t.summary
          ? ` → ${t.summary.slice(0, 60)}${t.summary.length > 60 ? '...' : ''}`
          : '';

        contextParts.push(`  [✓] ID:${t.id} | ${t.title} ${priority}${summary}`);
      });
    }
  }

  // Scratchpad section
  if (scratchpad.length > 0) {
    contextParts.push(`\n**Scratchpad:** ${scratchpad.length} items`);
    scratchpad.slice(0, 5).forEach((m, idx) => {
      const titlePart = m.title ? `**${m.title}**` : '';
      const tagsPart = Array.isArray(m.tags) && m.tags.length > 0
        ? ` [${m.tags.join('] [')}]`
        : '';
      const contentPreview = m.title
        ? ` - ${m.content.slice(0, 50)}${m.content.length > 50 ? '...' : ''}`
        : m.content.slice(0, 60) + (m.content.length > 60 ? '...' : '');

      contextParts.push(
        `  ${idx + 1}. **ID:${m.id}** ${titlePart}${contentPreview}${tagsPart}`
      );
    });
  }

  return { contextPrompt: contextParts.join('\n'), structuredState };
}
```

**Knowledge Server** (`src/lib/web-mcp/modules/knowledge-server/server.ts:358`):

```typescript
async getServiceContext(options?: ServiceContextOptions) {
  const assistantId = options?.assistantId || 'default';
  const knowledgeCount = await db.knowledge
    .where('assistantId')
    .equals(assistantId)
    .count();

  const contextParts = ['## Knowledge Base'];

  if (knowledgeCount === 0) {
    contextParts.push(
      '\n**No knowledge entries yet.**',
      '*Use saveKnowledge to store important information for future reference.*',
      '*Tip: Save key facts, decisions, or context that might be useful later.*',
    );
  } else {
    contextParts.push(
      `\n**${knowledgeCount} knowledge ${knowledgeCount === 1 ? 'entry' : 'entries'} available**`,
    );
    // ❌ 도구 설명은 제거 필요 (중복)
  }

  return { contextPrompt: contextParts.join('\n'), structuredState };
}
```

---

## 4. 변경 이후 상태 및 해결 판정 기준

### 4.1. Planning Server 완성 기준

**Context 포함 내용:**

- ✅ Current Goal (활성화 여부, 내용)
- ✅ Todos: unchecked 항목 (최대 5개, index/ID/title/priority/description 표시)
- ✅ Todos: checked 항목 (최근 3개, summary 포함)
- ✅ Scratchpad: 메모 항목 (최대 5개, ID/title/content preview/tags 표시)

**검증 방법:**

```rust
#[tokio::test]
async fn test_planning_service_context() {
    let server = PlanningServer::new(session_id, db_pool).await?;

    // Set goal and add todos
    server.set_goal(json!({"goal": "Test goal"})).await?;
    server.add_todo(json!({"title": "Task 1", "priority": "high"})).await?;

    let context = server.get_service_context(None);

    assert!(context.context_prompt.contains("Test goal"));
    assert!(context.context_prompt.contains("Task 1"));
    assert!(context.context_prompt.contains("Priority:high"));
}
```

### 4.2. Knowledge Server 완성 기준

**Context 포함 내용:**

- ✅ Knowledge 항목 개수
- ✅ Empty state 안내 (0개일 때)
- ❌ 도구 설명 제거 (중복)

**검증 방법:**

```rust
#[tokio::test]
async fn test_knowledge_service_context() {
    let server = KnowledgeServer::new(session_id, db_pool).await?;

    // Empty state
    let context = server.get_service_context(None);
    assert!(context.context_prompt.contains("No knowledge entries yet"));

    // After adding knowledge
    server.save_knowledge(json!({"title": "Test", "content": "..."})).await?;
    let context = server.get_service_context(None);
    assert!(context.context_prompt.contains("1 knowledge entry available"));
    assert!(!context.context_prompt.contains("searchKnowledge")); // 도구 설명 없음
}
```

### 4.3. Content Store Server 완성 기준

**Context 포함 내용:**

- ✅ Content 항목 개수
- ✅ Content 목록 (최대 5개, Scratchpad 스타일)
  - ID, 제목 (metadata.title), 내용 미리보기 (50자), 총 글자 수

**검증 방법:**

```rust
#[tokio::test]
async fn test_content_store_service_context() {
    let server = ContentStoreServer::new(session_manager).await?;

    // Add content
    server.add_content(json!({
        "content": "Test content...",
        "metadata": {"title": "Test File"}
    })).await?;

    let context = server.get_service_context(Some(&json!({
        "sessionId": session_id
    })));

    assert!(context.context_prompt.contains("1 file"));
    assert!(context.context_prompt.contains("Test File"));
    assert!(context.context_prompt.contains("Test content")); // Preview
}
```

---

## 5. 수정이 필요한 코드 및 코드 스니핏

### 5.0. Async Trait Migration (Phase 0)

**목적**: `get_service_context()` 메서드를 async trait로 변환하여 성능 최적화 및 `block_on()` overhead 제거

---

#### 5.0.1. BuiltinServer Trait 정의 변경

**파일**: `src-tauri/src/mcp/builtin/mod.rs` (Lines 70-85)

**Before:**

```rust
#[async_trait::async_trait]
pub trait BuiltinServer: Send + Sync {
    // ... other methods

    /// Returns a markdown-formatted string describing the server's current status and context.
    fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: format!(/* ... */),
            structured_state: None,
        }
    }
}
```

**After:**

```rust
#[async_trait::async_trait]
pub trait BuiltinServer: Send + Sync {
    // ... other methods

    /// Returns a markdown-formatted string describing the server's current status and context.
    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: format!(/* ... */),
            structured_state: None,
        }
    }
}
```

**변경사항:**

- `fn` → `async fn`
- Default implementation 유지 (기본 동작 변경 없음)

---

#### 5.0.2. Built-in Server 구현체 수정 (7개 서버)

**패턴 (반복 적용):**

**Before (예시: WorkspaceServer):**

```rust
fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    let tree = self.get_directory_tree();  // Sync method
    ServiceContext {
        context_prompt: format!("## Workspace\n{}", tree),
        structured_state: None,
    }
}
```

**After:**

```rust
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    let tree = self.get_directory_tree();  // Sync method (변경 없음)
    ServiceContext {
        context_prompt: format!("## Workspace\n{}", tree),
        structured_state: None,
    }
}
```

**적용 대상:**

1. `planning/mod.rs:538` - PlanningServer (DB query with `block_on()` → `.await`)
2. `knowledge/mod.rs:344` - KnowledgeServer (DB query with `block_on()` → `.await`)
3. `content_store/mod.rs:45` - ContentStoreServer (in-memory, 변경 최소)
4. `workspace/mod.rs:681` - WorkspaceServer (sync method, async 선언만)
5. `playbook/mod.rs:606` - PlaybookServer (sync method, async 선언만)
6. `bootstrap/mod.rs:112` - BootstrapServer (sync method, async 선언만)
7. `assistant/mod.rs:435` - AssistantServer (sync method, async 선언만)

---

#### 5.0.3. MCPServiceProxy 호출부 수정

**파일**: `src-tauri/src/mcp/service_proxy.rs`

**Before:**

```rust
pub async fn get_service_contexts(
    &self,
    options: ServiceContextOptions,
) -> HashMap<String, String> {
    let mut contexts = HashMap::new();

    for (tool_id, server) in &self.builtin_servers {
        let context = server.get_service_context(options_value.as_ref());  // ← Sync call
        if !context.context_prompt.trim().is_empty() {
            contexts.insert(tool_id.clone(), context.context_prompt);
        }
    }

    contexts
}
```

**After:**

```rust
pub async fn get_service_contexts(
    &self,
    options: ServiceContextOptions,
) -> HashMap<String, String> {
    let mut contexts = HashMap::new();

    for (tool_id, server) in &self.builtin_servers {
        let context = server.get_service_context(options_value.as_ref()).await;  // ← Async call
        if !context.context_prompt.trim().is_empty() {
            contexts.insert(tool_id.clone(), context.context_prompt);
        }
    }

    contexts
}
```

**변경사항:**

- `.await` 추가 (1줄 변경)

---

#### 5.0.4. Planning/Knowledge Server `block_on()` 제거 예시

**PlanningServer (Lines 538-548):**

**Before:**

```rust
fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    let rt = tokio::runtime::Handle::try_current().ok()?;

    let state = rt.block_on(async {  // ← block_on overhead
        let goal = self.db.query_goal().await?;
        let todos = self.db.query_todos().await?;
        Some((goal, todos))
    });

    // Format context...
}
```

**After:**

```rust
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    // Natural async/await
    let goal = self.db.query_goal().await.ok();
    let todos = self.db.query_todos().await.ok();

    // Format context...
}
```

**장점:**

- ✅ `block_on()` overhead 제거
- ✅ Runtime handle 에러 처리 불필요
- ✅ 더 간결하고 읽기 쉬운 코드

---

### 5.1. Planning Server (Phase 1)

**파일**: `src-tauri/src/mcp/builtin/planning/mod.rs`

**수정 대상**: `get_service_context()` 메서드 (Lines 538-548)

**수정 내용**:

```rust
fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    // Get current runtime to block on async DB queries
    let rt = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle,
        Err(_) => {
            // Fallback if no runtime available
            return ServiceContext {
                context_prompt: "Planning Server: Unable to retrieve state (no async runtime)".to_string(),
                structured_state: None,
            };
        }
    };

    // Load state from database
    let state = rt.block_on(async {
        let mut conn = self.db_pool.acquire().await.ok()?;

        // Load goal
        let goal = sqlx::query_scalar::<_, Option<String>>(
            "SELECT goal FROM planning_goals WHERE session_id = ? AND status = 'active' ORDER BY created_at DESC LIMIT 1"
        )
        .bind(&self.session_id)
        .fetch_optional(&mut *conn)
        .await
        .ok()
        .flatten();

        // Load todos
        let todos: Vec<Todo> = sqlx::query_as::<_, Todo>(
            "SELECT * FROM planning_todos WHERE session_id = ? ORDER BY created_at ASC"
        )
        .bind(&self.session_id)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();

        // Load scratchpad
        let scratchpad: Vec<ScratchpadMemo> = sqlx::query_as::<_, ScratchpadMemo>(
            "SELECT * FROM planning_scratchpad WHERE session_id = ? ORDER BY created_at DESC"
        )
        .bind(&self.session_id)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();

        Some((goal, todos, scratchpad))
    });

    let Some((goal, todos, scratchpad)) = state else {
        return ServiceContext {
            context_prompt: "Planning Server: Error loading state".to_string(),
            structured_state: None,
        };
    };

    // Build context prompt
    let mut parts = vec!["## Planning".to_string()];

    // Goal section
    if let Some(goal_text) = &goal {
        parts.push(format!("\n**Current Goal:** \"{}\"", goal_text));
        parts.push("*Goal is active. Track progress with todos below.*".to_string());
    } else {
        parts.push("\n**No Goal Set**".to_string());
        parts.push("*Consider using createGoal to establish a clear objective for this planning session.*".to_string());
    }

    // Todos section
    if !todos.is_empty() {
        let unchecked: Vec<_> = todos.iter().filter(|t| !t.checked).collect();
        let checked: Vec<_> = todos.iter().filter(|t| t.checked).collect();

        parts.push(format!(
            "\n**Todos:** {} unchecked / {} checked ({} total)",
            unchecked.len(), checked.len(), todos.len()
        ));

        // Unchecked items (top 5)
        if !unchecked.is_empty() {
            parts.push("\n**Unchecked Items:**".to_string());
            for (idx, todo) in unchecked.iter().take(5).enumerate() {
                let priority = todo.priority.as_ref()
                    .map(|p| format!("Priority:{}", p))
                    .unwrap_or_else(|| "Priority:none".to_string());

                let description = todo.description.as_ref()
                    .map(|d| {
                        let preview = if d.len() > 80 {
                            format!("{}...", &d[..80])
                        } else {
                            d.clone()
                        };
                        format!("\n     {}", preview)
                    })
                    .unwrap_or_default();

                parts.push(format!(
                    "  [{}] ID:{} | {} | {}{}",
                    idx, todo.id, todo.title, priority, description
                ));
            }

            if unchecked.len() > 5 {
                parts.push(format!(
                    "  ...and {} more (use listTodos to see all)",
                    unchecked.len() - 5
                ));
            }

            parts.push("\n*Use Index or ID when calling checkTodo/updateTodo*".to_string());
        }

        // Checked items (last 3 completed)
        if !checked.is_empty() {
            parts.push("\n**Checked Items (Completed):**".to_string());
            let recent_completed: Vec<_> = checked.iter().rev().take(3).collect();

            for todo in recent_completed {
                let priority = todo.priority.as_ref()
                    .map(|p| format!("[{}]", p))
                    .unwrap_or_default();

                let summary = todo.summary.as_ref()
                    .map(|s| {
                        let preview = if s.len() > 60 {
                            format!("{}...", &s[..60])
                        } else {
                            s.clone()
                        };
                        format!(" → {}", preview)
                    })
                    .unwrap_or_default();

                parts.push(format!(
                    "  [✓] ID:{} | {} {}{}",
                    todo.id, todo.title, priority, summary
                ));
            }

            if checked.len() > 3 {
                parts.push(format!("  ...and {} more completed", checked.len() - 3));
            }
        }
    }

    // Scratchpad section
    if !scratchpad.is_empty() {
        parts.push(format!("\n**Scratchpad:** {} items", scratchpad.len()));
        parts.push("".to_string()); // Empty line for readability

        for (idx, memo) in scratchpad.iter().take(5).enumerate() {
            let title_part = memo.title.as_ref()
                .map(|t| format!("**{}**", t))
                .unwrap_or_default();

            let tags_part = if !memo.tags.is_empty() {
                format!(" [{}]", memo.tags.join("] ["))
            } else {
                String::new()
            };

            let content_preview = if memo.title.is_some() {
                let preview = if memo.content.len() > 50 {
                    format!("{}...", &memo.content[..50])
                } else {
                    memo.content.clone()
                };
                format!(" - {}", preview)
            } else {
                let preview = if memo.content.len() > 60 {
                    format!("{}...", &memo.content[..60])
                } else {
                    memo.content.clone()
                };
                preview
            };

            parts.push(format!(
                "  {}. **ID:{}** {}{}{}",
                idx + 1, memo.id, title_part, content_preview, tags_part
            ));
        }

        if scratchpad.len() > 5 {
            parts.push(format!(
                "  ...and {} more items. Use listScratchpad to view all.",
                scratchpad.len() - 5
            ));
        }
    }

    ServiceContext {
        context_prompt: parts.join("\n"),
        structured_state: Some(json!({
            "goal": goal,
            "todos": {
                "unchecked": unchecked.len(),
                "checked": checked.len(),
                "total": todos.len()
            },
            "scratchpad": scratchpad.len()
        })),
    }
}
```

### 5.2. Knowledge Server

**파일**: `src-tauri/src/mcp/builtin/knowledge/mod.rs`

**수정 대상**: `get_service_context()` 메서드 (Lines 344-356)

**수정 내용**:

```rust
fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    // Get current runtime to block on async DB queries
    let rt = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle,
        Err(_) => {
            return ServiceContext {
                context_prompt: "Knowledge Server: Unable to retrieve state (no async runtime)".to_string(),
                structured_state: None,
            };
        }
    };

    // Get knowledge count from database
    let count = rt.block_on(async {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM knowledge WHERE session_id = ?"
        )
        .bind(&self.session_id)
        .fetch_one(&*self.db_pool)
        .await
        .unwrap_or(0)
    });

    // Build context prompt
    let mut parts = vec!["## Knowledge Base".to_string()];

    if count == 0 {
        parts.push("\n**No knowledge entries yet.**".to_string());
        parts.push("*Use saveKnowledge to store important information for future reference.*".to_string());
        parts.push("*Tip: Save key facts, decisions, or context that might be useful later.*".to_string());
    } else {
        let entry_label = if count == 1 { "entry" } else { "entries" };
        parts.push(format!("\n**{} knowledge {} available**", count, entry_label));
        // ❌ 도구 설명 제거 (중복이므로)
    }

    ServiceContext {
        context_prompt: parts.join("\n"),
        structured_state: Some(json!({
            "session_id": self.session_id,
            "knowledge_count": count
        })),
    }
}
```

### 5.3. Content Store Server

**파일**: `src-tauri/src/mcp/builtin/content_store/server.rs`

**수정 대상**: `get_service_context()` 메서드 (Lines 124-150)

**수정 내용**:

```rust
pub fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
    info!("ContentStore get_service_context called with options: {options:?}");

    // Extract session ID from options if provided
    let session_id = options
        .and_then(|opts| opts.get("sessionId"))
        .and_then(|sid| sid.as_str())
        .filter(|s| !s.is_empty());

    let Some(session_id) = session_id else {
        return ServiceContext {
            context_prompt: "## Content Store\n\nNo session ID provided".to_string(),
            structured_state: None,
        };
    };

    // Try to get storage lock without blocking
    let Ok(storage) = self.storage.try_lock() else {
        return ServiceContext {
            context_prompt: "## Content Store\n\nActive (unable to retrieve details)".to_string(),
            structured_state: None,
        };
    };

    // Get content summary for this session
    let count = storage.get_content_count(session_id);
    let contents = storage.get_content_list(session_id, 5); // Get top 5

    // Build context prompt
    let mut parts = vec!["## Content Store".to_string()];

    if count == 0 {
        parts.push("\n**No content stored yet.**".to_string());
        parts.push("*Use addContent to store files, documents, or text for later retrieval.*".to_string());
    } else {
        let file_label = if count == 1 { "file" } else { "files" };
        parts.push(format!("\n**{} {} stored**", count, file_label));
        parts.push("".to_string()); // Empty line

        for (idx, content) in contents.iter().enumerate() {
            // Extract title from metadata or use ID
            let title = content.metadata.as_ref()
                .and_then(|m| m.get("title"))
                .and_then(|t| t.as_str())
                .unwrap_or(&content.id);

            // Content preview (first 50 chars)
            let preview = if content.content.len() > 50 {
                format!("{}...", &content.content[..50])
            } else {
                content.content.clone()
            };

            // Total character count
            let char_count = content.content.len();

            parts.push(format!(
                "  {}. **ID:{}** {} - {} ({} chars)",
                idx + 1, content.id, title, preview, char_count
            ));
        }

        if count > 5 {
            parts.push(format!("  ...and {} more files. Use listContent to view all.", count - 5));
        }
    }

    ServiceContext {
        context_prompt: parts.join("\n"),
        structured_state: Some(json!({
            "session_id": session_id,
            "content_count": count
        })),
    }
}
```

---

## 5.4. AgentSessionManager - System Prompt 구성 메서드 추가

**파일**: `src-tauri/src/agent/session_manager.rs`

**목적**: Built-in service contexts와 Agent base prompt를 결합하여 완성된 system prompt 생성

```rust
impl AgentSessionManager {
    /// Build complete system prompt for session
    ///
    /// Combines:
    /// - Agent base prompt (assistant.system_prompt)
    /// - Built-in service contexts (Planning, Knowledge, ContentStore, Workspace)
    /// - (Optional) Extension prompts
    pub async fn build_system_prompt(&self, session_id: &str) -> Result<String, String> {
        let mut parts = Vec::new();

        // 1. Load Agent base prompt
        let session = self.sessions.read().await
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?
            .clone();

        if let Some(base_prompt) = &session.assistant.system_prompt {
            if !base_prompt.trim().is_empty() {
                parts.push(base_prompt.clone());
            }
        }

        // 2. Get Built-in service contexts
        if let Some(proxy) = self.proxy_manager.get_proxy(session_id).await {
            let context_options = ServiceContextOptions {
                session_id: Some(session_id.to_string()),
                limit: Some(5), // Top 5 items
            };

            let contexts = proxy.get_service_contexts(&context_options).await;

            if !contexts.is_empty() {
                parts.push("\n\n## Available Tools & Current State\n".to_string());

                for (_tool_id, context_prompt) in contexts {
                    if !context_prompt.trim().is_empty() {
                        parts.push(context_prompt);
                    }
                }
            }
        }

        // 3. (Optional) Add extension prompts
        // TODO: Time/Location, Agent Identity, etc.

        Ok(parts.join("\n"))
    }

    /// Request LLM completion with auto-generated system prompt
    pub async fn request_llm_completion(
        &self,
        session_id: String,
        user_message: String,
    ) -> Result<(), String> {
        // Build system prompt
        let system_prompt = self.build_system_prompt(&session_id).await?;

        // Emit to Frontend with complete prompt
        self.emit_llm_request(LLMCompletionRequest {
            session_id,
            user_message,
            system_prompt: Some(system_prompt), // ← 완성된 prompt
            options: None,
        })?;

        Ok(())
    }
}
```

**변경사항:**

- **Before**: `emit_llm_request()` - systemPrompt 없이 emit
- **After**: `build_system_prompt()` - 완성된 prompt 생성 후 emit
- **통합**: Agent base + Built-in contexts + Extensions

---

## 5.5. MCPServiceProxy - Context 수집 메서드 (기존 유지)

**파일**: `src-tauri/src/mcp/service_proxy.rs`

**목적**: 모든 Built-in service contexts를 수집 (AgentSessionManager.build_system_prompt()에서 사용)

**추가 내용**: `get_service_contexts()` 메서드

```rust
impl MCPServiceProxy {
    /// Collect service contexts from all builtin servers
    /// Returns a map of tool_id -> context_prompt
    pub async fn get_service_contexts(
        &self,
        options: ServiceContextOptions,
    ) -> HashMap<String, String> {
        let mut contexts = HashMap::new();

        // Convert options to Value for trait method
        let options_value = serde_json::to_value(&options).ok();

        for (tool_id, server) in &self.builtin_servers {
            let context = server.get_service_context(options_value.as_ref());

            // Only include non-empty contexts
            if !context.context_prompt.trim().is_empty() {
                contexts.insert(tool_id.clone(), context.context_prompt);
            }
        }

        log::debug!(
            "Collected {} service contexts for session '{}'",
            contexts.len(),
            self.session_id
        );

        contexts
    }
}
```

### 5.5. Tauri Command 추가

**파일**: `src-tauri/src/commands/agent_commands.rs`

**추가 내용**: `get_builtin_service_contexts` command

```rust
use std::collections::HashMap;
use crate::mcp::types::ServiceContextOptions;

#[tauri::command]
pub async fn get_builtin_service_contexts(
    session_id: String,
    state: tauri::State<'_, Arc<AgentSessionManager>>,
) -> Result<HashMap<String, String>, String> {
    log::info!("Getting builtin service contexts for session: {}", session_id);

    // Get proxy for this session
    let proxy = state.proxy_manager
        .get_proxy(&session_id)
        .await
        .ok_or_else(|| format!("No proxy found for session: {}", session_id))?;

    // Collect contexts from all builtin servers
    let context_options = ServiceContextOptions {
        session_id: Some(session_id.clone()),
        assistant_id: None, // Will be populated by Frontend if needed
        thread_id: None,     // Will be populated by Frontend if needed
    };

    let contexts = proxy.get_service_contexts(context_options).await;

    log::debug!(
        "Collected {} builtin service contexts for session '{}'",
        contexts.len(),
        session_id
    );

    Ok(contexts)
}
}
```

---

## 5.6. Frontend 변경사항 - System Prompt 처리 간소화

**파일**: `src/context/ChatContext.tsx` (또는 LLM request handler)

**목적**: Rust Backend에서 완성된 systemPrompt를 받아 그대로 사용

**변경사항:**

```typescript
// Before: Frontend가 systemPrompt 구성
const handleLLMCompletionRequest = async (event: {
  payload: LLMCompletionRequest;
}) => {
  const { sessionId, messages } = event.payload;

  // Frontend에서 systemPrompt 구성
  const systemPrompt = await buildSystemPrompt(); // ← 삭제

  await useAIService.submit(messages, systemPrompt);
};

// After: Rust에서 완성된 systemPrompt 사용
const handleLLMCompletionRequest = async (event: {
  payload: LLMCompletionRequest;
}) => {
  const { sessionId, messages, systemPrompt } = event.payload; // ← systemPrompt 포함

  // systemPrompt가 있으면 그대로 사용, 없으면 기본값
  const finalPrompt = systemPrompt || currentAssistant?.systemPrompt || '';

  await useAIService.submit(messages, finalPrompt);
};
```

**핵심 변경:**

- ❌ **삭제**: `buildSystemPrompt()` 호출 로직
- ❌ **삭제**: `buildToolPrompt()` - Rust Built-in contexts 수집 로직 (Section 5.5 Tauri command)
- ✅ **유지**: Web MCP contexts는 여전히 Frontend에서 수집 (향후 개선 대상)
- ✅ **간소화**: Rust에서 완성된 prompt를 받아 바로 사용

**Note:** Web MCP service contexts는 아직 Frontend에서 처리되므로, 완전한 통합은 향후 개선 작업에서 진행

---

## 6. Tauri Command 등록

**파일**: `src-tauri/src/lib.rs` (또는 main.rs)

**목적**: `get_builtin_service_contexts` 명령어 노출 제거 (불필요)

```rust
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // ... existing commands
            // get_builtin_service_contexts,  // ← 삭제 (더 이상 불필요)
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**이유:** Option B에서는 Frontend가 contexts를 요청할 필요가 없음 (Backend에서 완성된 prompt 제공)

---

## 7. 재사용 가능한 연관 코드

            .count()
    }

    /// Get content list for a session (limited)
    pub fn get_content_list(&self, session_id: &str, limit: usize) -> Vec<&ContentEntry> {
        self.contents.values()
            .filter(|c| c.session_id == session_id)
            .take(limit)
            .collect()
    }

}

````

---

## 6. 재사용 가능한 연관 코드

### 6.1. Workspace Server (참고용 - 이미 상세 구현)

**파일**: `src-tauri/src/mcp/builtin/workspace/mod.rs` (Lines 681-720)

**주요 기능:**
- Session-specific workspace directory 표시
- Directory tree 생성 (2 levels deep)
- Running process count 표시
- Platform 정보 (OS, architecture)

**재사용 패턴:**
```rust
fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    // 1. Get runtime data
    let workspace_dir = self.get_workspace_dir();
    let tree_output = self.get_workspace_tree(&workspace_dir, 2);
    let running_count = self.get_running_process_count();

    // 2. Format as markdown
    let context_prompt = format!(
        "## Workspace\n\
        **Directory:** {}\n\
        **Running Processes:** {}\n\n\
        **Directory Tree:**\n{}",
        workspace_dir, running_count, tree_output
    );

    // 3. Include structured state for UI
    ServiceContext {
        context_prompt,
        structured_state: Some(json!({
            "workspace_dir": workspace_dir,
            "running_processes": running_count
        })),
    }
}
````

### 6.2. ServiceContext Type Definition

**파일**: `src-tauri/src/mcp/types.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceContext {
    /// Markdown-formatted context prompt for LLM
    pub context_prompt: String,

    /// Structured state for UI components (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_state: Option<Value>,
}
```

### 6.3. Runtime Blocking Pattern

모든 Built-in servers에서 공통으로 사용할 패턴:

```rust
fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    // Get current runtime handle
    let rt = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle,
        Err(_) => {
            return ServiceContext {
                context_prompt: "Error: No async runtime available".to_string(),
                structured_state: None,
            };
        }
    };

    // Block on async DB queries
    let data = rt.block_on(async {
        // Your async queries here
    });

    // Format and return context
    ServiceContext {
        context_prompt: format_context(&data),
        structured_state: Some(json!(data)),
    }
}
```

---

## 7. Test Code 추가 및 수정 가이드

### 7.1. Planning Server Tests

**파일**: `src-tauri/src/mcp/builtin/planning/tests.rs` (신규 또는 기존 test 확장)

**테스트 시나리오:**

1. `test_service_context_empty_state` - Goal/Todos/Scratchpad 없을 때
2. `test_service_context_with_goal` - Goal만 설정된 경우
3. `test_service_context_with_todos` - Todos만 있는 경우 (unchecked/checked 구분)
4. `test_service_context_full_state` - 모든 데이터가 있는 경우
5. `test_service_context_truncation` - 5개 이상 항목일 때 truncation 확인

**테스트 예시:**

```rust
#[tokio::test]
async fn test_service_context_full_state() {
    let db_pool = setup_test_db().await;
    let session_id = "test-session".to_string();
    let server = PlanningServer::new(session_id.clone(), db_pool).await.unwrap();

    // Set goal
    server.set_goal(json!({"goal": "Complete project"})).await.unwrap();

    // Add todos
    server.add_todo(json!({
        "title": "Task 1",
        "priority": "high",
        "description": "Important task"
    })).await.unwrap();

    server.add_todo(json!({
        "title": "Task 2",
        "priority": "low"
    })).await.unwrap();

    // Add scratchpad memo
    server.add_scratchpad(json!({
        "content": "Test memo",
        "title": "Memo 1",
        "tags": ["important", "review"]
    })).await.unwrap();

    // Get context
    let context = server.get_service_context(None);

    // Verify context includes all information
    assert!(context.context_prompt.contains("Complete project"));
    assert!(context.context_prompt.contains("Task 1"));
    assert!(context.context_prompt.contains("Priority:high"));
    assert!(context.context_prompt.contains("Important task"));
    assert!(context.context_prompt.contains("2 unchecked"));
    assert!(context.context_prompt.contains("Scratchpad: 1 items"));
    assert!(context.context_prompt.contains("Memo 1"));
    assert!(context.context_prompt.contains("[important]"));

    // Verify structured state
    let state = context.structured_state.unwrap();
    assert_eq!(state["goal"], "Complete project");
    assert_eq!(state["todos"]["unchecked"], 2);
    assert_eq!(state["scratchpad"], 1);
}
```

### 7.2. Knowledge Server Tests

**파일**: `src-tauri/src/mcp/builtin/knowledge/tests.rs`

**테스트 시나리오:**

1. `test_service_context_empty` - Knowledge 없을 때 안내 메시지 확인
2. `test_service_context_with_entries` - Knowledge 항목 개수 표시 확인
3. `test_service_context_no_tool_description` - 도구 설명이 포함되지 않는지 확인

```rust
#[tokio::test]
async fn test_service_context_no_tool_description() {
    let db_pool = setup_test_db().await;
    let session_id = "test-session".to_string();
    let server = KnowledgeServer::new(session_id, db_pool).await.unwrap();

    // Add knowledge
    server.save_knowledge(json!({
        "title": "Test Knowledge",
        "content": "Test content",
        "tags": ["test"]
    })).await.unwrap();

    let context = server.get_service_context(None);

    // Should show count
    assert!(context.context_prompt.contains("1 knowledge entry available"));

    // Should NOT include tool descriptions (중복 제거)
    assert!(!context.context_prompt.contains("searchKnowledge"));
    assert!(!context.context_prompt.contains("listKnowledge"));
    assert!(!context.context_prompt.contains("Available Operations"));
}
```

### 7.3. Content Store Server Tests

**파일**: `src-tauri/src/mcp/builtin/content_store/tests.rs`

**테스트 시나리오:**

1. `test_service_context_empty` - Content 없을 때
2. `test_service_context_with_content` - Content 목록 표시 확인
3. `test_service_context_preview_truncation` - 50자 미리보기 확인
4. `test_service_context_char_count` - 글자 수 표시 확인

```rust
#[tokio::test]
async fn test_service_context_with_content() {
    let session_manager = setup_test_session_manager().await;
    let server = ContentStoreServer::new(session_manager).await.unwrap();

    // Add content
    server.add_content(json!({
        "content": "This is a test content with more than fifty characters to test truncation",
        "metadata": {
            "title": "Test Document"
        }
    })).await.unwrap();

    let context = server.get_service_context(Some(&json!({
        "sessionId": "test-session"
    })));

    // Should show count
    assert!(context.context_prompt.contains("1 file stored"));

    // Should show title
    assert!(context.context_prompt.contains("Test Document"));

    // Should show truncated preview (50 chars)
    assert!(context.context_prompt.contains("This is a test content with more than fifty ch..."));

    // Should show character count
    assert!(context.context_prompt.contains("75 chars"));
}
```

---

## 8. 추가 분석 과제

### 8.1. System Prompt Rust Migration 검토

**현재 상황:**

- Frontend (TypeScript)에서 system prompt 구성
- `BuiltInToolProvider.buildToolPrompt()` → `ChatContext.buildSystemPrompt()`

**검토 필요 사항:**

- Rust Backend에서 system prompt 구성 시 성능 이점
- Frontend에서 dynamic prompt 구성 필요성 (Extension prompts)
- Migration 우선순위 (현재 작업 vs 별도 작업)

**제안:**

- 현재 작업: Built-in service contexts만 Rust로 이관
- 추후 작업: System prompt 전체 구성 로직 Rust 이관 검토

### 8.2. Database Query 성능 최적화

**현재 구현:**

- `block_on()` 사용으로 동기 context에서 async DB query 실행
- 각 service context 호출 시 별도 DB query

**개선 방향:**

- Service context를 한 번에 batch로 조회하는 방법 검토
- Cache 전략 검토 (context가 자주 변경되지 않는 경우)

### 8.3. Structured State 활용 방안

**현재:**

- `structured_state` 필드가 있지만 Frontend에서 제한적으로 사용

**확장 가능성:**

- UI components가 structured state를 활용한 시각화
- Planning panel, Knowledge browser 등에서 실시간 상태 표시

---

## 9. Clarification Questions

### Q1. System Prompt Migration Scope

**질문:** 현재 작업에서 system prompt 구성 로직을 Rust로 완전히 이관할 것인가, 아니면 service context만 개선하고 prompt 조립은 Frontend에 유지할 것인가?

**옵션:**

- **A:** Service context만 개선, prompt 조립은 Frontend 유지
  - 장점: 단계적 이관, Frontend Extension prompts 유연성
  - 단점: 불필요한 IPC round-trip, Multi-Agent 복잡도 증가
- **B:** System prompt 구성 전체를 Rust로 이관 (`AgentSessionManager.build_system_prompt()` 구현)
  - 장점: Zero round-trip, Session isolation, Multi-Agent ready, idea.md 목표 달성
  - 단점: Extension prompts는 별도 메커니즘 필요

**현재 제안:** B (System prompt 전체 Rust 이관) - **Multi-Agent 지원 및 성능 최적화**

**선택 이유:**

1. **Round-trip 제거**: Frontend에서 context 조합 시 Rust ↔ Frontend 왕복 필요
2. **Session isolation**: Multi-Agent 환경에서 각 session의 context가 자동 분리
3. **idea.md 목표**: "Frontend 상태와 독립적인 Agentic Workflow" 달성
4. **성능**: IPC overhead 제거, Backend에서 완전한 제어

> 답변: **Option B 적용**

---

### Q2. Performance vs Simplicity Trade-off

**질문:** `block_on()` 사용으로 인한 잠재적 성능 이슈를 어떻게 처리할 것인가?

**옵션:**

- **A:** 현재 구현 유지 (단순성 우선, context 조회 빈도 낮음)
- **B:** Cache 레이어 추가 (복잡도 증가, 성능 개선)
- **C:** Async trait로 변경 (`async fn get_service_context()`) - Breaking change

**영향 범위 분석 (Option C):**

**수정 대상 파일:** 12개

1. **Trait 정의** (1개): `src-tauri/src/mcp/builtin/mod.rs` - `BuiltinServer` trait
2. **구현체** (7개 Built-in servers):
   - `planning/mod.rs` - PlanningServer
   - `knowledge/mod.rs` - KnowledgeServer
   - `content_store/mod.rs` + `server.rs` - ContentStoreServer
   - `workspace/mod.rs` - WorkspaceServer
   - `playbook/mod.rs` - PlaybookServer
   - `bootstrap/mod.rs` - BootstrapServer
   - `assistant/mod.rs` - AssistantServer
3. **호출 부분**: `MCPServiceProxy.get_service_contexts()` - 이미 async이므로 `.await` 추가만

**변경 패턴 (반복적, 단순):**

```rust
// Before: Sync
fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
    let data = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            self.db.query().await
        })
    });
    // ...
}

// After: Async
async fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
    let data = self.db.query().await;  // ← block_on 제거, 자연스러운 await
    // ...
}
```

**복잡도 평가:**

- 수정할 파일: 12개 (많지만 패턴 동일)
- 패턴: `fn` → `async fn`, `block_on()` 제거, `.await` 추가
- 복잡도: **낮음** (반복적, 기계적 수정)
- 추가 작업량: **1-2시간** (패턴 적용 + 테스트)

**장점:**

- ✅ `block_on()` 성능 overhead 제거
- ✅ Async 생태계와 자연스러운 통합
- ✅ Multi-Agent 환경에서 더 나은 동시성
- ✅ 향후 확장성 개선 (async/await 표준 패턴)

**현재 제안:** **C (Async trait로 변경)** - 작업량 대비 장점이 명확, 반복적이고 단순

**선택 이유:**

1. **낮은 복잡도**: 12개 파일이지만 패턴이 동일하여 기계적 수정
2. **성능 개선**: `block_on()` overhead 제거
3. **Multi-Agent Ready**: 동시성 환경에서 자연스러운 async/await
4. **표준 패턴**: Rust async 생태계 best practice 준수

> 답변: **Option C 적용** (추가 작업량: 1-2시간)

---

### Q3. Content Store Storage Helper Methods

**질문:** Content Store의 `get_content_list()` 메서드를 `storage.rs`에 추가해야 하는데, in-memory와 SQLite 두 구현 모두에 추가할 것인가?

**옵션:**

- **A:** 두 구현 모두 추가 (일관성)
- **B:** In-memory만 추가 (현재 사용 중)
- **C:** Trait 메서드로 정의 후 구현

**현재 제안:** A (두 구현 모두 추가) - 향후 SQLite 전환 대비

> 답변: Option A 적용

---

## 10. Implementation Timeline

### Phase 0: Async Trait Migration (1-2 hours) **NEW**

- **목적**: `get_service_context()` async 변환으로 성능 최적화
- **작업**:
  - `BuiltinServer` trait 정의 변경 (`fn` → `async fn`)
  - 7개 Built-in servers 구현체 수정 (`block_on()` 제거 → `.await`)
  - `MCPServiceProxy.get_service_contexts()` 호출부 수정 (`.await` 추가)
- **복잡도**: 낮음 (반복적, 기계적 패턴)
- **타이밍**: Day 1 Morning (가장 먼저 진행)

### Phase 1: Planning Server (2-3 hours)

- Day 1 Morning: Database query 로직 구현 (async 활용)
- Day 1 Afternoon: Context formatting 로직 구현, 테스트 작성

### Phase 2: Knowledge Server (1-2 hours)

- Day 1 Late Afternoon: Knowledge count query 구현 (async 활용)
- Day 1 Evening: Context formatting (도구 설명 제거), 테스트 작성

### Phase 3: Content Store Server (1-2 hours)

- Day 2 Morning: Storage helper methods 추가
- Day 2 Morning: Context formatting (Scratchpad 스타일), 테스트 작성

### Phase 4: Backend System Prompt Integration (2-3 hours)

- Day 2 Afternoon: `AgentSessionManager.build_system_prompt()` 구현
- Day 2 Afternoon: `MCPServiceProxy.get_service_contexts()` 구현 (async 활용)
- Day 2 Afternoon: `request_llm_completion()` 수정 (systemPrompt 전달)

### Phase 5: Frontend Simplification (1 hour)

- Day 2 Late Afternoon: `handleLLMCompletionRequest()` 수정
- Day 2 Late Afternoon: Built-in context 수집 로직 제거 (더 이상 불필요)

### Phase 6: Integration Testing (1-2 hours)

- Day 2 Evening: E2E 테스트 - Multi-Agent 시나리오 포함
- Day 2 Evening: System Prompt 구성 검증 (Rust Backend에서 완성)
- Day 2 Evening: Async 성능 검증 (block_on overhead 제거)
- Day 2 Evening: Documentation update

**Total Estimated Time**: 2 days (Option B + Option C 추가 작업 포함)

**Phase 별 검증:**

- **Phase 0**: Async trait 변환 (성능 최적화 기반)
- Phase 1-3: Built-in service contexts 개선 (async 활용)
- **Phase 4**: Rust Backend에서 System Prompt 전체 구성
- **Phase 5**: Frontend 간소화 (context 수집 로직 제거)
- **Phase 6**: Multi-Agent + Async 성능 검증

---

## 11. Success Criteria

**Async Trait Migration (Phase 0):**

- ✅ `BuiltinServer` trait async 변환 완료
- ✅ 7개 Built-in servers `block_on()` 제거, `.await` 사용
- ✅ `MCPServiceProxy.get_service_contexts()` async 호출 완료
- ✅ 성능 측정: `block_on()` overhead 제거 확인

**Built-in Service Context 개선:**

- ✅ Planning Server: Goal, Todos (5개), Checked (3개), Scratchpad (5개) 표시
- ✅ Knowledge Server: Knowledge count, empty state 안내 (도구 설명 없음)
- ✅ Content Store Server: Content 목록 (5개), 제목/미리보기/글자 수 표시

**Backend System Prompt 통합:**

- ✅ `AgentSessionManager.build_system_prompt()` 구현 완료
- ✅ Rust Backend에서 완성된 systemPrompt 전달
- ✅ Frontend는 systemPrompt를 받아 바로 사용 (context 수집 로직 제거)

**Multi-Agent 지원:**

- ✅ 여러 session이 동시 실행 시 context가 올바르게 분리됨
- ✅ Session별로 독립적인 systemPrompt 생성
- ✅ IPC round-trip 제거로 성능 향상

**검증 항목:**

- ✅ All tests pass (Unit + Integration)
- ✅ LLM이 context를 활용하여 정확한 응답 생성 확인
- ✅ Multi-Agent 시나리오에서 context 분리 확인
- ✅ Frontend 코드 간소화 (불필요한 context 수집 로직 제거)

---

**End of Refactoring Plan**
