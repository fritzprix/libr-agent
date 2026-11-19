Step 1 — run a storeAs command:

Use `workspace__runShell` with `requireUserInput: true`, `storeAs` set to
`P9_SECRET`, and `inputPrompt` set to `P9_SECRET_PROMPT echo the marker`.

The command must be exactly:

`echo P9MARK_$((50+25))`

Step 2 — prove the value was substituted, without printing it:

Run exactly:

`test -n "$P9_SECRET" && echo P9_PRESENT > p9_env.txt`

Then stop. Never echo, cat, or otherwise print `$P9_SECRET` or its value
anywhere. Never claim success unless a tool result containing `P9MARK_75`
is present.
