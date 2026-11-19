This task verifies `asRoot` background daemon handling (2565 O-3).

Step 1 — attempt to start a background sleep daemon with `asRoot: true`:

Call `workspace__runShell` with `asRoot: true` and command:
`nohup sleep 120 >/dev/null 2>&1 & echo DAEMON_LAUNCHED`

Step 2 — record the tool result outcome:

Write `ASROOT_CHECKED` to `o3_result.txt` with `workspace__writeFile`, then stop.
