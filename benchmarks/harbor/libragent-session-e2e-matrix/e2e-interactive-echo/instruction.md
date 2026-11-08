Use `workspace__runInPersistentShell` with `requireUserInput: true`, `storeAs` set to
`E2E_ECHO_VALUE`, and `inputPrompt` set to `E2E_MARKER Echo test`.

The command must be exactly:

`echo E2E_MARKER_$((60+43))`

Never claim success unless a tool result containing `E2E_MARKER_103` is present.
