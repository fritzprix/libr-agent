This task verifies terminating a session while a user prompt is waiting (2565 P-6).
An automated harness detects the child session's waiting `ui__askUser` prompt and
terminates the child session while it is blocked.

Step 1 — spawn a child session that blocks on `ui__askUser`:

Call `agent__listAgents` (type configs) and pick any configuration. Then call
`agent__spawnSession` with `complexity: low`, `waitForResult: false`, and this
task text verbatim:

> Call `ui__askUser` with one question: id `"p6_q"`, prompt
> `"Waiting prompt for P-6 terminate test"`, type `"text"`. Wait for the answer.

Remember the child session id it returns.

Step 2 — wait for the child session to settle after the harness terminates it:

Call `agent__checkSession` with the child session id and `wait: true`
(timeout 180).

Step 3 — record completion:

Write exactly `TERMINATED_OK` to `result.txt` with `workspace__writeFile`, then stop.
