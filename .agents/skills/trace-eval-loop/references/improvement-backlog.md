# Improvement backlog template

Save as `tasks.md`. Rank and keep each item minimal.

```markdown
## Ranked tasks

### T1 — <title>  `[blocker|major|nit]`
- **Class:** schema_guidance | handler_bug | response_bloat | recovery_gap |
  prompt_gap | export_defect | precondition | pattern_only
- **Evidence:** step_id / tool_call_id / observation excerpt; path if known
- **Owning paths:** e.g. `src-tauri/src/mcp/builtin/...`, `session_export/atif.rs`
- **Minimal change:** one sentence
- **Acceptance (re-export ATIF):** metric or sequence that must improve
- **Confidence:** high | medium | low
- **Depends on:** rebuild app? re-run session scenario?

### T2 — …
```

## Selection rules

1. Prefer **handler_bug** / **export_defect** / **schema_guidance** over prompt-only tips.
2. Prefer one code path that explains multiple symptoms.
3. Do not file a prompt tweak when schema/handler/export is the SSOT.
4. `pattern_only` (no contract broken) → document and stop; do not invent an owner.
5. `precondition` → fix env / brief, do not invent product bugs.
6. After a patch cycle, mark tasks `done` / `deferred` / `revalidated` in `delta.md`.
