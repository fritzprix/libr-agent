# agy CLI recipes

Load when building the exact `agy` invocation.

## Binary

```bash
AGY=$(command -v agy || echo "$HOME/.local/bin/agy")
"$AGY" --help
"$AGY" models
```

## Print mode (one-shot)

```bash
agy -p "$TASK" \
  --model gemini-3.8-flash-high \
  --dangerously-skip-permissions \
  --print-timeout 300s \
  --add-dir /absolute/path/to/workspace
```

Aliases: `-p` ≡ `--print` ≡ `--prompt`.

## JSON result

```bash
agy -p "$TASK" \
  --model gemini-3.8-flash-high \
  --dangerously-skip-permissions \
  --output-format json \
  --print-timeout 300s \
  --add-dir /absolute/path/to/workspace
```

Useful fields:

| Field | Meaning |
| --- | --- |
| `status` | Expect `SUCCESS` |
| `response` | Final assistant text |
| `conversation_id` | Resume with `--conversation` |
| `duration_seconds` | Wall time |
| `num_turns` | Turns in this run |

Example success shape:

```json
{
  "conversation_id": "…",
  "status": "SUCCESS",
  "response": "…",
  "duration_seconds": 2.8,
  "num_turns": 1
}
```

## Resume

```bash
# same conversation by id (from JSON)
agy -p "$FOLLOW_UP" --conversation "$CONV_ID" --model gemini-3.8-flash-high \
  --dangerously-skip-permissions --print-timeout 300s

# most recent conversation
agy -p "$FOLLOW_UP" --continue --model gemini-3.8-flash-high \
  --dangerously-skip-permissions --print-timeout 300s
```

`-c` ≡ `--continue`.

## Useful flags

| Flag | Use |
| --- | --- |
| `--model <id>` | Use skill default `gemini-3.8-flash-high` unless the user names another id |
| `--add-dir <path>` | Extra workspace root (repeatable) |
| `--dangerously-skip-permissions` | Unattended tool approval |
| `--print-timeout <dur>` | e.g. `60s`, `300s`; `0` = wait forever |
| `--output-format text\|json\|stream-json` | Default `text` |
| `--effort low\|medium\|high\|max` | Reasoning effort when supported |
| `--mode accept-edits\|plan` | Execution mode |
| `--sandbox` | Restrict terminal |
| `--disable-slash-commands` | No slash/skill expansion in print mode |
| `--agent <name>` | Named agent profile if configured |

## Multiturn stdin (advanced)

`--input-format stream-json` + `--output-format stream-json`: one NDJSON message
per stdin line → one turn each. Prefer single `-p` unless batching turns.

## Failure handling

- Non-zero exit → fail the handoff; do not invent a result
- JSON with `status` other than `SUCCESS` → fail
- Empty `response` / empty stdout after “success” → re-prompt or escalate
- `agy: command not found` → install/path issue; do not fall back silently to
  another provider
