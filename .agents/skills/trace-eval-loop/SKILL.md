---
name: trace-eval-loop
description: >
  Extract, parse, and analyze agent session traces / trajectories (ATIF v1.x JSON,
  .trace.json, session messages export) to identify execution bottlenecks across
  Tool, Harness, and Prompt layers, and derive actionable, evidence-backed improvements
  for builtin tools and runtime harness. Use when reviewing session performance,
  diagnosing high token consumption or polling storms, debugging subagent coordination,
  or improving builtin MCP tools from real-world execution traces.
  Triggers: trace-eval-loop, trace_eval_loop, trace eval, 트레이스 분석, trajectory 분석,
  도구 구현 및 harness 개선안, trace postmortem, ATIF 분석, 세션 트레이스 평가.
---

# Trace Evaluation Loop

Extract and analyze agent execution trajectories (ATIF v1.x, `.trace.json`, session messages) to uncover root bottlenecks in **Builtin Tools**, **Runtime Harness**, and **Prompt Policies**, and derive concrete, prioritized improvement proposals.

## Core Rules

1. **Evidence over intuition**: Base every finding on measured facts from the trace: exact step IDs, tool call signatures, raw exit codes, observation error strings, and token metrics.
2. **Tri-layer root cause mapping**: Categorize every breakdown into its owning layer:
   - **Tool Layer**: Schema defects, non-idempotent operations, shell dependencies, missing tools.
   - **Harness Layer**: Polling storms, lack of reactive event push, context token explosion, ID truncation.
   - **Prompt / Policy Layer**: Instruction drift, lack of negative constraints, premature completion.
3. **No dual-state / dual-ID**: Never truncate or alias session IDs (Rule 1). If an agent used a 10-char prefix resulting in "Session not found", treat as a critical SSOT breach.
4. **Don't blame the model**: In a desktop agent harness, prompt and tool contracts must be robust. If a tool failed or looped, determine what tool hint, schema constraint, or harness safeguard was missing.

---

## Workflow

### 1. Trace Extraction via Session Chat UI

Extract the trajectory directly from the LibrAgent session chat window:

1. In the target agent session chat window, locate the top-right header toolbar.
2. Click the **Export** icon button (`FileDown` icon / `세션 내보내기`).
3. Select **"Export as ATIF trajectory (.json)"** (한글: **"ATIF 궤적으로 내보내기 (.json)"**).
4. Save the file to your local path (default: `Downloads/<Session_Title>_trajectory.json`, e.g. `AI Daily News TF_trajectory.json`).

*(Headless/API alternative: fetch messages via `GET /api/sessions/:id/messages?limit=1000` or invoke Tauri `export_session_file` with `{ format: "atif" }`).*

### 2. Automated Diagnostic Run

Run the bundled analyzer script on the exported trajectory file:

**PowerShell (Windows):**
```powershell
uv run python .agents/skills/trace-eval-loop/scripts/analyze_trace.py "<path-to-trajectory.json>" --output .libragent/work/trace-analysis/report.md
```

**Bash (Linux/macOS):**
```bash
python3 .agents/skills/trace-eval-loop/scripts/analyze_trace.py "<path-to-trajectory.json>" --output .libragent/work/trace-analysis/report.md
```

#### Diagnostic CLI Options:
- `--format summary`: Quick one-line overview (steps, tokens, tools, error count, polling checks).
- `--format markdown`: Full structured report (default).
- `--format json`: Machine-readable diagnostic payload.
- `--top <N>`: Adjust top errors and loops displayed (default: 15).

---

### 3. Deep-Dive Diagnostic Breakdown

Inspect the generated report and trace file across 5 key dimensions:

1. **Execution Scope & Token Efficiency**:
   - Total steps vs user turns (e.g., 800+ agent steps for 50 user turns indicates spinning or deep subagent delegation).
   - Prompt tokens (>20M tokens indicates lack of tool observation pruning/compaction).
   - Cache hit ratio (Gemini / Anthropic prompt caching efficiency).
2. **Tool Telemetry & Failure Hotspots**:
   - Identify tools with high error rates (`workspace__runPowerShell`, `agent__createAgent`, etc.).
   - Distinguish environment/shell errors (exit code 9009 = command not found) from logic errors.
3. **Subsession & Polling Overhead**:
   - Count calls to `agent__checkSession` and `agent__messageToSession`.
   - Identify polling storms (dozens of consecutive checks waiting for child sessions).
4. **Stuck Loops & Thrashing**:
   - Detect repeated calls (≥3 consecutive identical calls without state change).
   - Inspect `workspace__strReplace` target mismatches and `workspace__writeFile` rewrites.
5. **Human-in-the-Loop Friction Points**:
   - Trace user turns containing friction keywords (`왜`, `링크`, `누락`, `출처`, `에러`, `아니`).
   - Pinpoint recurring user complaints (e.g. missing source URLs, outdated time window, wrong search tool).

---

### 4. Layered Root Cause Mapping

Consult [references/bottleneck_taxonomy.md](references/bottleneck_taxonomy.md) to classify each failure:

| Layer | Symptom in Trace | Root Cause | Example Fix |
|---|---|---|---|
| **Tool Layer** | `Exit code 9009` (CLI not found)<br>`Assistant already exists` (409)<br>Browser used instead of search | Tool lacks preflight, lacks upsert idempotency, or search MCP tool is missing from session | Add get-or-create semantics; expose `exa__search` directly; provide native builtin tool |
| **Harness Layer** | 100+ `agent__checkSession` calls<br>20M+ prompt tokens<br>`Session 'xyz' not found` | Synchronous active polling storm; no observation compaction; ID truncation | Implement event-driven wakeup / long poll; compact old tool results; enforce full SSOT session IDs |
| **Prompt Layer** | User repeatedly reminds agent to add source URLs or enforce 24h window | Invariant constraints lost during multi-turn history accumulation | Re-anchor invariants in dynamic context or system prompt injection |

---

### 5. Formulating Actionable Improvement Backlog

Structure the findings into a prioritized action plan using [references/report_template.md](references/report_template.md):

- **P0 (Critical / Blocker)**:
  - Eliminates crashes, polling storms, and token explosions.
  - Fixes SSOT ID violations or infinite retry loops.
- **P1 (Reliability & Robustness)**:
  - Adds idempotency (`get-or-create` on creation tools).
  - Eliminates ambient shell dependencies by building native builtin tools.
  - Implements observation truncation / compaction.
- **P2 (Usability & Developer Experience)**:
  - Enriches next-step hints in tool results.
  - Improves diff feedback on string replace failures.

---

### 6. Verification & Closed-Loop Validation

1. **Tool Unit / Integration Test**:
   - For Rust backend changes: run `pnpm rust:test --test <target>` (never raw `cargo test`).
   - For TypeScript/frontend changes: run `pnpm test:run`.
2. **Harness E2E Re-run**:
   - Re-execute the scenario via `libr-delegate` or `libr-tool-e2e-harness-loop`.
   - Verify step count reduction, zero polling timeouts, and 100% adherence to user constraints.

---

## References

- [Bottleneck Taxonomy & Classification Catalog](references/bottleneck_taxonomy.md)
- [Trace Evaluation Report Template](references/report_template.md)
- Automated Analyzer Script: `scripts/analyze_trace.py`
