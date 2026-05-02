# Framework Selection Matrix

Choose the coordination model from the work shape, not from vibes.

| Task shape | Use this model | Why |
| --- | --- | --- |
| Clear dependency chain, low ambiguity, outputs feed the next step | **Sequential** | Minimizes coordination overhead and keeps ownership obvious |
| Several parallel contributors but one place must integrate and prioritize | **Hub-and-spoke** | Coordinator absorbs complexity and prevents worker collision |
| Broad exploration, uncertain next steps, many small discoveries | **Swarm** | Shared board lets specialists self-select work as information emerges |

## Selection heuristics

### Choose Sequential when

- downstream work depends on upstream approval
- quality gates matter more than speed
- roles should not overlap
- examples: research -> spec -> implementation -> QA

### Choose Hub-and-spoke when

- the user wants a manager, editor, or architect role
- multiple workers will produce incompatible outputs unless someone integrates them
- priorities may change during execution
- examples: product strategy, multi-stream implementation, report assembly

### Choose Swarm when

- the question is open-ended
- specialists can contribute independently from a shared backlog
- progress is discovery-driven rather than milestone-driven
- examples: market scan, exploratory debugging, evidence gathering

## Anti-patterns

### Too many roles

If two roles produce the same artifact or read the same source and "kind of help," collapse them.

### Wrong model for the task

- Sequential for a chaotic research problem -> stalls waiting on fake dependencies
- Swarm for tightly coupled implementation -> collision city
- Hub-and-spoke without a real coordinator -> everyone waits for nobody

## Recommended default team sizes

| Model | Recommended size |
| --- | --- |
| Sequential | 2-4 roles |
| Hub-and-spoke | 3-5 roles |
| Swarm | 3-6 roles with strict board discipline |

## Execution substrate rules

After choosing the coordination model, choose the execution substrate explicitly.

| Execution need | Use this substrate | Then follow | Why |
| --- | --- | --- | --- |
| One-off specialist delegation | `startSession(...)` | `delegate` | Lightweight child session without org coupling |
| Org-visible lineage under a governing teamwork session | `prepareTeamworkWorkspace()`, then `createOrg(...)`, then `startSession(..., includeCurrentOrg=true)` | `org` | Preserves explicit org membership, Org view semantics, and parent-workspace inheritance while keeping teamwork artifacts in app-local storage |
| Recurring, cron, heartbeat, or resumable automation | Scheduled task groups via `createScheduledTask(...)` and related `scheduled_task` tools | `schedule` | Keeps recurring collaboration separate from org lineage and under policy control |

## Hard separation rules

- Do not use explicit org lineage just because work is recurring.
- Do not use scheduled task groups just because there are multiple roles.
- Do not add redundant org-specific spawn wrappers. `startSession(...)` is the primitive; explicit org inheritance is automatic under an org root unless `includeCurrentOrg=false`.
- Do not invent a separate org-only workspace unless the task has a concrete reason to use a different `workspaceOverride`.
- Do not keep execution-specific org and scheduled-task operating rules mixed together once the substrate is chosen. Route to the specialist skill.
- If the user wants an org chart, root-session resume, or explicit lineage visibility, choose explicit org lineage.
- If the user wants periodic wake-ups, background recurrence, or governed automation cohorts, choose scheduled task groups.
