# Pre-post checklist

Run this **immediately before** `gh pr comment` with a new gatekeeper marker. Skipping a step has caused duplicate comments, stale-head reviews, severity drift, and approve text that contradicts the latest `clean` marker.

## 1. Head SHA still matches

```bash
gh pr view N --json headRefOid --jq .headRefOid
```

The value must equal the `headSha` you reviewed and the `head=` in the marker. If the worker pushed during review, **do not post** — leave the PR queued; the next tick will pick it up as a new head.

## 2. No duplicate marker

Read `postingChecks.markersAtHead` from `review_queue.py` output (or scan PR comments). **Do not post** if a gatekeeper-loop marker already exists for this `headSha` and the **same** `verdict` you are about to post (`changes-requested`, `approve`, `ready-for-owner`, `clean`, or `escalate`). Identical double-posts waste round budget.

Posting a **different** verdict at the same head is allowed (e.g. `changes-requested` after CI failure following `clean`).

## 3. Severity classification (required `G` vs `S`)

For each finding you plan to put under `### Required`:

| Planned item | Rubric | Action |
| ------------ | ------ | ------ |
| New logic with no tests | **major** | `G` item, or do not block |
| Missing i18n on a non-critical UI path | **minor** | `S` only — never `G` |
| Naming, comment noise, style | **minor** / **nit** | `S` only |
| Wrong behavior, security leak, acceptance criteria not met | **blocker** / **major** | `G` |
| Technical debt in the diff (duplication, dead code, workaround, unneeded fallback, lint suppression, stale test) | **major** | `G` with a concrete refactor |

If every open issue is minor/nit **and** the score is 86 or higher, verdict is `clean` or `approve`, not `changes-requested`. A score of 85 or lower is always `changes-requested`.

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
5. **Base-branch or out-of-scope CI/tooling failures do not become required `G` items** on the PR under review. Note them in the verdict (e.g. under `### Checked` or a short out-of-scope note) and file follow-up per §8 — do not request changes solely because the base is red for unrelated reasons.

## 7. Code quality score (gate: ≥86 to approve)

A total of **85 or lower** blocks `clean` and `approve`: post `changes-requested` with `G` items for the lowest-scoring categories. When upgrading an existing `clean` marker to `approve`, re-check its score; a clean marker scored 85 or lower must become `changes-requested`. Use the full range so scores are meaningful, and never inflate a score to cross the gate:

| Band | Typical meaning |
| ---- | ---------------- |
| 50–69 | Multiple major gaps or weak test coverage on new logic |
| 70–85 | Solid but notable major, robustness, or tech-debt gaps — changes requested |
| 86–92 | Clean merge with minor suggestions |
| 93–100 | Exceptional: thorough tests on production paths, tight scope, strong error handling |

Avoid clustering every clean PR at 87–96; differentiate test depth and robustness.

## 8. Follow-up work the PR did not cause (gatekeeper files the issue)

When review finds work that is **not this PR's fault**, the **gatekeeper** must open a GitHub issue in the same tick — before or when posting the verdict — and link it in the PR comment. Do **not** ask the worker or the owner to open it, do **not** record `needsUser`, and do **not** treat “another open PR will fix the base” as a substitute.

Applies when **all** of the following hold:

- The problem is outside the PR diff, on the PR's **base branch**, or is **tech debt** explicitly deferred for a later change; **and**
- It would block or distract if folded into the current PR; **and**
- It is **not** a non-blocking suggestion (`S` item) — **never** file an issue for optional polish.

Examples:

1. Base-branch CI is red for rustfmt/clippy/tests on files this PR does not touch (leftover from a merged base change).
2. Tech debt outside the diff that the verdict or worker response defers (major/blocker class only — not `S`).

**Before creating:** search open issues (`gh issue list --search "..."`) for the same cause; link the existing issue instead of duplicating.

**Create (English body, public repo):**

```bash
gh issue create --title "<short noun phrase>" --label "auto:producer" --body-file <tmp>
```

Body must include:

- **Cause** — what failed or what debt, with `path:line` or CI job evidence.
- **Expected fix** — concrete acceptance check or command that must pass.
- **Target branch** — the PR's base (e.g. `dev/0.9.x`). Workers use this as the PR base after a human adds `approved`.

Label **only** `auto:producer`. Do **not** add `approved` or `validated` on create (step 4 of the gatekeeper tick may add `validated` later, same as producer-filed issues).

In the PR verdict, cite the issue (e.g. “Out-of-scope base CI: #N”) and keep required `G` items limited to what **this diff** must fix. Waiting for another PR to merge does not remove the need for #N.

