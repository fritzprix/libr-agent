---
name: soul-delegation
description: Guide for creating sub-agents with dedicated workspaces and soul files (soul.md / agent.md). Use when delegating tasks to child sessions, building organizational hierarchies of AI agents, or setting up recursive multi-agent architectures with workspace-based identity.
license: MIT
---

# Soul-Based Agent Delegation Guide

## Overview

LibrAgent supports **recursive organizational hierarchies** where a Master agent can spawn child agents, each with:

- **Dedicated workspace directory** (subdirectory under parent workspace)
- **Soul file** (`SOUL.md`) defining personality, values, and behavioral principles
- **Role file** (`agent.md`, optional) defining the agent's specialized capabilities
- **Skills directory** (`skills/`, optional) for domain-specific knowledge

This is the **Game of Life pattern**: simple rules (soul) → complex emergent behavior (intelligence).

---

## Architecture

```
Parent Workspace/
├── SOUL.md              ← Parent's soul (if any)
├── agent.md             ← Parent's role definition
│
├── Marketing/           ← Child workspace (sub-agent)
│   ├── SOUL.md          ← "Brand consistency, data-driven, ethical"
│   ├── agent.md         ← "Marketing specialist with SEO expertise"
│   └── skills/          ← Marketing-specific skills
│       └── SKILL.md
│
├── Engineering/         ← Another child workspace
│   ├── SOUL.md          ← "Code quality, security, performance"
│   ├── agent.md         ← "Implementation specialist"
│   └── skills/
│
└── Research/
    ├── SOUL.md
    └── Creative/        ← Recursive nesting (grandchild)
        ├── SOUL.md
        └── agent.md
```

Each level has independent soul but operates within its own workspace. Infinite recursion depth is possible (controlled by `maxDepth`).

---

## Step-by-Step: Creating a Sub-Agent with Soul

### Step 1: Create the child workspace directory

Use workspace file tools to create the sub-directory structure:

```
writeFile({
  "filePath": "Marketing/SOUL.md",
  "content": "# Soul\n\n## Vibe\nBrand-obsessed, data-driven, creatively bold.\n\n## Rules\n1. Every claim needs data backing\n2. Brand voice consistency is non-negotiable\n3. Test before scaling"
})
```

Optionally create `agent.md` for role-specific instructions:

```
writeFile({
  "filePath": "Marketing/agent.md",
  "content": "# Marketing Specialist\n\nYou are a marketing specialist focused on SEO, content strategy, and brand management.\n\n## Capabilities\n- Content calendar management\n- SEO keyword research and optimization\n- Brand voice enforcement"
})
```

### Step 2: Create child session with workspace override

Use `createChildSession` with `workspacePath` pointing to the sub-directory:

```
createChildSession({
  "assistantId": "<assistant-id>",
  "request": "Read SOUL.md in your workspace root and internalize the principles. Then read agent.md if it exists. These define who you are. After that, proceed with: <actual task>",
  "name": "Marketing Agent",
  "workspacePath": "<absolute-path-to-Marketing-directory>",
  "maxDepth": 3,
  "maxFanout": 5
})
```

**Critical**: The `workspacePath` must be an **absolute path**. To get the absolute path of your workspace, check workspace context or use the current working directory info.

### Step 3: Monitor and coordinate

```
waitForSessionIdle({
  "sessionId": "<child-session-id>",
  "timeoutSeconds": 300,
  "includeLastAssistantMessage": true
})
```

---

## SOUL.md Format

The SOUL.md file defines the agent's **personality and behavioral principles**. Recommended structure:

```markdown
# SOUL.md

## Vibe

<One paragraph describing the agent's character and communication style>

## Rules

1. <Core behavioral rule>
2. <Core behavioral rule>
3. ...

## Values (optional)

- <What this agent prioritizes>
- <What this agent avoids>
```

**Key Properties of a Good Soul:**

- **Short** — Under 500 words. Souls are principles, not instruction manuals.
- **Opinionated** — Clear stance on HOW the agent should behave.
- **Character-driven** — Not "what to do" but "who you are."
- **No task instructions** — Tasks go in requests, not souls.

---

## agent.md Format (Optional)

Defines the agent's **specialized role and capabilities**:

```markdown
# <Role Title>

<Brief role description>

## Capabilities

- <What this agent can do>

## Domain Knowledge

- <Specific expertise areas>

## Working Style

- <How this agent approaches problems>
```

---

## Workspace Path Resolution

To construct the absolute workspace path for a child:

1. Your workspace tools operate relative to your session workspace root
2. When you create a subdirectory (e.g., `Marketing/`), the absolute path is: `<your-workspace-root>/Marketing/`
3. Check service context or workspace info for your current workspace root path
4. Pass this absolute path as `workspacePath` to `createChildSession`

---

## Available Tools Reference

| Tool                 | Purpose                                           |
| -------------------- | ------------------------------------------------- |
| `writeFile`          | Create SOUL.md, agent.md, and skills in workspace |
| `readFile`           | Read files (to copy skills for inheritance)       |
| `createChildSession` | Spawn child agent with workspace override         |
| `sendMessage`        | Send follow-up tasks to child                     |
| `waitForSessionIdle` | Wait for child to finish                          |
| `getMessages`        | Read child's outputs                              |
| `getChildSessions`   | List all children                                 |
| `terminateSession`   | Stop a child agent                                |
| `listAssistants`     | Find available assistant IDs for binding          |

---

## Skill Inheritance (Critical for Recursion)

For child agents to spawn their own children, they need the soul-delegation skill. Add this step when creating child workspaces:

```
# Step 0: Copy soul-delegation skill to child workspace
readFile("skills/soul-delegation/SKILL.md")  # Read this skill
writeFile("ChildWorkspace/skills/soul-delegation/SKILL.md", "<content from above>")

# Step 1: Create SOUL.md for child
writeFile("ChildWorkspace/SOUL.md", "...")

# Step 2: Create child session
createChildSession(...)
```

This ensures **infinite recursion**: every agent can spawn sub-agents who can spawn their own sub-agents, and so on.

---

## Patterns

### Pattern 1: Quick Specialist Deployment

For one-off delegated tasks:

```
1. writeFile("Research/SOUL.md", "<soul content>")
2. createChildSession(assistantId, "Read SOUL.md first, then: <task>", workspacePath=".../Research")
3. waitForSessionIdle(childId)
```

### Pattern 2: Persistent Department

For ongoing work requiring multiple interactions:

```
1. Create workspace with SOUL.md + agent.md + skills/
2. createChildSession(...) with initial bootstrapping request
3. sendMessage(childId, "New task: ...")  // reuse same session
4. sendMessage(childId, "Another task: ...")
```

### Pattern 3: Recursive Organization (with Skill Inheritance)

For deep hierarchies where every level can spawn children:

```
CEO (root, maxDepth=5)
 └─ createChildSession → VP Marketing (depth=1)
     └─ VP creates → Content Lead (depth=2)
         └─ Content Lead creates → Writer (depth=3)
```

**Critical:** Each parent must copy soul-delegation skill to child workspace:

```
# Parent (CEO) creating VP Marketing:
1. readFile("skills/soul-delegation/SKILL.md")
2. writeFile("Marketing/skills/soul-delegation/SKILL.md", "<skill content>")
3. writeFile("Marketing/SOUL.md", "Brand consistency, data-driven...")
4. createChildSession(workspacePath=".../Marketing")

# VP Marketing can now repeat the pattern to create Content Lead
# Content Lead can repeat to create Writer
# Infinite recursion enabled
```

### Pattern 4: Soul-First Initialization

The recommended request format for child sessions:

```
"Before doing anything else:
1. Read SOUL.md from your workspace root — this is your identity
2. Read agent.md if it exists — this is your role specification
3. Internalize these principles for ALL subsequent actions

Then proceed with your actual task: <task description>"
```

---

## Important Notes

- **SOUL.md is NOT automatically injected into system prompts** (yet). The child agent must be instructed to read it.
- Use the `request` field in `createChildSession` to tell the child to read its soul files first.
- **For infinite recursion**: Copy `soul-delegation` skill to child workspace so children can spawn their own children.
- Each child session gets its own isolated workspace, planning state, knowledge base, and browser sessions.
- Parent and child share NO state. Communication is only via `sendMessage` / `getMessages`.
- `maxDepth` controls recursion depth. Set reasonable limits to prevent runaway spawning.
- `maxFanout` controls how many direct children a session can create.
- **session_api access is guaranteed** — it's a fundamental right for all agents, cannot be disabled via config.
