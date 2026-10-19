---
title: LibrAgent — Local AI Agent Operating Environment
description: 'LibrAgent (libr-agent) is a local agent operating environment — pick your model, install MCP tools like apps, and run multi-agent coordination patterns.'
---

# LibrAgent User Documentation

> **A local agent operating environment. Select your model, install tools with one click, and choose a coordination pattern.**
> _No vendor harness or manual JSON configuration required — completed work is saved as files on your machine._

Language: [한국어](/) · **English** · [**Download Latest Release (Windows / macOS / Linux)**](https://github.com/fritzprix/libr-agent/releases/latest) · [GitHub Repository](https://github.com/fritzprix/libr-agent)

---

## Why LibrAgent?

- **Direct desktop UI without manual setup**: Install one-click MCP presets in **Extensions** or import configurations from Cursor, VS Code, Claude Code, and Windsurf.
- **Built-in multi-agent orchestration**: Run standard coordination patterns (`pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, `teamwork`, `schedule`) as bundled skills without assembling framework code.
- **Model and stack freedom (Local-First, MIT)**: Connect OpenAI, Anthropic, Gemini, Groq, or local **Ollama** models while keeping your workspace, shell, and browser sessions on your machine.

---

## Get started

- [**5-minute tutorial**](getting-started/5-minute-tutorial.md) — Install, **AI & Models**, Chat, **App Wizard**
- [**First agent chat**](getting-started/first-agent.md)
- [**Connecting models**](getting-started/connecting-models.md)

## Guides

- [**Assistants**](guides/assistants.md) — Custom agents (General / Tools / Skills)
- [**Playbooks**](guides/playbooks.md) — Re-run successful workflows
- [**Solution Recipes**](guides/recipes.md) — One-click workflow bundles
- [**Scheduled Tasks**](guides/scheduled-tasks.md) · [**Automation Overview**](guides/automation.md)
- [**Sessions**](guides/sessions.md) · [**Session Export**](guides/session-export.md)
- [**Skills**](guides/skills.md) · [**Sub-agents**](guides/sub-agents.md)
- [**Extensions (MCP Tools)**](guides/extensions.md) · [**Custom MCP**](guides/custom-mcp.md)
- [**Browser automation**](guides/browser-sidecar.md) (built-in browser · [your Chrome extension](https://chromewebstore.google.com/detail/libragent-browser-bridge/eponcjengahlpkgbaolhemhmckopmfbe)) · [**Built-in tools**](guides/builtin-tools.md)
- [**Navigation Guide**](guides/navigation-guide.md) · [**Troubleshooting**](guides/troubleshooting.md)

## FAQ · scenarios

- [**Common questions**](faq/common-questions.md) · [**Error codes**](faq/error-codes.md)
- [Automation](scenarios/automation.md) · [Code review](scenarios/code-review.md) · [Research](scenarios/research.md) · [Files](scenarios/file-management.md) · [Web](scenarios/web-browsing.md)

### Onboarding in one line

1. **Settings → AI & Models → Provider API Keys** → **Save Changes**
2. Start a session from an assistant card in **Chat**
3. MCP lives under **Extensions**
4. Repeat work with **Playbooks** / **Scheduled Tasks**

App UI language follows **Settings → General → Language**.

---

## Tips

- Settings tab name: **AI & Models**. MCP: sidebar **Extensions** (not a Settings tab).
- Contributors: [getting-started-dev.md](https://github.com/fritzprix/libr-agent/blob/main/docs/guides/getting-started-dev.md)
- Questions: [Discussions](https://github.com/fritzprix/libr-agent/discussions)
