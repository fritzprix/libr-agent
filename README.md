<p align="center">
  <img src="./src-tauri/icons/128x128.png" width="96" alt="LibrAgent" />
</p>

# LibrAgent

> **A local agent operating environment. Select your model, install tools with one click, and choose a coordination pattern.**
> _You do not need a vendor harness or manual JSON configuration. The software saves completed work as files on your machine._

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

Many agent harnesses require you to edit JSON configuration files, use command-line terminals, and assemble code frameworks. Other products lock you into a single vendor stack.

LibrAgent provides a **desktop product** for these tasks:

| Traditional Setup | With LibrAgent |
| ----------------- | -------------- |
| Edit MCP configuration files manually | **Extensions** — One-click presets (GitHub, Brave Search, Filesystem) and import from Cursor, VS Code, Claude Code, and Windsurf |
| Assemble multi-agent systems from code | **Standard coordination patterns** as bundled skills (`pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`) |
| Restricted to one provider model and toolset | **Choose your LLM** (API keys or local [Ollama](https://ollama.com)) and your tool stack with MIT, local-first architecture |

[Download the latest release](https://github.com/fritzprix/libr-agent/releases/latest) · [5-minute onboarding](#the-5-minute-onboarding-path) · [Hero demo spec](docs/contributing/hero-demo-spec.md)

---

## What You Can Do in the First 10 Minutes

### 1. Install tools with one click, then generate a deliverable

- Open **Extensions** and install a preset (such as GitHub) without manual JSON editing.
- Point **Workspace** to a local directory on your machine.
- Submit your request: _"Review this repo for the top risk for a new contributor and save `DELIVERABLE.md`"_

### 2. Run an automated workflow recipe

- Launch the **Morning Briefing** recipe from Chat home or [Scheduled Tasks](docs/user/guides/scheduled-tasks.md).
- The recipe installs Hacker News and Yahoo Finance presets, configures an assistant, and schedules execution for 09:00 daily.
- Review your synthesized technology and market briefing automatically.

### 3. Select a coordination pattern without code assembly

- Describe your workflow, or attach a skill directly by name:
  - _"@skill:pipeline — research, draft, and review; generate one final report"_
  - _"@skill:divide-conquer — divide this task into independent steps and merge the results"_
- Coordination patterns are packaged skills that do not require external SDK setup. See [Sub-agents & orchestration](docs/user/guides/sub-agents.md).

### 4. Maintain control of your models

- Cloud: Paste an API key for OpenAI, Anthropic, Gemini, or Groq.
- Local: Run `ollama pull qwen3:14b` and select Ollama in settings to use the same desktop environment locally.

---

## Three product promises

1. **Direct user interface without manual configuration** — Graphical interface, one-click extensions, automated recipes, and in-app approvals. You do not need to edit configuration files or open a terminal.
2. **Orchestration as a built-in feature** — Select Sequential, Hub-and-spoke, or Swarm workflows through skills. Expand to `teamwork` and `schedule` when you need persistent teams or scheduled tasks.
3. **Model and stack freedom** — Connect to any supported LLM, use MCP as open infrastructure, and import IDE configurations. Workspaces and browser sessions remain local by default under an MIT license.

**Target users:** Operators who require agent capabilities without manual configuration; developers who avoid vendor lock-in; researchers who need browser, knowledge, and scheduled execution in one application.

---

## Coordination patterns (bundled skills)

Select a pattern that matches the structure of your task, then execute it from the chat interface:

| Skill | Pattern | When to use |
| ----- | ------- | ----------- |
| `pipeline` | Sequential stages | Output from each step supplies the next step (research → draft → review) |
| `hub-spoke` | Hub-and-spoke | One coordinator delegates tasks to multiple specialized workers |
| `divide-conquer` | Parallel split | Divide work into independent tasks, then merge the results |
| `consensus-delegation` | Multi-perspective | Send the same question to multiple specialists, then reconcile differences |
| `gatekeeper` / `pair-programming` | Review loops | Enforce strict code review or collaborative two-agent programming |
| `delegate` | Lightweight handoff | Run one child session with lineage tracking |
| `teamwork` → `org` | Persistent team | Create durable multi-agent teams with shared guidelines |
| `schedule` / `loop` / `call-me-back` | Time & events | Execute cron schedules, in-session delays, or resume on process events |

Selection heuristics: [framework-selection](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · Full guide: [Sub-agents](docs/user/guides/sub-agents.md).

Additional default skills: `setup-wizard`, `tool-installer`, `playbook-creator`, and more — **[Bundled Skills](docs/user/guides/skills.md)**.

---

## MCP platform (advanced capabilities)

- Transports: stdio, HTTP, SSE, OAuth 2.1
- 15+ built-in servers (Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks, and others)
- One-click presets and agent-assisted installation (`tool-installer`)
- Tool isolation per session, file path guards, command guards, and optional automated execution modes

### Execution substrate

| Substrate     | Capabilities                                                                                         |
| ------------- | ---------------------------------------------------------------------------------------------------- |
| **Workspace** | Line-precise editing, multi-file operations, and `@file` / `@skill` / `@playbook` context            |
| **Shell**     | Isolated and persistent terminal sessions with asynchronous process monitoring                       |
| **Browser**   | Isolated browser sidecar and persistent agent profile support                                        |
| **Knowledge** | Graph-based knowledge store and BM25 search                                                          |
| **Export**    | Markdown reports and ATIF trajectory exports ([session export](docs/user/guides/session-export.md))  |

Long-running sessions maintain stability through context compaction, loop prevention, circuit breakers, and stale-response protection.

---

## Real-World Scenarios

### Operator — Automated daily briefing

1. Run the **Morning Briefing** recipe (installs presets, sets up assistant, schedules execution at 09:00).
2. Click **Run now** to verify the configuration.
3. Receive the generated briefing report directly in the application without using a terminal.

### Solo developer — Direct tool installation without configuration files

1. Open Extensions and install the GitHub MCP preset.
2. Connect a local repository using the Workspace selector.
3. Prompt the agent to generate a Markdown security and code review report saved to disk.

### Non-framework power user — Standard orchestration patterns

1. Enter `@skill:pipeline` (or `hub-spoke` / `divide-conquer`) to specify workflow structure.
2. Agents coordinate according to the selected pattern.
3. Review the consolidated deliverable in your workspace without maintaining external framework code.

### Privacy-sensitive team — Local models with zero cloud transmission

1. Run `ollama pull qwen3:14b`.
2. Keep Workspace and Shell interactions strictly on your local machine.
3. Add cloud API keys later if necessary; the application workflow remains unchanged.

---

## Documentation

- **[User Guide](docs/user/README.md)** — Installation, initial setup, models, and skills ([documentation website](https://fritzprix.github.io/libr-agent/))
- **[Hero Demo Spec](docs/contributing/hero-demo-spec.md)** — Canonical product demonstration specification (EN/KO/ZH subtitles)
- **[Product Messaging Guide](docs/contributing/product-messaging-guide.md)** — Product positioning and documentation style
- **[Recipes](docs/user/guides/recipes.md)** · **[Scheduled Tasks](docs/user/guides/scheduled-tasks.md)** · **[Sub-agents](docs/user/guides/sub-agents.md)** · **[Skills](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — Remote control interface and programmatic approvals
- **[Architecture](docs/architecture/agent-workflow-architecture.md)** — Session isolation and Think-Act-Observe cycle

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

- Cloud: Paste an OpenAI, Anthropic, Gemini, or Groq API key.
- Local: Run `ollama pull qwen3:14b`, then select Ollama in Settings.

**Step 2 — Add tools without manual configuration**

- Open Extensions and install a preset (such as GitHub), **or**
- Instruct the agent: _"Import my MCP servers from Cursor"_

**Step 3 — Attach a workspace and generate files**

- Select a local project directory in Workspace.
- Instruct the agent: _"Review this workspace, then write findings to `DELIVERABLE.md`."_

**Next — Coordination and automation**

- _"@skill:pipeline — research, draft, then review; save one final report."_
- _"Prepare a teamwork workspace for this repo."_
- _"Set up a scheduled daily competitor brief at 7am."_ (or launch the Morning Briefing recipe)

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

| If you want... | LibrAgent is suitable because... |
| -------------- | -------------------------------- |
| **Comprehensive capabilities without manual setup** | Extensions presets, recipes, `@skill:` patterns, and UI approvals replace JSON-first configuration |
| **Orchestration without framework assembly** | Standard patterns (`pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, `teamwork`, `schedule`) are built into the product |
| **Independence from proprietary agent stacks** | Use your choice of model and MCP tools under an MIT, local-first architecture |
| **Dedicated execution substrate** | Local workspace, shell, browser, knowledge store, playbooks, and session safety guards |
| **Native MCP desktop environment** | Direct presets, configuration import, and 15+ built-in servers |

---

## Design Philosophy

- **Product over kit**: The runtime is ready for immediate use without manual code assembly.
- **Orchestration as skills**: Coordination patterns are named, selectable, and documented.
- **Independence of stack**: Models and tools remain user choices; the software does not depend on a single AI vendor.
- **Local First**: Workspaces, sessions, skills, and browser states remain on your machine. Cloud LLM and remote MCP connections occur only when explicitly enabled.
- **Harness over Model**: Tools, session state, delegation, and governance provide more value than any individual model.
- **Stability over Features**: Isolation, context compaction, and loop prevention take precedence over feature quantity.
- **Open Standards**: MIT license with MCP as the foundational interoperability layer.

---

## Contributing & License

LibrAgent is MIT licensed and built in the open. Contributions are welcome, including bundled skills, MCP integrations, bug fixes, and architectural improvements.

- [Contributing Guide](CONTRIBUTING.md)
- [Issue Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- [Discussions](https://github.com/fritzprix/libr-agent/discussions)
- Benchmarks (Harbor / Terminal-Bench): See [Harbor guide](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`)

**License**: MIT
