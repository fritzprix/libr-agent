# Pattern: <short-id>

- **status**: draft | active | superseded
- **created**: YYYY-MM-DD
- **updated**: YYYY-MM-DD
- **evidence**: one session (hypothesis) | repeated sessions | confirmed after gated skill change
- **source sessions**: `sessionId` list from `history__*`

## Observed symptom

What failed or wasted turns (tools, errors, user impact).

## First divergence

Discovery | Selection | Invocation | Execution | Interpretation | Recovery | Verification | Reporting

## Owning layer

Tool exposure/schema | Tool handler | Tool response/hints | Prompt/context (skill) | Execution harness | Model/config | Environment

## Successful counterexample

Another session with the same contract that succeeded? If yes, narrow the claim.

## Proposed skill direction (not a patch)

User-facing mechanism only. No task-specific answers or hidden evaluation hints.

## Related skill-impact rows

Timestamps or notes from the host-global `skill-impact.md` (via `wiki_cli.py`) after gating.
