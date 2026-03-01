# 🤖 LibrAgent

![LibrAgent Banner](/public/banner.png)

> **Autonomous AI agent platform with built-in tools, persistent state, and MCP extensibility**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

**Less glue code. Less setup pain. More real execution.**

[Quick Start](#installation) • [Features](#key-built-in-tools) • [Architecture](#architecture) • [Contributing](#contributing)

For the project's engineering ethos and open-source direction, read the [Open Source Launch Manifesto](docs/architecture/open-source-launch-manifesto.md).
For the autonomy-first operational doctrine, read the [AI Soul Manifesto](docs/architecture/ai-soul-manifesto.md).

---

## Demo

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

This demo shows the default flow LibrAgent is built for:

- **Integrated Browser + Computer Use**: Browser control and Shell/PowerShell execution in one place.
- **Agentic Execution Loop**: Planning, tool use, and result capture inside a single workflow.

## Why This Exists

MCP is powerful. Real-world MCP usage is often painful.

### Dependency Hell

- Every server drags runtime/version baggage.
- Setup becomes a compatibility mini-game.
- Local onboarding becomes fragile and slow.

### Trust Issues

- Third-party servers execute code locally.
- Visibility into behavior is limited.
- Sandboxing is not guaranteed by default.

### Stateless Tools

- Tool calls often lose context between turns.
- Agents repeatedly rediscover the same state.
- Multi-step work becomes expensive and brittle.

Classic failure mode: open page → wait → click button → wait → lose context → repeat.

## What Makes LibrAgent Different

### Built-in Tools with Persistent State

- Built-in browser, workspace, terminal, and code tools.
- Persistent state across steps (tabs, command history, filesystem context).
- State is surfaced to the agent so it can continue, not restart.

Result:

- Understand current browser/terminal/workspace state.
- Execute longer workflows with fewer redundant calls.
- Keep strategic focus instead of tool thrash.

### Still Supports MCP

- External MCP servers remain first-class for niche capabilities.
- Built-ins cover most daily execution paths.
- You can start fast, then extend as needed.

## Trade-offs

### Larger Binary

- Bigger binary than minimal clients.
- Trade chosen deliberately: reliability and speed of execution over tiny footprint.

### Limited Tool Selection

- Built-ins are opinionated toward common workflows.
- Niche work still belongs to external MCP servers.

## Is This Production Ready?

Not fully. It is already useful for real work, but we still have sharp edges:

- Error handling could be better
- Some tools need more sandboxing
- Performance isn't optimized

If you're shipping serious agent systems, contributions are welcome.

## Installation

Download from [releases](https://github.com/fritzprix/libr-agent/releases/latest).

> ⚠️ **macOS Users:** The app is not code-signed. On first launch, you'll see a security warning. **Right-click the app → Open** to bypass Gatekeeper. Subsequent launches will work normally.

> ⚠️ **Linux (Debian/Ubuntu) Users:** After downloading the `.deb` package, install dependencies and the package:
>
> ```bash
> sudo apt install -f
> ```
>
> Or install dependencies manually before installing:
>
> ```bash
> sudo apt install gstreamer1.0-plugins-bad gstreamer1.0-gtk3 gstreamer1.0-pulseaudio
> sudo dpkg -i LibrAgent_*_amd64.deb
> ```

Or build from source:

> **Linux (Debian/Ubuntu) Build Prerequisites:**
>
> ```bash
> sudo apt install libglib2.0-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libwebkit2gtk-4.1-dev
> ```

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

## Supported LLMs

OpenAI, Anthropic (Claude), Google (Gemini), Groq, Ollama, Cerebras, Fireworks.

Uses standard APIs. Add your API key in settings.

## Key Built-in Tools

Primary user-facing tools:

- **Browser**: Headless Chrome automation, session persistence
- **Workspace**: Unified Terminal, File Manager, and Shell Execution (supports Python/Node.js via CLI) with sandboxing
- **Content Store**: Persistent file content storage and retrieval
- **Planner**: Task tracking and goal management
- **Knowledge**: Semantic search and memory retrieval
- **Skills**: Reusable capabilities and tool definitions
- **Playbook**: Workflow automation and process templates
- **Assistant**: Role management and system prompt configuration
- **Swarm**: Spawn and orchestrate child agents to delegate tasks in parallel

> Note: Additional internal modules (Bootstrap, Content Store, UI, MCP Manager) handle infrastructure and state.

## Agent Features

### @mention Reference System

Type `@` in the chat input to inject rich context directly into your message:

| Syntax | What it injects |
|---|---|
| `@skill:name` | Full skill documentation (available in all chat views) |
| `@tool:name` | Soft attention hint for a specific MCP/builtin tool |
| `@file:path` | File content from the session workspace |

Autocomplete suggests matching skills, tools, and files as you type. On submit, mentions are resolved and injected into the message before it reaches the LLM. Unresolved references are appended as a `⚠️` warning so the agent knows what was missing.

### Workspace Agent Instructions

Drop any of the following files into a session workspace and LibrAgent will automatically inject their contents into the system prompt before the session starts:

```
agents.md  AGENTS.md  soul.md  CLAUDE.md  GEMINI.md
```

This lets you define project-specific agent behavior, coding conventions, or constraints at the workspace level — no global settings change needed.

### Session Bookmarks

Mark important sessions as bookmarks to keep them pinned in the session list. Bookmarks persist across restarts.

## Architecture

### Tauri 2.x + Rust Backend

- Smaller binaries than Electron (~50MB vs ~150MB)
- Better sandboxing for tool execution
- Native performance
- Uses Rust backend for robust local state storage (SQLite via SeaORM)

### React + TypeScript Frontend

- Uses Rust backend for robust local state storage (SQLite via SeaORM)
- No server needed, everything runs locally

### Built-in vs MCP

- Built-in tools: Rust implementations, directly integrated
- MCP tools: Child process communication via stdio

## Why These Choices?

### Why Tauri?

- Smaller binaries
- Better security model than Electron
- Rust's memory safety for tool execution

### Why Built-in Tools?

- Most workflows need the same 5-6 tools
- Eliminates installation friction
- Allows stateful integration with LLM context

### Why Still Support MCP?

- Can't predict every use case
- MCP ecosystem has specialized tools
- Users should have the option

## Contributing

See CONTRIBUTING.md.

Areas that need work:

- Better error messages
- More robust sandboxing
- Tool state serialization
- Performance optimization

## License

MIT
