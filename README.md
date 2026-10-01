# 🤖 LibrAgent

> **An agent operating environment you run — pick the model, one-click the tools, pick the coordination pattern.**
> _No vendor harness. No JSON homework. Work finishes as files on your machine._

[한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![Hero demo — Install tools like apps. Keep your model. Keep the file.](./assets/hero-demo-60s.gif)

[HD WebM](./assets/hero-demo-60s.webm) · _Install tools like apps. Keep your model. Keep the file._

---

## How LibrAgent is different

Most agent harnesses assume you can edit MCP JSON, live in a terminal, and assemble orchestration in code (or lock you to one vendor’s stack).

LibrAgent is a **desktop product** for the same job:

| Instead of… | You get… |
| ----------- | -------- |
| Hand-editing MCP configs | **Extensions** — one-click presets (GitHub, Brave Search, Filesystem, …) and import from Cursor / VS Code / Claude Code / Windsurf |
| “We have multi-agent” | **Named coordination patterns** as bundled skills — `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, … |
| One provider’s model + tools | **Your** LLM (API key or [Ollama](https://ollama.com)) and **your** MCP stack — MIT, local-first |

[Download the latest release](https://github.com/fritzprix/libr-agent/releases/latest) · [5-minute onboarding](#the-5-minute-onboarding-path) · [Hero demo spec](docs/contributing/hero-demo-spec.md)

---

## What You Can Do in the First 10 Minutes

### 1. One-click tools, then a deliverable

- Open **Extensions** and install a preset (e.g. GitHub) — no JSON
- Point **Workspace** at a real folder
- Ask: _"Review this repo for the top risk for a new contributor and save `DELIVERABLE.md`"_

### 2. Deploy a one-click workflow recipe

- Launch the **Morning Briefing** recipe from Chat home or [Scheduled Tasks](docs/user/guides/scheduled-tasks.md)
- Installs Hacker News + Yahoo Finance presets, configures an assistant, and schedules a daily 9 AM run
- Wake up to a synthesized tech & market briefing — unattended

### 3. Pick a coordination pattern (no framework assembly)

- Say what the work looks like, or attach a skill by name:
  - _"@skill:pipeline — research, then draft, then review; leave one final report"_
  - _"@skill:divide-conquer — split this into independent pieces and merge the results"_
- Patterns are productized skills — not an SDK you wire yourself. See [Sub-agents & orchestration](docs/user/guides/sub-agents.md).

### 4. Keep your model freedom

- Cloud: paste an OpenAI / Anthropic / Gemini / Groq API key
- Local: `ollama pull qwen3:14b` and select Ollama — same harness either way

---

## Three product promises

1. **Surface without harness homework** — GUI, Extensions one-click, recipes, in-app approvals, `@skill:` — not “open a config and a shell first.”
2. **Orchestration as product** — choose Sequential / Hub-and-spoke / Swarm-style flows via skills; grow into `teamwork` → `org` and `schedule` when you need durable teams or cron — still without assembling LangGraph/CrewAI yourself.
3. **Provider & stack freedom** — any supported LLM, MCP as infrastructure, import existing IDE MCP configs, MIT license, local workspaces and browser state by default.

**Best fit:** operators and power users who want harness depth without living in JSON; developers who refuse a single-vendor agent stack; researchers who need browser + knowledge + schedules in one product.

---

## Coordination patterns (bundled skills)

Choose the model from the **shape of the work**, then run it from chat:

| Skill | Pattern | When to use |
| ----- | ------- | ----------- |
| `pipeline` | Sequential stages | Outputs feed the next step (research → draft → review) |
| `hub-spoke` | Hub-and-spoke | One coordinator integrates many workers |
| `divide-conquer` | Parallel split | Independent pieces, then merge |
| `consensus-delegation` | Multi-perspective | Same question to several specialists, then reconcile |
| `gatekeeper` / `pair-programming` | Review loops | Strict review or two-agent coding |
| `delegate` | Lightweight handoff | One child session, lineage tracked |
| `teamwork` → `org` | Durable team | Shared constitution + Org UI |
| `schedule` / `loop` / `call-me-back` | Time & events | Cron, in-session delays, or resume on process/webhook |

Selection heuristics: [framework-selection](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · full guide: [Sub-agents](docs/user/guides/sub-agents.md).

Other day-one skills: `setup-wizard`, `tool-installer`, `playbook-creator`, and more — **[Bundled Skills](docs/user/guides/skills.md)**.

---

## MCP platform (still power-user capable)

- Transports: stdio, HTTP, SSE, OAuth 2.1
- 15+ built-in servers (Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks, …)
- One-click presets + agent-assisted install (`tool-installer`)
- Per-session tool isolation; path/command guards; optional YOLO / unsafe modes for automation

### Execution substrate

| Substrate     | Capabilities                                                                                         |
| ------------- | ---------------------------------------------------------------------------------------------------- |
| **Workspace** | Line-precise editing, multi-file ops, `@file` / `@skill` / `@playbook` context                       |
| **Shell**     | Isolated and persistent shells with async process monitoring                                         |
| **Browser**   | Isolated browser sidecar; optional saved-login profiles                                              |
| **Knowledge** | Graph knowledge + BM25 search                                                                        |
| **Export**    | Markdown reports and ATIF trajectory exports ([session export](docs/user/guides/session-export.md)) |

Long sessions stay productive via context compaction, loop prevention, circuit breakers, and stale-response guards.

---

## Real-World Scenarios

### Operator — from empty app to daily briefing

1. Run the **Morning Briefing** recipe (presets + assistant + 9 AM schedule)
2. Click **Run now** once to verify
3. Leave it — the report lands without opening a terminal

### Solo developer — preset, not config files

1. Extensions → install the GitHub MCP preset
2. Attach a local repo via Workspace
3. Ask for a Markdown security/review report you keep on disk

### Non-framework power user — named orchestration

1. `@skill:pipeline` (or `hub-spoke` / `divide-conquer`) for the work shape
2. Agents coordinate under that pattern
3. One merged deliverable in the workspace — no orchestration library to maintain

### Privacy-sensitive team — same product, local model

1. `ollama pull qwen3:14b`
2. Workspace + Shell stay on the machine
3. Swap cloud keys later if you want — the harness does not change vendors for you

---

## Documentation

- **[User Guide](docs/user/README.md)** — install, first chat, models, skills ([docs site](https://fritzprix.github.io/libr-agent/))
- **[Hero Demo Spec](docs/contributing/hero-demo-spec.md)** — canonical product demo (EN/KO/ZH subtitles)
- **[Product Messaging Guide](docs/contributing/product-messaging-guide.md)** — positioning and copy
- **[Recipes](docs/user/guides/recipes.md)** · **[Scheduled Tasks](docs/user/guides/scheduled-tasks.md)** · **[Sub-agents](docs/user/guides/sub-agents.md)** · **[Skills](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — remote control and programmatic approvals
- **[Architecture](docs/architecture/agent-workflow-architecture.md)** — session isolation and Think-Act-Observe

---

## Getting Started

Download the latest installer from the **[Releases page](https://github.com/fritzprix/libr-agent/releases/latest)**.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.22_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64-setup.exe) · [`LibrAgent_0.9.22_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.22_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.22_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.AppImage) · [`LibrAgent_0.9.22_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.deb) · [`LibrAgent-0.9.22-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent-0.9.22-1.x86_64.rpm)
- **All release assets:** [Releases page](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.22)
<!-- RELEASE_DOWNLOADS_END -->

### The 5-minute onboarding path

**Step 1 — Connect a model** (Settings → LLM Providers)

- Cloud: paste an OpenAI / Anthropic / Gemini / Groq API key
- Local: `ollama pull qwen3:14b`, then select Ollama in Settings

**Step 2 — Add tools without JSON**

- Extensions → install a preset (e.g. GitHub), **or**
- Tell an agent: _"Import my MCP servers from Cursor"_

**Step 3 — Attach a workspace and ask for a file you keep**

- Point Workspace at a real project folder
- _"Review this workspace, then write findings to `DELIVERABLE.md`."_

**Next — coordination and automation**

- _"@skill:pipeline — research, draft, then review; one final report."_
- _"Prepare a teamwork workspace for this repo."_
- _"Set up a scheduled daily competitor brief at 7am."_ (or run the Morning Briefing recipe)

### First prompts to copy-paste

- _"Import my MCP servers from Cursor and show me what was added."_
- _"Install the GitHub MCP preset and attach it to a coding agent."_
- _"Review this workspace, then write findings to `DELIVERABLE.md`."_
- _"@skill:pipeline — research this topic, draft a summary, then review; save the final report."_
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

| If you want... | LibrAgent is strong because... |
| -------------- | ------------------------------ |
| **Harness depth without harness homework** | Extensions presets, recipes, `@skill:` patterns, and approvals — not JSON-first onboarding |
| **Orchestration without building a framework** | `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, `teamwork` / `org`, `schedule` ship as product |
| **Freedom from a vendor agent stack** | Bring your model and MCP tools; MIT; local-first by default |
| **A real execution substrate** | Workspace, shell, browser, knowledge, playbooks, and long-running session guards |
| **An MCP-native desktop product** | Presets, import, and 15+ builtins — not a thin chat wrapper |

---

## Design Philosophy

- **Product over kit**: The harness is usable without assembling it.
- **Orchestration as skills**: Coordination patterns are named, selectable, and documented — not buried in sample repos.
- **Freedom of stack**: Models and tools are user choices; the product does not require one AI vendor.
- **Local First**: Workspaces, sessions, skills, and browser state stay under your control. Cloud LLM / remote MCP only when you opt in.
- **Harness over Model**: Tools, session state, delegation, and governance matter more than any single model.
- **Stability over Features**: Isolation, compaction, loop prevention — before feature chase.
- **Open Standards**: MIT. MCP as the interoperability layer.

---

## Contributing & License

LibrAgent is MIT licensed and built in the open. Contributions are welcome — bundled skills, MCP integrations, bug fixes, or architecture improvements.

- 📖 [Contributing Guide](CONTRIBUTING.md)
- 🐛 [Issue Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- 💬 [Discussions](https://github.com/fritzprix/libr-agent/discussions)
- 🧪 Benchmarks (Harbor / Terminal-Bench): see [Harbor guide](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`, …)

**License**: MIT
