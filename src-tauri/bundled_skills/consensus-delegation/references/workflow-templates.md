# Consensus Delegation Templates

Use these when every child must return comparable sections. Adjust labels to the task; keep the section list identical across all children.

## Shared Output Shape

Ask every reviewer to use this structure:

```text
Verdict: [approve | approve-with-caveats | reject | inconclusive]
Confidence: [high | medium | low]

Findings:
- [finding]: [evidence: path, symbol, line, or command output]

Risks:
- [risk]: [why it matters]

Scope notes:
- [anything outside scope that might affect the verdict]
```

Rule: if a section is empty, the reviewer must write `None` explicitly so the parent can detect gaps.

## Panel Handoff Wrapper

Use the same wrapper for each child; change only `[PERSPECTIVE]` and optional lens-specific instructions.

```text
You are reviewing as: [PERSPECTIVE — e.g. security reviewer, performance reviewer]

Shared objective:
- [one sentence decision or review target]

Scope:
- In scope: [paths/modules/constraints]
- Out of scope: [explicit exclusions]
- Do not modify code unless explicitly allowed

Required output format:
[Paste the Shared Output Shape section above]

Lens-specific focus:
- [2-4 bullets unique to this perspective]

Context the parent requires you to respect:
- [critical rules copied from workspace instructions if needed]
```

## Scenario: Code Change Review

Spawn 2-3 children with lenses such as correctness, security, and performance.

```text
Shared objective:
- Review the proposed change for [feature/fix summary] before merge.

Scope:
- In scope: [files or diff range]
- Out of scope: unrelated refactors, style-only nits unless they hide bugs

Lens-specific focus (example — security):
- Injection, authz, secret handling, unsafe deserialization
```

Parent synthesis sections:

```text
Agreed findings:
Conflicting findings:
Unresolved risks:
Recommendation:
```

## Scenario: Architecture Option Evaluation

Use when comparing one proposal (or option A vs B stated in the shared objective).

```text
Shared objective:
- Evaluate [option/proposal] for production use in [system/context].

Scope:
- In scope: [components, interfaces, deployment model]
- Out of scope: full rewrites not requested in the proposal

Lens-specific focus (example — operability):
- Observability, rollback, failure modes, runbook burden
```

## Scenario: Risk / Tradeoff Assessment

```text
Shared objective:
- Assess whether [decision] is acceptable given [constraints].

Scope:
- In scope: [systems, time horizon, compliance boundaries]

Lens-specific focus (example — compliance):
- Data residency, audit trail, access control, retention
```

## Workspace Reminder

If reviewers must read the same repository:

- pass the same `workspaceOverride` to every `startSession`, or
- paste the minimum excerpts each reviewer needs when sharing a workspace is impossible

Never assume children inherit the parent workspace or workspace `agents.md` without explicit action. See `delegate` for the full isolation matrix.
