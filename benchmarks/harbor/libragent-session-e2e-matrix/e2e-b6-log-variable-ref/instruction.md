Using `workspace__runInPersistentPowerShell` with `requireUserInput: true`, store a session value as environment variable `E2E_DEMO_TOKEN` (use `storeAs` if the tool supports it, otherwise set `$env:E2E_DEMO_TOKEN` via the prompt).

Then run `Write-Output "val=$env:E2E_DEMO_TOKEN"`.

The harness supplies a synthetic value via E2E control; do not embed secrets in chat text.
