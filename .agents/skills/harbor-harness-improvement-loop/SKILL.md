---
name: harbor-harness-improvement-loop
description: Analyze Harbor benchmark jobs and ATIF trajectories, identify evidence-backed weaknesses across LibrAgent builtin tools, prompts, agent guidance, execution policy, and benchmark instrumentation, then design controlled improvements and rerun comparisons. Use when analyzing Harbor or Terminal-Bench results, optimizing the harness, investigating tool-call failures or retries, auditing prompt/context efficiency, or running repeated BM → analysis → improvement cycles.
---

# Harbor Harness Improvement Loop

Build a repeatable **benchmark → diagnosis → smallest intervention → rerun**
cycle. Optimize general agent capability, not benchmark-specific shortcuts.

## Hard rules

- Treat reward as an outcome, not a cause. Inspect trajectories and source before
  recommending changes.
- Separate **measured fact**, **trace interpretation**, and **hypothesis**.
- Do not claim causality from one trial unless a deterministic contract violation
  is visible. Label single-trial findings as hypotheses.
- Compare runs only when dataset/task selection, model/provider, assistant,
  attempts, concurrency, execution mode, workspace mode, timeout/resource policy,
  and verifier configuration are equivalent.
- Never modify benchmark tasks, verifiers, official timeout/resource limits, or
  add task-specific prompt clues to improve scores.
- Do not recommend a prompt change when a schema, handler, response, or
  instrumentation fix is the narrower source-of-truth solution.
- Do not duplicate the same instruction across system prompt, assistant prompt,
  tool description, and result hint.
- Ask before editing protected or git-tracked product files. Reports and experiment
  ledgers go under `.libragent/work/`.

## Cycle

### 1. Freeze the experiment contract

Record:

- job paths and git revision
- dataset, included tasks, attempts, and concurrency
- reported model/provider and actual session model/provider
- assistant ID/version, execution mode, workspace mode
- timeout/resource and verifier settings

If these cannot be established, analyze the run but mark cross-run conclusions
as unverified.

### 2. Inventory and validate artifacts

Collect recursively:

- job `result.json`
- trial `verifier/reward.txt`
- agent `trajectory.json` (ATIF)
- relevant adapter/session logs when present

Run the bundled analyzer:

```bash
python .agents/skills/harbor-harness-improvement-loop/scripts/analyze_harbor_results.py \
  jobs/<baseline> [jobs/<candidate>] \
  --output .libragent/work/harbor-harness-cycle/summary.json
```

The script produces descriptive metrics and explicitly labels heuristic error
signals. It does not establish causality.

Before interpretation, flag:

- missing or invalid trajectories
- reward without completed trajectory
- absent token data
- model/workspace-mode mismatch
- errors, cancelled/incomplete runs, and verifier failures
- unequal task or attempt coverage between runs

### 3. Analyze outcomes and traces

Analyze successful and failed trials separately. For each representative trace:

1. Reconstruct the sequence: intent → tool call → observation → next decision.
2. Find the **first divergence** from an efficient successful path.
3. Count tool selection, invalid arguments, retries, repeated calls, oversized
   outputs, missing verification, premature completion, and unrecovered errors.
4. Compare with successful traces for the same task family.
5. Check token/turn/tool-call distributions; do not rely on means alone.

Read [references/evidence-model.md](references/evidence-model.md) before assigning
a root cause.

### 4. Map each symptom to the owning layer

Use the narrowest layer that can fix the observed contract:

| Symptom                                                  | Inspect first                                                         |
| -------------------------------------------------------- | --------------------------------------------------------------------- |
| Correct tool is unavailable or consistently not selected | tool exposure, name, description, input schema                        |
| Tool selected with invalid/missing arguments             | schema required fields, enums, examples, validation error             |
| Same failed action repeats                               | error recovery hint, state feedback, loop/escalation prompt           |
| Tool succeeds but result misleads or bloats context      | handler output, truncation/pagination, structured content, hints      |
| Agent uses many micro-tools for one outcome              | tool boundaries, consolidation, backend automation                    |
| Agent skips planning/verification across unrelated tools | assistant/system/workspace prompt                                     |
| Prompt tokens grow or cache ratio degrades               | stable/volatile prompt split, tool schema volume, repeated context    |
| Correct actions still produce wrong workspace state      | handler semantics, isolation/sync, process lifecycle                  |
| Metrics are missing or contradictory                     | Harbor adapter, Session API telemetry, aggregation script             |
| Failure varies only by model/configuration               | model binding, provider behavior, sampling/config; do not blame tools |

Inspect the actual schema, dispatcher, handler, response builder, prompt assembly,
and tests for the suspected layer. Use `critique-builtin-tool`,
`lean-builtin-tool-auditor`, or `refactor-builtin-tool` when the evidence points
to builtin implementation details.

### 5. Form and rank hypotheses

For every proposal record:

- observed evidence and affected trial count
- successful-trace counterexample check
- owning layer and inspected code paths
- mechanism: why the change should alter behavior
- expected measurable effect
- regression and benchmark-overfitting risk
- confidence: high / medium / low

Prefer broad, repeated, high-confidence defects. Keep low-confidence ideas in a
backlog. Do not convert all correlations into work items.

### 6. Choose the smallest controlled intervention

Change one causal variable per experiment when practical. Examples:

- clarify one ambiguous schema field
- make one error response actionable and state-aware
- trim or paginate one oversized result
- hide one internal tool
- move one repeated instruction to its owning layer
- add missing telemetry without changing agent behavior

Define acceptance criteria before editing. Preserve a held-out task set to detect
overfitting.

### 7. Implement and verify

After user approval:

1. Apply the minimal change.
2. Run focused unit/contract tests.
3. Run `pnpm refactor:validate`.
4. Rerun a small matched task slice.
5. If the signal is positive and no regression appears, rerun the broader suite
   with multiple attempts.

Do not call a change successful from reward alone. Compare:

- task reward/pass rate and errors
- turns and total/per-task tool calls
- input/output/cache tokens
- repeated/failed calls and recovery rate
- completion integrity and workspace correctness

### 8. Record and continue

Write each cycle under:

```text
.libragent/work/harbor-harness-cycle/<cycle-id>/
├── experiment.md
├── baseline-summary.json
├── candidate-summary.json
└── analysis.md
```

Use [references/report-template.md](references/report-template.md). End each cycle
with one decision: **adopt**, **revert**, **iterate**, or **inconclusive**.

## Stop conditions

Stop and report instead of changing code when:

- baseline and candidate are not comparable
- telemetry is insufficient to localize the issue
- the proposed fix depends on benchmark-specific knowledge
- the only evidence is hidden reasoning text
- variance exceeds the observed difference
- the issue belongs to model/provider behavior and no harness contract is broken
