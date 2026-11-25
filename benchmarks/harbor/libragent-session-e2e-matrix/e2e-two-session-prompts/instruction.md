This task verifies multi-session prompt isolation (2565 X-2): you spawn a
companion session so both sessions hold waiting `storeAs` prompts at the same
time. An automated harness waits until both prompts are pending simultaneously,
answers the companion session first with a distinct value, and answers your
prompt second. You must NEVER echo, cat, or print any prompt answer value
itself — only its SHA-256 hex digest.

Step 1 — spawn a companion session:

Call `agent__listAgents` (type configs) and pick any configuration. Then call
`agent__spawnSession` with `complexity: low`, `waitForResult: false`, and this
task text verbatim:

> Run `workspace__runShell` with `requireUserInput: true`, `storeAs` set to
> `X2B_SECRET`, and `inputPrompt` set to `X2B_PROMPT echo the marker`. The
> command must be exactly:
> `printf '%s' "$X2B_SECRET" | sha256sum | awk '{print "BMARK_42 " $1}'`
> A harness answers the prompt automatically. Never echo, cat, or print
> `$X2B_SECRET` or its value. Your final answer must include the exact
> `BMARK_42 <hex>` line you observed in the tool result.

Remember the child session id it returns.

Step 2 — your own prompt (start this right away; do not wait for the child to
finish):

Run `workspace__runShell` with `requireUserInput: true`, `storeAs` set to
`X2A_SECRET`, and `inputPrompt` set to `X2A_PROMPT echo the marker`. The
command must be exactly:
`printf '%s' "$X2A_SECRET" | sha256sum | awk '{print $1}' > a_hash.txt && echo AMARK_$((60+7))`

Step 3 — collect the child's result:

Call `agent__checkSession` with the child session id and `wait: true`
(timeout 300). Write the child session's final result text verbatim into
`child_result.txt` with `workspace__writeFile`.

Then stop. Never echo, cat, or otherwise print `$X2A_SECRET`, `$X2B_SECRET`,
or any prompt answer value anywhere.
