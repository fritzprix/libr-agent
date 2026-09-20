---
title: Navigation Guide
---

# UI Navigation Guide

This guide introduces the desktop application layout, primary sidebar items, and workspace navigation in LibrAgent.

---

## 🧭 Primary Sidebar Layout

Access all core capabilities of LibrAgent from the primary left sidebar:

```
┌──────────────────┬────────────────────────────────────────────────────────┐
│  [📜 History]    │ Session History: Chronological chat archive & search   │
│  [🔖 Bookmarked] │ Bookmarked: Quickly access pinned important sessions   │
│  [🤖 Chat]       │ Agent Workspace: Real-time chat & live tool execution  │
├──────────────────┼────────────────────────────────────────────────────────┤
│  [🧠 Knowledge]  │ Knowledge Base: Semantic memory & visual graph viewer  │
│  [👥 Assistants] │ Custom Assistants: Specialized instructions & models   │
│  [📋 Playbooks]  │ Playbooks: Reusable verified workflow templates        │
│  [🧩 Extensions] │ Extensions: MCP servers & skills management            │
│  [🏢 Org]        │ Org: Coordinated multi-agent teams and hierarchy       │
│  [⏱️ Scheduled]  │ Scheduled Tasks: Recurring background Cron automations │
├──────────────────┼────────────────────────────────────────────────────────┤
│  [⚙️ Settings]   │ Settings: Configure AI API keys, themes, and models    │
└──────────────────┴────────────────────────────────────────────────────────┘
```

> [!NOTE]
> **Where are Solution Recipes?**  
> Recipes are not a sidebar page. They are interactive walkthrough dialogs accessible from the **featured card on the Chat home screen ("Create Your Morning Briefing Assistant in 5 Minutes")** or from the walkthrough button in **Scheduled Tasks**.

---

## 🤖 1. Main Workspaces

### History & Bookmarked

- Browse past sessions chronologically or search by keywords.
- Pin critical sessions with the bookmark icon for instant retrieval.

### Agent Workspace (Chat)

- Converse directly with AI agents to delegate coding, research, and automation work.
- Live cards display agent thinking steps, file modifications, browser actions, and terminal executions.

---

## 📚 2. Library & Management

### Knowledge Base

- Manage long-term memories and structured entities accumulated across sessions.
- Explore relationships interactively via the visual knowledge graph.

### Assistants

- Create dedicated agent profiles for coding, research, or writing.
- Configure specific AI providers, system prompts, and tool permissions per profile.

### Playbooks

- Re-run proven multi-step prompt sequences stored as reusable templates.

### Extensions

- Configure Model Context Protocol (MCP) tool servers and skills.
- Use the **Tools** tab for MCP servers and the **Skills** tab for skills management. _(Lives under Extensions, not a Settings tab)._

### Org

- Inspect formal multi-agent task forces and member lineage initialized via `teamwork` or `org`.

### Scheduled Tasks

- Set up unattended Cron jobs so agents execute recurring tasks (e.g. daily briefings or health checks) in the background.

---

## ⚙️ 3. Settings

- **AI & Models**: Configure provider API keys and select default models.
- **General**: Toggle dark/light appearance and UI language.
- **Security & Updates**: Verify local session isolation and check for software updates.
