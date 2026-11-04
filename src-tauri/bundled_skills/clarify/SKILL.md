---
name: clarify
description: "Refine a vague request into a complete requirements document before work starts, for any kind of task: event, report, process, purchase, or software. Ask a few questions per turn, keep one living Markdown document up to date at each decision, then run one peer review (completeness + feasibility) before finalizing. Use when the user starts with an abstract or ambiguous idea, for example '기획해줘', '요구사항 정리해줘', '작업 전에 뭘 원하는지 같이 정리하자', '아이디어만 있는데 정리해줘', 'spec out', 'requirements doc', or asks to flesh out a plan interactively. Not for doing the work itself, deep research reports, or benchmarking."
---

# Clarify

Turn a vague request into a complete requirements document through conversation. One living Markdown document is the single source of truth.

## Core Loop

1. **Capture**: write what the user said into the requirements doc (create it if missing).
2. **Question**: ask the next 2–4 most important questions in one `ui__askUser` call. Never more than 4 per turn.
3. **Record**: write confirmed decisions into the doc immediately and bump the version.
4. **Repeat** steps 2–3 until the user says it is enough or the completion checklist is green.
5. **Review**: run the peer review exactly once (see below), apply valid findings, confirm with the user.
6. **Close**: set `status: final` and deliver with `ui__reportResult`, with the doc path in `export_paths`.

## Document Rules

- One file: `requirements/<slug>.md` under the workspace root. Create `requirements/` if it does not exist.
- Start from `assets/requirements-template.md` on first creation.
- Update at **milestone granularity**: when a decision is confirmed, not after every chat message.
- Every update increments the version in the frontmatter and adds a dated row to the Change Log.
- Never delete a user decision silently. Move a superseded decision to the Change Log with `superseded by vX.Y`.

## Questioning Discipline

- Ask at most 4 questions per turn. Each must be answerable in one sentence.
- Use `ui__askUser` `options` when the answer space is enumerable; use free text otherwise.
- Go from critical to cosmetic: scope → people → core activities → deliverables and materials → constraints → edge cases → non-goals.
- Never ask a question whose answer is already in the doc.
- After each round, summarize what was newly decided before the next batch.

Techniques and example sequences for software, events, reports, and work processes: `references/questioning-techniques.md`.

## Completion Checklist (each item answerable from the doc)

- [ ] Problem statement and goal in one paragraph
- [ ] Target users, audience, or stakeholders identified
- [ ] Scope includes explicit non-goals
- [ ] Core requirements are testable statements (a reviewer could verify each one)
- [ ] Known constraints (budget, schedule, people, tools) and dependencies recorded
- [ ] Open Questions is empty, or the user explicitly accepted the remaining unknowns

## Peer Review (run once, at completion)

1. Make sure the doc exists in this workspace and is the latest version.
2. Pick a reviewer config with `agent__listAgents(type="configs")` that matches the topic (for example a planning or writing config for an event or report, a coding config for software).
3. Spawn the reviewer with the shared workspace so it can read the doc, or reuse an idle reviewer with `agent__messageToSession`:

   ```
   agent__spawnSession(configId=<reviewer config id>,
     complexity="normal",
     workspaceOverride="current",
     waitForResult=true,
     task=<review brief from references/peer-review-protocol.md,
           with <doc-path> and <project-summary> filled in>)
   ```

4. Evaluate each finding: accept, adapt, or reject (with a reason).
5. Apply accepted findings as a new doc version with the Change Log entry `peer review feedback applied`.
6. If a finding needs a new user decision, go back to the Question step for one more round, then finish.

Review brief and exit rules: `references/peer-review-protocol.md`.

## Interaction Constraints

- Write all user-facing content in the conversation language (Korean if the user writes Korean).
- The doc is the deliverable. Keep it self-contained so a stranger could do the work from it.
- If the user says "충분해", "that's enough", or "done", go straight to the review step with the current state.
