# Pair Programming (teamwork reference)

Session-level pattern of the **teamwork** skill. It needs no team scaffold. Spawn and isolation rules: `delegate`.

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

1. **Session Selection**: Inspect existing child sessions and reuse an Idle Driver/Navigator session with the matching assistant ID and exact workspace contract through `agent__messageToSession(sessionId, complexity, message)`. Use `reset=true` when beginning a fresh pairing task. Always pass `complexity` (`low`|`normal`|`high`). Start a new session only when no compatible pair member exists or the required role/workspace differs.
2. **Workspace Configuration**: Ensure both sessions share the exact same `workspaceOverride` directory.
3. **Assign Roles**: Allocate the Driver role to a code-generation model and the Navigator role to an analysis-focused model.
4. **Turn-based Interaction**:
   - The Driver implements a section of code and writes a summary of the edits for the Navigator.
   - The Navigator runs `workspace__readFile` or `git diff` on the workspace, checks for issues, and sends feedback/next instructions to the Driver.
5. **Role Rotation**: Swap roles when moving between tasks (e.g., swapping to write test suites). See [role-rotation.md](pair-programming-role-rotation.md).
6. **Validation**: Run the final project test pipeline to verify correctness.

---

## 🛠️ MCP Tools Guide

- **Turn Coordination**: The parent session collects the Driver's changes summary and forwards them to the Navigator via `agent__messageToSession(sessionId, complexity, message)` to invoke the next turn, maintaining a lock-step loop.

## References

- [Role rotation](pair-programming-role-rotation.md)
- [Turn protocol](pair-programming-turn-protocol.md)
- [Diff review checklist](pair-programming-diff-review-checklist.md)
