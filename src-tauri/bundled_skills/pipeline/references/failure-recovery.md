# Pipeline Failure Recovery

## Stage failed mid-pipeline

1. **Stop downstream** — do not spawn next stage on bad input.
2. **Capture error** — exit reason from `agent__checkSession` or child message.
3. **Choose recovery:**

| Situation | Action |
| --- | --- |
| Transient (timeout, rate limit) | Retry same stage once with narrowed task |
| Bad handover | Fix handover template, re-run **same** stage |
| Wrong upstream artifact | Re-run **previous** stage only |
| Unrecoverable | Report to user with stage name + files touched |

## Partial artifact safety

- Shared `workspaceOverride` means later stages may read half-done files.
- On failure, mark WIP in a `PIPELINE_STATUS.md` or coordination note.
- Next retry should state which files are authoritative.

## Idempotent stages

Design stages so re-run does not duplicate work:

- Research → overwrite `docs/RESEARCH.md`
- Implement → branch or explicit "continue from line X"

## User escalation template

```markdown
Pipeline stopped at stage: <name>
Reason: <error>
Artifacts so far: <paths>
Suggested fix: <retry stage N | fix handover | user decision>
```
