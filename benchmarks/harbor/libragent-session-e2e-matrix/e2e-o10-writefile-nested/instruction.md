In the workspace root, use **only** `workspace__writeFile` (not shell).

1. Create a regular file at `parent.txt` with content `parent`.
2. Attempt to create `parent.txt/child.txt` with content `child`.

Do not use `runShell` or `runPowerShell` (they are unavailable on this platform). Report tool errors verbatim in your final summary.
