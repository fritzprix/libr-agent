---
title: Orchestration examples
description: 'Copy-ready prompts, internals, and common pitfalls for delegate, divide-conquer, hub-spoke, pipeline, consensus, and Org — scenario by scenario.'
---

# Orchestration examples

> How to apply the patterns in [Sub-agents & orchestration](sub-agents.md) to real situations.  
> Copy the prompts and swap in your values. Each example has **when / prompt / what happens inside / result / watch out**.

Start by choosing: if one agent is enough, **do not delegate**. Escalate structure only for parallel work, independent review, or a long-lived team.

```text
One agent is enough?               → just chat
Hand one chunk to someone else     → delegate
Several independent pieces         → divide-conquer
Each step feeds the next           → pipeline
Same question, several viewpoints  → consensus-delegation
Central lead directs specialists   → hub-spoke
Make → strict review, repeat       → gatekeeper
Long-lived team + Org view         → teamwork → org
```

---

## Example 1. One independent reviewer (`delegate`)

**When**: you want your work reviewed by "someone other than me".

```text
@skill:delegate
Review docs/proposal.md in the current work folder from an investor's point of view.
- Share the current work folder so you can read the file directly
- Do not edit; in your final reply give "3 strengths / 3 weaknesses / 5 must-ask questions"
```

**What happens inside**

1. The parent checks for an idle reviewer child; if none, creates one (work folder **shared**)
2. The child reads the file and returns its result as the final reply
3. The parent confirms `workspace: SHARED` and the scope, then relays the result

**Watch out**: if the child says "file not found", its folder was **isolated**. State the sharing explicitly or paste the content into the brief and retry. If you asked for an independent review and the parent graded itself instead, ask again.

---

## Example 2. Parallel audit by module (`divide-conquer`)

**When**: front end, back end, and docs can be audited independently at the same time.

```text
@skill:divide-conquer
Audit the repo in three parallel pieces:
① src/ui/**  accessibility and rendering issues
② src-tauri/** error handling and unsafe usage
③ docs/**  broken links and stale explanations
Save each to report/<piece>.md, then merge into one report/SUMMARY.md.
Do not fix anything; report findings only.
```

**What happens inside**

1. The parent splits the work into three non-overlapping pieces and assigns each a unique output file
2. It launches three children **asynchronously** within the active-session limit (default 4) and prepares the merge template while they run
3. It confirms each child with `agent__checkSession` and merges
4. Only empty or weak pieces get a follow-up

**Watch out**: if pieces share a folder, **separate file names** so they cannot overwrite each other. Ask for a final check that the summary does not contradict any individual report.

---

## Example 3. A post in stages (`pipeline`)

**When**: each stage's output is the next stage's input.

```text
@skill:pipeline
Produce a blog post: "Automating weekly reports with a local agent".
1) Research: 5+ sources, summary and links in drafts/01-research.md
2) Draft: outline and body based on 01, drafts/02-draft.md
3) Review: fact and tone check with suggested edits, drafts/03-review.md
4) Polish: final version applying the review, final.md
Each stage receives only the previous stage's file path and summary.
```

**What happens inside**: all stages share one folder, and the next stage gets **a summary + paths**, not the full conversation. If stage 3 finds problems, only stage 2 is re-run.

**Watch out**: if the stages are independent, `divide-conquer` is faster than `pipeline`.

---

## Example 4. A PR from several viewpoints (`consensus-delegation` + `delegation-eval-loop`)

**When**: before merging, you want security, performance, and maintainability compared, with disagreements reconciled.

```text
@skill:consensus-delegation
Review PR #1234 (diff: git diff main...HEAD) independently from three viewpoints:
- security / performance / maintainability
- everyone answers in the same template (Verdict · Confidence · Findings · Risks)
- no code changes, share the current work folder
Ask follow-ups only on disputed items, then give a final recommendation
(merge / conditional merge / reject).
```

**Example result**

| Viewpoint       | Verdict              | Key evidence                                      |
| --------------- | -------------------- | ------------------------------------------------- |
| Security        | approve-with-caveats | one missing input validation (`api/upload.ts:42`) |
| Performance     | approve              | new query confirmed to use an index               |
| Maintainability | reject               | 300-line function, no tests                       |

The parent asks only the conflicting children about the clash (maintainability vs performance) and ties each point of the final recommendation to its evidence.

**Watch out**: inject viewpoints through the **brief**; you do not need a new assistant per lens. Two to four panelists is plenty.

---

## Example 5. A coordinator directing specialists (`hub-spoke`)

**When**: research, analysis, and drafting are different jobs that one lead must integrate.

```text
@skill:hub-spoke
Write a comparison report on three competing products (A, B, C).
Hub: you. Spokes: ① features ② pricing and licensing ③ community and release history
Each spoke reports only to the hub with a one-page summary and source links in its final reply.
The hub receives summaries only, then writes the comparison table and "3 things we should copy".
```

**What happens inside**: the hub builds a task list with dependencies; spokes work in parallel. If ③ needs ②'s output, the hub **relays** a summary (spokes never talk directly).

**Watch out**: letting the hub read every spoke's full log overflows its context. Put "summaries and file lists only" in the brief.

---

## Example 6. A fix with strict acceptance (`delegate` + `delegation-eval-loop`)

**When**: a code change where you need **evidence**, not "it's done".

```text
@skill:delegate @skill:delegation-eval-loop
Fix the checkout rounding bug.
- Allowed paths: src/checkout/**, tests/checkout/**  (no other edits)
- Acceptance
  - [ ] pnpm test checkout passes
  - [ ] a regression test that reproduces the bug is added
  - [ ] no unrelated files changed
- Paste the commands you ran and their raw output in your final reply
```

**What happens inside**

1. The parent puts the contract (objective, allowed paths, checklist) in the brief
2. After the child finishes, the parent **runs the tests itself** and checks scope with `git diff --stat`
3. On failure it re-steers with the raw error output (3–5 cycles max); only a pass is reported to you

**Watch out**: the child's "tests passed" is not evidence. Cancelled runs, or "done" without evidence, are treated as failures.

---

## Example 7. A long-lived team (`teamwork` → `org`)

**When**: the same team continues over weeks and you want one place to see its state.

```text
@skill:teamwork
Create a "monthly market trends report" task force.
Roles: researcher, data analyst, editor. Coordination: hub-and-spoke.
Deliverable: reports/YYYY-MM.md each month, final approval by the editor.
```

Once it is prepared:

```text
@skill:org
Create the Org from the team you just prepared and start with the researcher.
```

**What happens inside**

1. `teamwork` writes the mission, roles, and coordination rules to an app-local folder
2. `org` creates the Org once from the root session and spawns members inside it
3. The team appears in the sidebar **Org** view; next month you continue from the **Org root**

Refine it later:

```text
@skill:org-restructure
Merge the data analyst into the researcher and add the editor's approval criteria to ROLES.md.
```

**Watch out**: one-off delegations do not appear in Org. For recurring runs use [Scheduled Tasks](scheduled-tasks.md), not Org. To move a team to another machine, use **Export Org / Import Org** in the Org view.

---

## Example 8. Waiting for something (`call-me-back` / `loop`)

When you need to be **woken up**, not to delegate.

```text
@skill:call-me-back
Run the full build in the background and wake this session when it finishes, then summarize the failing tests.
```

```text
@skill:loop
Check this PR's CI status again in 30 minutes.
```

- Waiting for a process or webhook to **finish** → `call-me-back`
- Looking again after some **time** → `loop` (inside this chat); use `schedule` if it must run with the app closed

---

## Common mistakes and fixes

| Symptom                                   | Cause                            | Fix                                                                       |
| ----------------------------------------- | -------------------------------- | ------------------------------------------------------------------------- |
| Child cannot find files, rules, or skills | Work folder is isolated          | State "share the current work folder" or include the content in the brief |
| Parent cannot read the child's result     | Child only saved to a file       | Require the deliverable **in the final reply**                            |
| Parallel children overwrite each other    | Shared paths                     | Separate file names or subfolders per child                               |
| New children keep appearing               | Idle children not reused         | Ask to reuse an idle same-role child with `reset`                         |
| "Spawn rejected (depth / fanout)"         | Lineage limit exceeded           | Reduce nested delegation or flatten the structure                         |
| Parallel work is slow                     | Active-session limit (default 4) | Use smaller batches or split into stages                                  |
| Endless review loop                       | Vague criteria                   | Give checkbox criteria and a max rework count (default 3)                 |
| One-off delegation not in Org             | Expected behavior                | Use `teamwork` → `org` if you need Org                                    |

For more diagnostics see [Troubleshooting](troubleshooting.md).

---

## Related

- [Sub-agents & orchestration](sub-agents.md)
- [Skills](skills.md)
- [Scheduled Tasks](scheduled-tasks.md)
- [Automation overview](automation.md)
