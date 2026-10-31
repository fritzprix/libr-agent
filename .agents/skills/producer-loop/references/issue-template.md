# Issue template

Write the issue in English. Follow ASD-STE100: short sentences, one idea each, active voice, simple verbs. State requirements at a high level. Do not write a patch.

Do not include secrets, tokens, private paths (`/home/<user>/...`), or personal data. Quote at most a few lines of evidence. Use repository-relative paths.

`file_issue.py` adds the labels `auto:producer` and `auto:<task>` and a hidden fingerprint marker. Do not add them yourself.

```markdown
## Summary
One or two sentences. Say what is wrong and where.

## Evidence
- `path/to/file.rs:123` - what the code does.
- How to observe it (command, input, or log pattern) in one line.

## Impact
Who or what is affected, and how badly.

## Requirements
1. What the fixed system must do. Do not describe how.
2. A check that proves the fix.

## Found by
`producer-loop` task `<task>`. A gatekeeper must validate this finding before a human approves it.
```

Title: a short noun phrase for the problem, at most 80 characters, no task prefix.
