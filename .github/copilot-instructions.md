# 🚀 LibrAgent Project Guidelines

## Project Overview

**LibrAgent: A High-Freedom AI Agent Platform - Infinitely Expandable with MCP!**

LibrAgent is a next-generation desktop AI agent platform that combines the lightness of Tauri with the intuitiveness of React. Users can automate all daily tasks by giving AI agents their own unique personalities and abilities.

## Key Architecture Patterns

**Dual MCP Backend System:**

- **Rust Tauri Backend**: Native stdio MCP server communication via `MCPServerManager`
- **Web Worker Backend**: Browser-based MCP servers for dependency-free execution (`src/lib/web-mcp/`)
- **Unified API**: `rust-backend-client.ts` provides consistent interface using `safeInvoke()` wrapper

**Feature-Based Organization:**

- Each feature in `src/features/` contains components, hooks, and README documentation
- Compound component patterns (e.g., `Chat.Header`, `Chat.Messages`, `Chat.Input`)
- React Context providers for state sharing (`ChatProvider`, `EditorProvider`, `WebMCPProvider`)

**Service Layer Pattern:**

- `src/lib/` contains business logic and Tauri command wrappers
- Centralized logging via `getLogger('ComponentName')` instead of console methods
- All API communication through service classes with error handling

**Key Features:**

- **AI Agent Management**: Role-based system prompts and multi-agent collaboration
- **LLM Provider Support**: 8 providers, 50+ models including reasoning models (o3, DeepSeek R1)
- **Built-in Tool Ecosystem**: SecureFileManager, code execution, browser automation
- **MCP Integration**: Real-time stdio protocol with security validation

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
- Dexie (TypeScript-friendly IndexedDB wrapper)
- Zustand (Lightweight state management)
- Vite (Fast development and build tool)

**Backend Technologies:**

- Rust (High-performance native operations)
- Tokio (Async runtime for concurrent operations)
- SecurityValidator (Built-in security validation)
- Warp (HTTP server for browser automation)

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
- **CRITICAL: Never use blind casting anti-patterns:**
  - **Blind Type Assertions**: Never use `as` or `<Type>` casting without runtime validation
  - **Unsafe unknown handling**: When using `unknown`, ALWAYS validate before casting
  - **No blind any conversion**: Never cast `any` directly to a specific type without validation

  #### ❌ Bad (Blind Casting Anti-Patterns)

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
  ```

  #### ✅ Good (Type-Safe Validation)

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
    const user = UserSchema.parse(input); // Runtime validation
    return user.name;
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
  ```

- **Use the centralized logger instead of console.log**: Import `getLogger` from `@/lib/logger` and use context-specific logging (e.g., `const logger = getLogger('ComponentName')`) instead of `console.*` methods for better debugging and log management.
- **Never use inline import() types in interfaces.** Always use proper import statements at the top of the file instead of `import('../path').Type`. This improves readability, maintainability, and IDE support.

#### ❌ Bad (Inline Import Types)

```typescript
interface Config {
  tools?: import('../mcp-types').MCPTool[];
  messages: import('@/models/chat').Message[];
}
```

#### ✅ Good (Proper Import Statements)

```typescript
import type { MCPTool } from '../mcp-types';
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

- Use `shadcn/ui` components as the primary building blocks for UI, customizing as needed for project requirements.
- Manages local UI state and user input validation.
- Communicates with Tauri backend through the service layer.

#### Service Layer (`src/lib/`)

- Business logic and data transformation.
- Tauri command invocations and API integrations.
- IndexedDB operations and local data management.
- MCP client communication protocols.

#### Backend Layer (`src-tauri/src/`)

- Native system operations and file I/O.
- MCP server process management and stdio communication.
- Cross-platform compatibility handling.
- Security and permission management.

### Data Flow

1. User interaction in React components
2. Service layer processes requests and calls Tauri commands
3. Rust backend executes native operations or MCP communications
4. Results flow back through the same layers
5. UI updates reflect the changes

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
- `vite`: Version 4.x - Build tool and dev server
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
// src/lib/service-name.ts
export class ServiceName {
  // Public methods for component usage
}

export const serviceInstance = new ServiceName();
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

### Critical Development Patterns

**MCP Communication:**

- Always use `safeInvoke()` from `rust-backend-client.ts` for Tauri command calls
- MCP servers are managed through global `MCPServerManager` in Rust backend
- Web Worker MCP servers use `WebMCPProvider` context for browser-based tools

**Component Architecture:**

- Feature components follow compound patterns: `Chat.Header`, `Chat.Messages`, `Chat.Input`
- Each feature directory contains `components/`, `hooks/`, and `README.md`
- Use React Context for cross-component state sharing, not prop drilling

**Error Handling:**

- Backend commands return `Result<T, String>` in Rust
- Frontend wraps all Tauri calls in try-catch with centralized error logging
- Use structured error objects, never throw raw strings

**Development Commands:**

- `pnpm tauri dev` - Development with hot reload (port 1420)
- `pnpm tauri build` - Production build for distribution
- `pnpm dead-code` - Find unused code with unimported tool
- `pnpm refactor:validate` - Complete validation pipeline

**⚠️ CRITICAL: Content Security Policy (CSP) Warning:**

- **DO NOT add CSP configuration to `tauri.conf.json`** for desktop applications
- CSP is designed for web browsers, not desktop environments
- Tauri desktop apps using Web Workers and WASM require unrestricted access
- Adding CSP will cause blank white screens in release builds due to Worker blob URL blocking
- Dev mode has relaxed CSP enforcement, masking production issues
- Industry-standard practice (validated against Jan project): No CSP in Tauri desktop apps
- If security restrictions are absolutely necessary, use Tauri's native security features instead

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
- Optimize IndexedDB queries

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

- [Chat Feature Architecture & Implementation Manual](../docs/architecture/chat-feature-architecture.md)
- [UI Resource Implementation Guide](../docs/guides/ui-resource-implementation.md)
