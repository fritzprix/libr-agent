# Bottleneck Taxonomy for Agent Traces

When analyzing real-world agent trajectories (ATIF v1.x, `.trace.json`, session messages), failures and inefficiencies typically fall into three primary architectural layers: **Tool Layer**, **Harness Layer**, and **Prompt / Policy Layer**.

---

## 1. Tool Layer Bottlenecks

| Code | Bottleneck | Symptoms in Trace | Root Cause | Recommended Fix |
|---|---|---|---|---|
| **T1** | **Capability Mismatch / Missing Tool** | Agent uses `browser` or writes scraping scripts when search is requested; high latency, user complaints. | No dedicated fast search tool (e.g., `exa__search`, `web__search`) available in session. | Provide dedicated builtin MCP tools; do not force agents to use generic browsers for simple search. |
| **T2** | **Non-Idempotent Tool Calls** | `agent__createAgent` returns `Assistant '...' already exists` (409 Conflict); `planning__addTodo` fails with `already exists`. | Builtin tools enforce strict create-only semantics instead of get-or-create / upsert. | Support `upsert: true` or transparently return existing entity info when already present. |
| **T3** | **Shell Environment Fragility** | `workspace__runPowerShell` fails with exit code 9009 (Command Not Found) or exit code 1/2. | Agent relies on ambient PATH (`python`, `pip`, CLI tools) that differ across platforms. | Prefer native builtin MCP tools over shell scripts; if shell is needed, use runtime preflight (`uv run`, full paths). |
| **T4** | **Observation Payload Bloat** | Tool observation returns tens of kilobytes of raw HTML, logs, or file dumps. | Tool lacks pagination, truncation, or smart summary formatting. | Enforce observation truncation (e.g. max 4KB per result with line ranges or summary hints). |
| **T5** | **Fragile Text Manipulation** | `workspace__strReplace` fails with `old_string was not found` or `matched 2 times`. | LLM generates inexact indentation or ambiguous matching target. | Return surrounding line context suggestions on mismatch; support single-match target hints. |

---

## 2. Harness & Orchestration Bottlenecks

| Code | Bottleneck | Symptoms in Trace | Root Cause | Recommended Fix |
|---|---|---|---|---|
| **H1** | **Polling Storm (Check Loops)** | Dozens or hundreds of consecutive `agent__checkSession` calls consuming turns and tokens. | Parent session actively loops waiting for child session completion. | Implement reactive push notifications / long polling with backoff; avoid active polling in prompts. |
| **H2** | **Context & Token Explosion** | Prompt tokens escalate past 10M–50M+ over multi-turn sessions; high latency and token cost. | Conversation history retains full unabridged tool observations without compaction. | Apply aggressive context compaction (pruning old tool observations, keeping only high-level outcomes). |
| **H3** | **Dual-ID / Truncation Leak (SSOT Breach)** | `agent__checkSession` fails with `Session 'xyz' not found`; ID passed is a truncated 10-char hash. | Prompt or model truncates session IDs to save tokens, violating SSOT. | Enforce strict SSOT: session IDs must remain untruncated everywhere; ban dual-ID representations. |
| **H4** | **Subsession State Desynchronization** | Parent continues after child cancellation or timeout; child results discarded or unparsed. | Lack of formal handoff verification gate between parent and child sessions. | Apply layered evaluation contracts (`delegation-eval-loop`) with strict acceptance criteria. |
| **H5** | **Platform Shell Trap** | Commands fail with Windows PowerShell syntax errors (`&&` vs `;`, escaping issues). | Prompt or agent assumes POSIX / Linux bash shell on a Windows environment. | Inject OS/Shell profile into agent context; enforce cross-platform recipes. |

---

## 3. Prompt & Policy Bottlenecks

| Code | Bottleneck | Symptoms in Trace | Root Cause | Recommended Fix |
|---|---|---|---|---|
| **P1** | **Constraint Forgetting (Instruction Drift)** | User repeatedly reminds agent: "include source URLs", "24h window only", "filter business items". | Long multi-turn conversation pushes initial prompt constraints out of immediate attention. | Inject persistent session invariants into system prompt or dynamic context providers. |
| **P2** | **Premature / Fake Done** | Agent declares task complete without verifying deliverables or actual command exit codes. | Lack of proof-of-work check before final user presentation. | Require deterministic verification step (Layer 1 gate) before emitting `ui__reportResult`. |
| **P3** | **Over-Summarization / Hallucination** | Truncated nonsense phrases (e.g. broken headlines, placeholder links) in final output. | Agent forced to compress too much data in a single turn without intermediate verification. | Stage multi-step pipelines: collect raw items → verify each item → synthesize final report. |
