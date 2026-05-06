# Product Messaging Guide

This document is a positioning and PR messaging guide for LibrAgent. The goal is not to list features mechanically, but to explain why the product matters and how to describe it persuasively without drifting away from what the codebase actually supports.

---

## 1. Core positioning

**LibrAgent is not just an app that connects to a good model. It is a local-first agent harness and an MCP-native agent operating environment.**

In practical terms:

- it goes beyond a chat UI,
- it connects to external tools through MCP,
- it combines workspace, browser, shell, knowledge, and skills in one runtime,
- and it can grow from a single agent into delegated, team-oriented, and schedule-driven coordination.

---

## 2. The manifesto

### The problem

Many AI products still get trapped by the same three mistakes:

1. **assuming a better model automatically means a better product**
2. **offering lots of tools without an operating system around them**
3. **supporting individual agents without supporting real coordination and long-running work**

Even a strong model will drift, lose context, and fail at sustained execution if the harness around it is weak.

### LibrAgent's answer

LibrAgent responds with a harness-first design:

- Rust-centered orchestration
- MCP-first architecture
- practical execution through workspace, browser, shell, and knowledge systems
- multi-agent coordination through `delegate`, `teamwork`, `org`, and `schedule`
- local-first control over security and runtime boundaries

---

## 3. The strongest message

## **"LibrAgent is not a chat app. It is an execution environment for agents."**

That is the sharpest message.

Many competing products are strong in one narrow category:

- a polished coding assistant
- a flexible framework that must be assembled manually
- a powerful cloud agent with weaker local control
- an open platform with uneven governance and operational discipline

LibrAgent is unusual because it is simultaneously:

- a **product**
- a **harness**
- an **MCP platform**
- and a **swarm-orchestration layer**

---

## 4. Competitive framing

### Market-level framing

The market has shifted from model wars toward harness wars.

That means the real differentiators are:

- execution loops rather than raw model quality
- durable sessions rather than one-shot prompts
- tool operating systems rather than isolated integrations
- delegation and coordination rather than a single agent in a box

### Useful comparison frame

| Competitor group                                  | Strength                      | Limitation                                       | LibrAgent advantage                                                                       |
| ------------------------------------------------- | ----------------------------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------- |
| Cursor / Claude Code style tools                  | excellent coding productivity | centered on developer workflows                  | extends beyond coding into MCP, browser, knowledge, workspace, schedules, and swarm flows |
| LangGraph / CrewAI / Pydantic AI style frameworks | deep flexibility              | requires assembly work                           | gives users a ready-to-run product experience                                             |
| Open-source free-form agent platforms             | openness and extensibility    | uneven governance and operational consistency    | stronger local-first discipline, validation, and session isolation                        |
| Cloud automation agents                           | broad automation imagination  | remote-first and less tied to local work context | stays closer to the user's real machine and workspace                                     |

### The most persuasive advantages

1. **The harness is unusually complete**
2. **MCP is treated as a platform layer, not a bolt-on**
3. **The growth path from one agent to coordinated teams is natural**
4. **Users keep meaningful control over their own agent stack**

---

## 5. Onboarding story

The introduction becomes convincing when it answers a simple question: **What do I do first?**

### 1. Connect a model

- local LLM via Ollama
- hosted models via API keys such as OpenAI, Anthropic, or Gemini

### 2. Add MCP servers

- install from presets,
- or let an agent help with setup through bundled skills such as `mcp-installer` and `mcp-importer`

### 3. Accelerate setup with bundled skills

- `system-setup`
- `mcp-installer`
- `mcp-importer`
- `specialist-creator`

### 4. Turn tools into agent capability

- `crew-constructor` can create specialist teams
- `agent-tooling` can improve tool selection for existing agents

### 5. Grow from one agent to coordination

- parallel work via `delegate`
- shared operating rules via `teamwork`
- durable team identity via `org`
- recurring automation via `schedule`

---

## 6. Real usage stories

### Solo developer

- connect a local repository
- install the GitHub MCP preset
- run code analysis, security review, and documentation drafting

### Operator or researcher

- combine browser, search, and knowledge flows
- automate recurring scans or briefs
- accumulate results into reusable context

### Team workflow

- create specialist agents
- distribute work with `delegate`
- establish more explicit coordination with `org`

### Offline or privacy-sensitive setup

- run Ollama with local MCP servers and a local workspace
- keep sensitive data under local control

---

## 7. Copy-ready lines

### Short introduction

**LibrAgent is an MCP-native, local-first platform that expands a single AI assistant into a working team of agents.**

### Stronger introduction

**Most AI apps stop at a chat window wrapped around a good model. LibrAgent goes further by combining models, tools, workspace, browser, knowledge, sessions, delegation, and team coordination into one agent harness.**

### One-line position

**LibrAgent's key advantage is the harness, not just the model.**

### Onboarding call-to-action

**Connect one model, add a few MCP servers, wake up your first agent with bundled skills, and then grow from delegation to swarm and org-style coordination.**

---

## 8. Recommended narrative order

For PRs, launch posts, and product intros, this sequence works best:

1. **Define the problem:** the market has moved beyond model quality alone
2. **Declare the identity:** LibrAgent is an agent operating environment, not just an AI app
3. **Show the core appeal:** MCP, local-first execution, workspace, and delegation live in one product
4. **Frame the competitive edge:** it is not only a framework, not only a coding tool, and not a chaotic runtime
5. **Make the journey concrete:** connect a model, add MCP, use bundled skills, create specialists, delegate, then coordinate
6. **Close with the point:** LibrAgent is a platform for operating agents, not merely chatting with them

---

## 9. Final take

Weak messaging says, "it has a lot of features."

The better message is:

> **LibrAgent is a product for the harness era.**
>
> It is not about attaching one more model to a chat UI. It is about giving agents a real environment in which they can work, collaborate, recover, and scale.

And the most attractive closing angle is this:

> **You can start locally, extend through MCP, and grow naturally from one agent to swarm and org coordination.**
