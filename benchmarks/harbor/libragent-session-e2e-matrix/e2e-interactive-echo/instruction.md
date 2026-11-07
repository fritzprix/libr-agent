Use `workspace__runInPersistentPowerShell` with `requireUserInput: true` and `inputPrompt` set to
`E2E_MARKER Echo test`.

The command to run after input is collected must be exactly:

`Write-Output "E2E_MARKER_OK"`

Do not use `runShell` (unavailable on Windows).
