# 🚀 LibrAgent Project Guidelines

## Project Overview

**LibrAgent: A High-Freedom AI Agent Platform - Infinitely Expandable with MCP!**

LibrAgent is a next-generation desktop AI agent platform that combines the lightness of Tauri with the intuitiveness of React. Users can automate all daily tasks by giving AI agents their own unique personalities and abilities.

## Key Features

**AI Agent Management:**

- Advanced Role Management: Create/edit/delete AI agent roles with sophisticated configurations
- Custom System Prompts: Define unique AI personalities and behaviors for each role
- Multi-Agent Collaboration: Coordinate multiple agents working together on complex tasks
- Agent Configuration Sharing: Export and import agent configurations

**LLM Provider Support:**

- 8 Major Providers: OpenAI, Anthropic, Google, Groq, Fireworks, Cerebras, Ollama, Empty
- 50+ Models: Including reasoning models (o3, DeepSeek R1, Qwen3)
- Cost Optimization: Real-time cost tracking and model comparison
- Massive Context: Up to 2M tokens support (Gemini 2.5 Pro)

**Built-in Tool Ecosystem:**

- SecureFileManager: Advanced file operations with path validation
- Code Execution: Python and TypeScript sandboxed environments
- Browser Automation: Interactive web scraping and automation
- Content Processing: PDF, DOCX, XLSX parsing with full-text search

**MCP Integration:**

- Real-time MCP Connection: Advanced stdio protocol implementation
- Dual Backend Support: Both Rust Tauri and Web Worker implementations
- Security Validation: Built-in SecurityValidator with comprehensive protection
- Tool Execution Context: Unified calling interface across all backends

**User Experience:**

- Modern UI: Clean, terminal-style interface with 20+ shadcn/ui components
- Session Management: Organize conversations with file attachments and context
- Centralized Configuration: All settings managed within the app
- Performance: Ultra-fast responses with Cerebras (1,800+ tokens/sec)

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
- **Do not use `any` in TypeScript.** The lint configuration is extremely strict; always use precise types and interfaces. Use unknown or generics if absolutely necessary, but avoid `any` as much as possible.
  - Do not add ESLint-disable comments that permanently or locally disable rules (for example: `// eslint-disable-next-line @typescript-eslint/no-explicit-any`). Instead, refactor the code to avoid `any` or use `unknown`/proper typing and document the rationale in a code comment and PR description when an exception is truly necessary.
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

- Use `shadcn/ui` components as the primary building blocks for UI, customizing as needed for project requirements.
- Manages local UI state and user input validation.
- Communicates with Tauri backend through the service layer.

#### Service Layer (`src/lib/`)

- Business logic and data transformation.
- Tauri command invocations and API integrations.
- Tauri command wrappers for SQLite/SeaORM-backed persistence and local data management.
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

### Agent V2 Architecture

The system implements a "Dual-Track" architecture to support advanced agentic workflows while maintaining legacy compatibility.

1.  **Dual-Track System:**
    - **Track 1 (Legacy):** Standard request/response chat handled by `ChatContext` (React).
    - **Track 2 (Agent V2):** Autonomous, multi-turn workflows orchestrated entirely by the Rust backend.

2.  **Rust-Based Orchestration:**
    - **`AgentSessionManager`**: The central brain residing in Rust (`src-tauri/src/agent/`). It manages the "Think-Act-Observe" loop independently of the frontend.
    - **Event-Driven:** Uses an event bus (`agent:event`) to push status updates and messages to the UI. The frontend (`AgentChatContext`) is purely reactive.

3.  **Session Isolation:**
    - **Session-Per-Proxy**: Each agent session gets its own isolated `MCPServiceProxy`.
    - **Stateful Tools**: Built-in tools (e.g., Planning, Knowledge) are instantiated per-session, ensuring data isolation (e.g., unique Todo lists per agent).

4.  **Service Context Pattern:**
    - Tools can dynamically inject context (e.g., browser state, current time) into the system prompt via the `get_service_context` trait method.

### MCP Tool Response Design Principles

**🚨 CRITICAL: What AI Agents Actually See vs. What They Don't**

This is **essential knowledge** for implementing any builtin tool or MCP server integration.

**The MCPResult Structure (LibrAgent-Specific):**

```rust
pub struct MCPResult {
    content: Vec<MCPContent>,           // Standard MCP: Agents READ this ✅
    structured_content: Option<Value>,  // LibrAgent extension: UI only, agents NEVER see ❌
    is_error: Option<bool>,             // Standard MCP
}
```

**Critical Context:** `structured_content` is a **LibrAgent-specific internal extension**, not part of the official MCP specification. The MCP protocol standard only defines:

- `content`: Array of MCPContent (text, images, resources) - what agents see
- `isError`: Boolean indicating success/failure

We added `structured_content` as a non-standard extension for LibrAgent's UI layer to render rich components without text parsing. External MCP servers connecting to LibrAgent won't have this field - only builtin tools use it.

**Data Visibility Matrix:**

| Component      | Sees Text Content | Sees structured_content |
| -------------- | ----------------- | ----------------------- |
| AI Agents      | ✅ YES            | ❌ **NEVER**            |
| UI Components  | ✅ Yes            | ✅ Yes                  |
| External Tools | ✅ Yes            | ✅ Yes                  |
| Debug Logs     | ✅ Yes            | ✅ Yes                  |

**Why This Matters:**

AI agents make decisions based ONLY on text content. If critical information (IDs, paths, status) is only in `structured_content`, agents cannot:

- Extract process IDs to call pollProcess
- Find file paths to read or modify
- Understand execution context (persistent vs isolated)
- Make informed decisions about next steps

**Design Rules:**

1. **All Critical Information in Text**
   - Process IDs, session IDs, file paths
   - Status information (running, finished, failed)
   - Exit codes, error messages, warnings
   - Directory locations, workspace context

2. **Use structured_content For:**
   - UI component rendering (charts, tables, formatted views)
   - Machine-readable data for external tools
   - Debugging and logging metadata
   - **NOT for agent decision-making**

3. **Format Text for Copy-Paste**
   - List IDs line-by-line for easy extraction
   - Use clear labels: "Process ID:", "Session:", "Path:"
   - Include IDs in follow-up suggestions

**Examples:**

```rust
// ❌ BAD: Agent can't extract process ID
MCPResult {
    content: vec![text("Process started successfully")],
    structured_content: Some(json!({"process_id": "abc123"})),
}
// Agent response: "How do I check the process status?"
// Problem: Agent doesn't know process_id is "abc123"

// ✅ GOOD: Agent can extract and use process ID
MCPResult {
    content: vec![text(
        "Background process started (ID: abc123)\n\n"
        "Use pollProcess(\"abc123\") to check status"
    )],
    structured_content: Some(json!({"process_id": "abc123"})),
}
// Agent response: "Let me check: pollProcess(\"abc123\")"
```

```rust
// ❌ BAD: Agent can't identify processes
MCPResult {
    content: vec![text("Found 3 processes (1 running, 2 finished)")],
    structured_content: Some(json!({
        "processes": [
            {"id": "abc", "status": "running"},
            {"id": "def", "status": "finished"},
            {"id": "ghi", "status": "finished"}
        ]
    })),
}
// Agent problem: Can't extract IDs to call readProcessOutput

// ✅ GOOD: Agent can see and use process IDs
MCPResult {
    content: vec![text(
        "Found 3 processes (1 running, 2 finished):\n\n"
        "• abc123 [running]: npm build --watch\n"
        "• def456 [finished]: cargo test\n"
        "• ghi789 [finished]: python train.py"
    )],
    structured_content: Some(json!({/* same as above */})),
}
// Agent can easily extract "abc123" for follow-up actions
```

```rust
// ❌ BAD: State only indicated in JSON
MCPResult {
    content: vec![text("Command executed: echo hello\n\nhello")],
    structured_content: Some(json!({
        "execution_type": "persistent",
        "cwd": "/home/user/project"
    })),
}
// Agent confusion: Is this a persistent shell or isolated execution?

// ✅ GOOD: State explicitly indicated in text
MCPResult {
    content: vec![text(
        "Command executed: echo hello\n\n"
        "hello\n\n"
        "Persistent shell state (maintained for next runInPersistentShell call):\n"
        "  Working directory: /home/user/project\n"
        "  Exit code: 0"
    )],
    structured_content: Some(json!({/* same */})),
}
// Agent clearly understands persistent execution context
```

**Implementation Checklist:**

When implementing a builtin tool:

- [ ] Critical IDs (process, session, file) appear in text output
- [ ] Status information is explicit in text (not just JSON fields)
- [ ] Follow-up suggestions include actual IDs/parameters to use
- [ ] State context (persistent, isolated, workspace) is clear from text
- [ ] Error messages include diagnostic info in text, not just JSON
- [ ] Test by reading ONLY the text content - pretend JSON doesn't exist

**Common Mistakes:**

1. **Hidden IDs**: Process/session IDs only in structured_content
2. **Implicit State**: execution_type="persistent" in JSON but no text indicator
3. **Generic Messages**: "3 items found" without listing the items
4. **Missing Context**: Paths/IDs in suggestions without showing actual values

**Testing Strategy:**

```rust
// Test your tool response
let result = your_tool_handler(args).await?;

// Extract text content
let text = result.content.iter()
    .filter_map(|c| match c {
        MCPContent::Text { text, .. } => Some(text),
        _ => None,
    })
    .collect::<Vec<_>>()
    .join("\n");

// Ask yourself:
// 1. Can I extract all IDs needed for follow-up actions?
// 2. Is the execution context (persistent/isolated) clear?
// 3. Are error details understandable without JSON?
// 4. Can I understand what happened by reading text only?
```

**Remember:** Design tool responses as if `structured_content` doesn't exist. If agents can understand and act on the text content alone, you've done it right.

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

### ⚠️ CRITICAL: Content Security Policy (CSP) Warning

**DO NOT add CSP configuration to `tauri.conf.json`** for desktop applications:

- CSP is designed for web browsers, not desktop environments
- Tauri desktop apps using Web Workers and WASM require unrestricted access
- Adding CSP will cause blank white screens in release builds due to Worker blob URL blocking
- Dev mode has relaxed CSP enforcement, masking production issues
- Industry-standard practice (validated against Jan project): No CSP in Tauri desktop apps
- If security restrictions are absolutely necessary, use Tauri's native security features instead

**Why this matters:**

- Vite bundles Web Workers as blob URLs at runtime
- CSP blocks blob: scheme by default, preventing Worker initialization
- Release builds fail silently without proper logging configuration
- Symptoms: Blank white screen on Windows, works fine in dev mode

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
- Minimize database round-trips

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

- [Agent Workflow Architecture](docs/architecture/agent-workflow-architecture.md)
- [Built-in Tools Documentation](src-tauri/src/mcp/builtin/README.md)
- [External MCP Server Integration](docs/architecture/external-mcp-integration.md)
- [UI Resource Implementation Guide](docs/guides/ui-resource-implementation.md)
