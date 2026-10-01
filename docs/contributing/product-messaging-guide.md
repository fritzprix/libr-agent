# Product Messaging Guide

This document is a positioning and PR messaging guide for LibrAgent. The goal is not to list features mechanically, but to explain why the product matters and how to describe it persuasively without drifting away from what the codebase actually supports.

Canonical public README: [README.md](../../README.md)  
Canonical demo filming: [hero-demo-spec.md](./hero-demo-spec.md)

---

## 1. Core positioning

**LibrAgent is an agent operating environment you run — not a vendor chat shell, and not a harness kit you assemble.**

In practical terms:

- **Surface without harness homework** — GUI, Extensions one-click presets, recipes, in-app approvals, `@skill:` invocation
- **Orchestration as product** — named coordination patterns (`pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, …) plus `teamwork` / `org` / `schedule` when you need teams or cron
- **Provider & stack freedom** — any supported LLM (API or Ollama), MCP as infrastructure, import from Cursor / VS Code / Claude Code / Windsurf, MIT, local-first by default

---

## 2. The manifesto

### The problem

The harness era still traps people in three ways:

1. **Vendor harnesses** — great UX, but model/tool policy follows one company
2. **Developer harnesses** — real power, but onboarding is JSON, `npx`/`uvx`, shell, and SDK graphs
3. **Fake differentiation** — “not a chat app,” “agents use tools,” “local desktop,” “we have multi-agent” — table stakes every serious agent claims

### LibrAgent's answer

Ship **harness depth as a product**:

- one-click MCP presets and IDE config import
- bundled orchestration skills chosen from work shape (see `teamwork` framework-selection)
- user-chosen models and tools — no required AI vendor
- durable local execution (workspace, shell, browser, knowledge, sessions)

---

## 3. The strongest messages

### Primary (use in README / hero / Show HN)

**Install tools like apps. Keep your model. Keep the file.**  
**No vendor harness. No JSON homework.**

### Secondary (orchestration)

**Pick a coordination pattern — don’t assemble a framework.**  
(`@skill:pipeline` / `hub-spoke` / `divide-conquer` / …)

### Do not lead with (alone)

- “Not a chat app. It is an execution environment.” — category noise
- “Local-first desktop with MCP and tools” — saturated
- “Multi-agent system” — undersells named patterns and non-dev surface

---

## 4. Competitive framing

### Useful comparison frame

| Competitor group | Strength | Limitation | LibrAgent advantage |
| ---------------- | -------- | ---------- | ------------------- |
| Cursor / Claude Code | excellent coding productivity | developer-centric; often vendor-tied model story | product surface beyond IDE; presets/recipes; any model; orchestration skills |
| LangGraph / CrewAI / Agents SDK | deep flexibility | you assemble and operate the harness | ready-to-run patterns as `@skill:` / GUI |
| Hermes / CLI OSS agents | strong autonomy / open weights | terminal-first homework | desktop product + same freedom ethos |
| ChatGPT / Claude Desktop | polished consumer UI | vendor stack gravity | bring your model and MCP; MIT local-first |
| CC Switch | multi-CLI switchboard | no step-loop of its own | full execution + orchestration product |

### The three persuasive advantages

1. **You can use the harness without being a harness engineer**
2. **Orchestration is a menu of patterns, not a checkbox labeled multi-agent**
3. **Stack freedom — model, tools, and coordination stay user choices**

---

## 5. Hero demo (visual story)

Do **not** invent a different demo story for README, releases, or Show HN.

Canonical filming + acceptance criteria: **[hero-demo-spec.md](./hero-demo-spec.md)**

Locked summary:

- Line: *Install tools like apps. Keep your model. Keep the file.*
- Beat: Extensions **Install** → workspace → deliverable on disk
- Secondary clips: `@skill:pipeline` (or peers), Morning Briefing recipe, Cursor MCP import
- Out of **primary** hero: org/swarm laundry lists, Settings tours

---

## 6. Onboarding story

Answer: **What do I do first?**

### 1. Connect a model (freedom)

- local LLM via Ollama
- hosted models via API keys (OpenAI, Anthropic, Gemini, Groq, …)

### 2. Add tools without JSON

- Extensions → one-click presets, or
- agent + `tool-installer` (_Import my MCP servers from Cursor_)

### 3. Attach a workspace and finish with a file

- real folder + deliverable on disk (`DELIVERABLE.md` / reportResult)

### 4. Grow with named orchestration

- `@skill:pipeline` / `hub-spoke` / `divide-conquer` / `consensus-delegation` / …
- `teamwork` → `org` for durable teams
- `schedule` / recipes for recurring work
- `loop` / `call-me-back` for in-session or event-driven resume

### 5. Accelerate with other bundled skills

- `setup-wizard`, `tool-installer`, `recruit`, `boost`, `playbook-creator`, domain skills (`git-workflow`, `calendar-mgmt`, …)

---

## 7. Real usage stories

### Operator

- Morning Briefing recipe → presets + assistant + 9am schedule → file without terminal

### Solo developer

- GitHub preset from Extensions → workspace → Markdown report on disk

### Power user (non-framework)

- `@skill:pipeline` or `divide-conquer` → merged deliverable — no orchestration library

### Privacy-sensitive

- Ollama + local MCP + local workspace; swap cloud keys later without changing product identity

---

## 8. Copy-ready lines

### Short introduction

**LibrAgent is a desktop agent operating environment: one-click MCP, named orchestration skills, and any model you choose.**

### Stronger introduction

**Most “harnesses” are either locked to one AI vendor or assume you can edit JSON and assemble frameworks. LibrAgent ships the operating surface — presets, recipes, `@skill:` coordination patterns — and leaves the model and tool stack to you.**

### One-line position

**Install tools like apps. Keep your model. Keep the file.**

### Onboarding CTA

**Connect a model, one-click a preset, finish with a file you keep — then attach `@skill:pipeline` (or a recipe) when one pass is not enough.**

---

## 9. Recommended narrative order

For PRs, launch posts, and product intros:

1. **Name the trap:** vendor lock-in *or* developer homework
2. **Declare the product:** operating environment with one-click tools + pattern skills + stack freedom
3. **Show the hero beat:** Extensions Install → deliverable ([hero-demo-spec.md](./hero-demo-spec.md))
4. **Show orchestration without frameworks:** one `@skill:` pattern clip
5. **Close:** your model, your tools, your coordination pattern

---

## 10. Final take

Weak messaging says, "it has a lot of features" or "it's not a chat app."

The better message is:

> **LibrAgent is a product for people who want harness power without harness homework — and without a vendor owning the stack.**
>
> One-click tools. Named coordination patterns. Your model. Files you keep.

---

## 11. Locale sync

After changing English README or this guide’s locked lines, update locale READMEs (`README.ko.md`, `README.zh.md`, …) to the same beats — translate wording, do not invent a different product story.
