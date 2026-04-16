---
name: review-pr-comments
description: Evaluate pull requests and PR review comments against the real codebase, current file state, and PR diff. Use when a user wants to review PR feedback, decide which comments are valid, judge overall code quality or risk, determine whether a PR is worth merging, or apply approved review fixes.
---

# Review PR Comments and Merge Worthiness

Systematically inspect the PR diff, review comments, current file state, and CI signals before making a merge recommendation.

## Workflow

### 1. Fetch PR data

Use the GitHub MCP PR tools, not repository guesses.

- `github-mcp-server-pull_request_read` with:
  - `method: "get"` for title, body, base/head, mergeability context
  - `method: "get_files"` for changed files
  - `method: "get_diff"` for the patch
  - `method: "get_review_comments"` for inline review threads
  - `method: "get_comments"` for general PR discussion
  - `method: "get_reviews"` for review summaries
  - `method: "get_check_runs"` and/or `method: "get_status"` for CI state when merge-worthiness depends on runtime confidence

Extract:

- PR goal from title/body
- changed files and hotspots
- inline review comments and general comments
- review state and CI/check results

### 2. Establish what the PR is trying to do

Write a 1-3 sentence summary of the PR's intended value before judging it.

Answer:

- What user or engineering problem is this PR trying to solve?
- Which files actually carry that behavior change?
- Is the scope tight, or is the PR doing extra unrelated work?

If the stated goal and the actual diff do not match, treat that as a PR-level problem even if no reviewer mentioned it.

### 3. Read the actual code

Read the current file contents for:

- every file referenced by a comment
- every major changed file in the diff
- any "follow this pattern in X:Y-Z" references mentioned by reviewers

Read enough context to understand whether the comment is already satisfied, partially satisfied, obsolete, or still correct. Do not judge from the diff alone.

### 4. Evaluate each review comment

For every comment assess:

| Dimension     | Questions                                                                                                                               |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| **Valid?**    | Does the issue actually exist in the current code? Has it already been fixed?                                                           |
| **Accurate?** | Are the file path, line number, and cited example correct?                                                                              |
| **Priority**  | Would ignoring it cause a bug, regression, policy violation, or bad UX (High), a maintainability issue (Medium), or mostly style (Low)? |
| **Action**    | Apply immediately / Apply optionally / Skip with reason                                                                                 |

Call out reviewer mistakes explicitly. A comment can be directionally useful while still being inaccurate.

### 5. Evaluate the PR itself

Do not stop at comment triage. Make an independent judgment about the PR.

Answer:

- Does the implementation actually solve the stated problem?
- Does it introduce regressions, risky edge cases, broken invariants, or missing migrations?
- Is the design coherent with existing project patterns?
- Is the scope reasonable for one PR, or is it a grab bag?
- Do tests/checks and the changed code provide enough confidence?
- Are there serious issues that no reviewer caught?

List independent findings separately from reviewer comments. Do not hide new issues inside the comment table.

### 6. Decide merge-worthiness

Choose exactly one recommendation:

- **Merge**: The PR is sound. No unresolved High-priority issue remains.
- **Merge after fixes**: The PR direction is good, but there are unresolved issues that should be fixed before merging.
- **Do not merge**: The PR has flawed logic, unacceptable risk, weak value, or unresolved blockers that make merging a bad call.

Be blunt. If the PR is not worth merging in its current form, say so directly.

### 7. Report in this structure

Use this exact section order:

```markdown
## Merge recommendation

**Decision:** Merge | Merge after fixes | Do not merge
**Why:** [2-4 sentence judgment of value vs risk]

## PR-level findings

| #   | Area      | Severity | Summary                                   | Why it matters          |
| --- | --------- | -------- | ----------------------------------------- | ----------------------- |
| 1   | auth flow | High     | Refresh token path can dead-end after 401 | Breaks session recovery |

## Review comment triage

| #   | File    | Location | Valid? | Priority | Action |
| --- | ------- | -------- | ------ | -------- | ------ |
| 1   | foo.tsx | line 29  | ✅ Yes | Medium   | Apply  |
| 2   | bar.tsx | line 193 | ❌ No  | Low      | Skip   |

## Per-comment rationale

1. [Short explanation]
2. [Short explanation]

## Merge blockers

- [Only unresolved issues that truly block merge]
```

If there are no PR-level findings or no blockers, say that explicitly instead of omitting the section.

### 8. Apply fixes if instructed

When the user approves fixes:

1. Re-read the current file state.
2. Apply only approved changes with targeted edits.
3. Preserve unrelated user changes.
4. Re-evaluate the final merge recommendation after the fixes land.

### 9. Generate a live test prompt after fixes

After approved fixes are applied, or when the user asks for a test prompt, produce a ready-to-paste agent prompt that verifies the PR's real runtime behavior.

Template:

```markdown
다음 PR 변경 사항의 실동작을 순서대로 검증해줘.

## 테스트 N: [변경 이름]

1. [도구 호출 또는 명령]
2. [예상 출력 또는 확인 기준]
3. [엣지 케이스가 있다면 추가 단계]

각 단계의 실제 출력을 그대로 보고해줘.
```

Rules:

- Make each test self-contained.
- Use concrete, observable expectations.
- Cover the happy path and at least one edge/error case for each changed behavior.
- Clean up temp files or state created during testing.

## Evaluation heuristics

### Mark as High priority if

- the change can break runtime behavior, data integrity, auth, permissions, or recovery paths
- the change introduces unsafe typing, missing validation, or logic/race bugs
- the change causes accessibility regressions or user-visible incorrect output
- the reviewer found a real problem that should block merge

### Mark as Medium priority if

- the change conflicts with established project patterns
- the change weakens maintainability, error handling, or observability
- the change likely works but has meaningful rough edges or avoidable complexity

### Mark as Low / Optional if

- the suggestion is stylistic and multiple valid choices exist
- the issue is already covered nearby
- the cited pattern/example does not actually support the comment strongly

### Mark as Invalid if

- the issue no longer exists in the current file state
- the suggestion would introduce a regression
- the reviewer referenced the wrong file, wrong line, or wrong example

### Treat as PR-level blockers if

- the PR does not actually deliver its stated goal
- the implementation is riskier than the value it adds
- the PR mixes unrelated changes that hide review risk
- required tests, migrations, or rollout safeguards are missing for a risky change
- CI/check failures or code evidence leave merge confidence too low

## Notes

- Bot reviewers often cite stale lines. Verify against the current file.
- A reviewer can be wrong on the exact line and still right about the underlying issue.
- If comments are sparse or low quality, perform an independent PR review anyway.
