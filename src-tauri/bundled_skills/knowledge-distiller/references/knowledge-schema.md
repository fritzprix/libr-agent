# Knowledge Schema

Fields for `knowledge__recordKnowledge` distillation workflow.

## Required concepts

| Field | Purpose |
| --- | --- |
| `content` | Human-readable summary; must stand alone without chat context |
| `source` | Traceability: `sessionId`, date, or message ref |
| `tags` | Always include `distilled`; add domain tags. Tags are retrieval labels only — they do **not** create graph entities |

## Optional structured fields

| Field | When to use |
| --- | --- |
| `entities` | Technologies, projects, people, tools mentioned |
| `relationships` | `USES`, `DEPENDS_ON`, `REPLACES`, `CONFIGURED_WITH` |
| `auto_extract` | Default **false**. Set `true` only when you intentionally want heuristic gap-fill for missing graph fields |

## Entity example

```json
{
  "name": "LibrAgent",
  "entity_type": "Project",
  "description": "Local-first agent desktop platform"
}
```

## Relationship example

```json
{
  "source": "LibrAgent",
  "target": "SeaORM",
  "relation_type": "USES"
}
```

Field names must match the tool schema (`entity_type`, `source`, `target`, `relation_type`). Older aliases (`type`, `from`, `to`) are accepted for compatibility but prefer the canonical names.

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
4. Are entities few, named, and preferably described — not section headers, dates, or short acronyms?
5. If you omit entities/relationships, leave `auto_extract` false unless heuristic fill is intentional
