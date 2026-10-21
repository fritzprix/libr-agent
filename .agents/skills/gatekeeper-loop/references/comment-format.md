# Comment protocol

Worker and gatekeeper share one GitHub account, and GitHub rejects Approve / Request changes on your own PR. The protocol therefore uses plain PR comments with a hidden marker on the first line. `review_queue.py` (gatekeeper) and `pr_status.py` (worker) only trust markers on comments authored by the authenticated user.

## Gatekeeper verdict

`head` = full SHA of the PR head that was reviewed. `round` starts at 1 and increments per verdict.

```markdown
<!-- gatekeeper-loop: round=2 verdict=changes-requested head=<sha> -->
## Gatekeeper · round 2 · Changes requested

Scope: <one line — what the PR claims and whether the diff matches it>

### Required
- **G2.1** `src-tauri/src/agent/tool_approvals.rs:212` — <problem>. Expected: <concrete fix or acceptance check>.
- **G2.2** <missing test> — Expected: a unit test covering <case>.

### Suggestions (non-blocking)
- **S2.1** <optional improvement>

### Previous findings
- G1.1 ✅ fixed in <short-sha> · G1.2 ✅ decline accepted (<reason>)
```

Approve:

```markdown
<!-- gatekeeper-loop: round=3 verdict=approve head=<sha> -->
## Gatekeeper · round 3 · ✅ Approve

<2–3 lines: what was verified (diff, tests, CI green at <short-sha>), residual risks, suggestions left open>
```

Escalate (round limit or unresolved disagreement):

```markdown
<!-- gatekeeper-loop: round=5 verdict=escalate head=<sha> -->
## Gatekeeper · round 5 · Needs maintainer decision

<the open disagreement, both positions, and the decision needed>
```

## Worker response

Posted after addressing a `changes-requested` verdict, in the same round number.

```markdown
<!-- worker-loop: response round=2 head=<new-sha> -->
## Worker response · round 2

- **G2.1** fixed in <short-sha> — <one line>
- **G2.2** declined — <reason grounded in code, issue scope, or project rules>
- **S2.1** deferred — <reason or follow-up issue link>
```

Every `G` item must be answered as **fixed** or **declined**. Suggestions may be skipped silently.

## Labels

| Label | Set by | Meaning |
| --- | --- | --- |
| `gatekeeper:changes-requested` | gatekeeper | Latest verdict requests changes |
| `gatekeeper:approved` | gatekeeper | Approved at the current head; ready for a human merge |
| `gatekeeper:escalated` | gatekeeper | Needs a maintainer decision |

Keep exactly one of these labels on a PR; swap them with each verdict. Create missing labels once with `gh label create <name> --color <hex> --description <text>`.
