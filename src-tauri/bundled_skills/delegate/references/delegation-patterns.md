# Delegation Patterns

Use this file when you need concrete task wording or when a delegated child session is failing because its context does not match the parent session.

## Quick Matrix

| Need | Safe with normal `agent__spawnSession`? | What to do |
| --- | --- | --- |
| Existing Idle child has the same assistant and compatible workspace | Prefer reuse | Inspect with `agent__listAgents(type="sessions")`, then use `agent__messageToSession`; add `reset=true` only for a fresh assignment |
| Child runs a bounded task with its own workspace | Yes | Delegate normally |
| Child sees parent workspace files automatically | No (plain default) | Pass `workspaceOverride="current"` (or `"."` / `"inherit"`), or inline content |
| Child inherits parent workspace `agents.md` / `CLAUDE.md` | Only if SHARED | Share with `workspaceOverride="current"`, or copy critical rules into the handoff |
| Child uses assistant-scoped skills from chosen assistant | Yes | Pick the right `configId` |
| Child uses parent workspace-local `skills/` | Only if SHARED | Share workspace, use global/assistant skills, or inline the procedure |
| Child receives arbitrary parent files through a `contextFiles` parameter | No | Put critical context in the task text |
| Child works in the same workspace as the parent | Yes | `workspaceOverride="current"` (preferred) or an absolute path to that root |

## Delegation Recipe: Isolated Research Child

Use for code reading, trace analysis, or investigations where the child can work independently.

Suggested flow:

1. Pick the most relevant assistant.
2. Inspect existing child sessions and reuse a suitable Idle matching-role child with `agent__messageToSession` when its workspace is compatible.
3. Write a task that includes the exact question, output shape, and any hard constraints.
4. Start a new child asynchronously with `agent__spawnSession` only when no suitable child exists or a distinct role, workspace, or parallel slot is needed.
5. Keep working in the parent or poll later.

Task template:

```text
Investigate the following bounded question and return a concise technical summary.

Goal:
- [exact question]

Scope:
- Only inspect [paths/modules]
- Do not modify code

Deliverable:
- Summary of findings
- Key file paths
- Concrete caveats or follow-up risks
```

## Delegation Recipe: Child Needs Parent Rules

Use when the parent workspace has important local instructions that the child will not automatically see.

Task template:

```text
Work on the following task, but also obey these workspace-specific rules from the parent session:

- [rule 1 copied from parent workspace instructions]
- [rule 2 copied from parent workspace instructions]

Task:
- [exact task]

Deliverable:
- [expected output]
```

Rule: copy only the critical rules, not the whole file unless necessary.

## Delegation Recipe: Child Needs a Specific Skill

Choose the skill source deliberately:

1. Prefer a global skill if multiple assistants need it.
2. Prefer an assistant-scoped skill when the behavior belongs to a specialist assistant.
3. Treat parent workspace-local skills as parent-only unless the child is started in that same workspace.
4. If the procedure is short, inline it in the task instead of assuming the child can discover it.

Task template:

```text
Use the assistant's available skills for this task.

If the needed skill is not available in your session, follow this explicit procedure instead:
- [short procedure]
- [short procedure]

Task:
- [exact task]
```

## Delegation Recipe: Synchronous Child

Use `waitForResult=true` only when:

- the child task is tightly bounded
- the parent cannot make progress in parallel
- the answer is needed immediately for the next step

Avoid synchronous waits for open-ended debugging or implementation tasks. That just blocks the parent for no good reason.

## When to Load `delegation-eval-loop`

Stay on `delegate` for research summaries, soft drafts, or low-risk exploration.

Load **`delegation-eval-loop`** when any of these apply:

- user or task requires tests/build/lint proof before "done"
- child may edit a shared workspace and path invariants matter
- implementation or refactor where fake completion / self-grading is likely
- you need a bounded reject → `agent__messageToSession` → retry loop

Pattern: child generates; parent evaluates. Do not ask the child to grade its own acceptance as the final gate.

## Troubleshooting

### Symptom: Child cannot find a file the parent just created

Likely cause:

- different workspace (`ISOLATED` Metadata)

Response:

- re-spawn or continue with `workspaceOverride="current"` so Metadata shows `SHARED with caller`
- or put the file content / required excerpts into the follow-up message
- if the user asked for another agent to review the deliverable, do **not** substitute a parent self-audit

## Delegation Recipe: Deliverable Audit / Independent Review

Use when the user asks another agent to audit, review, or critique existing workspace files.

1. `agent__spawnSession(configId="...", complexity="normal", task="...", workspaceOverride="current", waitForResult=true)` (or reuse an Idle SHARED child via `agent__messageToSession`).
2. Task text names relative paths under the shared root and the review criteria.
3. Present the child's final text as the review result.
4. On missing-file failure under isolation, fix the binding — never grade your own homework as the independent audit.

### Symptom: Child ignores a local `agents.md` rule the parent followed

Likely causes:

- child workspace differs from parent workspace
- child session started before an updated workspace file could matter

Response:

- restate the critical rule in the task text
- assume prompt refresh only in a later execution step

### Symptom: Child cannot use a workspace-local skill

Likely cause:

- workspace-local skills are resolved from the child workspace

Response:

- switch to an assistant with the needed assistant-scoped skill
- move the skill to global scope
- inline the minimum viable procedure in the task

### Symptom: Child cannot see the workspace you expected

Likely cause:

- no explicit workspace override was provided

Response:

- start the child in that same workspace when shared workspace access is required
- still pass critical context in the task text; workspace sharing is not the same thing as copying instructions into the prompt
