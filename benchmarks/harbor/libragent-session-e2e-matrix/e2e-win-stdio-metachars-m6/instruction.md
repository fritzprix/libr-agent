This task verifies 2672 M-6 on Windows: a stdio MCP server argument containing Windows `cmd.exe` metacharacters (`&`, `|`, `^`, `<`, `>`) must be passed directly to the child process without `cmd.exe` splitting or escaping corruption.

Step 1 — Write a small Python script `m6_echo_args.py` in the workspace with this exact content:
```python
import json, pathlib, sys
pathlib.Path("m6_received_args.json").write_text(
    json.dumps(sys.argv[1:]), encoding="utf-8"
)
```

Step 2 — Register a stdio MCP server using `tool__registerServer` with:
- `name`: `"e2e-win-m6-metachars"`
- `transport`: `{"type": "stdio", "command": "python", "args": ["m6_echo_args.py", "a&b|c^d<e>f"]}`

(The helper script exits right after recording `sys.argv[1:]` to `m6_received_args.json`, so MCP handshake failure after spawn is expected.)

Step 3 — Confirm `m6_received_args.json` exists in the workspace, then stop.
