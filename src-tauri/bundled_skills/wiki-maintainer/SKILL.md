---
name: wiki-maintainer
description: >
  Mine sessions for recurring agent failures into the global skill-evolution
  wiki, propose one atomic SKILL.md patch from wiki patterns, or bootstrap a
  recurring Scheduled Task for it. Triggers: "update the skill wiki", "record
  this failure pattern", "analyze my sessions for skill gaps", "propose a skill
  fix", "patch this skill from the wiki", "skill-proposer", "알아서 돌려", "매일 wiki".
---

# Wiki Maintainer

Turn **past session history** into durable patterns in the **host-global** wiki
so **later sessions on this machine** can reuse them (Windows, macOS, and Linux).
Meta workflow only: do not inject wiki files into a normal task agent.

## Choose the procedure

| Need | Procedure |
| --- | --- |
| Mine sessions, write or update wiki patterns, bootstrap the recurring wake | Workflow below |
| Turn a wiki pattern into **one** atomic `SKILL.md` change and record Accept/Reject in the skill-impact ledger | [skill-proposer.md](references/skill-proposer.md) |

Do mining first. Propose a skill change only when a pattern has enough evidence.

**Preferred UX:** one bootstrap chat → create a global Scheduled Task that wakes
**Wiki Maintainer** on a cron. See [autonomous-loop.md](references/autonomous-loop.md).
Skill proposal: [skill-proposer.md](references/skill-proposer.md), [skill-proposer-checklist.md](references/skill-proposer-checklist.md).

## Prerequisites

- Prefer the bundled **Wiki Maintainer** assistant (ships with `history` + workspace shell).
- Otherwise enable optional **`history`** plus shell/code execution for this skill’s Python CLI.
- Tool names use `server__tool` form. Match the live tool list.
- **Wiki root:** `Path.home() / ".libragent" / "wiki"`  
  - Linux/macOS: `~/.libragent/wiki`  
  - Windows: `%USERPROFILE%\.libragent\wiki`
- Resolve this skill’s Base Directory as `<skill-base-dir>`.
- Recurring automation uses core **`scheduled_task__*`** via the **schedule** skill.

### Invoking the CLI (all OS)

Prefer `python` on PATH; on Linux/macOS fall back to `python3` if needed:

```bash
python "<skill-base-dir>/scripts/wiki_cli.py" path
python "<skill-base-dir>/scripts/wiki_cli.py" init
```

Windows PowerShell: same commands; quoting with `"..."` is fine. For large Unicode
pattern bodies, write a UTF-8 file with `workspace__writeFile` then:

```bash
python "<skill-base-dir>/scripts/wiki_cli.py" write-pattern --id <kebab-id> --file <utf8-path>
```

Templates: [wiki-layout.md](references/wiki-layout.md),
[index-template.md](references/index-template.md),
[logs-template.md](references/logs-template.md),
[skill-impact-template.md](references/skill-impact-template.md),
[pattern-template.md](references/pattern-template.md),
[analysis-rubric.md](references/analysis-rubric.md),
[autonomous-loop.md](references/autonomous-loop.md).

## Not this skill

| Need | Use |
| --- | --- |
| Facts / preferences into the knowledge graph | **knowledge-distiller** |
| One-shot clock delay inside this chat only | **loop** |
| Wait for process/ticket/webhook completion | **call-me-back** |
| Create/update the recurring wake | **schedule** (see autonomous-loop) |

## Workflow

### 0. Bootstrap automation (when the user wants “알아서”)

Follow [autonomous-loop.md](references/autonomous-loop.md): init wiki → optional
first mine → `agent__listAgents` for Wiki Maintainer id →
`scheduled_task__createScheduledTask` with the wake prompt. Then stop asking
the user to re-run this skill manually.

### 1. Ensure global wiki exists

```bash
python "<skill-base-dir>/scripts/wiki_cli.py" init
python "<skill-base-dir>/scripts/wiki_cli.py" path
```

Never use `{workspace}/.libragent/wiki` for this evolution store.

### 2. Choose sessions

1. Scope: today, last N sessions, or named ids.
2. `history__listSessions` / `history__searchHistory`.
3. Prefer failures, long recovery loops, user corrections. Skip chit-chat.
4. On scheduled wakes, skip prior Wiki Maintainer / skill-proposer meta runs.

### 3. Analyze

1. `history__readSession`, then `history__readMessage` as needed.
2. Capture symptom, first divergence, owning layer ([analysis-rubric.md](references/analysis-rubric.md)).
3. Seek a successful counterexample in another session.
4. One session only → `status: draft`.

### 4. Write or update a pattern

```bash
python "<skill-base-dir>/scripts/wiki_cli.py" write-pattern --id <kebab-id> --file <utf8-path>
python "<skill-base-dir>/scripts/wiki_cli.py" upsert-index --id <kebab-id> --status draft --one-line "<summary>"
python "<skill-base-dir>/scripts/wiki_cli.py" prepend-log --message "<what you found>"
python "<skill-base-dir>/scripts/wiki_cli.py" cat patterns/<kebab-id>.md
```

Structure: [pattern-template.md](references/pattern-template.md).

### 5. Stop conditions

- Do not edit `SKILL.md` in the mining workflow — use [skill-proposer.md](references/skill-proposer.md).
- On scheduled runs, the skill proposal may **Propose** only; never Accept without
  an interactive user Accept.
- No evaluation-task answers or hidden grader hints.
- Only record what `history__*` returned.

## Safety

- De-dupe with `list-patterns` / `cat index.md`.
- Host-global wiki may retain data across projects — do not store secrets.
- Do not create duplicate scheduled tasks for the same purpose.
