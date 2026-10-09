# Session ATIF eval report template

Write in the user's language. Replace placeholders. Save as `analysis.md`.

```markdown
# Trace eval cycle: <cycle-id>

## Decision

**adopt | defer | iterate | inconclusive**

One sentence explaining the decision.

## Eval contract

| Field | Value |
| --- | --- |
| Git revision | |
| Session id / title | |
| ATIF path(s) | |
| Export provenance | UI export / provided file / other |
| User goal (from first user step) | |
| In-scope tools | |
| Comparable to prior cycle? | yes / no / n/a |

## Trajectory summary

Report counts, not bare percentages without denominators.

| Metric | Value |
| --- | ---: |
| Steps / agent steps | |
| Tool calls | |
| Distinct tools | |
| Adjacent repeated calls | |
| Heuristic error observations | |
| Oversized observations (≥N chars) | |
| Prompt / completion / cached tokens | |

Top tools by frequency: …

## Trace findings

### <finding>

- Evidence level: E0–E4
- Measured fact (step_id / tool_call_id):
- First divergence class:
- Downstream effect:
- Successful counterexample check:
- Relevant source:
- Interpretation:
- Remaining uncertainty:

## Root-cause hypotheses

| Priority | Hypothesis | Owning layer | Evidence | Breadth | Risk | Confidence |
| --- | --- | --- | --- | --- | --- | --- |
| | | | | | | |

## Intervention (if acting)

- Single variable changed:
- Why this layer owns the fix:
- Expected observable effect:
- Acceptance criteria (next ATIF):
- Regression guard:

## Next cycle

One next **tool/harness** hypothesis or export/instrumentation gap.
Don't-care: model, serving engine.
```
