# Gatekeeper (delegation-eval-loop reference)

Use this pattern when a separate Reviewer session must gate the work. For checks that the parent runs itself, use the main `SKILL.md` workflow. Apply the same layered evidence rules (`SKILL.md` § The 4-Layer Evaluation Stack) in the Reviewer's audit.

The Gatekeeper pattern sets up a loop between a Creator session and a Reviewer/Gatekeeper session. The reviewer audits the output against explicit quality criteria, approving it for delivery or rejecting it for rework.

## 4-Stage Workflow

```
 ┌───────────┐      Submit Artifact      ┌──────────────┐
 │  Creator  ├──────────────────────────►│  Reviewer/   │
 └─────▲─────┘                           │  Gatekeeper  │
       │                                 └──────┬───────┘
       │            Reject (Rework)             │
       └────────────────────────────────────────┤ Approve ──► Ship
                                                ▼
```

1. **Specs & Criteria**: Setup target task goals for the Creator and quality criteria for the Reviewer. See [criteria.md](gatekeeper-criteria.md).
2. **Draft & Submit**: The Creator implements the code/artifact in the shared workspace and notifies the Reviewer.
3. **Audit**:
   - The Reviewer evaluates the artifact against criteria.
   - **Approve**: If all criteria pass, the loop exits.
   - **Reject**: List the failed rules and provide actionable feedback.
4. **Rework Loop**: The Creator modifies the files based on the feedback and resubmits. This loops until approved or loop count limit is reached.

## 🛠️ MCP Tools Guide

Spawn Creator and Reviewer children with `agent__spawnSession(configId, complexity, task, workspaceOverride="current")` when both must share the same workspace. Always pass `complexity` (`low`|`normal`|`high`) on spawn and on every `agent__messageToSession(sessionId, complexity, message)`.

- **Feedback Delivery**: Inject the reviewer's audit report with `agent__messageToSession` to wake and direct the Creator to rework.
- **Infinite Loop Avoidance**: The parent session must track and enforce a **maximum rework limit (default: 3)** to prevent endless loops over minor stylistic differences. See [rework-limit-examples.md](gatekeeper-rework-limit-examples.md).

## References

- [Quality criteria](gatekeeper-criteria.md)
- [Rework limit examples](gatekeeper-rework-limit-examples.md)
- [Troubleshooting](gatekeeper-troubleshooting.md)
