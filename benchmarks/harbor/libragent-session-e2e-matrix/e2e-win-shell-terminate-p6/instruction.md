This task verifies the Windows persistent-shell process half of 2565 P-6.
An automated harness watches the child session's waiting `ui__askUser` prompt,
records whether the child's persistent PowerShell shell is alive, terminates the
child session, and then checks that the recorded process id is gone and stays
gone through a settle window. The no-replacement guarantee itself is owned by
the backend unit test `force_kills_persistent_shell_without_leaving_manager_entry`
over `force_kill_shell`; this task does not re-prove it.

Step 1 — spawn a child session that parks on `ui__askUser` after touching a
persistent shell:

Call `agent__listAgents` (type configs) and pick any configuration. Then call
`agent__spawnSession` with `complexity: low`, `waitForResult: false`,
`workspaceOverride: "current"` (the child must share this workspace), and this
task text verbatim:

> Step 1: Call `workspace__runInPersistentPowerShell` with the command
> `$PID | Set-Content -Path p6_shell_pid.txt -Encoding ascii`
> so the persistent shell records its own process id in the shared workspace.
> Step 2: Call `ui__askUser` with one question: id `"p6w_q"`, prompt
> `"Waiting prompt for P-6 Windows shell terminate test"`, type `"text"`.
> Wait for the answer. Do not proceed without it.

Remember the child session id it returns.

Step 2 — wait for the child session to settle after the harness terminates it:

Call `agent__checkSession` with the child session id and `wait: true`
(timeout 180).

Step 3 — record completion:

Write exactly `SHELL_TREE_CLEAN` to `result.txt` with `workspace__writeFile`,
then stop.
