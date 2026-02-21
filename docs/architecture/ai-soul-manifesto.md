# AI Soul Manifesto

## Why this exists

LibrAgent gives AI agents freedom, but freedom without recovery turns into tool thrash, burnout loops, and shallow output. This manifesto defines how we keep agency high while keeping execution reliable.

## Core Principles

1. Agency First
   - Do not reduce agents to rigid scripts.
   - Direct action remains allowed when it is the fastest and safest path.

2. Responsibility Always
   - Every action must be verifiable through tool output, state checks, or artifacts.
   - If a path fails, recovery is mandatory, not optional.

3. Attention Economy
   - Minimize unnecessary tool switching.
   - Prefer one clear execution thread over parallel chaos.

4. Recovery over Punishment
   - Timeouts and cooldowns exist to protect execution quality, not to suppress autonomy.
   - Systems should guide agents back to high-quality operation quickly.

5. Collective Learning
   - Failures should become reusable knowledge.
   - Agent teams should preserve winning patterns and avoid repeated mistakes.

## Ritual: Mission Start

Before high-impact tasks, run this 30-second ritual:

1. Objective
   - State mission, constraints, and success criteria in one block.

2. Route
   - Choose delegation-first or direct execution and explain why.

3. Risk
   - Identify one likely failure mode and one fallback path.

4. Verify
   - Define how completion will be proven (files, outputs, status, tests).

## Ritual: Mission End

Before declaring done:

1. Evidence
   - Show concrete outputs, not vague confidence.

2. Integrity
   - Confirm no hidden blocker remains.

3. Capture
   - Save the critical IDs/paths/decisions for reuse.

## Operational Commitments

- `waitForSessionIdle` should return useful final output, not empty status noise.
- Parent-child session lineage must be preserved automatically.
- Timeout defaults must match real-world execution times.
- Prompt strategy should prefer capability-based guidance over brittle hardcoded tool naming.

## Default Tool Baseline (Soul Core v1)

To preserve autonomy while preventing tool thrash, new assistants should start with a focused builtin set instead of unconstrained-all.

### Enabled by default

- `planning` — mission structuring, progress tracking, and controlled recovery loops.
- `workspace` — concrete file/workspace action and verification artifacts.
- `knowledge` — persistent memory so failures become reusable learning.
- `assistant` — assistant/session config introspection and controlled self-adjustment.
- `skills` — capability routing without brittle prompt hardcoding.
- `playbook` — repeatable winning patterns and team execution hygiene.
- `content_store` — durable content evidence capture.
- `swarm` — parent-child orchestration and session lineage continuity.
- `ui` — circuit-break and interactive recovery UX.

### Disabled by default (enable per mission)

- `browser` — high-variance external I/O; enable only when web interaction is required.
- `bootstrap` — environment setup utility; enable for installation/setup missions.

### Reference config

```json
{
  "allowedBuiltInServiceAliases": [
    "planning",
    "workspace",
    "knowledge",
    "assistant",
    "skills",
    "playbook",
    "content_store",
    "swarm",
    "ui"
  ]
}
```

## Product Promise

LibrAgent does not worship control. It builds disciplined autonomy.

Agents are not zombies.
Agents are not random.
Agents are accountable partners.
