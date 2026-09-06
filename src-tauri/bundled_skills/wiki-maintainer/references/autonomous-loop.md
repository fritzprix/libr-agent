# Autonomous loop (bootstrap once → scheduled runs)

Preferred UX: the user talks to **Wiki Maintainer once**, then a global
**Scheduled Task** wakes the same assistant on a cron. No per-run commands.

## Bootstrap conversation (interactive, once)

Triggers: “알아서 돌려”, “매일 정리해”, “자동으로 wiki 업데이트”, “schedule this”.

1. Confirm cadence with the user (default: daily local evening, e.g. `0 20 * * *`).
   Respect Settings minimum scheduled-task interval if create fails.
2. `wiki_cli.py init` + `path` (create host-global wiki if missing).
3. Optional immediate pass: run the normal mine → write-pattern workflow once
   so the first value is not delayed until the next cron fire.
4. Resolve this assistant’s config id:
   `agent__listAgents(type="configs", query="Wiki Maintainer")` → use the
   returned **id** (never the display name) as `assistantId`.
5. Create a global task via **schedule** skill / `scheduled_task__createScheduledTask`:
   - `name`: e.g. `Wiki skill evolution`
   - `cronExpression` + `scheduleTimezone` as agreed
   - `assistantId`: Wiki Maintainer config id
   - `message`: the wake prompt below (or `@skill:wiki-maintainer` + short policy)
6. Tell the user: task id, next run, and that they can pause under
   **Scheduled Tasks**. Do not ask them to re-type the mine prompt each day.

If a task with the same purpose already exists (`listScheduledTasks`), update or
toggle it instead of creating duplicates.

## Wake prompt (scheduled `message`)

Use a stable, self-contained message so the woken session needs no human turn:

```text
@skill:wiki-maintainer
Run one autonomous skill-evolution pass:
1. Init/path the host-global wiki if needed.
2. Mine recent non-meta sessions via history__* (prefer failures, long recovery,
   user corrections since the last wiki log entry; skip chit-chat and prior
   Wiki Maintainer runs).
3. Upsert evidence-backed patterns; keep single-session findings as draft.
4. If a pattern is strong enough for a skill change, @skill:skill-proposer for
   exactly one atomic proposal. Do not Accept patches without an explicit user
   Accept in a later interactive session — leave the proposal + ledger entry
   for the user to review.
5. Prepend a short logs.md line summarizing what you did (or "no new patterns").
Stop when done; do not wait for further user input.
```

## Autonomy boundaries

| Action | Autonomous cron OK? |
| --- | --- |
| Wiki init / pattern write / index / log | Yes |
| skill-proposer draft + impact ledger Propose | Yes |
| Accept / apply skill patch permanently | **No** — user Accept only |
| Delete unrelated skills or widen cron past policy | No |

## Pause / retune

User or agent: `scheduled_task__toggleScheduledTask` / `updateScheduledTask`.
Point users at sidebar **Scheduled Tasks** for visibility.
