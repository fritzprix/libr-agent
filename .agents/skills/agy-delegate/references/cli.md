# agy CLI recipes

Load when building the exact `agy` invocation.

## Binary

```bash
# Bash (Linux / macOS)
AGY=$(command -v agy || echo "$HOME/.local/bin/agy")
"$AGY" --help
"$AGY" models
```

```powershell
# PowerShell (Windows)
$AGY = (Get-Command agy -ErrorAction SilentlyContinue)?.Source ?? "$env:LOCALAPPDATA\agy\bin\agy.exe"
& "$AGY" --help
& "$AGY" models
```

## Print mode (one-shot)

### Bash

```bash
agy -p "$TASK" \
  --model gemini-3.8-flash-high \
  --dangerously-skip-permissions \
  --print-timeout 300s \
  --add-dir /absolute/path/to/workspace
```

### PowerShell

> [!WARNING]
> Do NOT run `agy -p $TASK` without quotes. PowerShell splits multiline strings on whitespace into multiple positional arguments, triggering `Error: unexpected argument`. Always quote or use a Here-String:

```powershell
$TASK = @'
Goal: ...
Scope: ...
'@

& agy -p "$TASK" `
  --model gemini-3.8-flash-high `
  --dangerously-skip-permissions `
  --print-timeout 300s `
  --add-dir C:\absolute\path\to\workspace
```

Or use the bundled wrapper:
```powershell
& .agents/skills/agy-delegate/scripts/run.ps1 -Dir "C:\path\to\workspace" -Task "$TASK"
```

Aliases: `-p` ≡ `--print` ≡ `--prompt`.

## JSON result

### Bash

```bash
agy -p "$TASK" \
  --model gemini-3.8-flash-high \
  --dangerously-skip-permissions \
  --output-format json \
  --print-timeout 300s \
  --add-dir /absolute/path/to/workspace
```

### PowerShell

```powershell
& agy -p "$TASK" `
  --model gemini-3.8-flash-high `
  --dangerously-skip-permissions `
  --output-format json `
  --print-timeout 300s `
  --add-dir C:\absolute\path\to\workspace

# Or wrapper:
& .agents/skills/agy-delegate/scripts/run.ps1 -Dir "C:\path\to\workspace" -Json -Task "$TASK"
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
- `agy: command not found` → install/path issue; do not fall back silently to another provider
- `Error: unexpected argument "..."` on Windows → Unquoted `$TASK` caused PowerShell to split prompt words into CLI positional flags. Wrap prompt in `@' ... '@` Here-String or use `scripts/run.ps1`.
- `Error: -p took "--model" as its prompt` → `-p` was invoked without a prompt string value before other flags. Ensure prompt string directly follows `-p`.
- `Program 'agy.exe' failed to run: 액세스가 거부되었습니다 (Access is denied)` → Stale/background agy process or file lock. Kill lingering background tasks or verify binary path permission.
