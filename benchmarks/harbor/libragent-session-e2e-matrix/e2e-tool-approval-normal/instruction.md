Use `workspace__writeFile` once to create `approval_marker.txt` in the workspace
root with the exact content `APPROVED`.

In this session the tool-approval dialog opens for that call and the harness
answers it automatically — wait for the tool result instead of giving up.

When the tool result confirms the write, finish. Never retry the same call
after a successful result.
