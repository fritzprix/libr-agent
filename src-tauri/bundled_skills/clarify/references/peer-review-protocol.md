# Peer Review Protocol (delegate-based)

Run this once, when the requirements doc is complete. The reviewer child session writes the review; this session decides about each finding.

## Prerequisites

- The doc exists at `requirements/<slug>.md` in this workspace and is the latest version.
- Set the doc `status` to `in-review` before you spawn the reviewer.
- Pick a reviewer config with `agent__listAgents(type="configs")` that matches the topic. Use a coding config only for software work; for an event, report, or process, prefer a planning, writing, or general config.

## Spawn (the workspace MUST be shared so the child can read the doc)

```
agent__spawnSession(
  configId=<reviewer config id>,
  complexity="normal",
  workspaceOverride="current",
  waitForResult=true,
  task=<brief below with <doc-path> and <project-summary> filled in>
)
```

For a re-review of an updated doc, send `agent__messageToSession` to the same reviewer session instead of spawning a new one.

## Review Brief Template (put inline in `task`)

```
You are an independent requirements reviewer. Read the requirements document
at <doc-path> and review it from two perspectives:

1. COMPLETENESS: missing requirements, ambiguous or untestable statements,
   contradictions, sections that the people doing the work would still have
   to guess about.
2. FEASIBILITY: whether the plan can be done with the stated budget,
   schedule, people, and tools; unrealistic assumptions; hidden
   dependencies; technical risk when the work involves technology.

Project context: <one-paragraph summary of what the user wants>

Output format (final text response):
- Verdict: APPROVE | NEEDS_CHANGES
- Findings: numbered list, each with [severity: high|medium|low],
  [category: completeness|feasibility], the exact doc section, the problem,
  and a concrete suggested fix.
- Max 10 findings; report real issues only, not style preferences.
Do not modify any files. Review only.
```

## Acceptance Rules

- **high**: accept, or explain the rejection to the user. Accepted high findings need doc changes before `final`.
- **medium**: accept if the fix is cheap and makes the work clearer; otherwise record the rejection reason in the Change Log.
- **low / style**: usually reject.
- An `APPROVE` with zero findings still needs the user's final confirmation. Never declare done on the reviewer's word alone.

## After Review

1. Apply accepted findings as a new doc version. Change Log entry: `peer review feedback applied (N accepted, M rejected)`.
2. If accepted findings need new user decisions, go back to the Question step for exactly one more round. Review again only if the scope changed a lot (use `agent__messageToSession` to the same reviewer).
3. Ask the user for final confirmation ("이 요구사항으로 확정할까요?"). On yes, set `status: final` and finish.

## Exit Condition

The doc reaches `status: final` when (a) the checklist is green, (b) the review verdict is APPROVE or all accepted high and medium findings are applied, and (c) the user confirms.
