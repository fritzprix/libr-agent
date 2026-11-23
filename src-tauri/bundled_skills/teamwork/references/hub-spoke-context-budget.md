# Hub Context Budget

The Hub coordinates many spokes; its context window is the bottleneck.

## What the Hub should keep

- Task queue and dependencies
- Spoke session IDs and statuses
- **Summaries** and artifact paths — not full spoke transcripts

## What spokes should return

End each spoke task with a fixed block:

```markdown
## Spoke Result
- Status: completed | blocked | failed
- Artifacts: path/to/file1, path/to/file2
- Summary: (3-5 sentences)
- Blockers: (if any)
```

## Message bridging (A → B)

When spoke B needs A's output:

1. Hub reads A's **Result block** only.
2. Hub sends B a task containing: relevant file paths, summary, explicit acceptance criteria.
3. Do not forward A's entire tool trace.

## Hub prompt hygiene

- Cap sibling list in org layer context (backend already truncates).
- Archive completed spoke IDs from active working set once artifacts are captured.
