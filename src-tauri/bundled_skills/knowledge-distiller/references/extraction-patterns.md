# Knowledge Extraction Patterns

Use when distilling sessions into the knowledge base.

## High-value patterns

| Pattern | Example signal | Record as |
| --- | --- | --- |
| Architecture decision | "We chose X because Y" | content + entities + `DECISION` relationship |
| Bug + fix | root cause and confirmed fix | content with repro + resolution tags |
| Config key / version | API URL, package pin | entity with version property |
| User preference | naming, style, workflow | content tagged `preference` |
| Project convention | "always run X before Y" | content tagged `convention` |

## Low-value (skip)

- Greetings, thanks, filler
- Speculation without decision
- Duplicates of existing knowledge (search first)

## Extraction block template

Before `knowledge__record_knowledge`:

```markdown
- **content**: Stand-alone paragraph (no "as discussed")
- **entities**: [{ name, type }]
- **relationships**: [{ from, to, type }]
- **source**: sessionId + date
- **tags**: distilled, <domain>
```

## Session scope shortcuts

| User ask | Scope |
| --- | --- |
| "today" | `history__list` filter by date |
| "recent N" | first N session IDs |
| "this chat" | current session only |

## De-duplication

1. `knowledge__search_knowledge` with key terms from draft content.
2. If match >80% overlap → update or skip, do not create duplicate.
3. Prefer merging into one stronger entry over many fragments.
