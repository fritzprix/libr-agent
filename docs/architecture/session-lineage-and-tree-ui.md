# Session Lineage & Tree UI Architecture

## Why this exists

This document captures the full picture of the new nested-session direction so it can be resumed quickly without losing context.

Goals:

- Enable parent/child session orchestration (ouroboros-ready foundation)
- Expose session management through built-in MCP tools
- Visualize session hierarchy in UI with tree affordances

---

## Naming: three different depth-limit surfaces

Do not conflate these — they live in different layers and are not interchangeable:

| Surface                                  | Where                         | Purpose                                                                                 |
| ---------------------------------------- | ----------------------------- | --------------------------------------------------------------------------------------- |
| **HTTP `POST /api/sessions` `maxDepth`** | `src-tauri/src/server`        | Optional request body when creating a session via the internal Session HTTP API         |
| **MCP `agent__startSession`**            | Removed from tool schema      | Depth/fanout limits come from assistant config and Settings, not per-call MCP overrides |
| **`sessions.max_depth` column**          | DB entity + `SessionMetadata` | Persisted lineage cap for a session row; used by `resolve_agent_config` and tree UI     |

Settings **Session Branching Limit** writes the default `max_depth` for new desktop sessions (`0` = unlimited). Child sessions inherit parent lineage columns at creation time.

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
- Lineage metadata is tracked in-memory (`OnceLock + RwLock<HashMap<...>>`) for HTTP orchestration helpers
- Parent/child relation is removed from the in-memory lineage map on session termination
- Tauri/desktop sessions also persist lineage on `sessions` table columns (canonical for UI and `resolve_agent_config`)

Notes:

- HTTP path still uses in-memory map for some validation; DB columns are the durable source for desktop sessions and list/tree UI

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

- Read lineage fields from session metadata (`parentSessionId`, `lineageId`, `depth`, `maxDepth`)
- Render sessions as nested tree (parent -> child)
- Per-node expand/collapse
- Global `Expand all` / `Collapse all`
- Per-node direct child count badge
- Search mode auto-expands (prevents hidden matches)

---

## Contract currently used for hierarchy

Session lineage and org metadata are persisted on the `sessions` table:

- `assistant_id` — FK to `assistants.id` (assistant settings SSOT)
- `parent_session_id`, `lineage_id`, `depth`, `max_depth`, `max_fanout`
- `org_id`, `org_name`, `org_root_session_id`

At session creation (Tauri and HTTP paths), create-time `AgentConfig` / request fields populate these columns. The runtime resolves effective config via `resolve_agent_config(session)` → load assistant row + overlay session lineage columns.

Branching limit from Settings:

- `Settings > Advanced > Session Branching Limit (Advanced)`
- value `0` means unlimited (default)
- value `1+` is stored on new sessions as `max_depth`

---

## Known Limitations (intentional for MVP)

1. In-memory lineage map still supplements HTTP API for some orchestration paths

- Lost on app restart for map-only data
- DB columns are canonical for list/tree UI

2. No orchestration guardrails yet

- No enforced `maxFanout` at scale
- No token/time budget enforcement by lineage
- No per-lineage SLA/policy controls (beyond depth)

---

## Next Recommended Steps (priority order)

### P1. Consolidate lineage source-of-truth

HTTP API in-memory lineage map should read/write the same DB columns the Tauri path uses. Frontend already maps top-level `SessionMetadata` fields only.

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

### P3.5. Canonicalize orchestration limits

`max_depth` and `max_fanout` are stored on session rows; enforce consistently across HTTP and Tauri creation paths.

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
