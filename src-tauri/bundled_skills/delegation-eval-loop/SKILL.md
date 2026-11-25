---
name: delegation-eval-loop
description: >
  Verify delegated work before accepting it: the parent evaluates a child with
  layered checks, or a separate reviewer session gates a creator (gatekeeper).
  Use for strict acceptance criteria, proof before done, or rework loops.
  Triggers: generator-evaluator, delegation eval, verify subagent, acceptance criteria, gatekeeper,
  게이트키퍼, 리뷰 루프, 검증 게이트, 품질 검증, reviewer loop, quality gate.
---

# Delegation Eval Loop

**Generator–evaluator:** the child session **generates** (implements, researches, edits); the **parent** session **evaluates** and alone decides acceptance. Never treat the child's self-reported success as the final grade.

For spawn, isolation, workspace inheritance, and tool naming, follow **`delegate` first**. This skill owns briefing contracts, layered grading, reject/re-steer, and bounded retries.

## Choose the pattern

| Situation | Pattern |
| --- | --- |
| The parent can check the result itself (run tests, inspect diffs, compare to requirements) | **Parent evaluates** — the workflow in this file |
| The review needs its own context, role, or model, or must stay independent of the parent's plan (strict code review, compliance or style audit, long rework cycles) | **Gatekeeper** — a separate Reviewer session gates a Creator session: [gatekeeper.md](references/gatekeeper.md) |

Default to **Parent evaluates**. It needs fewer sessions and the parent keeps the original requirements. Use **Gatekeeper** only when an independent reviewer adds real value. Both patterns use bounded rework (3–5 cycles) and the same evidence rules below.

## Core Rules

1. **Evidence over assertion**: "all tests pass" from the child is a hypothesis. Run or inspect deterministic verification in the parent (or from parent-visible raw output).
2. **Layered grading order**: Deterministic → Invariant → Trajectory/reliability → Semantic. Fail fast on earlier layers.
3. **Sprint contract before spawn**: Negotiate objective, authorized paths, and checkbox acceptance criteria in the brief. Do not invent pass criteria after the child returns.
4. **Targeted feedback**: On reject, send exact failure output via `agent__messageToSession` — not "try again".
5. **Bounded loop**: Cap at 3–5 cycles. Then escalate to the user with diagnostics; do not spin forever.
6. **Reliability hard-fail**: Incomplete, cancelled, timed-out, circuit-broken, or evidence-free runs fail evaluation even if the prose claims completion.

## The 4-Layer Evaluation Stack

| Layer | Check | Verification Method | Action on Failure |
|---|---|---|---|
| **1. Deterministic** | Build, types, tests, lint | Parent runs the agreed commands (or requires raw exit-0 output in evidence) | Reject with compiler/test log |
| **2. Invariant Gate** | Path/policy boundaries | `git status` / `git diff --stat` vs authorized paths | Reject; force revert of forbidden edits |
| **3. Trajectory / reliability** | Real execution, no fake done | Result text + actions: missing cmds, loops, cancel/incomplete signals | Reject; demand missing evidence or stop |
| **4. Semantic** | User intent, edge cases | Parent LLM vs original requirements (only after 1–3 pass) | Reject with specific missed requirements |

Composite cheerleading is forbidden: Layer 1 pass does **not** override Layer 2/3 failure.

## Workflow

### 1. Brief with a Sprint Contract

When calling `agent__spawnSession(..., complexity="...")` or `agent__messageToSession(..., complexity="...")`, include:

- **Exact Objective** — concrete deliverable
- **Authorized paths / FORBIDDEN paths** — invariant gate inputs
- **Acceptance Criteria** — checkbox list of commands/checks that must pass (`- [ ]`)
- **Deliverable Channel** — status + raw evidence in **final text** (not child scratchpad)

See [handoff-templates.md](references/handoff-templates.md).

### 2. Poll and Capture Child Result

- `agent__checkSession` until terminal idle/error (or use `wait=true` when appropriate)
- Capture final text. If the session errored, cancelled, or returned empty/incomplete diagnostics → **Layer 3 fail**; do not soft-accept.

### 3. Execute Layered Evaluation

Do **not** present the result to the user until layers pass (or you escalate).

1. Deterministic checks in the child's effective workspace (see Metadata `workspace:`)
2. Invariant gates (`git status` / scope)
3. Trajectory & reliability (claims vs evidence; incomplete/circuit-break)
4. Semantic review only if 1–3 pass

Details: [evaluation-protocol.md](references/evaluation-protocol.md).

### 4. Reject & Re-steer

If any layer fails:

1. Increment iteration counter (abort if over max, typically 4)
2. Structured correction: which layer, exact error snippet, single next action
3. `agent__messageToSession(sessionId, complexity="...", message="...")`
4. Return to Step 2

Templates: [handoff-templates.md](references/handoff-templates.md).

### 5. Final Acceptance

When all layers pass, synthesize for the user with verified evidence (commands run, scope clean). Mark which criteria were checked.

## References

- [Evaluation protocol](references/evaluation-protocol.md)
- [Handoff templates](references/handoff-templates.md)
- [Gatekeeper pattern](references/gatekeeper.md) — [quality criteria](references/gatekeeper-criteria.md), [rework limit examples](references/gatekeeper-rework-limit-examples.md), [troubleshooting](references/gatekeeper-troubleshooting.md)
