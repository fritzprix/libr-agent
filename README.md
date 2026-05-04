# 🤖 LibrAgent

> **The Agent Harness for the Age of Autonomous Intelligence.**
> _Not just a chat app. An execution substrate where agents work, collaborate, and scale._

[한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent is a **local-first Agent Operating System** built on Tauri + Rust + React. It goes far beyond chat interfaces — providing a secure execution substrate, an MCP-native tool ecosystem, and a recursive delegation architecture that scales a single agent into a coordinated swarm.

Connect any LLM (cloud or local via Ollama), extend with any MCP server, and let agents do real work: editing files, running shells, browsing the web, managing knowledge — autonomously, for as long as it takes.

---

## Why LibrAgent?

The AI industry's focus has shifted. Recent 2026 benchmark analyses showed that the **same model can produce double-digit task-success gaps depending on the harness around it**. The model is the engine — but the harness determines how far it goes.

Every current option still forces a tradeoff:

| Platform | The Catch |
|---|---|
| **OpenClaw** | High-flexibility open ecosystem, but early-2026 analyses highlighted exposed instances, plaintext secret handling, and prompt-injection risk in community skills. |
| **Claude Cowork** | Strong local UX, but still limited on complex autonomous tasks. Closed ecosystem. Not extensible. |
| **Claude Code / Cursor** | Developer-only. Requires terminal fluency. Not general-purpose. |
| **Google Mariner** | Your work runs on Google's cloud VMs. You don't control your data. |
| **LangGraph / CrewAI** | Powerful frameworks, but you assemble everything yourself. No product experience. |

**LibrAgent is built to collapse that tradeoff.** Local-first security. MCP-native extensibility. Swarm-to-Org multi-agent coordination. A polished GUI that works for non-developers. All in one open-source desktop app.

### Who LibrAgent Is For

- **Solo developers** who want agents that can actually read, edit, run, browse, and persist context locally
- **Power users and operators** who want to compose their own stack from local models, API providers, MCP servers, and scheduled workflows
- **Researchers and analysts** who need browser automation, knowledge capture, repeatable playbooks, and long-running sessions
- **Privacy-sensitive teams** who want local execution, explicit governance, and a path from one agent to a coordinated org

---

## 🎬 Platform in Action

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_From a single agent to a coordinated swarm — recursive delegation, MCP tooling, and persistent workspace in one unified substrate._

---

## Core Pillars

### 1. 🔐 Local-First Security — Your Data Stays on Your Machine

LibrAgent treats security as a first-class architectural concern:

- **Session Isolation**: Every agent session gets its own dedicated `MCPServiceProxy` instance — zero cross-session data leakage
- **Built-in SecurityValidator**: Path traversal attacks and command injection blocked at the system level
- **No cloud substrate required**: All execution happens locally; only LLM API calls leave your machine
- **Full offline support**: Pair with [Ollama](https://ollama.ai) for a completely air-gapped agent stack

#### What stays local vs. what leaves your machine

- **Always local**: workspaces, local files, bundled skills, session state, MCP server configs, browser state, and local tool execution
- **Leaves your machine only when you choose it**: requests to cloud LLM providers or remote MCP/HTTP services you explicitly configure
- **Fully offline mode**: use Ollama or another local runtime plus local MCP servers for an air-gapped workflow

### 2. 🧩 MCP-Native Ecosystem — Infinite Extensibility by Design

MCP (Model Context Protocol) became a Linux Foundation standard in 2026. LibrAgent treats it not as a feature — but as the architectural backbone:

- **Full transport support**: stdio, HTTP, SSE, and OAuth 2.1 — the complete spec
- **12+ built-in servers**: Planning, Knowledge (RAG), Browser Automation, Workspace, Shell Execution, Content Store, and more
- **Preset catalog**: Install GitHub, Brave Search, Filesystem, and other popular servers in one click
- **Session-isolated instances**: Each agent session has independent MCP server state — no interference between parallel agents
- **Import from anywhere**: Migrate MCP configs from Cursor, VS Code, Claude Code, or Windsurf automatically

### 3. 🦾 Production-Grade Execution Substrate

Most AI tools are impressive in demos and brittle in production. LibrAgent is obsessively engineered for long-running, real work:

| Substrate | Capabilities |
|---|---|
| **Workspace** | Line-precise editing, multi-file ops, unified search, `@file`/`@skill`/`@playbook` context injection |
| **Shell** | Isolated execution AND persistent shells — async process monitoring (`poll`, `read output`, `list`) |
| **Browser** | Playwright-style tools (`goto`, `click`, `fill`, `screenshot`) with cache consistency guarantees |
| **Knowledge** | Graph-based knowledge management with entity/relation extraction (v2), BM25 full-text search |

**Reliability engineering included**: Context compaction, loop prevention, circuit breakers, and stale-response guards keep agents productive in sessions that last hours — not minutes.

### 4. 🤝 Swarm → Team → Org: Multi-Agent at Every Scale

LibrAgent has a coherent multi-agent story from solo execution to explicit org coordination:

- **`delegate`**: Parent agents spawn, brief, and monitor child sessions with explicit lineage tracking
- **`teamwork`**: Scaffold a full task-force workspace (agents.md, MISSION.md, KANBAN.md) with one command
- **`org`**: Formalize teams with durable org identity, root-session resume, and org-visible member hierarchy
- **`schedule`**: CRON-based automation — agents run unattended, on a schedule, with workspace constitution
- **Concurrency Gate**: Hard limits on parallel sessions and shell processes prevent deadlocks and runaway costs

### 5. ⚡ Bundled Skills — The Fastest Way to Go From Blank Install to Working Swarm

LibrAgent ships with a growing library of **Bundled Skills**. They are not random prompts bolted on top — they are reusable operating procedures that any agent can invoke by name.

The most important day-one skills are:

| Skill | What it does |
|---|---|
| `system-setup` | Detects and installs missing runtimes (Python, Node.js, uv) across all platforms |
| `mcp-installer` | Registers MCP servers from npm packages, GitHub URLs, or JSON config blocks |
| `mcp-importer` | Imports existing MCP configs from Cursor, VS Code, Windsurf, and similar setups |
| `specialist-creator` | Designs a full agent config (system prompt, model, tools) from a role description |
| `crew-constructor` | Scans available tools and batch-creates a matched specialist team automatically |
| `agent-tooling` | Audits agents, detects capability mismatches, and rebalances tool assignments dynamically |
| `delegate` | Guides parent→child session handoff with explicit context transfer and lineage tracking |
| `teamwork` | Scaffolds the shared workspace constitution for coordinated multi-agent work |
| `org` | Formalizes durable org identity and org-visible member hierarchy |
| `schedule` | Creates and manages recurring scheduled task groups for unattended automation |
| `soul-awakening` | Anchors an agent to a `SOUL.md` persona — tone, stance, identity |

And that's just the operator layer. LibrAgent also ships domain skills for:

- **knowledge and research**: `deep-research-report`, `knowledge-distiller`
- **document workflows**: `document-to-markdown`, `docx`, `pptx`
- **skill and workflow authoring**: `skill-creator`, `skill-deployer`, `playbook-creator`, `mcp-builder`
- **specialized operations**: `computer-diagnosis` and other focused helpers

_Important: `bootstrap` is a builtin capability often used alongside these skills. Bundled Skills are the reusable procedures; builtins and MCP tools are the execution substrate underneath._

---

## 🌍 Real-World Scenarios

### Solo Developer — Automated Code Review
1. Connect your local repo via the Workspace tool
2. Install the GitHub MCP preset (one click)
3. Ask: _"Find security issues in PR #42 and produce a Markdown report"_
4. Agent reads code, runs analysis, saves findings to the Knowledge server for future reference

### Marketer — Competitive Intelligence on Autopilot
1. Configure 5 competitor blogs via the Browser tool
2. Tell an agent: _"Create a scheduled competitor brief every morning at 7am"_ — the agent can use the `schedule` skill to wire up the recurring task group for you
3. Agent browses, summarizes, and appends to Knowledge store
4. Ask anytime: _"Summarize last week's competitor moves"_

### Engineering Team — Offline Agent Stack
1. `ollama pull qwen3:14b` — no API keys, no cloud
2. Connect Workspace + Shell tools to your codebase
3. Sensitive IP never leaves the machine
4. Agents read, modify, test, and commit — fully local

### Power User — Multi-Agent Research Pipeline
1. Use `crew-constructor` to auto-generate: Researcher × 3, Analyst × 1, Writer × 1
2. Orchestrator delegates in parallel via `delegate` skill
3. Results merge into a single structured report in Content Store
4. Schedule the entire workflow weekly via `schedule`

---

## 📖 Documentation & Guides

- **[Navigation Guide](docs/guides/navigation-guide.md)**: The Command & Control hub — `/assistants` (Role Definitions) and `/playbooks` (Workflow Blueprints).
- **[Architecture Guide](docs/architecture/agent-workflow-architecture.md)**: Session isolation, orchestration engine, and the Rust-driven Think-Act-Observe loop.
- **[Built-in Tools Guide](docs/guides/builtin_tool_bp.md)**: Tool design standards and MCP response patterns.

---

## 📦 Getting Started

Download the latest installer for your platform from the **[Releases page](https://github.com/fritzprix/libr-agent/releases/latest)**.

```
Windows  →  LibrAgent_x.x.x_x64-setup.exe
macOS    →  LibrAgent_x.x.x_aarch64.dmg
Linux    →  libragent_x.x.x_amd64.AppImage
```

**Developer Setup:**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

### The 5-Minute Onboarding Path

**Step 1 — Connect a model** (Settings → LLM Providers)
- Cloud: paste an OpenAI / Anthropic / Gemini / Groq API key
- Local: `ollama pull qwen3:14b` then select Ollama in Settings
- Already use Cursor or VS Code? Tell any agent: _"Import my MCP servers from Cursor"_ → `mcp-importer` handles it

**Step 2 — Add MCP tools** (Extensions sidebar)
- Browse the preset catalog and click Install, or
- Tell an agent: _"Install @modelcontextprotocol/server-everything"_ → `mcp-installer` registers it automatically

**Step 3 — Create your first agent**
- _"Create a researcher agent for competitive intelligence"_ → `specialist-creator` designs the full config
- _"Build a research team from my current tools"_ → `crew-constructor` batch-creates matched specialists
- _"Optimize tool assignments across all my agents"_ → `agent-tooling` audits and rebalances automatically

**Step 4 — Go parallel with `delegate`**
- Ask any agent to delegate sub-tasks to child sessions
- The `delegate` skill manages context handoff, lineage tracking, and result merging

**Step 5 — Build a persistent team**
- `teamwork` → scaffolds shared workspace with `agents.md`, `MISSION.md`, `KANBAN.md`
- `org` → formalizes the team with durable identity and org-root session management
- `schedule` → lets an agent create and manage CRON-based automation for you, unattended

### First prompts to copy-paste

- _"Import my MCP servers from Cursor and show me what was added."_
- _"Create a researcher agent for competitive intelligence using my current tools."_
- _"Install the GitHub MCP preset and attach it to a coding agent."_
- _"Delegate repository analysis to a child session and bring me back a summary."_
- _"Prepare a teamwork workspace for this repo, then create an org-ready specialist team."_
- _"Set up a scheduled daily competitor brief at 7am and keep everything in the shared teamwork workspace."_

---

## How LibrAgent Compares

```
                    Privacy/Local  MCP Ecosystem  Non-Dev UX  Multi-Agent  Open Source
LibrAgent              ★★★★★          ★★★★★         ★★★★☆       ★★★★★           ✅
OpenClaw               ★★☆☆☆          ★★★★☆         ★★★☆☆       ★★★☆☆           ✅
Claude Cowork          ★★★★☆          ★★☆☆☆         ★★★★★       ★★☆☆☆           ❌
Claude Code            ★★★★☆          ★★★☆☆         ★☆☆☆☆       ★★★☆☆           ❌
Google Mariner         ★★☆☆☆          ★★★☆☆         ★★★★☆       ★★★★☆           ❌
LangGraph / CrewAI     ★★★☆☆          ★★★☆☆         ★★☆☆☆       ★★★☆☆           ✅
```

---

## Design Philosophy

- **Local First**: Your data, keys, and agent "souls" remain under your exclusive control. No cloud substrate required.
- **Harness over Model**: The execution environment — tools, session state, delegation, governance — matters more than any individual model. LibrAgent is engineered to maximize what any model can do.
- **Stability over Features**: The CHANGELOG reflects an obsessive focus on runtime correctness — session isolation, compaction, loop prevention, stale-response guards — not just shipping new capabilities.
- **MCP as Infrastructure**: Not a plugin system. The entire tool ecosystem is organized around MCP as the primary interoperability layer.
- **Open Standards**: MIT licensed. Fully committed to MCP, open-source interoperability, and user data sovereignty.

---

## Contributing & License

LibrAgent is MIT licensed and built in the open. Contributions are welcome — whether that's new bundled skills, MCP integrations, bug fixes, or architecture improvements.

- 📖 [Contributing Guide](CONTRIBUTING.md)
- 🐛 [Issue Tracker](https://github.com/fritzprix/libr-agent/issues)
- 💬 [Discussions](https://github.com/fritzprix/libr-agent/discussions)

**License**: MIT
