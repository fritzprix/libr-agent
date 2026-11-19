This task is a smoke test for multi-session prompt answering: you spawn a
companion session and an automated harness answers every waiting prompt. It
checks that the harness plumbing works across sessions; it does not prove
that the sessions' answer channels stay separate. You must never print any
prompt answer value yourself.

Step 1 — spawn a companion session:

Call `agent__listAgents` (type configs) and pick any configuration. Then call
`agent__spawnSession` with `complexity: low`, `waitForResult: false`, and this
task text verbatim:

> Run `workspace__runShell` with `requireUserInput: true`, `storeAs` set to
> `X2B_SECRET`, and `inputPrompt` set to `X2B_PROMPT echo the marker`. The
> command must be exactly: `echo BMARK_$((20+22))`. A harness answers the
> prompt automatically. Never echo, cat, or print `$X2B_SECRET` or its value.
> Your final answer must state the exact marker you observed in the tool
> result.

Remember the child session id it returns.

Step 2 — your own prompt (start this right away; do not wait for the child to
finish):

Run `workspace__runShell` with `requireUserInput: true`, `storeAs` set to
`X2A_SECRET`, and `inputPrompt` set to `X2A_PROMPT echo the marker`. The
command must be exactly: `echo AMARK_$((60+7))`

Step 3 — collect the child's result:

Call `agent__checkSession` with the child session id and `wait: true`
(timeout 300). Write the child session's final result text verbatim into
`child_result.txt` with `workspace__writeFile`.

Step 4 — prove your own answer arrived, without printing it:

Run exactly:

`test -n "$X2A_SECRET" && echo A_PRESENT > a_env.txt`

Then stop. Never echo, cat, or otherwise print `$X2A_SECRET`, `$X2B_SECRET`,
or any prompt answer value anywhere.
