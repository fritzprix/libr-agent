# Performance Action Plan Report Template

Standard template for writing performance analysis reports. Output files are typically written to `.libragent/work/perf_audit_<target>_<YYYYMMDD>.md` or directly provided in the conversation.

---

# Performance Audit & Action Plan: [Target Component / Flow]

**Date**: YYYY-MM-DD  
**Auditor**: Performance Audit Agent  
**Scope**: [e.g., Session Transcript Loading / Rust IPC / React Chat View / SQLite History]

---

## 1. Executive Summary

| Priority | Finding Count | Key Impact |
|:---:|:---:|:---|
| **P0 (Critical)** | N | Main thread blocking, UI freezes (>100ms), potential OOM or deadlocks |
| **P1 (High)** | N | Noticeable latency, redundant I/O, heavy render cascades, missing indexes |
| **P2 (Medium)** | N | Micro-optimizations, excessive cloning, minor state churn, cleanups |

> **Summary**: [Brief 2-3 sentence overview of the current performance bottlenecks and anticipated benefits after applying the action plan.]

---

## 2. Priority Findings & Action Items

### [P0 / P1 / P2] Issue Title: [Short Descriptive Title]

- **Location**: `[relative/path/to/file.ext:line]`
- **Layer**: `[Rust / Tokio / Tauri IPC / React / SQLite]`
- **Root Cause**:
  [Explain why this is slow or risky, citing specific code logic or patterns.]
- **Impact & Symptoms**:
  [e.g., "Blocks Tokio runtime for ~200ms on large files", "Triggers 50+ re-renders of the entire message list", "Performs full table scan on 10,000 rows"]

#### Before (Current Implementation)
```typescript // or rust / sql
// Snippet of the current bottleneck
```

#### After (Proposed Improvement)
```typescript // or rust / sql
// Snippet of the optimized implementation
```

#### Verification & Side Effect Check
- [ ] **Risk assessment**: [Any breaking changes or behavioral differences?]
- [ ] **Verification method**: [How to verify without heavy test suites, e.g. `pnpm rust:check` or targeted unit test]

*(Repeat section for each finding)*

---

## 3. Implementation Roadmap

1. **Phase 1 (P0 Immediate Fixes)**: [Targeted high-risk fixes]
2. **Phase 2 (P1 Efficiency Improvements)**: [Latency and render optimizations]
3. **Phase 3 (P2 Polish & Cleanups)**: [Refinements and micro-optimizations]

---

## 4. Guardrails & Compliance Checklist

- [ ] **KISS / YAGNI**: No dual-state / dual-ID or speculative over-engineering introduced.
- [ ] **Desktop Fit**: Solution fits local desktop environment (no unnecessary distributed caching).
- [ ] **Resource Safety**: Verified using lightweight checks (`pnpm rust:check`, etc.); raw `cargo` avoided.
