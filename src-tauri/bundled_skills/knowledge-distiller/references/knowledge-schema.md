# Knowledge Schema

Fields for `knowledge__recordKnowledge` distillation workflow.

## Required concepts

| Field | Purpose |
| --- | --- |
| `content` | Human-readable summary; must stand alone without chat context |
| `source` | Traceability: `sessionId`, date, or message ref |
| `tags` | Always include `distilled`; add domain tags |

## Optional structured fields

| Field | When to use |
| --- | --- |
| `entities` | Technologies, projects, people, tools mentioned |
| `relationships` | `USES`, `DEPENDS_ON`, `REPLACES`, `CONFIGURED_WITH` |

## Entity example

```json
{
  "name": "LibrAgent",
  "type": "project"
}
```

## Relationship example

```json
{
  "from": "LibrAgent",
  "to": "SeaORM",
  "type": "USES"
}
```

## Tag conventions

- `distilled` — auto-extraction run
- `auto-knowledge` — machine-assisted capture
- `context-sync` — project state sync
- `decision` — architectural choice
- `preference` — user workflow preference
- `runbook` — operational procedure

## Quality bar

Before recording, ask:

1. Would a new agent understand this without reading the original chat?
2. Is it actionable or referenceable later?
3. Is it already in the knowledge base?
