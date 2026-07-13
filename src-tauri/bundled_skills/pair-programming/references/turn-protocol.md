# Pair Programming Turn Protocol

Driver and Navigator alternate via parent-mediated messages.

## Roles

| Role | Does | Does not |
| --- | --- | --- |
| **Driver** | Edit files, run commands, implement | Redesign architecture without Navigator sign-off |
| **Navigator** | Review diff, guide structure, set next micro-task | Large unsolicited rewrites |

## Turn cycle

1. Navigator sends **micro-task** (one logical unit).
2. Driver implements and posts **Change Summary**.
3. Navigator reviews (diff/view) and replies **Approve next** or **Revise**.
4. Repeat until milestone; optionally rotate roles.

## Change Summary (Driver → Navigator)

```markdown
## Driver Turn Complete
- Task: <micro-task>
- Files changed: <paths>
- Approach: <2-3 sentences>
- Tests run: <command + result or "not run">
- Ready for: review | next task
```

## Navigator feedback

```markdown
## Navigator Review
- Verdict: approve | revise
- Issues: <numbered list>
- Next micro-task: <single clear instruction>
```

## Parent coordination

Use `agent__messageToSession` to pass Change Summary and Review between sessions. Parent enforces turn order — spokes do not message each other directly unless Hub pattern explicitly allows.
