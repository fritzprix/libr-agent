# Wiki layout (host-global)

Cross-session store on the **user home account** (not the session workspace).

Resolved in code as `Path.home() / ".libragent" / "wiki"`:

| OS | Typical location |
| --- | --- |
| Linux | `~/.libragent/wiki` |
| macOS | `~/.libragent/wiki` |
| Windows | `%USERPROFILE%\.libragent\wiki` |

```text
<wikiRoot>/
  index.md
  logs.md
  skill-impact.md
  patterns/
    <id>.md
```

Print the absolute root on any OS:

```bash
python "<skill-base-dir>/scripts/wiki_cli.py" path
```

On Linux/macOS, `python3` is fine if `python` is missing. On Windows, use `python` (or the py launcher) from PATH.

## Init

```bash
python "<skill-base-dir>/scripts/wiki_cli.py" init
```

Creates missing files from:

| Wiki file | Template |
| --- | --- |
| `index.md` | [index-template.md](index-template.md) |
| `logs.md` | [logs-template.md](logs-template.md) |
| `skill-impact.md` | [skill-impact-template.md](skill-impact-template.md) |
| `patterns/<id>.md` | [pattern-template.md](pattern-template.md) |

Do **not** put this wiki under `{workspace}/.libragent/wiki` — that is session-local and cannot be reused by other sessions.
