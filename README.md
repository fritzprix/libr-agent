# 🤖 LibrAgent

![LibrAgent Banner](/public/banner.png)

> **Lightning-fast AI agent platform with built-in tools and MCP support**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

**Automate everything with intelligent AI agents. No complex setup. No dependency hell.**

[Quick Start](#installation) • [Features](#built-in-tools) • [Architecture](#architecture) • [Contributing](#contributing)

---

## Demo

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

This demo highlights the core capabilities of LibrAgent:

- **Integrated Browser & Computer Use Tools**: Seamlessly control the browser and execute Shell/PowerShell commands.
- **AI Agent Assistance**: Unified planning and UI bubbles to assist the agent in complex tasks.

## The Problem

MCP (Model Context Protocol) is a good idea, but using it has friction:

### Dependency Hell

- Each MCP server needs Node.js, Python, or other runtimes
- Version conflicts between servers
- Installation is manual and error-prone

### Trust Issues

- MCP servers run arbitrary code on your machine
- Hard to verify what third-party servers actually do
- No sandboxing by default

### Stateless Tools

- Tools don't maintain state between calls
- LLM can't see what's currently in the browser, terminal, etc.
- Makes multi-step workflows clunky

Example: LLM asks to open a website. You wait. It asks to click a button. You wait again. It has no idea what's on screen because the browser state isn't in context.

## How LibrAgent is Different

### Built-in Tools with Persistent State

- Browser, terminal, file manager, code execution included
- Tools maintain their state (browser tabs, terminal history, file system)
- State is always visible to the LLM in context

This means the LLM can:

- See what's currently rendered in the browser
- Know what commands were run in the terminal
- Track file operations across multiple steps

### Still Supports MCP

- Connect external MCP servers when you need specialized tools
- Built-in tools handle 80% of common workflows
- You're not forced to install anything for basic use

## Trade-offs

### Larger Binary

- Built-in tools make the app ~50MB instead of ~5MB
- We chose convenience over size

### Limited Tool Selection

- Only common tools are built-in
- For niche tools, you still need MCP servers
- But most workflows don't need niche tools

## Is This Production Ready?

Not yet. It works for daily use but has rough edges:

- Error handling could be better
- Some tools need more sandboxing
- Performance isn't optimized

Contributions welcome.

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

OpenAI, Anthropic (Claude), Google (Gemini).

Uses standard APIs. Add your API key in settings.

## Key Built-in Tools

- **Browser**: Headless Chrome automation, session persistence
- **Workspace**: Unified Terminal, File Manager, and Shell Execution (supports Python/Node.js via CLI) with sandboxing
- **Planner**: Task tracking and goal management
- **Knowledge**: Semantic search and memory retrieval
- **Skills**: Reusable capabilities and tool definitions
- **Playbook**: Workflow automation and process templates
- **Assistant**: Role management and system prompt configuration

> Note: Additional internal modules (Bootstrap, Content Store, UI, MCP Manager) handle infrastructure and state.

## Architecture

### Tauri 2.x + Rust Backend

- Smaller binaries than Electron (~50MB vs ~150MB)
- Better sandboxing for tool execution
- Native performance

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
