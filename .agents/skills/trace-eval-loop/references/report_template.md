# Trace Evaluation Report Template

Use this template when producing evidence-backed trajectory evaluation reports.

```markdown
# 🔍 Trajectory Evaluation Report: `<session-or-trace-name>`

## 1. Executive Summary

- **Source File**: `<path-to-trace.json>`
- **Session ID**: `<session-id>`
- **Agent Model & Version**: `<agent-name> <version>` (`<model-name>`)
- **Execution Scope**: `<total_steps>` steps (`<user_turns>` user turns, `<agent_turns>` agent turns)
- **Token Telemetry**: Prompt: `<prompt_tokens>` | Completion: `<completion_tokens>` | Cache Hit: `<cache_pct>%`
- **Tool Metrics**: `<total_tool_calls>` calls across `<distinct_tools>` tools | `<total_errors>` errors (`<error_pct>%` failure rate)
- **Primary Bottlenecks**:
  1. `<Brief 1-line bottleneck 1>`
  2. `<Brief 1-line bottleneck 2>`

---

## 2. Evidence-Based Telemetry

### 2.1 Tool Execution Breakdown
| Tool Name | Calls | Errors | Failure Rate | Output Size (KB) | Common Failure / Symptom |
|---|---|---|---|---|---|
| `<tool>` | `0` | `0` | `0.0%` | `0 KB` | `<symptom>` |

### 2.2 Sub-session Lineage & Coordination
| Child Session ID | Messages Sent | Polling Checks | Timeouts / Errors | Final Status |
|---|---|---|---|---|
| `<sid>` | `0` | `0` | `0` | `<Idle/Error/Cancelled>` |

### 2.3 User Intervention & Friction Points
- **Step `<id>`**: `<Quote of user correction or complaint>` (Trigger: `<keyword>`)

---

## 3. Root Cause Analysis by Layer

### 3.1 Tool Layer
- **[T1/T2/...] `<Bottleneck Title>`**:
  - **Measured Fact**: `<Exact error message, exit code, or tool call sequence>`
  - **Trace Interpretation**: `<Why this happened in this step>`
  - **Hypothesis / System Flaw**: `<Architectural flaw in tool design or contract>`

### 3.2 Harness & Orchestration Layer
- **[H1/H2/...] `<Bottleneck Title>`**:
  - **Measured Fact**: `<Token count, polling storm count, or timeout duration>`
  - **Trace Interpretation**: `<Orchestration failure pattern>`
  - **Hypothesis / System Flaw**: `<Harness limitation, missing push mechanism, compaction lack>`

---

## 4. Prioritized Action Backlog

### 🛠️ Tool Layer Improvements
| Priority | Target Tool | Proposed Change | Expected Benefit |
|---|---|---|---|
| **P0** | `<tool_name>` | `<Specific schema / handler change>` | `<Eliminate errors, reduce turns>` |
| **P1** | `<tool_name>` | `<Preflight / Idempotency / Output format>` | `<Improve reliability>` |

### ⚙️ Harness Layer Improvements
| Priority | Owning Module | Proposed Change | Expected Benefit |
|---|---|---|---|
| **P0** | `<module_name>` | `<Compaction, Polling backoff, Event push>` | `<Slash token cost, prevent timeout>` |
| **P1** | `<module_name>` | `<SSOT ID enforcement, Invariant prompt injection>` | `<Prevent instruction drift & mismatch>` |

---

## 5. Verification Plan

- [ ] Re-run trajectory simulation or synthetic unit test for modified tool.
- [ ] Measure token reduction and step count decrease on similar task.
- [ ] Verify zero 409 conflicts and zero Command Not Found (9009) errors.
```
