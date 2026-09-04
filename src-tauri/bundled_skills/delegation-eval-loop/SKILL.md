---
name: delegation-eval-loop
description: >
  Generator-evaluator loop for delegated sub-agent work: parent evaluates,
  child generates. Use layered verification (Deterministic, Invariant Gate,
  Trajectory/reliability, Semantic), sprint contracts with checkbox acceptance
  criteria, and targeted re-steers via agent__messageToSession. Use when
  briefing with strict acceptance criteria, evaluating child results from
  agent__checkSession, preventing premature "done" without proof, or when
  delegate routes here for high-stakes handoffs. Triggers: generator-evaluator,
  generator evaluator, delegation eval, subagent evaluation, verify subagent,
  delegation loop, eval loop, proof before done, acceptance criteria,
  delegation-eval-loop.
---

# Delegation Eval Loop

**Generator–evaluator:** the child session **generates** (implements, researches, edits); the **parent** session **evaluates** and alone decides acceptance. Never treat the child's self-reported success as the final grade.

For spawn, isolation, workspace inheritance, and tool naming, follow **`delegate` first**. This skill owns briefing contracts, layered grading, reject/re-steer, and bounded retries.

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

When calling `agent__startSession` (or assigning via `agent__messageToSession`), include:

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
3. `agent__messageToSession(sessionId, message)`
4. Return to Step 2

Templates: [handoff-templates.md](references/handoff-templates.md).

### 5. Final Acceptance

When all layers pass, synthesize for the user with verified evidence (commands run, scope clean). Mark which criteria were checked.
