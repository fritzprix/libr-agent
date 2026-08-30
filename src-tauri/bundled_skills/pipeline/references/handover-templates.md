# Pipeline Handover Templates

Bind only what the next stage needs.

## Stage completion block (every worker)

```markdown
## Stage Complete: <Stage Name>
- Input used: <paths or prior stage id>
- Output artifacts:
  - <path> — <one line description>
- Decisions made: <bullets>
- Open questions: <bullets or "none">
- Recommended next stage focus: <one paragraph>
```

## Handover to next stage (parent binds into task)

```markdown
You are stage "<Next Stage>" in a pipeline.

Prior stage output:
<paste Stage Complete block>

Read these files first:
- <path1>
- <path2>

Your deliverable:
- <explicit output>

Do not restart prior work; extend or refine only.
```

## Sequential spawn pattern

```text
agent__listAgents(type="sessions")
sessionA = agent__messageToSession(sessionId=idle_matching_id, ..., reset=true)
           or agent__startSession(..., waitForResult=true)
handover = extract Stage Complete from A
agent__listAgents(type="sessions")
sessionB = agent__messageToSession(sessionId=idle_matching_id, ..., reset=true)
           or agent__startSession(task=handover + criteria, waitForResult=true)
```

Reuse only when the child has the same assistant configuration and a compatible
workspace. Create a new session when the next stage needs a different role,
workspace isolation, or additional parallel capacity.

## Context filtering rule

Never attach full prior-stage message history. File paths + Stage Complete block are sufficient.
