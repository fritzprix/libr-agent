# Improvement backlog template

Save as `tasks.md`. Rank and keep each item minimal.

```markdown
## Ranked tasks

### T1 — <title>  `[blocker|major|nit]`
- **Class:** contract_lie | handler_bug | …
- **Evidence:** report-raw / messages quote; code path if already known
- **Owning paths:** e.g. chrome-extension/service_worker.js, …/bridge.rs
- **Minimal change:** one sentence
- **Acceptance (re-E2E):** matrix cells that must become PASS; contamination still NO
- **Confidence:** high | medium | low
- **Depends on:** reload extension? rebuild app?

### T2 — …
```

## Selection rules

1. Prefer **contract_lie** / **contamination** / **handler_bug** over docs-only.
2. Prefer one code path that explains multiple matrix FAIL cells.
3. Do not file a prompt tweak when schema/handler/bridge is the SSOT.
4. `capability_gap` → document + matrix SKIP guidance, not a fake PASS.
5. `precondition` → fix env / brief, do not invent product bugs.
6. After a patch cycle, mark tasks `done` / `deferred` / `revalidated` in the next `delta.md`.
