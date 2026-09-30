# 🤖 LibrAgent

> **A local-first desktop app for AI agents that use real tools, run in parallel, and stay under your control.**
> _Connect any LLM, add any MCP server, and let agents read files, run shells, browse the web, and finish work on your machine._

[한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

---

## 🎬 Execution story

**LibrAgent is not a chat app. It is an execution environment for agents.**

**One session. One goal. One deliverable on your machine.**

1. Connect a model (API key or [Ollama](https://ollama.com))
2. Point **Workspace** at a real project folder
3. Ask the agent to **read, run, and leave a file you keep** — not a suggestion stuck in a bubble

[Download the latest release](https://github.com/fritzprix/libr-agent/releases/latest) · [5-minute onboarding](#the-5-minute-onboarding-path) · [Hero demo spec](docs/contributing/hero-demo-spec.md)

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

### 3. Deploy a one-click workflow recipe

- Launch the **Morning Briefing** recipe from Chat home or [Scheduled Tasks](docs/user/guides/scheduled-tasks.md)
- Installs Hacker News + Yahoo Finance presets, configures an assistant, and schedules a daily 9 AM run
- Wake up to a synthesized tech & market briefing — unattended

### 4. Turn research into a repeatable workflow

- Add Browser + Knowledge
- Ask: _"Track these 5 competitor blogs and give me a summary every morning"_
- Convert a one-off task into a scheduled pipeline with [Scheduled Tasks](docs/user/guides/scheduled-tasks.md)

---

## Why LibrAgent?

Most agent products force a tradeoff: easy UI with weak execution, strong automation without a product, cloud convenience with weak privacy, or a flexible framework you assemble yourself.

LibrAgent is built for the middle people actually want:

- **Local-first control** for files, workspaces, sessions, and browser state
- **Open extensibility** through MCP instead of a closed plugin story
- **Real execution** across shell, browser, workspace, and knowledge tools
- **A GUI** that does not give up power-user depth
- **A path from one agent to many** when a single assistant stops being enough

**Best fit:** solo developers, power users/operators, researchers/analysts, and privacy-sensitive teams who need durable local execution.

---

## Why It Holds Up After the Demo

### Local-first security

- **Session isolation**: each agent session gets its own tool runtime — no cross-session leakage
- **Path and command guards**: traversal and injection attempts blocked at the system boundary
- **Core work stays local**; cloud LLM / remote MCP only when you configure them (plus optional update checks)
- **Fully offline**: pair with [Ollama](https://ollama.com) and local MCP servers

| Always local | Leaves only when you choose |
| ------------ | --------------------------- |
| Workspaces, files, skills, session state, browser state, local tools | Cloud LLM providers, remote MCP/HTTP, release update checks |

### MCP-native platform

- Transports: stdio, HTTP, SSE, OAuth 2.1
- 15+ built-in servers (Planning, Knowledge, Browser, Workspace, Shell, Content Store, …)
- One-click presets (GitHub, Brave Search, Filesystem, …)
- Import MCP configs from Cursor, VS Code, Claude Code, or Windsurf

### Execution substrate

| Substrate     | Capabilities                                                                                         |
| ------------- | ---------------------------------------------------------------------------------------------------- |
| **Workspace** | Line-precise editing, multi-file ops, `@file` / `@skill` / `@playbook` context                       |
| **Shell**     | Isolated and persistent shells with async process monitoring                                         |
| **Browser**   | Isolated browser sidecar (Playwright-style interaction)                                              |
| **Knowledge** | Graph knowledge + BM25 search                                                                        |
| **Export**    | Markdown reports and ATIF trajectory exports ([session export](docs/user/guides/session-export.md)) |

Long sessions stay productive via context compaction, loop prevention, circuit breakers, and stale-response guards.

### Day-one skills

Reusable operating procedures any agent can invoke by name:

| Skill            | What it does                                                              |
| ---------------- | ------------------------------------------------------------------------- |
| `setup-wizard`   | Detects and installs missing runtimes (Python, Node.js, uv)               |
| `tool-installer` | Registers MCP servers or imports configs from Cursor / VS Code / Windsurf |
| `schedule`       | Creates recurring scheduled task groups                                   |
| `delegate`       | Parent → child session handoff with lineage tracking                      |
| `teamwork`       | Scaffolds a shared multi-agent workspace constitution                     |

Full catalog: **[Bundled Skills guide](docs/user/guides/skills.md)**.

---

## When One Agent Is Not Enough

After the first deliverable works, scale without assembling a framework:

- **`delegate`** — spawn and monitor child sessions
- **`teamwork`** — shared workspace (`agents.md`, `MISSION.md`, `KANBAN.md`)
- **`org`** — durable team identity and hierarchy
- **`schedule`** — CRON automation with workspace constitution

Concurrency limits keep parallel sessions and shells from runaway cost.

---

## Real-World Scenarios

### Solo developer — automated code review

1. Connect a local repo via Workspace
2. Install the GitHub MCP preset
3. Ask: _"Find security issues in PR #42 and produce a Markdown report"_
4. Agent reads, analyzes, and leaves a report you keep

### Marketer — competitive intelligence on autopilot

1. Point Browser at competitor blogs
2. Ask: _"Create a scheduled competitor brief every morning at 7am"_
3. Agent browses, summarizes, and stores findings in Knowledge
4. Ask anytime: _"Summarize last week's competitor moves"_

### Engineering team — offline agent stack

1. `ollama pull qwen3:14b`
2. Connect Workspace + Shell
3. Sensitive code stays on the machine
4. Agents read, modify, test, and commit — fully local

### Power user — multi-agent research pipeline

1. `teamwork` scaffolds roles and shared docs
2. `delegate` fans out parallel research
3. Results merge into one report in Content Store
4. `schedule` runs the workflow weekly

---

## Documentation

- **[User Guide](docs/user/README.md)** — install, first chat, models, skills ([docs site](https://fritzprix.github.io/libr-agent/))
- **[Hero Demo Spec](docs/contributing/hero-demo-spec.md)** — canonical 60s product story (EN/KO/ZH subtitles)
- **[Recipes](docs/user/guides/recipes.md)** · **[Scheduled Tasks](docs/user/guides/scheduled-tasks.md)** · **[Browser](docs/user/guides/browser-sidecar.md)** · **[Skills](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — remote control and programmatic approvals
- **[Architecture](docs/architecture/agent-workflow-architecture.md)** — session isolation and Think-Act-Observe

---

## Getting Started

Download the latest installer from the **[Releases page](https://github.com/fritzprix/libr-agent/releases/latest)**.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.21_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64-setup.exe) · [`LibrAgent_0.9.21_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.21_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.21_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.AppImage) · [`LibrAgent_0.9.21_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.deb) · [`LibrAgent-0.9.21-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent-0.9.21-1.x86_64.rpm)
- **All release assets:** [Releases page](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.21)
<!-- RELEASE_DOWNLOADS_END -->

### The 5-minute onboarding path

**Step 1 — Connect a model** (Settings → LLM Providers)

- Cloud: paste an OpenAI / Anthropic / Gemini / Groq API key
- Local: `ollama pull qwen3:14b`, then select Ollama in Settings

**Step 2 — Attach a workspace and tools**

- Point Workspace at a real project folder
- Optional: install a preset (e.g. GitHub) from Extensions, or tell an agent _"Import my MCP servers from Cursor"_

**Step 3 — Ask for a deliverable**

- _"Review this repo for the top risk for a new contributor and save `DELIVERABLE.md` in the workspace."_
- Prefer outcomes you can open on disk — not chat-only suggestions

**Next (when you are ready for more than one agent)**

- _"Delegate repository analysis to a child session and bring me back a summary."_
- _"Prepare a teamwork workspace for this repo."_
- _"Set up a scheduled daily competitor brief at 7am."_

### First prompts to copy-paste

- _"Import my MCP servers from Cursor and show me what was added."_
- _"Install the GitHub MCP preset and attach it to a coding agent."_
- _"Review this workspace, then write findings to `DELIVERABLE.md`."_
- _"Create a researcher agent for competitive intelligence using my current tools."_
- _"Set up a scheduled daily competitor brief at 7am."_

### Developer setup

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

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

- **Local First**: Your data, keys, and agent personas stay under your control. No cloud substrate required.
- **Harness over Model**: Tools, session state, delegation, and governance matter more than any single model.
- **Stability over Features**: Runtime correctness — isolation, compaction, loop prevention — before feature chase.
- **MCP as Infrastructure**: The tool ecosystem is organized around MCP as the interoperability layer.
- **Open Standards**: MIT licensed. Committed to MCP, open-source interoperability, and user data sovereignty.

---

## Contributing & License

LibrAgent is MIT licensed and built in the open. Contributions are welcome — bundled skills, MCP integrations, bug fixes, or architecture improvements.

- 📖 [Contributing Guide](CONTRIBUTING.md)
- 🐛 [Issue Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
- 💬 [Discussions](https://github.com/fritzprix/libr-agent/discussions)
- 🧪 Benchmarks (Harbor / Terminal-Bench): see [Harbor guide](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`, …)

**License**: MIT
