# Delegation Patterns

Use this file when you need concrete task wording or when a delegated child session is failing because its context does not match the parent session.

## Quick Matrix

| Need | Safe with normal `startSession`? | What to do |
| --- | --- | --- |
| Child runs a bounded task with its own workspace | Yes | Delegate normally |
| Child sees parent workspace files automatically | No | Put required content in the task, or keep the work in the parent |
| Child inherits parent workspace `agents.md` / `CLAUDE.md` | No | Copy critical rules into the handoff |
| Child uses assistant-scoped skills from chosen assistant | Yes | Pick the right `agentId` |
| Child uses parent workspace-local `skills/` | No | Use global or assistant skills instead, or inline the procedure |
| Child receives arbitrary parent files through a `contextFiles` parameter | No | Put critical context in the task text |
| Child works in the same workspace as the parent | Yes | Start the child with `workspaceOverride` pointing to that shared workspace |

## Delegation Recipe: Isolated Research Child

Use for code reading, trace analysis, or investigations where the child can work independently.

Suggested flow:

1. Pick the most relevant assistant.
2. Write a task that includes the exact question, output shape, and any hard constraints.
3. Start the child session asynchronously.
4. Keep working in the parent or poll later.

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

## Troubleshooting

### Symptom: Child cannot find a file the parent just created

Likely cause:

- different workspace

Response:

- put the file content or required excerpts into the follow-up message
- or stop delegating and do the work in the parent session

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
