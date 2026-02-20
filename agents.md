# 🚀 LibrAgent Project Guidelines

## Project Overview

**LibrAgent: A High-Freedom AI Agent Platform - Infinitely Expandable with MCP!**

LibrAgent is a next-generation desktop AI agent platform that combines the lightness of Tauri with the intuitiveness of React. Users can automate all daily tasks by giving AI agents their own unique personalities and abilities.

## Key Architecture Patterns

**Agent V2 Architecture (Session-Isolated):**

- **Per-Session Tool Instances**: Each agent session gets isolated `MCPServiceProxy` with dedicated builtin server instances
- **Session-Specific MCP Managers**: Separate `HttpSessionManager` and `SessionMCPManager` per session
- **Context Registry System**: Dynamic context providers (time/location, skills) inject state into system prompts
- **Rust-Orchestrated Workflows**: Think-Act-Observe loop managed entirely in Rust backend (`AgentSessionManager`)

**MCP Integration Architecture:**

- **External MCP Servers**: Stdio/HTTP protocol via `rmcp` library, managed by session-isolated managers
- **Builtin MCP Servers**: Native Rust implementations via `BuiltinMCPServer` trait
  - Planning, Knowledge, Browser, Workspace, Content Store, etc.
  - Session-isolated instances with dedicated state
- **Unified Tool Discovery**: `MCPServiceProxy` routes calls to builtin or external servers transparently

**Feature-Based Organization:**

- Each feature in `src/features/` contains components, hooks, and README documentation
- Compound component patterns (e.g., `Chat.Header`, `Chat.Messages`, `Chat.Input`)
- React Context providers for state sharing (`ChatProvider`, `AgentSessionProvider`, `AgentChatProvider`)

**Service Layer Pattern:**

- `src/lib/backend/` contains Tauri command wrappers with centralized `safeInvoke()` utility
- Centralized logging via `getLogger('ComponentName')` instead of console methods
- All API communication through typed service modules with error handling

**Key Features:**

- **AI Agent Management**: Session-isolated agents with independent tool state and context
- **LLM Provider Support**: 8 providers, 50+ models including reasoning models (o3, DeepSeek R1)
- **Built-in Tool Ecosystem**: Planning, Knowledge, Browser, Workspace, Code Execution
- **MCP Integration**: Session-isolated stdio/HTTP protocol with security validation

## Technology Stack

**Core Framework:**

- PNPM (Package Manager)
- Tauri 2.x (Latest cross-platform desktop framework)
- React 18.3 (Modern UI with concurrent features)
- TypeScript 5.6 (Advanced type safety)
- `rmcp` 0.8.x (Rust-based Model Context Protocol client; see `src-tauri/Cargo.toml`)

**Frontend Technologies:**

- Tailwind CSS 4.x (Latest utility-first styling)
- Radix UI (Accessible component primitives)
- SeaORM (Database ORM for SQLite, used via Rust backend)
- React Context (State management with providers)
- Vite 6.x (Fast development and build tool)

**Backend Technologies:**

- Rust (High-performance native operations)
- Tokio 1.49+ (Async runtime for concurrent operations)
- SeaORM (Type-safe database ORM for SQLite)
- rmcp 0.8.1+ (MCP client library with stdio/HTTP transport)
- reqwest 0.12+ (HTTP client for MCP servers and browser automation)

## Development Scripts & Workflow

LibrAgent provides several useful scripts for development and code quality:

- `pnpm dev` – Start the Vite development server
- `pnpm tauri dev` – Start the Tauri desktop app with hot reload
- `pnpm build` – Build the frontend for production
- `pnpm lint` – Run ESLint checks for code quality
- `pnpm format` – Format code using Prettier
- `pnpm rust:fmt` – Check Rust code formatting
- `pnpm rust:clippy` – Run Rust linter
- `pnpm dead-code` – Find unused code with unimported
- `pnpm refactor:validate` – **Complete validation pipeline:**  
  Runs lint, format, Rust validation, build, and dead-code checks.  
  **Always run this after any development or refactoring work to ensure code quality and build integrity.**

**Workflow Recommendation:**  
After making any code changes, always run:

```sh
pnpm refactor:validate
```

This ensures:

- Code consistency and formatting
- No TypeScript or Rust compilation errors
- No unused code
- The application remains buildable

> **Note:** All contributors must follow this workflow before submitting PRs or merging changes.

## File Structure

```bash
libr-agent/
├── src/                        # React Frontend
│   ├── app/                    # App entry, root layout, global providers
│   ├── assets/                 # Static assets (images, svgs, etc.)
│   ├── components/             # Shared, generic UI components (reusable)
│   ├── features/               # Feature-specific components, logic, and hooks
│   ├── config/                 # Static config files
│   ├── context/                # React context providers
│   ├── hooks/                  # Generic, reusable hooks
│   ├── lib/                    # Service layer, business logic, data, API
│   ├── models/                 # TypeScript types and interfaces
│   ├── styles/                 # Global or shared CSS
│   ├── README.md
│   └── vite-env.d.ts
├── src-tauri/                 # Rust Backend
│   ├── src/
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/                      # Documentation
├── dist/                      # Build artifacts
├── package.json
├── tailwind.config.js
└── vite.config.ts
```

## Quick Start

1. Install Rust ([rustup.rs](https://rustup.rs/)), Node.js (v18+), and pnpm (`npm install -g pnpm`).
2. Run `pnpm install` to install dependencies.
3. Start development: `pnpm tauri dev`
4. Build for production: `pnpm tauri build`
5. API keys are managed in-app via the settings modal (not in .env files).

## CI / Release

- GitHub Actions are used for CI and releases. See `.github/workflows/ci.yml` for tests, linting and Rust checks, and `.github/workflows/release.yml` for multi-platform packaging.
- Node.js version in CI is pinned to 18; use a compatible Node LTS for local development.

## Coding Style

### General

- Use 2 spaces for indentation across all files.
- Use descriptive variable names in both Rust and TypeScript.
- Follow consistent naming conventions for files and directories.
- **All comments must be written in English.** Use clear, descriptive English comments for all code documentation, inline comments, and docstrings.

### Rust Backend (`src-tauri/`)

- Follow the [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/) and use `rustfmt`.
- Use snake_case for functions, variables, and module names.
- Use PascalCase for types, structs, and enums.
- Add comprehensive documentation comments (`///`) for public APIs.
- Handle errors explicitly using `Result<T, E>` types.

#### Rust Method/Function Declaration and Calling Guide

##### Method vs. Associated Function

- **Method**: Takes `self` (or `&self`, `&mut self`) as the first parameter in an `impl` block.  
  → Called through instance: `self.method_name(...)`
- **Associated Function**: No `self` parameter.  
  → Called through type name: `TypeName::function_name(...)`

##### Example

```rust
impl MyStruct {
    // Method: requires self
    fn do_something(&self, arg: i32) { ... }

    // Associated function: no self
    fn helper(arg: i32) { ... }
}

// Calling methods
let obj = MyStruct::new();
obj.do_something(42);           // ✅ Method call
MyStruct::helper(42);           // ✅ Associated function call
```

##### Error Prevention Checklist

- If using `self` in a function, declare `self` as the first parameter.
- Associated functions cannot use `self`.
- Call methods through instances, associated functions through type names.

##### Common Mistakes and Fixes

###### ❌ Wrong Example

```rust
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    self.copy_dir_contents(&src_path, &dst_path)?; // Error!
}
```

###### ✅ Correct Examples

- **If declared as associated function, call through type name:**

```rust
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    SessionManager::copy_dir_contents(&src_path, &dst_path)?;
}
```

- **If using as method, add self parameter:**

```rust
fn copy_dir_contents(&self, src: &Path, dst: &Path) -> Result<(), String> {
    self.copy_dir_contents(&src_path, &dst_path)?;
}
```

##### IDE/Compiler Usage

- Rust compiler clearly indicates these mistakes, so read error messages carefully and check function declarations/calls.
- Use "Go to Definition" in IDEs like VS Code or IntelliJ Rust to easily check if a function is a method or associated function.

**Summary:** Always remember that the presence/absence of `self` parameter determines calling method. When compilation errors occur, recheck function declaration and calling patterns.

### Frontend (`src/`)

- Follow Prettier and ESLint configurations for TypeScript/React code.
- Use camelCase for variables and functions.
- Use PascalCase for React components and TypeScript interfaces.
- Prefer functional components with hooks over class components.
- Use TypeScript interfaces for type definitions.
- **Principle: Never use `any` in TypeScript.** The lint configuration is extremely strict; always use precise types and interfaces.
  - **Data from Backend/External Sources:** Never type incoming data as `any`. Define a proper interface (e.g., `RustMessage`) or use `unknown` with type guards/validation.
  - Do not add ESLint-disable comments that permanently or locally disable rules (for example: `// eslint-disable-next-line @typescript-eslint/no-explicit-any`). Instead, refactor the code to avoid `any` or use `unknown`/proper typing and document the rationale in a code comment and PR description when an exception is truly necessary.

### Type Safety Principles & Anti-Patterns

#### 🚫 CRITICAL: Prohibited Patterns

1. **Blind Type Assertions** - Never use `as` or `<Type>` casting without runtime validation
2. **Unsafe unknown handling** - When using `unknown`, ALWAYS validate before casting
3. **Blind any conversion** - Never cast `any` directly to a specific type without validation
4. **JSON.parse without validation** - Never cast parsed JSON without runtime checks
5. **Backend response assumptions** - Never assume backend data structure without validation
6. **Generic type casts** - Never use `as T` in generic functions without validation

#### ❌ Bad (Anti-Patterns to Avoid)

```typescript
// ❌ Direct casting without validation
const data = response as MyInterface;
const result = <UserData>jsonData;

// ❌ Unknown to type without validation
function process(input: unknown) {
  const user = input as User; // Unsafe!
  return user.name;
}

// ❌ Any to specific type
function handle(data: any) {
  const config: Config = data; // Unsafe!
}

// ❌ JSON.parse without validation
const config = JSON.parse(jsonString) as AppConfig;

// ❌ Backend response without validation
const tools = (await getTools(sessionId)) as MCPTool[];

// ❌ Generic cast without validation
function getValue<T>(key: string, defaultValue: T): T {
  const value = storage.get(key);
  return (value ?? defaultValue) as T; // Unsafe!
}

// ❌ Double casting to bypass errors
const sdk = this.client as unknown as SpecificSDK;
```

#### ✅ Good (Type-Safe Patterns)

```typescript
// ✅ Type guard validation
interface User {
  name: string;
  age: number;
}

function isUser(obj: unknown): obj is User {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    'name' in obj &&
    typeof obj.name === 'string' &&
    'age' in obj &&
    typeof obj.age === 'number'
  );
}

function process(input: unknown) {
  if (isUser(input)) {
    return input.name; // Type-safe!
  }
  throw new Error('Invalid user data');
}

// ✅ Use Zod for complex validation
import { z } from 'zod';

const UserSchema = z.object({
  name: z.string(),
  age: z.number(),
});

function processWithZod(input: unknown) {
  const user = UserSchema.parse(input); // Runtime validation + type inference
  return user.name;
}

// ✅ JSON.parse with schema validation
const ConfigSchema = z.object({
  theme: z.enum(['light', 'dark']),
  fontSize: z.number().positive(),
});

const parsed = JSON.parse(jsonString);
const config = ConfigSchema.parse(parsed); // Safe!

// ✅ Backend response validation
async function getValidatedTools(sessionId: string): Promise<MCPTool[]> {
  const response = await getTools(sessionId);

  if (!Array.isArray(response)) {
    throw new Error('Expected array of tools');
  }

  return response.filter((tool): tool is MCPTool => {
    return isMCPTool(tool); // Use type guard
  });
}

// ✅ Generic with validator parameter
function getValue<T>(
  key: string,
  defaultValue: T,
  validator: (val: unknown) => val is T,
): T {
  const value = storage.get(key);
  if (value !== undefined && validator(value)) {
    return value;
  }
  return defaultValue;
}

// ✅ Narrow types progressively
function handleData(data: unknown) {
  if (typeof data !== 'object' || data === null) {
    throw new Error('Expected object');
  }
  if (!('type' in data) || typeof data.type !== 'string') {
    throw new Error('Missing type field');
  }
  // Now data is narrowed to { type: string } & object
  return data.type;
}

// ✅ SDK with proper interface definition
interface OpenAIModelsAPI {
  models: { list: () => Promise<{ data: unknown[] }> };
}

// Define explicit interface and document why cast is needed
const openaiModels = this.openai as OpenAIModelsAPI;
```

#### 📋 Type Safety Checklist

Before merging any PR, verify:

- [ ] No `as any` casts (use `grep "as any" src/`)
- [ ] No ESLint disable comments for type rules
- [ ] All `JSON.parse` operations have schema validation
- [ ] All backend responses validated with type guards
- [ ] All `unknown` types narrowed before use
- [ ] Generic functions include validator parameters
- [ ] Type assertions documented with rationale

#### 🔧 Refactoring Guidelines

When encountering type safety issues:

1. **Identify the root cause** - Why is the type unknown or any?
2. **Add runtime validation** - Use type guards or Zod schemas
3. **Update types at source** - Fix backend type definitions if possible
4. **Document exceptions** - If cast is truly necessary, document why
5. **Add tests** - Ensure validation catches invalid data

See [Type Safety Refactoring Plan](../../docs/refactoring/type-safety-refactoring-plan.md) for detailed migration guide.

#### 🎯 Acceptable `unknown` Usage

These patterns are acceptable:

- **Logger variadic arguments**: `...args: unknown[]` for flexible logging
- **Error catch blocks**: `catch (error: unknown)` per TypeScript best practice
- **Test environment mocking**: `(global as unknown as MockType)` for test setup
- **Protocol definitions**: JSON-RPC payloads where structure varies
- **Abstract base classes**: When subclasses define concrete types

**Key difference:** These use `unknown` as input that gets validated, not as output assumed to be valid.

- **Use the centralized logger instead of console.log**: Import `getLogger` from `@/lib/logger` and use context-specific logging (e.g., `const logger = getLogger('ComponentName')`) instead of `console.*` methods for better debugging and log management.
- **Never use inline import() types in interfaces.** Always use proper import statements at the top of the file instead of `import('../path').Type`. This improves readability, maintainability, and IDE support.

#### ❌ Bad (Inline Import Types)

```typescript
interface Config {
  tools?: import('@/lib/mcp').MCPTool[];
  messages: import('@/models/chat').Message[];
}
```

#### ✅ Good (Proper Import Statements)

```typescript
import type { MCPTool } from '@/lib/mcp';
import type { Message } from '@/models/chat';

interface Config {
  tools?: MCPTool[];
  messages: Message[];
}
```

### CSS/Styling

- Use `shadcn/ui` components for building accessible, consistent, and customizable UI elements. Prefer shadcn/ui for new UI components unless a custom solution is required.
- **Tailwind CSS Class Usage Guidelines:**
  - Avoid using arbitrary class names (e.g., `content-text`) that are not Tailwind utility classes, as they may be removed by PurgeCSS during build.
  - Use Tailwind utility classes instead: `className="text-sm text-gray-700 leading-relaxed"`
  - If custom classes are needed, define them in CSS files or add to Tailwind's safelist in `tailwind.config.js`
  - For dynamic or conditional styling, use Tailwind's arbitrary value syntax: `className="[custom-value]"`

## Architecture

- `shadcn/ui`: Component library for building accessible and customizable UI components

### Logging System

The project uses a centralized logging system located at `src/lib/logger.ts` that integrates with Tauri's native logging plugin. This provides better debugging capabilities and structured logging across the application.

#### Usage Guidelines

- **Always use the centralized logger instead of `console.*` methods**
- Import and use context-specific loggers:

  ```typescript
  import { getLogger } from '@/lib/logger';
  const logger = getLogger('ComponentName');

  // Use appropriate log levels
  logger.debug('Debug information', data);
  logger.info('General information', data);
  logger.warn('Warning message', data);
  logger.error('Error occurred', error);
  ```

- **Context naming**: Use descriptive context names that match the component/module name
- **Log levels**: Use appropriate log levels (debug, info, warn, error) based on the importance and type of information
- **Error logging**: When logging errors, pass the Error object as the last parameter for proper error handling

#### Benefits

- Centralized log management through Tauri's native logging system
- Better debugging capabilities in development and production
- Structured logging with context information
- Integration with Tauri's log viewing tools
- Consistent logging format across the application

### Layer Responsibilities

**Frontend Layer (`src/`):**

- Use `shadcn/ui` components as the primary building blocks for UI
- Manages local UI state via React Context providers
- Communicates with Tauri backend through `src/lib/backend/` service modules
- React components consume state from context providers (no prop drilling)

**Service Layer (`src/lib/backend/`):**

- Typed wrappers around Tauri commands using `safeInvoke()`
- Centralized error handling and logging
- Type-safe API contracts between frontend and backend
- Organized by domain (assistants, browser, mcp-server, workspace, etc.)

**Backend Layer (`src-tauri/src/`):**

- **Agent Orchestration** (`agent/`): Session lifecycle, LLM interaction, tool execution
- **MCP Integration** (`mcp/`): Session-isolated server managers and builtin tool implementations
- **Database Layer** (`repositories/`, `entities/`): SeaORM models and data access
- **Service Layer** (`services/`): Browser automation, workspace management
- **Commands** (`commands/`): Tauri command handlers exposing backend functionality

### Data Flow

**Legacy Chat V1 (React-Orchestrated):**

1. User sends message via `ChatInput` component
2. `ChatProvider` context invokes LLM service
3. LLM response triggers tool calls
4. `BuiltInToolProvider` routes to appropriate backend
5. Results returned to `MessageRenderer` for display

**Agent V2 (Rust-Orchestrated):**

1. User sends task via `AgentChatProvider`
2. Rust `AgentSessionManager` starts Think-Act-Observe loop
3. Backend builds system prompt with context providers + service contexts
4. LLM generates tool calls, backend executes via `MCPServiceProxy`
5. Backend emits events (`agent:event`) to update UI reactively
6. Frontend (`AgentSessionContext`) consumes events and updates message list

**Key Difference:** In Agent V2, frontend is purely reactive. All orchestration logic (loop control, tool execution, state management) resides in Rust.

### Service Context System

**⚠️ CRITICAL: Understanding Service Context Data Flow**

The `ServiceContext` struct has two fields, but **only one is actually used by AI Agents**:

```rust
pub struct ServiceContext {
    pub context_prompt: String,        // ✅ USED: AI sees this as text in system prompt
    pub structured_state: Option<T>,   // ❌ UNUSED: Currently ignored, NOT sent to AI
}
```

**How it works:**

1. **Backend (Rust)** - Builtin servers implement `get_service_context()`:

   ```rust
   // Example: browser/mod.rs
   async fn get_service_context(&self) -> ServiceContext {
       ServiceContext {
           context_prompt: "## Browser\n\nSession abc123: https://example.com",
           structured_state: Some(json!({
               "session_id": "full-uuid-here",  // NOT SEEN BY AI
               "url": "https://example.com"      // NOT SEEN BY AI
           })),
       }
   }
   ```

2. **Backend (Rust)** - System prompt builder extracts **ONLY** `context_prompt`:

   ```rust
   // agent/llm.rs - build_system_prompt()
   for (_tool_id, service_context) in contexts {
       parts.push(service_context.context_prompt);  // ✅ Text only
       // structured_state is completely ignored
   }
   ```

3. **Frontend** - LLM API receives the text-only system prompt:
   ```typescript
   // openai.ts - convertToOpenAIMessages()
   openaiMessages.push({
     role: 'system',
     content: systemPrompt, // ✅ Contains context_prompt text
   });
   ```

**What AI Actually Sees:**

```
## Browser

Session abc123: https://example.com (Example Domain)

## Planning

Current task: ...
```

**What AI DOES NOT See:**

- Any data in `structured_state` (JSON objects, full IDs, metadata)
- The JSON is never serialized into the system prompt
- The JSON is never sent to the LLM API

**Design Implications:**

- ✅ **Use `context_prompt` for**: Human-readable status, short IDs, current state descriptions
- ❌ **DON'T rely on `structured_state` for**: AI decision-making, tool parameter hints, critical IDs
- ⚠️ **If AI needs data**: Put it in `context_prompt` as plain text, not in `structured_state`

**Common Mistake:**

```rust
// ❌ WRONG: AI won't see the full session_id
ServiceContext {
    context_prompt: "Session abc123: active",  // AI sees short ID
    structured_state: Some(json!({
        "session_id": "abc123-full-uuid"  // AI NEVER sees this
    })),
}

// ✅ CORRECT: Include full ID in text if AI needs it
ServiceContext {
    context_prompt: "Session abc123-full-uuid: active",  // AI sees full ID
    structured_state: None,  // Or keep for potential UI use
}
```

**Remember:** `context_prompt` is the ONLY field that reaches the AI's system prompt. Everything else is discarded during prompt construction.

### Agent V2 Architecture

**Overview:**

Agent V2 is a "Dual-Track" architecture supporting autonomous, multi-turn workflows while maintaining legacy chat compatibility:

- **Track 1 (Legacy Chat V1)**: Standard request/response chat orchestrated by React (`ChatContext`)
- **Track 2 (Agent V2)**: Autonomous workflows orchestrated entirely by Rust backend (`AgentSessionManager`)

**Rust-Based Orchestration:**

```
AgentSessionManager (Rust)
  ├── Session Lifecycle (create, recover, cleanup)
  ├── Workflow Control (start, stop, pause, resume)
  ├── LLM Interaction (Think phase)
  └── Tool Execution (Act phase) → MCPServiceProxy
```

**Session Isolation:**

- **One MCPServiceProxy per Agent Session**: Each session gets isolated tool instances
- **Stateful Tools**: Planning todos, Knowledge items, Browser sessions scoped to session ID
- **No Global State**: Complete isolation prevents cross-session interference
- **Session-Specific Workspace**: Each agent operates in isolated directory

**Context Registry System:**

Dynamic context providers inject read-only information into system prompts:

```rust
// System Prompt Structure (Agent V2)
1. Agent Identity (system_prompt from config)
2. Session Context (user-defined session name)
3. Context Providers (ContextRegistry)
   ├── TimeLocationContextProvider (current time/location)
   └── SkillsContextProvider (available skills/documentation)
4. Service Contexts (from builtin tools)
   ├── Planning (current goal, todos)
   ├── Browser (active sessions, URLs)
   └── Workspace (file tree, recent changes)
```

**Event-Driven UI Updates:**

Frontend is purely reactive, consuming events from backend:

```typescript
// Frontend (AgentSessionContext)
useEffect(() => {
  const unlisten = listen('agent:event', (event) => {
    // Update session state based on backend events
    if (event.status) setWorkflowStatus(event.status);
    if (event.phase) setWorkflowPhase(event.phase);
  });
}, [sessionId]);
```

**Tool Execution Flow:**

```
1. LLM generates tool call
2. AgentSessionManager validates and routes to MCPServiceProxy
3. MCPServiceProxy checks if builtin or external:
   - Builtin: Direct method call on session-specific server instance
   - External: Route to SessionMCPManager/HttpSessionManager
4. Tool result converted to MCPContent and added to conversation
5. Loop continues until completion or error
```

**Migration Notes:**

- Legacy Chat V1 uses global MCP manager (deprecated)
- New Agent V2 sessions automatically use session isolation
- Builtin servers implement both APIs for backward compatibility
- Frontend gradually transitioning from `ChatContext` to `AgentSessionContext`

### MCP Tool Response Design

**🚨 CRITICAL: structured_content is ONLY for UI Rendering**

When implementing MCP tools, understand that AI agents and UI components see different parts of `MCPResult`:

**Data Flow Architecture (LibrAgent-Specific):**

```rust
pub struct MCPResult {
    content: Vec<MCPContent>,           // → Standard MCP: AI agents SEE this
    structured_content: Option<Value>,  // → LibrAgent extension: UI components only (agents DON'T)
    is_error: Option<bool>,             // → Standard MCP
}
```

**Important:** `structured_content` is a **non-standard LibrAgent internal extension**. The standard MCP protocol only defines `content` (array of MCPContent items) and `isError` (boolean). We added `structured_content` for LibrAgent's UI components to render rich data without parsing text. External MCP servers don't use this field.

**What Goes Where:**

| Information Type | Text Content (agents see) | structured_content (UI only) |
| ---------------- | ------------------------- | ---------------------------- |
| Process IDs      | ✅ **MUST include**       | ✅ Optional for UI parsing   |
| File paths       | ✅ **MUST include**       | ✅ Optional for UI parsing   |
| Status messages  | ✅ **MUST include**       | ✅ Optional for UI parsing   |
| Error details    | ✅ **MUST include**       | ✅ Optional for UI parsing   |
| Metadata         | ❌ Not critical           | ✅ For UI components         |
| Raw data arrays  | ❌ Summarize in text      | ✅ For UI rendering          |

**Anti-Patterns to Avoid:**

```rust
// ❌ WRONG: Critical ID only in structured_content
let result = MCPResult {
    content: vec![text("Background process started successfully")],
    structured_content: Some(json!({
        "process_id": "7573a69b",  // Agents can't see this!
        "status": "running"
    })),
    is_error: Some(false),
};

// ✅ CORRECT: ID visible in text output
let result = MCPResult {
    content: vec![text("Background process started (ID: 7573a69b)\n\nUse pollProcess(\"7573a69b\") to check status")],
    structured_content: Some(json!({
        "process_id": "7573a69b",  // Redundant but useful for UI
        "status": "running"
    })),
    is_error: Some(false),
};
```

**Listing Multiple Items:**

```rust
// ❌ WRONG: IDs buried in JSON
let hint = SuccessHint::new(
    "Found 3 processes (1 running, 2 finished)",
    vec!["Use pollProcess to check status"],
);

// ✅ CORRECT: IDs visible for copy-paste
let process_list = processes.iter()
    .map(|p| format!("• {} [{}]: {}", p.id, p.status, p.command))
    .collect::<Vec<_>>()
    .join("\n");

let hint = SuccessHint::new(
    format!("Found 3 processes:\n\n{}", process_list),
    vec!["Use pollProcess(processId) to check status"],
);
```

**State Information:**

```rust
// ❌ WRONG: Implicit state, only in JSON
let output = format!("Command executed\n{}", stdout);
let data = json!({"execution_type": "persistent", "cwd": "/project"});

// ✅ CORRECT: Explicit state in text
let output = format!(
    "Command executed\n\n{}\n\nPersistent shell state (maintained for next call):\n  Working directory: {}\n  Exit code: {}",
    stdout, cwd, exit_code
);
let data = json!({"execution_type": "persistent", "cwd": "/project"});
```

**Testing Your Tool Responses:**

1. **Text-Only Test**: Read only the `content` field - can an agent understand what happened?
2. **ID Extraction**: Can an agent copy process IDs, file paths, session IDs from the text?
3. **Follow-up Actions**: Does the text contain enough info for the next tool call?
4. **State Clarity**: Is execution context (persistent vs isolated) clear from text alone?

**Remember:**

- Agents ONLY see text content - design for text-first readability
- structured_content is purely for UI components and external tooling
- If an agent needs to use a value in a follow-up call, it MUST be in text
- Test by reading only the text field - pretend JSON doesn't exist

## Dependencies

### Core Framework

- `@tauri-apps/api`: Version 2.x - Enhanced frontend-backend communication
- `@tauri-apps/cli`: Version 2.x - Latest development and build tools
- `tauri`: Version 2.x - Advanced Rust backend framework with improved security

### Frontend Dependencies

- `react`: Version 18.x - UI library
- `react-dom`: Version 18.x - React DOM renderer
- `typescript`: Version 5.x - Type safety
- `vite`: Version 6.x - Build tool and dev server
- `tailwindcss`: Version 4.x - Utility-first CSS framework

### Backend Dependencies (Rust)

- `tauri`: Main framework for desktop app development
- `serde`: JSON serialization/deserialization
- `tokio`: Async runtime for concurrent operations
- `rmcp`: Model Context Protocol implementation

### Development Dependencies

- `@vitejs/plugin-react`: React support for Vite
- `autoprefixer`: CSS vendor prefixing
- `postcss`: CSS processing
- `eslint`: JavaScript/TypeScript linting
- `prettier`: Code formatting

## File Organization

### Component Structure

```typescript
// src/components/ComponentName.tsx
interface ComponentNameProps {
  // Type definitions
}

export default function ComponentName({ props }: ComponentNameProps) {
  // Component implementation
}
```

### Service Layer Structure

```typescript
// src/lib/backend/module-name.ts
import { safeInvoke } from './core';

export async function someBackendOperation(param: Type): Promise<Result> {
  return safeInvoke<Result>('rust_command_name', { param });
}
```

### Tauri Command Structure

```rust
// src-tauri/src/commands/module_name.rs
#[tauri::command]
pub async fn command_name(param: Type) -> Result<ReturnType, String> {
    // Implementation
}
```

## Development Workflow

### Environment Setup

1. Install Rust via rustup.rs
2. Install Node.js (v18+) and pnpm
3. Copy `.env.example` to `.env` and configure API keys
4. Run `pnpm install` for dependencies

### Development Commands

- `pnpm tauri dev` - Start development server
- `pnpm tauri build` - Create production build
- `pnpm lint` - Run ESLint checks
- `pnpm format` - Format code with Prettier
- `cargo fmt` - Format Rust code
- `cargo clippy` - Rust linting

### Testing Guidelines

- Write unit tests for utility functions
- Test Tauri commands with mock data
- Verify cross-platform compatibility
- Test MCP server integration scenarios

### Refactoring Guidelines

**Before completing any refactoring work, always run the following commands to ensure code quality and build integrity:**

1. **Code Quality Check**: `pnpm lint` - Verify ESLint rules compliance
2. **Code Formatting**: `pnpm format` - Apply Prettier formatting standards
3. **Build Verification**: `pnpm build` - Ensure the application builds without errors

These steps must be completed successfully before considering any refactoring task complete. This ensures:

- Code consistency across the project
- No TypeScript compilation errors
- Proper formatting standards are maintained
- The application remains buildable after changes

#### File Editing Best Practices: Micro-Edits vs. Wholesale Replacement

**⚠️ CRITICAL: Use micro-edits for large files to avoid token limit issues**

When editing large files (>500 lines) or making multiple updates to documentation/code:

**❌ AVOID: Wholesale file replacement**

- Regenerating entire files wastes tokens
- High risk of hitting output token limits (causing operation failure)
- Loses context and makes review difficult
- Can introduce unintended changes

**✅ PREFER: Targeted micro-edits**

- Use `replace_string_in_file` tool for each logical change
- Include 3-5 lines of context before/after the change
- Make changes incrementally, one section at a time
- Each edit is atomic and reviewable

**Example Scenario: Updating a 700-line migration plan**

```
❌ WRONG APPROACH:
- Try to regenerate entire 700-line document
- Hit token limit at line 400
- Operation fails, nothing happens
- Have to retry multiple times

✅ CORRECT APPROACH:
- Edit #1: Update Phase 1 schema (lines 100-150)
- Edit #2: Update Phase 2 tool responses (lines 300-400)
- Edit #3: Add Phase 3 validation (lines 500-580)
- Edit #4: Update frontend section (lines 620-680)
- Each edit succeeds independently
```

**When to use micro-edits:**

- Files > 500 lines
- Multiple unrelated changes
- Complex refactoring with review checkpoints
- Documentation updates with many sections
- When previous attempts hit token limits

**When wholesale replacement is acceptable:**

- New files being created (< 200 lines)
- Small files (< 100 lines) with pervasive changes
- Generated code (configs, types) where consistency matters

**Token Limit Indicators:**

- Response suddenly stops mid-generation
- Tool call succeeds but file unchanged
- "Token limit exceeded" errors
- Response cuts off during code block

**Recovery Strategy:**
If you hit token limits:

1. Identify what was successfully changed (check file state)
2. Break remaining changes into 3-5 smaller edits
3. Use specific line number ranges for targeting
4. Complete edits in priority order (P0 → P1 → P2)

### Core Design Principles

When refactoring or implementing new features, adhere to these fundamental software design principles:

1. **DRY (Don't Repeat Yourself)**
   - Eliminate code duplication through abstraction
   - Create reusable utilities and shared functions
   - Example: `command_helper.rs` consolidates Windows command wrapping logic

2. **SRP (Single Responsibility Principle)**
   - Each module/function should have one clear purpose
   - Separate concerns: UI logic, business logic, data access
   - Example: Command preparation logic separated from MCP server lifecycle management

3. **OCP (Open/Closed Principle)**
   - Code should be open for extension, closed for modification
   - Use patterns (strategy, factory) to add new features without changing existing code
   - Example: Pattern-based command detection allows adding new tools without modifying core logic

4. **ISP (Interface Segregation Principle)**
   - Keep interfaces simple and focused
   - Clients shouldn't depend on methods they don't use
   - Example: `prepare_command()` provides single, clean API for command preparation

5. **DIP (Dependency Inversion Principle)**
   - High-level modules should not depend on low-level modules; both should depend on abstractions
   - Depend on interfaces/traits, not concrete implementations
   - Example: MCP server managers depend on abstract command preparation, not platform-specific details

**Application in LibrAgent:**

- Extract common patterns into `src/lib/` or `src-tauri/src/utils/`
- Use Rust traits and TypeScript interfaces for abstraction
- Test utilities independently from business logic
- Document design decisions in `docs/refactoring/`

### Critical Development Patterns

**MCP Communication:**

- Always use `safeInvoke()` from `src/lib/backend/core.ts` for Tauri command calls
- MCP servers are session-isolated via `MCPServiceProxyManager` in Rust backend
- Builtin servers implement `BuiltinMCPServer` trait with session-specific state
- External servers managed by `SessionMCPManager` and `HttpSessionManager` per session

**Component Architecture:**

- Feature components follow compound patterns: `Chat.Header`, `Chat.Messages`, `Chat.Input`
- Each feature directory contains `components/`, `hooks/`, and `README.md`
- Use React Context for cross-component state sharing, not prop drilling
- Agent V2 uses `AgentSessionContext` + `AgentChatContext` for reactive state management

**Error Handling:**

- Backend commands return `Result<T, String>` in Rust
- Frontend wraps all Tauri calls via `safeInvoke()` with centralized error logging
- Use structured error objects with `MCPError` type for protocol errors
- Builtin tools return `Result<MCPResult, String>` for consistent error handling

**Session Isolation:**

- Each agent session has isolated `MCPServiceProxy` with dedicated builtin server instances
- No global state sharing between sessions
- Tool state (Planning todos, Knowledge items, etc.) scoped to session ID
- Workspace and Content Store use session-specific directories

**Development Commands:**

- `pnpm tauri dev` - Development with hot reload (port 1420)
- `pnpm tauri build` - Production build for distribution
- `pnpm dead-code` - Find unused code with unimported tool
- `pnpm refactor:validate` - Complete validation pipeline (lint, format, build, test)

**⚠️ CRITICAL: Content Security Policy (CSP) Warning:**

- **DO NOT add CSP configuration to `tauri.conf.json`** for desktop applications
- CSP is designed for web browsers, not desktop environments
- Tauri desktop apps require unrestricted access for native operations
- Adding CSP will cause blank white screens in release builds
- Dev mode has relaxed CSP enforcement, masking production issues
- Industry-standard practice: No CSP in Tauri desktop apps
- Use Tauri's native security features (allowlist, capability system) instead

## Security Considerations

### Tauri Security

- Use allowlist configuration to restrict API access
- Validate all input from frontend to backend
- Sanitize data before MCP server communication
- Handle sensitive data (API keys) securely

### API Key Management

- Store API keys in environment variables
- Never commit API keys to version control
- Use secure storage for production deployments
- Implement key rotation strategies

## Performance Guidelines

### Frontend Optimization

- Use React.memo for expensive components
- Implement proper dependency arrays in useEffect
- Lazy load components when appropriate
- Minimize database round-trips from the UI

### Backend Optimization

- Use async/await for non-blocking operations
- Implement proper error handling to prevent crashes
- Cache frequently accessed data
- Optimize MCP server communication protocols

## Documentation Standards

### Code Documentation

- Document all public APIs with clear examples
- Include type information in TypeScript interfaces
- Add inline comments for complex business logic
- Maintain up-to-date README files

### Architecture Documentation

- Document component relationships and data flow
- Maintain API documentation for Tauri commands
- Document MCP integration patterns
- Keep deployment guides current

## References

- [Project README](../README.md) - Overview, features, and quick start
- [Documentation Index](../docs/README.md) - Complete documentation structure
- [API Reference](../docs/api/tauri-commands.md) - Tauri command reference
- [Built-in Tools Guide](../docs/guides/builtin_tool_bp.md) - Tool design standards
- [UI Resource Implementation](../docs/guides/ui-resource-implementation.md) - Interactive HTML interfaces
- [External MCP Integration](../docs/architecture/external-mcp-integration.md) - Session isolation architecture
- [Workspace Tool Critique](../docs/analysis/workspace-tool-critique.md) - Implementation patterns
- [Type Safety Refactoring](../docs/refactoring/) - Migration guides and plans
