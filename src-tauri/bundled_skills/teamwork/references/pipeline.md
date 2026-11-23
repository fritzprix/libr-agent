# Pipeline (teamwork reference)

Session-level pattern of the **teamwork** skill. It needs no team scaffold. Spawn and isolation rules: `delegate`.

The Pipeline pattern organizes tasks into sequential stages, where the output (artifact) of a preceding stage serves as the input instruction/context for the next stage.

## 5-Stage Workflow

```
Input ──► [ Stage A ] ──► (Artifacts) ──► [ Stage B ] ──► (Artifacts) ──► Output
            Worker A                       Worker B
```

1. **Definition**: Outline the stages, assign specific assistant configurations, and define the handover criteria.
2. **Setup**: Provide a shared directory path via `workspaceOverride` so all stages modify the same project directory.
3. **Execution**:
   - Before each stage, inspect `agent__listAgents(type="sessions")` for an Idle child with the same assistant ID and a compatible workspace. Reuse it with `agent__messageToSession(..., complexity=...)`; set `reset=true` when the stage is a fresh assignment and the prior conversation/runtime state must be discarded.
   - Start a new stage with `agent__spawnSession(..., complexity=..., waitForResult=true)` when no suitable child exists, the stage needs a different role or workspace, or the pipeline requires another parallel capacity slot. Always pass `complexity` (`low`|`normal`|`high`).
   - Collect the output path and stage summary upon completion.
4. **Handover**: Bind the output paths and preceding summaries into the task prompt of the next stage session.
5. **Final Format**: Aggregate the final stage's output and present it to the user.

## 🛠️ MCP Tools Guide

- **Session Reuse**: Reusing a reset session preserves workspace files but closes its browser session. Treat each stage as runtime-isolated even when the session identity is reused, and do not reuse a child whose workspace contract does not match the stage.
- **Sequential Assignments**: Use `agent__messageToSession` with `waitForResponse=true` for a reused child, `agent__spawnSession` with `waitForResult=true` for a new child, or `agent__checkSession(sessionId, wait=true)` to synchronize.
- **Context Filtering**: Avoid passing the entire conversation logs of previous stages to prevent context bloat. Only pass the **summarized markdown text and file paths**.

See [pipeline-specs.md](pipeline-specs.md) for details.

## References

- [Pipeline specs](pipeline-specs.md)
- [Handover templates](pipeline-handover-templates.md)
- [Failure recovery](pipeline-failure-recovery.md)
