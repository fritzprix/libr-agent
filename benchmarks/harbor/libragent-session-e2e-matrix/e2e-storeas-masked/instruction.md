Step 1 — run a storeAs command:

Use `workspace__runShell` with `requireUserInput: true`, `storeAs` set to
`E2E_STORED_SECRET`, and `inputPrompt` set to `E2E_SECRET_PROMPT echo the marker`.

The command must be exactly:

`echo E2E_MARKER_$((70+32))`

Step 2 — prove the value was substituted, without printing it:

Run exactly:

`test -n "$E2E_STORED_SECRET" && echo SECRET_PRESENT > stored_env.txt`

Never echo, cat, or otherwise print `$E2E_STORED_SECRET` or its value anywhere.
Never claim success unless a tool result containing `E2E_MARKER_102` is present.
