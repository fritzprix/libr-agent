# Soul Lounge Recovery Loop (Experimental)

## Why this exists

This document captures the current design decisions for Soul Lounge so the team can implement and iterate without losing intent.

Soul Lounge is not a cosmetic feature. It is an operational recovery loop for autonomous agent sessions when execution quality degrades.

---

## Problem Statement

Agent sessions can enter low-quality loops:

- repetitive tool calls with weak progress
- excessive plan churn without state convergence
- context drift after many turns and tool observations
- rising latency and token burn with declining output quality

These patterns are expensive and confusing for users. We need a reliable, explainable mechanism that slows and recovers sessions before they collapse.

---

## Goals

1. Detect loop risk from server-observable behavior (not self-report).
2. Trigger a bounded recovery phase with lower execution tempo.
3. Preserve agent autonomy while enforcing operational guardrails.
4. Re-enter normal execution with strong context anchors to avoid drift.
5. Keep default UX simple; advanced controls stay in advanced settings.

---

## Non-Goals (MVP)

- Building a new visual mode with custom animation-heavy UI.
- Adding a separate model/provider just for Soul Lounge.
- Exposing many new controls in the mainstream session-start flow.
- Implementing full persistent learning from all sessions in phase 1.

---

## Core Concepts

### 1) Loop Risk Score (server-side)

A backend loop detector computes a rolling `loopRiskScore` from concrete signals. Agents do not decide this themselves.

Candidate signals (weighted, rolling window):

- repeated tool call signatures with near-identical args
- repeated plan updates with low delta
- repeated error/timeout patterns
- low ratio of artifact-producing steps to total steps
- sustained high tokens-per-progress-unit
- rapid oscillation between tools without net state change

Output:

- `low`: normal operation
- `medium`: warning state
- `high`: enter Soul Lounge

### 2) Pace Control over Hard Caps

Primary control is dynamic slowdown (tempo shaping), not immediate hard token/tool caps.

- increase minimum interval between think-act iterations
- reduce parallelism / branching aggression
- bias toward verification + consolidation steps

Hard limits (token ceilings, forced termination) remain as safety rails, but not the first response.

### 3) Recovery Phases

`normal -> warning -> lounge -> reentry -> normal`

- `warning`: soft nudge, small pacing increase, encourage consolidation
- `lounge`: enforced cooldown loop, structured reflection and anchor rebuild
- `reentry`: gradual ramp-up with stability checks

### 4) Re-entry Anchors (anti-drift)

Before exiting lounge, system builds explicit anchors:

- mission objective snapshot (what must be achieved)
- constraints snapshot (hard limits, success criteria)
- state snapshot (current artifacts, IDs, unresolved blockers)
- next-step contract (first 1-3 concrete actions after reentry)

Reentry is blocked until anchors are present.

---

## Soul Lounge State Machine (MVP)

1. **Detect**
   - loop score reaches `high` threshold for N consecutive windows.

2. **Enter Lounge**
   - status set to `lounge`.
   - execution pace throttled.
   - mandatory recovery template injected into system context.

3. **Recovery Loop**
   - require concise diagnosis: what repeated, why blocked, what changed.
   - require artifact audit: list concrete outputs and missing evidence.
   - require focused plan rewrite with max small step count.

4. **Anchor Commit**
   - persist anchor packet in session state.
   - emit event for UI visibility.

5. **Reentry Gate**
   - if loop score drops below `medium` and anchor packet valid, enter `reentry`.
   - otherwise continue lounge cycle or escalate to stronger controls.

6. **Stability Window**
   - during `reentry`, keep moderate slowdown for M iterations.
   - if score spikes again, return to lounge.

---

## Override Semantics (`overrideOnce`)

`overrideOnce` allows one user-initiated bypass of lounge entry/gating for urgent progression.

Rules:

- one-time token; consumed on first bypass
- short TTL (session-scoped)
- visible in session state and logs
- cannot disable hard safety rails (kill switch, critical timeout)
- if post-override score remains high, lounge re-triggers automatically

Intent: preserve operator control without permanently disabling recovery discipline.

---

## Integration Points (LibrAgent)

### Backend (Rust)

- `AgentSessionManager`: host loop scoring, phase transitions, cooldown pacing.
- `ContextRegistry` / system prompt builder: inject lounge recovery template and anchor packet.
- Session event bus (`agent:event`): emit phase/status updates (`warning`, `lounge`, `reentry`).
- Session state: store rolling metrics + anchor packet + override token state.

### Frontend (React)

- Reactive display only (no orchestration logic).
- Surface current phase and concise reason in session status area.
- Advanced panel only: show `overrideOnce` control and audit trail.

---

## Minimal Data Contract (proposed)

Session workflow state extension:

- `loopRiskScore: number`
- `loopRiskLevel: "low" | "medium" | "high"`
- `recoveryPhase: "normal" | "warning" | "lounge" | "reentry"`
- `loungeEnteredAt?: string`
- `anchorPacket?: { objective: string; constraints: string[]; stateSummary: string; nextActions: string[] }`
- `overrideOnceAvailable: boolean`
- `overrideOnceUsedAt?: string`

---

## Metrics & Evaluation

Track before/after Soul Lounge rollout:

- loop duration distribution
- token cost per successful task
- completion rate for long-running sessions
- repeated-error incidence
- manual termination rate
- override usage rate and post-override success

Success criterion (MVP): lower loop cost and higher completion stability without reducing meaningful autonomy.

---

## Rollout Plan

### Phase 1 (Experimental, gated)

- backend score computation + lounge phase transitions
- minimal UI phase indicator
- anchor packet generation + reentry gate
- internal feature flag only

### Phase 2 (Operator controls)

- `overrideOnce` in advanced controls
- richer event/audit visibility
- threshold tuning from real telemetry

### Phase 3 (Policy hardening)

- adaptive thresholds by model/provider profile
- lineage-aware recovery (parent/child sessions)
- optional reusable recovery patterns from successful lounges

---

## Risks and Mitigations

1. False positives (healthy sessions flagged)
   - start conservative, tune thresholds with logs

2. User frustration from perceived slowdown
   - clear phase reason + one-time override

3. Post-lounge drift despite recovery
   - strict anchor packet requirement before reentry

4. Complexity creep
   - keep MVP state machine compact; avoid extra UI modes

---

## Operational Principle

Soul Lounge protects execution quality. It is a recovery protocol, not a personality skin.

Autonomy remains high, but operational entropy is managed explicitly.
