---
name: agent-tooling
description: Systematically audit, analyze, and optimize tool assignments for AI agents in a dynamic environment. Use this skill when you need to: (1) Discover ALL currently available tools and agent configurations via live system calls, (2) Analyze tool capabilities vs agent role descriptions, (3) Optimize assignments based on current system inventory (which may change), (4) Map newly registered MCP servers to appropriate agents, or (5) Prune obsolete tool references. Triggers on requests like "audit agent tools", "optimize agent capabilities", "새로운 도구 자동 배치", "에이전트 도구 동적 최적화".
---

# Dynamic Agent Tooling Skill

This skill provides a framework for managing agent capabilities in a system where tools and environments change frequently. **Never rely on static lists; always perform live discovery.**

## Dynamic Workflow (Discovery-First)

### Phase 1 — Environment Discovery

Always start by capturing the ground truth of the current session:

1. **Tool Inventory**: Run `tool__list(forceVerify=true)` to get the latest list of MCP servers and their active toolsets. Capture the **current IDs** (IDs can change between sessions).
2. **Agent Inventory**: Run `agent__list(type="configs")` to see current configurations of all agents.
3. **Capability Mapping**: Extract the `description` field for each MCP server and each agent. This is the key for dynamic matching.

### Phase 2 — Capability-Based Analysis

Instead of matching specific names, match **Capabilities** provided by tools to **Requirements** defined by agent roles.

**Tool Categories to Identify:**
- **Search/Research**: Web search, academic paper lookup, trend tracking.
- **Financial/Data**: Stock prices, economic indicators, structured data.
- **AI/Multimedia**: Image/video generation, multimodal analysis.
- **Technical/Execution**: Coding assistants, library docs, system managers.

**Agent Role Requirements:**
- **Librarians/Brokers**: Require high search/research capability.
- **Strategists/Analysts**: Require high financial/data and research capabilities.
- **Experts/Wizards**: Require technical/execution and domain-specific tools.

See `references/tool-classification.md` for the logic of mapping tool descriptions to categories.

### Phase 3 — Gap Analysis & Optimization

Compare the discovered agent configuration against the identified categories.

| Logic | Action |
|-------|--------|
| **Matching Capability Found** | If a new tool provides a capability an agent needs (but doesn't have), **ADD** it. |
| **Obsolete Tool Found** | If an agent has a tool ID that is no longer in `tool__list`, **REMOVE** it. |
| **Role/Capability Mismatch** | If an agent's description and a tool's category are unrelated, **REMOVE** it. |
| **Minimum Viable Builtins** | Ensure every agent has its required core builtins (e.g., `planning` for everyone). |

### Phase 4 — Execution & Verification

1. **Plan & Confirm**: Present the optimization plan (which IDs to add/remove for which agents).
2. **Atomic Update**: Use `agent__update` with the **complete lists** of both builtins and MCP servers.
3. **Verify**: Call `agent__list` once more to ensure the changes took effect.

## Decision Support

- For tool categorization logic: Refer to `references/tool-classification.md`
- For agent requirement templates: Refer to `references/agent-requirements.md`
