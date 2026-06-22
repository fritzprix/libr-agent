---
name: pair-programming
description: >
  Implement a turn-based Driver-Navigator pair programming workflow. One agent (Driver)
  focuses on implementation and writes code, while the other (Navigator) reviews, validates,
  and suggests improvements in real time. Roles can swap dynamically based on tasks.
  Triggers: "페어 프로그래밍", "공동 작업", "드라이버 네비게이터", "pair programming", "driver navigator", "collaborative coding".
---

# Pair Programming

The Pair Programming pattern binds two sessions to a shared workspace (`workspaceOverride`) where one acts as the Driver (editing files, implementing logic) and the other acts as the Navigator (reviewing code, checking architecture, guiding steps) in a turn-based collaborative loop.

## Driver-Navigator Loop

```
┌────────────────────────────────────────────────────────┐
│                   Shared Workspace                     │
│               (Same Workspace Override)                │
└──────────────────────────┬─────────────────────────────┘
             ┌─────────────┴─────────────┐
             ▼                           ▼
      [ Driver Session ] ◄────────► [ Navigator Session ]
        - Write code and implement   - Review design, guide structure
        - Direct file modifications  - Provide feedback (turn-based)
```

## 5-Stage Workflow

1. **Workspace Configuration**: Ensure both sessions share the exact same `workspaceOverride` directory.
2. **Assign Roles**: Allocate the Driver role to a code-generation model and the Navigator role to an analysis-focused model.
3. **Turn-based Interaction**:
   - The Driver implements a section of code and writes a summary of the edits for the Navigator.
   - The Navigator runs `view_file` or `git diff` on the workspace, checks for issues, and sends feedback/next instructions to the Driver.
4. **Role Rotation**: Swap roles when moving between tasks (e.g., swapping to write test suites). See [role-rotation.md](references/role-rotation.md).
5. **Validation**: Run the final project test pipeline to verify correctness.

---

## 🛠️ MCP Tools Guide

- **Turn Coordination**: The parent session collects the Driver's changes summary and forwards them to the Navigator via `agent__messageToSession` to invoke the next turn, maintaining a lock-step loop.
