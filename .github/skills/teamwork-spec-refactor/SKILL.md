---
name: teamwork-spec-refactor
description: >
  Resumable spec-driven guide for the Team Workspace / org lineage /
  scheduled task group refactoring in LibrAgent. Use when continuing,
  implementing, or reviewing this teamwork architecture so changes stay
  aligned with the agreed contract: workspace-scaffolded SSOT,
  task-force-builder as a meta-meta-skill, org only for lineage-based
  sub-agent teamwork, scheduled collaboration as separate task groups,
  and Settings-backed backend-enforced governance.
---

# Teamwork Spec Refactor

Use this skill to continue the teamwork refactor without drifting from the contract.

## Resume Protocol

Start here before changing code:

1. Read the session plan snapshot in `~/.copilot/session-state/.../plan.md` if available.
2. Read `references/contract.md`.
3. Read `references/implementation-map.md`.
4. Inspect the current code surface you are about to change.
5. Only then edit code.

If a proposed change conflicts with the contract, follow the contract instead of improvising.

## Non-Negotiable Contract

1. **Team Workspace is the durable collaboration substrate**
   - teamwork base information, including the original user request, belongs in workspace scaffolding
   - SSOT, progress, and handoff memory live in scaffolded files and workspace instructions
2. **`task-force-builder` is a meta-meta-skill**
   - decide whether a team is needed
   - choose the execution pattern
   - scaffold the workspace
   - generate `agents.md`
   - instantiate the teamwork constitution
3. **Static code enforces runtime guarantees**
   - safety policy
   - validation
   - refresh semantics
   - provenance
   - observability
4. **`org` is only for lineage-based sub-agent teamwork**
   - do not reuse org identity for scheduled collaboration
5. **Scheduled collaboration uses scheduled task groups**
   - separate from org
   - governed by Settings-backed backend-enforced limits
6. **Scheduled wake-up may trigger a master agent**
   - this is an operational pattern, not a reason to merge org and scheduled models

## Primary Workstreams

Choose the workstream that matches the requested change.

### Workstream A: Workspace constitution and scaffolding

Use when changing:

- `task-force-builder`
- scaffold generation
- `agents.md`
- teamwork base information
- coordination file contracts

Do this:

1. Preserve workspace files as the canonical teamwork state.
2. Write the original user request and teamwork base information into the scaffold.
3. Generate `agents.md` with explicit read/write rules.
4. Keep backend models out of the canonical teamwork constitution.

### Workstream B: Scheduled task groups and policy

Use when changing:

- scheduled task data model
- scheduled task UI
- governance settings
- scheduler-side validation

Do this:

1. Treat scheduled collaboration as **grouped automation**, not org lineage.
2. Add group metadata, not org identity.
3. Put user-configurable policy in Settings.
4. Enforce the policy in backend so agents cannot bypass it.

### Workstream C: Org lineage and org view

Use when changing:

- lineage metadata
- org-focused UX
- sub-agent organization behavior

Do this:

1. Keep org tied to lineage-based teamwork only.
2. Preserve separation from scheduled task groups.
3. Build org-focused UX as a dedicated surface, not a small tweak to generic lineage history.

## Decision Rules

When uncertain, apply these rules in order:

1. Prefer workspace scaffolding over backend canonical state.
2. Prefer explicit contract files over hidden conventions.
3. Prefer separate models for org and scheduled groups.
4. Prefer Settings UX plus backend enforcement for governance.
5. Prefer dedicated org/group surfaces over overloaded generic views.

## Current Likely Next Changes

1. Upgrade `task-force-builder` and its scaffold script to generate:
   - `agents.md`
   - teamwork base information including the original user request
   - coordination scaffold
   - optional machine-readable teamwork manifest
2. Extend scheduled task support with:
   - `groupId`
   - `groupName`
   - optional role/kind metadata
   - provenance
3. Redesign Scheduled Tasks UX around:
   - personal tasks
   - scheduled task groups
   - governance settings
4. Add a dedicated Org view.

## References

- Read `references/contract.md` for the binding architecture contract.
- Read `references/implementation-map.md` for the current code surfaces and where changes should land.
