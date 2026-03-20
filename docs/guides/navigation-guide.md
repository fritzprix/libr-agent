# Navigation Guide

Welcome to the LibrAgent Navigation Guide. This document maps the application's internal structure (UI routes) to the features you use every day, helping you understand how to move through the workspace.

LibrAgent is a single-page application built on React, but it organizes features into distinct "routes" just like a traditional website. This structure allows you to focus on one specific aspect of your agent workflow at a time.

---

## 🤖 The Agent Workspace

The core of LibrAgent revolves around interacting with your AI assistants.

### `/agent`

**The Primary Chat Interface**
This is your main workspace. When you launch LibrAgent, you land here. This view provides a real-time chat interface where you can submit prompts, observe the agent's thought process, and watch it execute tools (like browsing the web or running terminal commands).

### `/agent/draft`

**The Sandbox**
Think of this as a scratchpad. If you want to configure a new agent interaction—perhaps selecting a specific model or attaching a specialized playbook—before committing it to a persistent session, you do it here. Once you send your first message, this draft converts into an active session.

### `/agent/:sessionId`

**Persistent Sessions**
LibrAgent is designed to remember context. Every time you start a conversation, it generates a unique `sessionId`. You can safely navigate away from this route and return later; the agent's memory, terminal state, and browser context remain exactly as you left them.

---

## 👥 Management & Configuration

To make your agents truly powerful, you need to manage their behaviors and capabilities.

### `/assistants`

**Assistant Profiles**
Not all tasks require the same approach. This route allows you to create and manage custom "Assistants." You can define unique system prompts, choose specific AI models (like Claude or GPT-4), and enable different toolsets for each profile. For example, you might have one Assistant specialized in Python debugging and another focused on creative writing.

### `/playbooks`

**Workflow Templates**
Playbooks are reusable behavior snippets and structured workflow templates. If you find yourself asking the agent to perform the same sequence of actions repeatedly, you can create a Playbook. This route lets you organize these templates, making them easy to deploy in future sessions.

### `/history` & `/history/search`

**Your Session Archive**
Because LibrAgent sessions are persistent, you need a way to find past work. The `/history` route provides a chronological view of all your past agent interactions. The `/history/search` sub-route allows you to quickly locate specific conversations based on keywords, ensuring you never lose valuable context.

---

## ⚙️ System Settings & Integrations

These routes control the underlying mechanics of LibrAgent.

### `/settings`

**Application Configuration**
This is the control center. Here, you configure API keys for your preferred LLM providers (Anthropic, OpenAI, Gemini), adjust UI preferences (like dark mode), and manage system-wide settings.

### `/mcp-servers`

**Extensions (Model Context Protocol)**
LibrAgent supports the open Model Context Protocol (MCP). This route is where you connect LibrAgent to external tools and data sources. Whether you are attaching a local filesystem server or a custom database connector, you manage those integrations here, expanding your agent's capabilities.

### `/scheduled-tasks`

**Automation**
Why run tasks manually when you can automate them? This route allows you to set up cron-based schedules for your agents. You can configure a specific Assistant to run a Playbook at a designated time, turning LibrAgent into a background automation engine.
