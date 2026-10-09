# Evidence and root-cause model (session ATIF)

## Evidence levels

| Level | Meaning | Allowed conclusion |
| --- | --- | --- |
| E0 | Aggregate tool/token correlation only | Observation; no root-cause claim |
| E1 | One trajectory shows a plausible failure sequence | Low-confidence hypothesis |
| E2 | Repeated trajectories (or same session segments) show the same divergence, or a schema/handler/export contract is deterministically wrong | Medium-confidence root cause |
| E3 | Controlled re-export after a single intervention changes the predicted metric without material regressions | High-confidence causal support |
| E4 | Repeated re-runs / held-out sessions confirm the effect | Adoptable general improvement |

`reasoning_content` is supporting context, not ground truth. Prefer tool calls,
observations, workspace outcomes, and source contracts.

Don't-care (never a cause): model, serving engine.

## Trace classification

Classify the **first** material divergence:

- **Discovery** — did not find relevant files/state/tool
- **Selection** — chose a tool/strategy that cannot achieve the goal
- **Invocation** — wrong argument, path, mode, or ordering
- **Execution** — handler/process/isolation failed despite a valid call
- **Interpretation** — misunderstood correct tool output
- **Recovery** — did not adapt after failure; repeated action; ignored hint
- **Verification** — stopped without checking the required outcome
- **Reporting** — work is correct but final user-facing response is wrong
- **Export/telemetry** — session may be valid but ATIF pairing, filtering, or
  metrics are missing/inconsistent

Record downstream consequences separately. Do not pile speculative root causes.

## Observation heuristics

`heuristic_error_observations` from the analyzer = keyword hits in observation
text. Treat as triage signal only. Verify against structured error fields,
handler contracts, and source before claiming a tool failure.

## Counterfactual checks

Before blaming a harness layer:

1. Did another segment/session succeed with the same tool contract?
2. Did the allegedly defective tool succeed earlier in the same trajectory?
3. Is failure concentrated by tool family or workspace mode?
4. Could the user brief / missing precondition explain the result?
5. Would the proposed change have been visible to the agent before divergence?
6. Does source code confirm the assumed schema/behavior?

If a successful counterexample disproves a universal claim, narrow the hypothesis.

## Intervention ownership

Choose one owner:

- **Tool exposure/schema** — discoverability, naming, inputs, defaults, enums
- **Tool handler** — correctness, lifecycle, validation, state mutation
- **Tool response/hints** — signal, size, next-step/recovery guidance
- **Prompt/context** — cross-tool strategy, planning, verification placement
- **Execution policy** — approval/YOLO, isolation/sync, process routing
- **Session export / ATIF builder** — filtering, observation pairing, metrics
- **Environment/precondition** — missing deps, offline bridge, stale build

If the harness/tool contract is intact, stop after documenting the pattern.
