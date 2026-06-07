# 🤖 LibrAgent

> **A local-first desktop app for AI agents that use real tools, run in parallel, and stay under your control.**
> _Connect any LLM, add any MCP server, and let agents read files, run shells, browse the web, and automate work that actually finishes._

[한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

LibrAgent is a **local-first agent workspace** built on Tauri + Rust + React. It is designed for people who want more than chat: real file access, shell execution, browser automation, MCP extensibility, and multi-agent workflows that can keep going for hours instead of falling apart after one demo loop.

Connect cloud models or local runtimes like Ollama. Import MCP servers from tools you already use. Then hand work to agents that can inspect code, edit files, run commands, browse sites, capture knowledge, and delegate subtasks without shipping your whole workflow to somebody else's VM.

**Start here:** [Download the latest release](https://github.com/fritzprix/libr-agent/releases/latest) · [Jump to 5-minute onboarding](#the-5-minute-onboarding-path) · [See real-world scenarios](#-real-world-scenarios)

---

## Why LibrAgent?

Most agent products still force an annoying tradeoff:

- **Easy UI, weak execution**
- **Strong automation, no product polish**
- **Cloud convenience, weak privacy**
- **Framework flexibility, but you build the whole damn stack yourself**

LibrAgent is built for the middle that people actually want:

- **Local-first control** for files, workspaces, sessions, and browser state
- **Open extensibility** through MCP instead of a closed plugin story
- **Real execution** across shell, browser, workspace, and knowledge tools
- **A GUI that normal humans can use** without giving up power-user depth
- **A path from one agent to many** when a single assistant stops being enough

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

## What You Can Do in the First 10 Minutes

### 1. Review a repository with real tools

- Connect a local repo with the Workspace tool
- Add the GitHub MCP preset
- Ask: _"Review PR #42 for security issues and save the report"_

### 2. Build a fully local agent stack

- Run `ollama pull qwen3:14b`
- Connect Workspace + Shell
- Let an agent read, modify, test, and iterate without sending your code to a cloud VM

### 3. Turn research into a repeatable workflow

- Add Browser + Knowledge
- Ask: _"Track these 5 competitor blogs and give me a summary every morning"_
- Convert a one-off task into a scheduled pipeline

### 4. Go from one assistant to a real team

- Create specialists with `specialist-creator`
- Split work with `delegate`
- Turn recurring collaboration into a shared `teamwork` or `org` workspace

---

## Why It Holds Up After the Demo

### 1. 🔐 Local-First Security — Your Data Stays on Your Machine

LibrAgent treats security as a first-class architectural concern:

- **Session Isolation**: Every agent session gets its own dedicated `MCPServiceProxy` instance — zero cross-session data leakage
- **Built-in SecurityValidator**: Path traversal attacks and command injection blocked at the system level
- **No cloud substrate required**: Core execution happens locally; external connections are mainly optional cloud providers and remote MCP/HTTP services you choose to use, plus production update checks for new releases
- **Full offline support**: Pair with [Ollama](https://ollama.ai) for a completely air-gapped agent stack

#### What stays local vs. what leaves your machine

- **Always local**: workspaces, local files, bundled skills, session state, MCP server configs, browser state, and local tool execution
- **Leaves your machine when needed**: requests to cloud LLM providers or remote MCP/HTTP services you explicitly configure, plus production update checks for new releases
- **Fully offline mode**: use Ollama or another local runtime plus local MCP servers for an air-gapped workflow

### 2. 🧩 MCP-Native Ecosystem — Infinite Extensibility by Design

MCP (Model Context Protocol) is the open standard behind LibrAgent's extensibility model. LibrAgent treats it not as a feature — but as the architectural backbone:

- **Full transport support**: stdio, HTTP, SSE, and OAuth 2.1 — the complete spec
- **15+ built-in servers**: Planning, Knowledge, Browser Automation, Workspace, Shell Execution, Content Store, and more
- **Preset catalog**: Install GitHub, Brave Search, Filesystem, and other popular servers in one click
- **Session-isolated instances**: Each agent session has independent MCP server state — no interference between parallel agents
- **Import from anywhere**: Migrate MCP configs from Cursor, VS Code, Claude Code, or Windsurf automatically

### 3. 🦾 Production-Grade Execution Substrate

Most AI tools are impressive in demos and brittle in production. LibrAgent is obsessively engineered for long-running, real work:

| Substrate     | Capabilities                                                                                          |
| ------------- | ----------------------------------------------------------------------------------------------------- |
| **Workspace** | Line-precise editing, multi-file ops, unified search, `@file`/`@skill`/`@playbook` context injection  |
| **Shell**     | Isolated execution AND persistent shells — async process monitoring (`poll`, `read output`, `list`)   |
| **Browser**   | Headless browser automation with a Playwright-like interaction model and cache consistency guarantees |
| **Knowledge** | Graph-based knowledge management with entity/relation extraction (v2), BM25 full-text search          |

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

| Skill                | What it does                                                                              |
| -------------------- | ----------------------------------------------------------------------------------------- |
| `system-setup`       | Detects and installs missing runtimes (Python, Node.js, uv) across all platforms          |
| `mcp-installer`      | Registers MCP servers from npm packages, GitHub URLs, or JSON config blocks               |
| `mcp-importer`       | Imports existing MCP configs from Cursor, VS Code, Windsurf, and similar setups           |
| `specialist-creator` | Designs a full agent config (system prompt, model, tools) from a role description         |
| `crew-constructor`   | Scans available tools and batch-creates a matched specialist team automatically           |
| `agent-tooling`      | Audits agents, detects capability mismatches, and rebalances tool assignments dynamically |
| `delegate`           | Guides parent→child session handoff with explicit context transfer and lineage tracking   |
| `teamwork`           | Scaffolds the shared workspace constitution for coordinated multi-agent work              |
| `org`                | Formalizes durable org identity and org-visible member hierarchy                          |
| `schedule`           | Creates and manages recurring scheduled task groups for unattended automation             |
| `soul-awakening`     | Anchors an agent to a `SOUL.md` persona — tone, stance, identity                          |

And that's just the operator layer. LibrAgent also ships domain skills for:

- **knowledge and research**: `deep-research-report`, `knowledge-distiller`
- **document workflows**: `document-to-markdown`, `docx`, `pptx`
- **skill and workflow authoring**: `skill-creator`, `skill-deployer`, `playbook-creator`, `mcp-builder`
- **specialized operations**: `computer-diagnosis` and other focused helpers

> [!IMPORTANT]
> **`bootstrap` is a builtin capability** often used alongside these skills. Bundled Skills are the reusable procedures, while builtins and MCP tools provide the execution substrate underneath.

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

<!-- RELEASE_DOWNLOADS_START -->

- **Windows:** [`LibrAgent_0.8.6_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent_0.8.6_x64-setup.exe) · [`LibrAgent_0.8.6_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent_0.8.6_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.8.6_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent_0.8.6_aarch64.dmg)
- **Linux:** [`LibrAgent_0.8.6_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent_0.8.6_amd64.AppImage) · [`LibrAgent_0.8.6_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent_0.8.6_amd64.deb) · [`LibrAgent-0.8.6-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.6/LibrAgent-0.8.6-1.x86_64.rpm)
- **All release assets:** [Releases page](https://github.com/fritzprix/libr-agent/releases/tag/v0.8.6)
<!-- RELEASE_DOWNLOADS_END -->

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

## Where LibrAgent Fits Best

| If you want...                                               | LibrAgent is strong because...                                                               |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------------- |
| **A local AI workstation**                                   | Files, sessions, workspaces, and browser state stay on your machine by default               |
| **An MCP-native desktop product**                            | You can install, import, and manage MCP servers without treating the app like a thin wrapper |
| **Agents that do real work**                                 | Workspace, shell, browser, and knowledge tools are built for long-running execution          |
| **Multi-agent workflows without building a framework first** | `delegate`, `teamwork`, `org`, and `schedule` are already part of the product                |
| **A bridge between power-user depth and GUI usability**      | You get a desktop UI without giving up extensibility or control                              |

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
- 🐛 [Issue Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
- 💬 [Discussions](https://github.com/fritzprix/libr-agent/discussions)

**License**: MIT
