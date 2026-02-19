# Session Lineage & Tree UI Architecture

## Why this exists

This document captures the full picture of the new nested-session direction so it can be resumed quickly without losing context.

Goals:

- Enable parent/child session orchestration (ouroboros-ready foundation)
- Expose session management through built-in MCP tools
- Visualize session hierarchy in UI with tree affordances

---

## Current State (Implemented)

### 1) Internal Session HTTP API extensions

Implemented in `src-tauri/src/server`:

- `GET /api/health`
- `POST /api/sessions` now accepts optional `parentSessionId`
- `POST /api/sessions` now accepts optional `maxDepth` (default: unlimited)
- `POST /api/sessions` response now includes:
  - `parentSessionId`
  - `lineageId`
  - `depth`
  - `maxDepth`
- `GET /api/sessions/:id/children`

Behavior:

- Child session creation validates parent existence
- Depth limit is enforced when `maxDepth` is set (`next depth > maxDepth` => `400`)
- If child request omits `maxDepth`, parent lineage limit is inherited
- Lineage metadata is tracked in-memory (`OnceLock + RwLock<HashMap<...>>`)
- Parent/child relation is removed from the in-memory lineage map on session termination

Notes:

- This is intentionally MVP-level lineage tracking (in-memory, not persistent)

---

### 2) Built-in MCP client for Session API (`session_api`)

Implemented in:

- `src-tauri/src/mcp/builtin/session_api/mod.rs`
- `src-tauri/src/mcp/builtin/session_api/tools.rs`

Registered into built-in discovery/routing:

- `src-tauri/src/mcp/builtin/mod.rs`
- `src-tauri/src/mcp/service_proxy.rs`
- `src-tauri/src/mcp/server/tools.rs`

Available tools:

- `healthCheck`
- `createSession`
- `createChildSession`
- `getSession`
- `getChildSessions`
- `getMessages`
- `sendMessage`
- `terminateSession`
- `listAssistants`

Result:

- Agent can control the internal Session API through MCP-style tool calls
- This is the integration bridge for recursive orchestration

---

### 3) E2E validation script updates

Updated:

- `scripts/test_api.py`

What is now tested:

- API health check
- Session create + initial workflow response poll
- Child session create from parent
- Parent child-list verification
- `maxDepth=1` branch: child allowed, grandchild rejected
- unlimited branch (`maxDepth` unset): depth-2 creation allowed
- Cleanup for both child and parent sessions (terminate)

---

### 4) Frontend Session Tree UI

Updated files:

- `src/models/agent.ts`
- `src/context/AgentSessionListContext.tsx`
- `src/features/session/SessionList.tsx`
- `src/features/session/SessionItem.tsx`

UI capabilities now:

- Parse lineage fields from `agentConfig` (`parentSessionId`, `lineageId`, `depth`)
- Render sessions as nested tree (parent -> child)
- Per-node expand/collapse
- Global `Expand all` / `Collapse all`
- Per-node direct child count badge
- Search mode auto-expands (prevents hidden matches)

---

## Contract currently used for hierarchy

At session creation time (HTTP path), lineage metadata is embedded into `agent_config`:

- `parentSessionId?: string`
- `lineageId?: string`
- `depth?: number`

This lets existing session list APIs surface hierarchy context without DB schema migration in MVP.

Current lineage contract also carries:

- `maxDepth?: number` (optional limit, omitted/`null` means unlimited)

The frontend now applies a user-configurable default via Settings:

- `Settings > Advanced > Session Branching Limit (Advanced)`
- value `0` means unlimited (default)
- value `1+` is injected into new session `agent_config.maxDepth`

---

## Known Limitations (intentional for MVP)

1. Lineage store is in-memory on backend

- Lost on app restart
- Not queryable historically

2. Hierarchy metadata piggybacks on `agent_config`

- Works now, but not ideal as final canonical source

3. No orchestration guardrails yet

- No `maxFanout`
- No token/time budget enforcement by lineage
- No per-lineage SLA/policy controls (beyond depth)

---

## Next Recommended Steps (priority order)

### P1. Persist lineage in DB

Introduce session lineage columns/table (canonical source):

- `parent_session_id`
- `lineage_id`
- `depth`
- optional `owner_session_id`, `ttl`, `budget`

Then update:

- repository model
- session query DTO
- frontend mapping (use top-level fields first, fallback to `agent_config` for compatibility)

### P2. Add orchestration tools

Add built-in MCP orchestration primitives:

- `delegateTask`
- `pollChild`
- `collectChildResult`
- `terminateTree`

### P3. Add guardrails

Before scaling recursive orchestration:

- max child fanout
- lineage cycle protection
- global kill switch
- token/time budget per lineage

### P3.5. Canonicalize maxDepth source

Today `maxDepth` works via `agent_config` + in-memory lineage metadata.
When DB lineage persistence lands, move depth limit source-of-truth to lineage table/columns and keep `agent_config` as compatibility fallback only.

### P4. Persist tree UI expand/collapse state

Optional UX polish:

- Save collapsed node IDs per view/session in frontend settings

---

## Quick Resume Checklist

When resuming this work:

1. Run `python scripts/test_api.py`
2. Confirm tree rendering in History/session list UI
3. Verify `session_api` tools are listed in built-in tool registry
4. Verify `Settings > Advanced > Session Branching Limit (Advanced)` affects new sessions as expected (`0` unlimited / `1+` bounded)
5. Decide whether current phase is:
   - DB persistence migration, or
   - orchestration tools expansion

---

## Summary

The project now has a working end-to-end foundation for nested sessions:

- API supports parent/child lineage metadata
- MCP can call session API tools directly
- UI visualizes hierarchy as a navigable tree

This is a solid base for controlled recursive orchestration (ouroboros), with persistence + guardrails as the next hardening phase.

As of now, depth limiting is already operational and user-configurable; persistence and broader guardrails are the remaining hardening work.

---

## Milestone Snapshot (2026-02-14)

This is the "do not forget this" checkpoint.

### What we decided

1. **Keep mainstream UX simple**

- Do not expose per-session depth controls in the start screen by default.
- Expose only one default control in Settings:
  - `Settings > Advanced > Session Branching Limit (Advanced)`
  - `0 = unlimited`, `1+ = bounded`

2. **Unlimited mode is allowed, but never unmanaged**

- `maxDepth=0` means no depth cap.
- Even with unlimited depth, lineage must still be governed by guardrails (fanout, budget, runtime, kill switch).

3. **`session_api` must be truly reachable by agents**

- Builtin registry/discovery wiring is in place.
- Agent-side builtin alias extraction now includes `session_api` so tools are actually available in live sessions.

### Why this matters

- We now have a practical bridge from normal product UX to recursive orchestration power.
- The product can stay beginner-friendly while still enabling advanced autonomous workflows.
- This is the exact pivot point from "feature demo" to "operable system".

### Next hardening trigger

If we move further into unlimited/recursive operation, implement this next in order:

1. DB-persistent lineage source of truth
2. Fanout + budget + runtime guardrails
3. Global lineage kill switch and pause/resume controls

If these are skipped, unlimited mode will eventually become unstable in real workloads.

### Red Zone: Recursive Ops Survival Card

When `maxDepth=0` (unlimited), treat the system as a distributed workload, not a chat feature.

#### Failure signals (heart-attack indicators)

- Child creation rate keeps rising while useful output quality drops.
- Same error/tool pattern repeats across sibling branches.
- Parent waits on children longer each cycle (latency amplification).
- Token/time burn grows faster than completed task count.

#### Non-negotiable invariants

1. **No unbounded fanout**

- Even with unlimited depth, fanout must be bounded.

2. **Budget before brilliance**

- If lineage budget is exhausted, pause/terminate regardless of partial progress.

3. **Single-click stop path**

- Global kill switch must terminate a lineage tree deterministically.

4. **Parent authority boundary**

- Children cannot escalate tool scope above parent policy.

#### Immediate action order during runaway behavior

1. Freeze new child creation for target lineage.
2. Collect top-K failing branches and error signatures.
3. Terminate lowest-value/highest-cost branches first.
4. Resume only with stricter fanout + budget settings.

#### Operational mindset

- Unlimited recursion is not "more capability" by default.
- It is controlled volatility: powerful only when bounded by policy and observability.
