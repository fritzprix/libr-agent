# Pre-post checklist

Run this **immediately before** `gh pr comment` with a new gatekeeper marker. Skipping a step has caused duplicate comments, stale-head reviews, severity drift, and approve text that contradicts the latest `clean` marker.

## 1. Head SHA still matches

```bash
gh pr view N --json headRefOid --jq .headRefOid
```

The value must equal the `headSha` you reviewed and the `head=` in the marker. If the worker pushed during review, **do not post** — leave the PR queued; the next tick will pick it up as a new head.

## 2. No duplicate marker

Read `postingChecks.markersAtHead` from `review_queue.py` output (or scan PR comments). **Do not post** if a gatekeeper-loop marker already exists for this `headSha` and the **same** `verdict` you are about to post (`changes-requested`, `approve`, `clean`, or `escalate`). Identical double-posts waste round budget.

Posting a **different** verdict at the same head is allowed (e.g. `changes-requested` after CI failure following `clean`).

## 3. Severity classification (required `G` vs `S`)

For each finding you plan to put under `### Required`:

| Planned item | Rubric | Action |
| ------------ | ------ | ------ |
| New logic with no tests | **major** | `G` item, or do not block |
| Missing i18n on a non-critical UI path | **minor** | `S` only — never `G` |
| Naming, comment noise, style | **minor** / **nit** | `S` only |
| Wrong behavior, security leak, acceptance criteria not met | **blocker** / **major** | `G` |

If every open issue is minor/nit, verdict is `clean` or `approve`, not `changes-requested`.

## 4. `clean` → `approve` body must match latest clean review

When status is “reviewed clean; CI green” and you only issue `approve` (no full re-review of the diff):

- Re-read the **latest** `verdict=clean` marker at this head (and your score bullets there).
- The `approve` comment's `### Checked`, suggestions, and score rationales must **reflect that clean review**, plus CI-at-head evidence — not text copied from an older round or a draft written before CI finished.
- Do not contradict the clean marker (e.g. clean said a suggestion was addressed, approve must not claim it is still open).

## 5. Security: behavior, not only paths

Add `gatekeeper:needs-human` when **any** of the following is true:

- `securitySensitivePolicyPaths` or `securitySensitiveBehaviorHints` from the queue (path first pass — keep using it).
- The diff's **behavior** touches secrets, credentials, API keys, auth surfaces, tool approval/shell policy/sandbox, or outbound transmission of user secrets — even if no path matched.

On `approve` for those PRs, `### Checked` must list bypass scenarios exercised (quoting, env, symlink, path traversal, etc.).

## 6. CI failure diagnosis (skeptical)

When a finding is `CI failure (<job>)`:

1. Read the **actual** failing step log for the reviewed head; name the failing test/command and file.
2. Do **not** treat the worker's explanation as root cause until logs support it.
3. If the proposed fix is only a **timeout increase** on a **pre-existing** test, you must: (a) show the log proves timeout (not cancel/fail-fast elsewhere), and (b) note whether the same CI step is green on other open PRs at the same base. If other PRs pass, the bump alone is insufficient — identify flake vs regression vs infra.
4. Distinguish PR-introduced failures from base-branch or toolchain drift (failures in files the PR did not touch).

## 7. Code quality score (informational only)

The score must **never** drive approve vs changes-requested. Use the full range so scores are meaningful:

| Band | Typical meaning |
| ---- | ---------------- |
| 50–69 | Multiple major gaps or weak test coverage on new logic |
| 70–84 | Solid but notable major or robustness gaps |
| 85–92 | Clean merge with minor suggestions |
| 93–100 | Exceptional: thorough tests on production paths, tight scope, strong error handling |

Avoid clustering every clean PR at 87–96; differentiate test depth and robustness.

