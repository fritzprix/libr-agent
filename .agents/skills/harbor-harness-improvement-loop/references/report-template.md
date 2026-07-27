# Harness improvement cycle report

Write in the user's language. Replace all placeholders.

```markdown
# Harbor harness improvement cycle: <cycle-id>

## Decision

**adopt | revert | iterate | inconclusive**

One sentence explaining the decision.

## Experiment contract

| Field                      | Baseline | Candidate | Comparable? |
| -------------------------- | -------- | --------- | ----------- |
| Git revision               |          |           |             |
| Dataset/tasks              |          |           |             |
| Attempts/concurrency       |          |           |             |
| Model/provider             |          |           |             |
| Assistant                  |          |           |             |
| Execution/workspace mode   |          |           |             |
| Timeout/resources/verifier |          |           |             |

## Outcome summary

Report distributions or numerator/denominator where available. Do not report a
percentage without its count.

| Metric                        | Baseline | Candidate | Difference |
| ----------------------------- | -------: | --------: | ---------: |
| Reward / pass count           |          |           |            |
| Error count                   |          |           |            |
| Median turns                  |          |           |            |
| Median tool calls             |          |           |            |
| Median input tokens           |          |           |            |
| Median output tokens          |          |           |            |
| Cache ratio                   |          |           |            |
| Repeated-call signals         |          |           |            |
| Heuristic failed-call signals |          |           |            |

## Trace findings

### <finding>

- Evidence level: E0–E4
- Measured fact:
- First divergence:
- Downstream effect:
- Successful counterexample check:
- Relevant source:
- Interpretation:
- Remaining uncertainty:

## Root-cause hypotheses

| Priority | Hypothesis | Owning layer | Evidence | Breadth | Risk | Confidence |
| -------- | ---------- | ------------ | -------- | ------- | ---- | ---------- |
|          |            |              |          |         |      |            |

## Intervention

- Single variable changed:
- Why this layer owns the fix:
- Expected observable effect:
- Acceptance criteria:
- Regression/overfitting guard:

## Verification

- Focused tests:
- `pnpm refactor:validate`:
- Matched targeted rerun:
- Broader/held-out rerun:
- Unexpected effects:

## Next cycle

One next hypothesis or instrumentation gap. Do not queue unrelated cleanup.
```
