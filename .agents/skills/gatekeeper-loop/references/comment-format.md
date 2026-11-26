# Comment protocol

Worker and gatekeeper share one GitHub account, and GitHub rejects Approve / Request changes on your own PR. The protocol therefore uses plain PR comments with a hidden marker on the first line. `review_queue.py` (gatekeeper) and `pr_status.py` (worker) only trust markers on comments authored by the authenticated user.

## Fixed marker kinds

One fixed kind list governs every loop comment (code: `loop-shared/scripts/comment_markers.py`). A marker whose kind or verdict is outside the list is ignored: not a verdict, not a worker response, not an owner comment.

- Gatekeeper verdicts (`verdict=` on a `<!-- gatekeeper-loop: ... -->` marker): `changes-requested`, `approve`, `clean`, `escalate`. The `clean` marker carries no `blocked-by` field; a blocked PR is recorded only as a blocked-by link.
- Worker kinds (first word after `<!-- worker-loop:`): `response`, `needs-owner`, `ci-attempt`, `reply`.

`reply` carries `reply-to=<comment id>`: the worker posts it when it answers a triage comment without a push. A comment targeted by a later `reply` is not a triage item.

## Gatekeeper verdict

`head` = full SHA of the PR head that was reviewed. `base` = the PR's base branch at review time (a later change forces re-review). `round` starts at 1 and increments per verdict (`clean` markers do not count).

**Before posting:** follow [pre-post-checklist.md](pre-post-checklist.md) (head drift, duplicate `head`+`verdict`, severity, clean→approve consistency).

```markdown
<!-- gatekeeper-loop: round=2 verdict=changes-requested head=<sha> base=<branch> -->

## Gatekeeper · round 2 · Changes requested

Scope: <one line — what the PR claims and whether the diff matches it>

### Required

- **G2.1** `src-tauri/src/agent/tool_approvals.rs:212` — <problem>. Expected: <concrete fix or acceptance check>.
- **G2.2** <missing test> — Expected: a unit test covering <case>.
- **G2.3** `CI failure (<job-name>)` — <diagnosed root cause from CI logs, PR-introduced only>. Expected: <concrete fix>.
  - Base-branch / out-of-scope CI: **no `G` item** — note “Out-of-scope base CI: #<issue>” (gatekeeper-created §8 follow-up).

### Suggestions (non-blocking)

- **S2.1** <optional improvement>

### Previous findings

- G1.1 ✅ fixed in <short-sha> · G1.2 ✅ decline accepted (<reason>)

### Code quality score (gate: ≥86 to approve)

Total **<sum>/100**. A total of 85 or lower requires changes (see the `G` items for the lowest categories).

- **Correctness incl. boundary conditions (<n>/30)** — `<file>:<line>` — <one-line rationale with evidence>.
- **Tests exercise the real production code path (<n>/20)** — `<file>:<line>` — <one-line rationale>.
- **Simplicity (KISS/YAGNI) (<n>/15)** — `<file>:<line>` — <one-line rationale>.
- **Readability and naming (<n>/15)** — `<file>:<line>` — <one-line rationale>.
- **Error handling and robustness (<n>/10)** — `<file>:<line>` — <one-line rationale>.
- **Scope discipline (<n>/10)** — `<file>:<line>` — <one-line rationale>.

Do not score CI status or the PR body. See [review-rubric.md](review-rubric.md#code-quality-score-100-points-gate).
```

Approve:

```markdown
<!-- gatekeeper-loop: round=3 verdict=approve head=<sha> base=<branch> -->

## Gatekeeper · round 3 · ✅ Approve

<2–3 lines: what was verified (diff, tests, CI green at <short-sha>), residual risks, suggestions left open>

### Checked

- `<acceptance criterion 1 from linked issue>` — verified in `<file>:<line>` (<brief explanation>).
- `<acceptance criterion 2 from linked issue>` — verified in `<file>:<line>` (<brief explanation>).

### Code quality score (gate: ≥86 to approve)

Total **<sum>/100** (≥86 required for approve).

- **Correctness incl. boundary conditions (<n>/30)** — `<file>:<line>` — <one-line rationale>.
- **Tests exercise the real production code path (<n>/20)** — `<file>:<line>` — <one-line rationale>.
- **Simplicity (KISS/YAGNI) (<n>/15)** — `<file>:<line>` — <one-line rationale>.
- **Readability and naming (<n>/15)** — `<file>:<line>` — <one-line rationale>.
- **Error handling and robustness (<n>/10)** — `<file>:<line>` — <one-line rationale>.
- **Scope discipline (<n>/10)** — `<file>:<line>` — <one-line rationale>.
```

> [!IMPORTANT]
> The `### Checked` section is **mandatory**. Every acceptance criterion from the linked issue, every finding from earlier rounds and every owner concern must be listed with concrete verification evidence (`file:line`). For security-sensitive PRs (shell policy, tool approval, permissions, secrets) also list the bypass scenarios checked. For OS-sensitive changes also list the per-OS CI job results (ubuntu / windows / macos) for the reviewed head. If this section is missing or empty, an `approve` verdict cannot be issued.

Clean, waiting for CI (no label change, not a round; lets any tick or machine see the review is done):

```markdown
<!-- gatekeeper-loop: verdict=clean head=<sha> base=<branch> -->

## Gatekeeper · reviewed clean · waiting for CI

<1–2 lines: what was reviewed, zero open required findings>

### Code quality score (gate: ≥86 to approve)

Total **<sum>/100** (≥86 required for a clean marker).

- **Correctness incl. boundary conditions (<n>/30)** — `<file>:<line>` — <one-line rationale>.
- **Tests exercise the real production code path (<n>/20)** — `<file>:<line>` — <one-line rationale>.
- **Simplicity (KISS/YAGNI) (<n>/15)** — `<file>:<line>` — <one-line rationale>.
- **Readability and naming (<n>/15)** — `<file>:<line>` — <one-line rationale>.
- **Error handling and robustness (<n>/10)** — `<file>:<line>` — <one-line rationale>.
- **Scope discipline (<n>/10)** — `<file>:<line>` — <one-line rationale>.
```

Escalate (round limit of 5, or a product/security/data decision for the user):

```markdown
<!-- gatekeeper-loop: round=5 verdict=escalate head=<sha> -->

## Gatekeeper · round 5 · Needs maintainer decision

<which of the two escalate cases applies, the evidence, and the decision needed>

### Code quality score (gate: ≥86 to approve)

Total **<sum>/100**.

- **Correctness incl. boundary conditions (<n>/30)** — `<file>:<line>` — <one-line rationale>.
- **Tests exercise the real production code path (<n>/20)** — `<file>:<line>` — <one-line rationale>.
- **Simplicity (KISS/YAGNI) (<n>/15)** — `<file>:<line>` — <one-line rationale>.
- **Readability and naming (<n>/15)** — `<file>:<line>` — <one-line rationale>.
- **Error handling and robustness (<n>/10)** — `<file>:<line>` — <one-line rationale>.
- **Scope discipline (<n>/10)** — `<file>:<line>` — <one-line rationale>.
```

## Worker response

Posted after addressing a `changes-requested` verdict, in the same round number.

```markdown
<!-- worker-loop: response round=2 head=<new-sha> -->

## Worker response · round 2

- **G2.1** fixed in <short-sha> — <one line>
- **G2.2** declined — <reason grounded in code, issue scope, or project rules>
- **S2.1** deferred — <reason> (non-blocking; gatekeeper does **not** file an issue for `S` items)
- **G2.x** declined — deferred out of scope — follow-up **#<issue>** (filed by gatekeeper, not the worker)
```

Every `G` item must be answered as **fixed** or **declined**. Suggestions may be skipped silently.

## Labels

| Label                          | Set by     | Meaning                                                                                  |
| ------------------------------ | ---------- | ---------------------------------------------------------------------------------------- |
| `gatekeeper:changes-requested` | gatekeeper | Latest verdict requests changes                                                          |
| `gatekeeper:approved`          | gatekeeper | Approved at the current head; ready for a human merge                                    |
| `gatekeeper:escalated`         | gatekeeper | Needs a maintainer decision; while it is on, the PR is not reviewed. The repository owner removes this label on GitHub to resume, which restarts the round count at 1 |
| `gatekeeper:needs-human`       | gatekeeper | Security-sensitive or loop-skill PR; human must confirm before merge (removing it never resumes an escalation) |
| `gatekeeper:review-clean`      | gatekeeper | Review is clean at the current head; macOS CI must run before approve (#3070). Extra label like `gatekeeper:needs-human` — it does **not** replace the verdict labels (exactly one of the first three still applies) |

Unmarked comments from the repo owner (or `@me`) posted after the last gatekeeper marker are read as required items and trigger a review (worker response markers do not hide them).

Keep exactly one of the first three labels on a PR; swap them with each verdict. Create missing labels once with `gh label create <name> --color <hex> --description <text>`.
