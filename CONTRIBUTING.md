# Contributing to LibrAgent

Thanks for your interest in contributing to LibrAgent! 🚀

LibrAgent is a lightning-fast AI agent platform that combines Tauri's
efficiency with React's flexibility. We're building the next generation
of agent automation with persistent tool state and built-in MCP support.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/fritzprix/libr-agent`
3. Install dependencies: `pnpm install`
4. Install Rust: [rustup.rs](https://rustup.rs/) (required for Tauri backend)
5. Run development mode: `pnpm tauri dev`

**System Requirements:**

- Node.js 18+
- pnpm (install with `npm install -g pnpm`)
- Rust (for native backend compilation)

## Development Guidelines

### Understanding LibrAgent Architecture

Before contributing, understand our core design:

- **Built-in Tools**: Persistent state, always in context (Browser, Terminal,
  File Manager, Code Execution)
- **Tauri Backend**: Rust for security and performance, stdio-based MCP protocol
- **React Frontend**: IndexedDB for local state, no server required
- **MCP Support**: External servers for specialized workflows

See [docs/architecture/overview.md](docs/architecture/overview.md) for detailed architecture.

### Code Style

**Frontend (TypeScript/React):**

- Use TypeScript with strict typing (no `any` unless absolutely necessary)
- Follow Prettier configuration
- Use React hooks and functional components
- Use Tailwind CSS for styling
- Import from `@/` for project files
- Use centralized logging: `import { getLogger } from '@/lib/logger'`

**Backend (Rust):**

- Follow Rust conventions (snake_case for functions, PascalCase for types)
- Use `cargo fmt` and `cargo clippy`
- Add documentation comments (`///`) for public APIs
- Handle errors explicitly with `Result<T, E>`
- Use `tokio` for async operations

### Validation Workflow

Before committing, run:

```bash
pnpm refactor:validate
```

This runs:

- ESLint checks
- Prettier formatting
- Rust formatting and clippy
- TypeScript build
- Dead code detection

### Pull Request Process

1. Create a feature branch: `git checkout -b feature/amazing-feature`
2. Make your changes following the code style guide
3. Run `pnpm refactor:validate` to ensure code quality
4. Test thoroughly (especially tool state persistence)
5. Commit with meaningful messages (use emojis: 🎨 style, ✨ feature,
   🐛 fix, 📝 docs)
6. Push and create a pull request with clear description
7. Address review feedback

### Areas We Need Help With

**High Priority:**

- 🧠 Built-in tool improvements (Browser state persistence, Terminal
  history, File sandbox)
- 🤖 LLM provider integrations (Support more models, improve streaming)
- 🔧 MCP server integration (Better error handling, stdio protocol
  robustness)
- 🔒 Security & sandboxing (Tool execution isolation, input validation)

**Medium Priority:**

- 🎨 UI/UX improvements (Better chat interface, settings management)
- 📚 Documentation (Architecture docs, API reference, tutorials)
- 🧪 Testing (Unit tests, integration tests, E2E tests)
- ⚡ Performance optimization (Faster tool execution, reduced memory usage)

**Longer Term:**

- 🌐 Internationalization (Multi-language support)
- 📊 Analytics & telemetry (Opt-in usage tracking)
- 🔄 Multi-agent orchestration (Agent collaboration)

## Bug Reports

Please use GitHub Issues with:

- Clear description of the problem
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs (check DevTools console)
- Screenshots if applicable
- Your environment:
  - OS (Windows, macOS, Linux)
  - LibrAgent version
  - Rust version (from `rustc --version`)
  - Node version (from `node --version`)
  - LLM provider being used

**Security Issues:** Please report security vulnerabilities privately to maintainers
instead of public issues.

## Feature Requests

We welcome feature requests! Before opening an issue:

- Check existing issues and discussions first
- Provide clear use cases (who needs this and why?)
- Consider how it aligns with LibrAgent's core mission:
  - Does it enhance built-in tools or MCP integration?
  - Does it improve tool state persistence?
  - Does it reduce friction in agent workflows?
- Be open to discussion about implementation complexity vs benefit

## Testing Guidelines

We take code quality seriously. Before submitting a PR:

### Manual Testing

1. **Tool Integration**: Test affected tools in development mode
   - Browser: Verify session persistence across agent calls
   - Terminal: Ensure command history is maintained
   - File operations: Test path validation and sandboxing
   - Code execution: Verify stdin/stdout/stderr handling

2. **Streaming**: Test model responses with streaming enabled
   - Check token accumulation
   - Verify tool calls parse correctly
   - Test error recovery mid-stream

3. **Multiple Providers**: Test with different LLM providers if applicable
   - Anthropic (Claude)
   - OpenAI (GPT)
   - Google (Gemini)

### Automated Testing

```bash
# Run all tests
pnpm test

# Run specific test file
pnpm test src/lib/ai-service/__tests__/openai.test.ts

# Test Rust backend
cd src-tauri && cargo test
```

## Community

- Be respectful and inclusive
- Help others when possible
- Share your use cases and feedback
- Check out [docs/](docs/) for architecture and design decisions

Thank you for contributing to LibrAgent! 🦀⚡
