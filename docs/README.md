# LibrAgent Documentation

Welcome to the official documentation for LibrAgent.

This documentation is divided into the following sections:

## User Documentation

End-user guides (no development environment required):

- **[User docs index](./user/README.md)**
  - Getting started: [5-min](./user/getting-started/5-minute-tutorial.md) · [first agent](./user/getting-started/first-agent.md) · [models](./user/getting-started/connecting-models.md)
  - Guides: [Assistants](./user/guides/assistants.md) · [Playbooks](./user/guides/playbooks.md) · [Automation](./user/guides/automation.md) · [Sessions](./user/guides/sessions.md) · [Skills](./user/guides/skills.md) · [Sub-agents](./user/guides/sub-agents.md) · [Extensions](./user/guides/extensions.md) · [Custom MCP](./user/guides/custom-mcp.md) · [Troubleshooting](./user/guides/troubleshooting.md)
  - [FAQ](./user/faq/common-questions.md) · [Error codes](./user/faq/error-codes.md)

Published site (GitHub Pages): https://fritzprix.github.io/libr-agent/ — enable **Settings → Pages → Source: GitHub Actions** if 404. See [`website/README.md`](../website/README.md).

LLM provider setup: [llm-services/provider-setup.md](./llm-services/provider-setup.md) (SDK copies in `llm-services/_archive/`).

## Developer Documentation

- **[Tauri API Reference](./api/tauri-commands.md)**: A detailed reference for all Tauri commands and data types.
- **[HTTP API Reference](./api/http_api.md)**: HTTP API documentation for remote management of AI agents and sessions.
- **[Guides](./guides/getting-started-dev.md)**: Developer environment setup (`getting-started.md` redirects here for contributors).
- **[Navigation (dev)](./guides/navigation-guide-dev.md)**: UI routes mapped to source.
- **[Troubleshooting (dev)](./guides/troubleshooting-dev.md)**: WebKit, build, MCP process debugging.
- **[Assistant System Prompt Guide](./guides/system-prompt-guide.md)**: Guidelines for writing robust and effective system prompts.
- **[Architecture](./architecture/agent-workflow-architecture.md)**: An overview of the system architecture, data flow, and security considerations.
- **[Gemini Request Caching Implementation](./architecture/gemini-caching-implementation.md)**: How Gemini request shaping, explicit cached-content reuse, and cache lifecycle management work.
- **[Session Lineage & Tree UI](./architecture/session-lineage-and-tree-ui.md)**: Design and implementation status for nested sessions, `session_api` MCP integration, and tree-based session UX.
- **[Agent Vibe Charter](./architecture/agent-vibe-charter.md)**: The operating personality and decision rules for this workspace's agent behavior.
- **[AI Soul Manifesto](./architecture/ai-soul-manifesto.md)**: Autonomy-first operating doctrine, mission rituals, and recovery principles for agent teams.
- **[Soul Lounge Recovery Loop (Experimental)](./architecture/soul-lounge-recovery-loop.md)**: Server-driven loop detection, recovery pacing, re-entry anchors, and one-time override policy.
- **[Open Source Launch Manifesto](./architecture/open-source-launch-manifesto.md)**: Public-facing engineering ethos, quality bar, and collaboration contract for contributors.
- **[Contributing](./contributing/coding-standards.md)**: Guidelines for contributing to the project, including coding standards, testing, and the release process.
- **[Product Messaging Guide](./contributing/product-messaging-guide.md)**: PR, launch, and positioning guidance for describing LibrAgent clearly and persuasively.
- **[Launch Finale Playbook](./contributing/open-source-launch-finale.md)**: Final pre-release and launch-day execution runbook.
- **[GitHub Release Notes Template](./contributing/github-release-notes-template.md)**: Copy-ready release note structure for public releases.

- **[Sprints](./sprints/README.md)**: Archived sprint logs and release notes.
