---
name: trace-eval-loop
description: >
  Analyze arbitrary session-exported ATIF (Agent Tool Interaction Format /
  Agent Trajectory Interchange Format, ATIF-v1.7) trajectories — not Harbor
  jobs/ and not raw .trace.json — reconstruct intent→tool→observation sequences,
  and derive evidence-backed improvements for LibrAgent builtin tools, prompts,
  execution policy, and export/harness contracts. Use for: session ATIF eval
  loops, exported *_trajectory.json review, tool/harness postmortems from UI
  export, "ATIF 분석", "AITF 분석", "trace-eval-loop", or "세션 trajectory 개선".
  Not for Harbor jobs/ BM cycles (harbor-harness-improvement-loop /
  jobs-trace-analyzer), live Session API E2E (libr-tool-e2e-harness-loop), or
  raw .trace.json dumps (trace-analyzer).
---

# Trace Eval Loop (Session ATIF)

Repeatable **export → analyze → diagnose → improve → re-export** cycle for
**session-exported ATIF-v1.7** trajectories.

Optimize tool/harness contracts (schema, handler, response/hints, prompt
assembly, export fidelity). **Don't care:** model, serving engine.

## Relationship to sibling skills

| Input / goal | Use |
| --- | --- |
| UI/session `*_trajectory.json` (ATIF-v1.7) → tool/harness backlog | **trace-eval-loop** (this skill) |
| `jobs/<run>/` Harbor rewards + ATIF BM cycles | `harbor-harness-improvement-loop` / `jobs-trace-analyzer` |
| Live Session API messages E2E | `libr-tool-e2e-harness-loop` |
| Raw `.trace.json` conversation dump | `trace-analyzer` |

Session API has **no** ATIF export endpoint. Obtain ATIF via UI session export
(`format: 'atif'`) or an already-exported file the user provides. See
`docs/guides/session-export-formats.md`.

## Hard rules

- Separate **measured fact**, **trace interpretation**, and **hypothesis**.
- Keyword hits in observation text are **heuristics**, not verified tool failures.
- Prefer the **narrowest owning layer** (schema → handler → response/hints →
  prompt → execution policy → export/telemetry). Do not paper over a contract
  bug with a prompt tip.
- One primary causal change per cycle when implementing. Ask before editing
  git-tracked product files.
- Artifacts under `.libragent/work/trace-eval/<cycle-id>/` unless the user says
  otherwise.
- Do not run `pnpm refactor:validate` unless explicitly requested.
- No harness/tool contract broken → report the tool **pattern** and **stop**.

## Cycle

Default max cycles: **3** (override if user says). Freeze `cycle-id` like
`20261007-session-export-1`.

### 1. Freeze the eval contract

Write `contract.md`:

- Source session id / title (if known), git revision, assistant id
- ATIF file path(s) and how they were obtained (UI export vs copy)
- User goal / success criteria for the original session (from first user step)
- In-scope tools / out-of-scope (model choice never in scope)
- Pass/fail for this eval (e.g. “no repeated identical tool call ≥3”, “no
  oversized observation without pagination hint”)

### 2. Inventory and run the analyzer

Accept one file, many files, or a directory of `*_trajectory.json` /
`trajectory.json`:

```bash
python .agents/skills/trace-eval-loop/scripts/analyze_atif_trace.py \
  path/to/session_trajectory.json \
  --output .libragent/work/trace-eval/<cycle-id>/summary.json \
  --markdown .libragent/work/trace-eval/<cycle-id>/summary.md
```

Multiple inputs:

```bash
python .agents/skills/trace-eval-loop/scripts/analyze_atif_trace.py \
  path/a_trajectory.json path/b_trajectory.json exports/ \
  --output .libragent/work/trace-eval/<cycle-id>/summary.json
```

Copy or symlink the raw ATIF into the cycle dir as `trajectory.json` (or keep
original names listed in `contract.md`).

Before deep interpretation, flag:

- missing/`schema_version` ≠ ATIF-v1.7
- empty `steps` / empty-trajectory placeholder message
- orphan tool observations
- absent `final_metrics` token fields
- unequal coverage when comparing baseline vs candidate exports

### 3. Reconstruct and classify

For each trajectory (and for representative slices when many):

1. Rebuild `intent → tool call → observation → next tool` from steps (not memory).
2. Find the **first material divergence** from an efficient path.
3. Count: invalid/missing args, adjacent repeats, heuristic error observations,
   oversized observations, unknown tools, skipped verification, unrecovered errors.
4. Read [references/evidence-model.md](references/evidence-model.md) before
   assigning root cause.

### 4. Map symptom → owning layer

| Symptom | Inspect first |
| --- | --- |
| Correct tool never selected / unavailable | exposure, name, description, input schema |
| Invalid/missing arguments | schema required/enums, validation error text |
| Same failed call repeats | error recovery hint, state feedback, loop/escalation |
| Success but misleading or bloated result | handler output, truncation/pagination, structured content, hints |
| Many micro-tools for one outcome | tool boundaries / consolidation |
| Skips planning/verification across tools | assistant/system/workspace prompt |
| Export missing steps / wrong observation pairing | `session_export` ATIF builder / filter rules |
| Metrics missing or contradictory | export `final_metrics`, adapter aggregation |

Use `critique-builtin-tool`, `lean-builtin-tool-auditor`, or
`refactor-builtin-tool` when evidence points at builtin implementation details.

### 5. Rank improvements

Write `tasks.md` using [references/improvement-backlog.md](references/improvement-backlog.md).

Each item: evidence pointer (step_id / tool_call_id), owning paths, minimal
change, re-eval acceptance check, confidence (high/medium/low).

Prefer repeated, high-confidence contract defects. Keep low-confidence ideas in
a backlog — do not convert every correlation into a work item.

### 6. Act (only if user approved patching)

1. Apply the smallest fix for the top task (or user-named task).
2. Focused tests (`pnpm rust:test --test …` / targeted lint).
3. Re-run the same user scenario (or comparable session), **re-export ATIF**.
4. Bump `cycle-id`, analyze again, write `delta.md`.

Planning-only requests: stop after `analysis.md` + `tasks.md` and summarize.

### 7. Record

```text
.libragent/work/trace-eval/<cycle-id>/
├── contract.md
├── trajectory.json          # or originals + manifest
├── summary.json
├── summary.md
├── analysis.md
├── tasks.md
└── delta.md                 # from cycle 2+
```

Use [references/report-template.md](references/report-template.md) for
`analysis.md`. End with **adopt | defer | iterate | inconclusive**.

## Stop conditions

Stop and report instead of changing code when:

- ATIF is invalid / empty / incomparable across cycles
- telemetry/export is insufficient to localize the issue
- the only evidence is hidden `reasoning_content`
- variance exceeds the observed difference (multi-trace)
- the tool pattern is clear but no schema/handler/response/export contract is broken
