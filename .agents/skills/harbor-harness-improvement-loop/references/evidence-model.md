# Evidence and root-cause model

## Evidence levels

Use the strongest available level and state it in the report.

| Level | Meaning                                                                                         | Allowed conclusion               |
| ----- | ----------------------------------------------------------------------------------------------- | -------------------------------- |
| E0    | Aggregate reward/token correlation only                                                         | Observation; no root-cause claim |
| E1    | One trace shows a plausible failure sequence                                                    | Low-confidence hypothesis        |
| E2    | Repeated traces show the same divergence, or schema/handler contract is deterministically wrong | Medium-confidence root cause     |
| E3    | Matched controlled rerun changes the predicted metric without material regressions              | High-confidence causal support   |
| E4    | Repeated broader/held-out rerun confirms the effect                                             | Adoptable general improvement    |

Reasoning text is supporting context, not ground truth. Tool calls, observations,
workspace state, verifier output, API payloads, and source contracts are stronger
evidence.

## Trace classification

For each trial, classify the first material divergence:

- **Discovery**: did not find relevant files/state/tool.
- **Selection**: chose a tool or strategy that cannot achieve the goal.
- **Invocation**: wrong argument, path, mode, or ordering.
- **Execution**: handler/process/isolation failed despite a valid call.
- **Interpretation**: misunderstood correct tool output.
- **Recovery**: did not adapt after failure, repeated action, or ignored hint.
- **Verification**: stopped without checking the required outcome.
- **Reporting**: workspace is correct but final response/format violates task.
- **Harness telemetry**: execution may be valid but measurements are absent,
  inconsistent, truncated, or attributed to the wrong model/trial.

Do not assign multiple speculative root causes. Record the first divergence and
downstream consequences separately.

## Counterfactual checks

Before blaming a harness layer, ask:

1. Did successful traces receive the same prompt/tool contract?
2. Did they use the allegedly defective tool successfully?
3. Is failure concentrated by task family, workspace mode, model, or provider?
4. Could the verifier or environment explain the result?
5. Would the proposed change have been visible to the agent before divergence?
6. Does the source code confirm the assumed schema/behavior?

If a successful counterexample disproves a universal claim, narrow the hypothesis
instead of ignoring the counterexample.

## Intervention ownership

Choose one owner:

- **Tool exposure/schema**: discoverability, naming, inputs, defaults, enums.
- **Tool handler**: correctness, lifecycle, validation, state mutation.
- **Tool response/hints**: output signal, size, next-step/recovery guidance.
- **Prompt/context**: cross-tool strategy, planning, verification, stable/volatile
  placement, contradictory instructions.
- **Execution harness**: approval mode, workspace isolation/sync, process routing.
- **Benchmark adapter/telemetry**: trajectory conversion, model attribution,
  timing, aggregation.
- **Model/config**: provider/model capability or sampling behavior without a
  broken harness contract.
- **Task/environment**: benchmark setup, verifier, nondeterminism, external outage.

The narrowest authoritative owner should change. Broad prompts must not compensate
for a deterministic tool contract defect.

## Guardrails against benchmark gaming

Reject proposals that:

- mention benchmark task names, expected answers, or verifier implementation in
  production prompts/tools
- detect Harbor specifically to alter agent behavior
- relax correctness checks only to raise reward
- increase timeout/resources contrary to submission rules
- optimize only the measured subset without a held-out check

General improvements discovered through benchmark evidence are allowed: clearer
schemas, correct handlers, concise outputs, robust recovery, better telemetry, and
task-agnostic planning/verification guidance.
