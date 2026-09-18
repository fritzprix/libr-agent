# 🚀 LibrAgent Project Guidelines

## Project Overview

**LibrAgent: A High-Freedom AI Agent Platform - Infinitely Expandable with MCP!**

LibrAgent is a next-generation desktop AI agent platform that combines the lightness of Tauri with the intuitiveness of React. Users can automate all daily tasks by giving AI agents their own unique personalities and abilities.

This workspace contains both application code (React/TypeScript frontend + Rust/Tauri backend) and extensive documentation. Follow the relevant section for each task.

---

## 🚫 절대 원칙: 오버엔지니어링 & 불필요한 추상화 금지 (KISS / YAGNI)

> [!CAUTION]
> **이 프로젝트는 100만 동접 분산 웹서비스가 아닌 개인 로컬(Tauri + SQLite) 데스크톱 앱입니다.**
> 수십~수백 개 수준의 데이터를 다루는 로컬 환경에 분산 시스템급의 과도한 계층 분리나 불필요한 다중 추상화를 절대 도입하지 마십시오.

1. **Dual State / Dual ID 설계 절대 금지**:
   - 프롬프트 토큰 몇 바이트 아끼겠다고 ID를 잘라내거나(Display vs Storage 분리), 불필요한 상태 이원화를 만들어 복잡한 역방향 매핑(reverse lookup) 및 불일치 버그를 유발하지 마십시오.
   - 단일 고유 ID(SSOT) 원칙을 철저히 유지하십시오. 에이전트가 보는 ID와 DB/내부 시스템이 찾는 ID는 100% 동일해야 합니다.
2. **방어 코드 남발보다 단순한 구조 우선**:
   - 복잡도를 키우는 수백 줄의 껍데기 래퍼, 뉴타입, 매핑 알고리즘보다 처음부터 문제가 생기지 않는 단순하고 직관적인 설계를 우선하십시오.
3. **YAGNI (You Aren't Gonna Need It)**:
   - "나중에 필요할지도 모른다"는 상상 속의 확장성을 위해 코드를 미리 꼬아놓지 마십시오. 당장 필요한 기능만 가장 단순한 형태로 구현하십시오.

---

## Code Guidelines

### Technology Stack

- **Package Manager**: pnpm@9.15.9 (pinned via `packageManager` in package.json and enforced by preinstall script)
- **Language**: TypeScript 5.6 (frontend), Rust 2021 edition (backend)
- **Framework**: React 18.3 + Vite 6.x (frontend), Tauri 2.x (desktop framework)
- **Build System**: Vite (frontend), Cargo (backend)
- **Test Framework**: Vitest (frontend), cargo test --tests (Rust integration tests in `src-tauri/tests/`)

### Environment Setup

1. Install Rust via [rustup.rs](https://rustup.rs/) and Node.js 20+
2. Enable pinned pnpm: `corepack enable && corepack prepare pnpm@9.15.9 --activate`
3. Install dependencies: `pnpm install --frozen-lockfile`
4. Start development: `pnpm tauri dev` (full desktop app with backend) or `pnpm dev` (frontend only)
5. Build for production: `pnpm tauri build`
6. API keys are managed in-app via Settings modal (not in .env files)

See [README.md](README.md) for detailed setup instructions.

### Development Scripts & Workflow

| Command                  | Purpose                                                                            |
| ------------------------ | ---------------------------------------------------------------------------------- |
| `pnpm dev`               | Start Vite dev server (frontend only)                                              |
| `pnpm tauri dev`         | Start full Tauri desktop app with hot reload                                       |
| `pnpm build`             | Build frontend for production                                                      |
| `pnpm tauri build`       | Create production desktop app bundle                                               |
| `pnpm lint`              | Run ESLint on TypeScript/React code                                                |
| `pnpm format`            | Format code with Prettier                                                          |
| `pnpm rust:fmt`          | Check Rust formatting with rustfmt                                                 |
| `pnpm rust:clippy`       | Run Rust linter (clippy)                                                           |
| `pnpm dead-code`         | Find unused code with unimported                                                   |
| `pnpm refactor:validate` | **Complete validation pipeline** (lint, format, Rust validation, build, dead-code) |

**Workflow Recommendation:** Run focused, lightweight unit tests or lint for affected files during development. `pnpm refactor:validate` is resource-heavy and must ONLY be executed when explicitly requested by the user or as a final check before a PR.

### Key Architecture Patterns

**Agent V2 Architecture (Session-Isolated):**

- Per-Session Tool Instances: Each agent session gets isolated `MCPServiceProxy` with dedicated builtin server instances
- Session-Specific MCP Managers: Separate `HttpSessionManager` and `SessionMCPManager` per session
- Context Registry System: Dynamic context providers (time/location, skills) inject state into system prompts
- Rust-Orchestrated Workflows: Think-Act-Observe loop managed entirely in Rust backend (`AgentSessionManager`)

**MCP Integration Architecture:**

- External MCP Servers: Stdio/HTTP protocol via `rmcp` library, managed by session-isolated managers
- Builtin MCP Servers: Native Rust implementations via `BuiltinMCPServer` trait (Planning, Knowledge, Browser, Workspace, Content Store, etc.)
- Unified Tool Discovery: `MCPServiceProxy` routes calls to builtin or external servers transparently

**Feature-Based Organization:**

- Each feature in `src/features/` typically contains components, hooks, and logic specific to that feature
- Compound component patterns (e.g., `Chat.Header`, `Chat.Messages`, `Chat.Input`)
- React Context providers for state sharing (`ChatProvider`, `AgentSessionProvider`, `AgentChatProvider`)

**Service Layer Pattern:**

- `src/lib/backend/` contains Tauri command wrappers with centralized `safeInvoke()` utility
- Centralized logging via `getLogger('ComponentName')` instead of console methods
- All API communication through typed service modules with error handling

### Coding Style

**General:**

- 2 spaces indentation across all files
- Descriptive variable names in both Rust and TypeScript
- Consistent naming conventions for files and directories
- All comments in English

**Rust Backend (`src-tauri/`):**

- Follow [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/) and use `rustfmt`
- snake_case for functions, variables, module names
- PascalCase for types, structs, enums
- Comprehensive doc comments (`///`) for public APIs
- Explicit error handling with `Result<T, E>` types

**Frontend (`src/`):**

- Follow Prettier and ESLint configurations
- camelCase for variables and functions
- PascalCase for React components and TypeScript interfaces
- Functional components with hooks over class components
- TypeScript interfaces for type definitions
- **Never use `any`** — use precise types, `unknown` with type guards, or Zod schemas for validation

**CSS/Styling:**

- Use `shadcn/ui` components as primary building blocks
- Tailwind CSS utility classes (avoid arbitrary class names)
- Custom classes in CSS files or safelist in `tailwind.config.js` if needed

### Type Safety Principles

- **No blind type assertions** — validate before casting
- **No `JSON.parse` without schema validation** — use Zod
- **No backend response assumptions** — validate with type guards
- **Generic functions require validator parameters**

See [Type Safety Refactoring Plan](../docs/refactoring/type-safety-refactoring-plan.md) for migration guide.

### Error Handling

- Backend commands return `Result<T, String>` in Rust
- Frontend wraps all Tauri calls via `safeInvoke()` with centralized error logging
- Structured error objects with `MCPError` type for protocol errors
- Builtin tools return `Result<MCPResult, String>` for consistent error handling

### File Organization

**Frontend (`src/`):**

```
src/
├── app/              # App entry, root layout, global providers
├── assets/           # Static assets (images, svgs)
├── components/       # Shared, generic UI components (reusable)
├── features/         # Feature-specific components, logic, hooks
├── config/           # Static config files
├── context/          # React context providers
├── hooks/            # Generic, reusable hooks
├── lib/              # Service layer, business logic, data, API
├── models/           # TypeScript types and interfaces
├── styles/           # Global or shared CSS
└── test/             # Test utilities
```

**Backend (`src-tauri/src/`):**

```
src-tauri/src/
├── agent/            # Agent orchestration (session lifecycle, LLM interaction, tool execution)
├── browser_sidecar/  # Browser automation
├── commands/         # Tauri command handlers
├── entity/           # SeaORM entities
├── lifecycle/        # Session creation/recovery
├── mcp/              # MCP integration (builtin servers, external managers)
├── models/           # Data models
├── repositories/     # Data access layer
├── scheduled/        # Scheduled tasks
├── search/           # Search functionality
├── server/           # HTTP server
├── services/         # Browser, workspace, etc.
├── session/          # Session management
├── session_isolation/# Session isolation logic
├── utils/            # Shared utilities
└── main.rs           # Entry point
```

### Testing

- Frontend: Vitest for unit/component tests (`pnpm test:run`)
- Backend: **Integration tests only** in `src-tauri/tests/` (CI runs `cargo test --tests`, NOT `cargo test --lib`)
- Rust `#[cfg(test)]` blocks in `src/` are never executed in CI
- Test Tauri commands with mock data
- Verify cross-platform compatibility

### CI / Pull Requests

- GitHub Actions for CI and releases (`.github/workflows/ci.yml`, `release.yml`)
- Node.js 20, pnpm@9.15.9 pinned
- CI runs `pnpm install --frozen-lockfile`, lint, format check, Rust fmt/clippy, build, tests
- `pnpm refactor:validate` mirrors the full CI pipeline locally

### Security

- Tauri allowlist configuration restricts API access
- Validate all frontend-to-backend input
- Sanitize data before MCP server communication
- Secure API key storage via in-app Settings modal
- Never commit API keys to version control

### Performance Guidelines

- React.memo for expensive components
- Proper useEffect dependency arrays
- Lazy load components when appropriate
- Minimize database round-trips from UI
- Async/await for non-blocking Rust operations
- Cache frequently accessed data
- Optimize MCP server communication

---

## Documentation Guidelines

**Audience:** Developers and contributors working on LibrAgent

### Documentation Tree

```
docs/
├── README.md                    # Documentation index
├── api/
│   ├── tauri-commands.md        # Tauri command reference
│   └── http_api.md              # HTTP API for remote management
├── guides/
│   ├── getting-started.md       # Setup and quick start
│   ├── navigation-guide.md      # Internal structure and UI routes
│   ├── system-prompt-guide.md   # Assistant prompt guidelines
│   └── builtin_tool_bp.md       # Built-in tool design standards
├── architecture/
│   ├── agent-workflow-architecture.md
│   ├── gemini-caching-implementation.md
│   ├── session-lineage-and-tree-ui.md
│   ├── agent-vibe-charter.md
│   ├── ai-soul-manifesto.md
│   ├── soul-lounge-recovery-loop.md
│   └── open-source-launch-manifesto.md
├── analysis/
│   ├── product-strengths.md
│   ├── competitive-landscape-2026.md
│   └── workspace-tool-critique.md
├── contributing/
│   ├── coding-standards.md
│   ├── product-messaging-guide.md
│   ├── open-source-launch-finale.md
│   └── github-release-notes-template.md
├── refactoring/
│   └── type-safety-refactoring-plan.md
└── sprints/
    └── README.md                # Archived sprint logs
```

### Document Conventions

- **Naming**: kebab-case for markdown files (e.g., `tauri-commands.md`)
- **Frontmatter**: None required (hand-written docs)
- **Linking**: Relative paths within docs; absolute URLs for external refs
- **Assets**: Place in `docs/assets/` or alongside related docs
- **Generated vs Hand-written**: Hand-written unless a path/header indicates generated content

### Documentation Workflow

- **Writing**: Match existing document tone and structure
- **Review**: PRs for doc changes follow same process as code
- **Update**: Keep docs current with code changes (update policy: update when changing user-facing behavior)

### Documentation Quality

- Spell-check and verify links before merging
- Maintain cross-references in `docs/README.md`

---

## Security

- Tauri security model: allowlist, capability system (no CSP in desktop apps)
- API keys: in-app secure storage, never in repo
- Input validation on all Tauri command boundaries
- MCP server communication: protocol validation, sandboxed execution

---

## Performance

- Frontend: React concurrent features, virtualization (react-virtuoso), SWR caching
- Backend: Tokio async runtime, connection pooling (reqwest), SeaORM query optimization
- MCP: Session-isolated tool instances prevent cross-session contention
- Bundle: Vite code-splitting, bundle size monitoring (`pnpm perf:bundle`)

---

## Additional Notes

- **CSP Warning**: Do NOT add Content Security Policy to `tauri.conf.json` for desktop apps — causes blank screens in release builds. Use Tauri's native security features instead.
- **Rust Tests**: Write integration tests in `src-tauri/tests/` only; `#[cfg(test)]` in `src/` is not run in CI.
- **Agent Sessions**: Each session has isolated workspace at `<data_dir>/workspaces/<session_id>/` — `agents.md` placed there is auto-loaded into system prompt.
- **Validation Command**: `pnpm refactor:validate` is the single command to run before any PR.
