This task verifies cancelling/stopping during a `SUDO_PASSWORD` prompt (2565 X-1).
An automated harness will CANCEL the waiting `SUDO_PASSWORD` prompt.

Step 1 — request `SUDO_PASSWORD` via `storeAs`:

Call `workspace__runInPersistentShell` with `requireUserInput: true`,
`inputType: "password"`, `storeAs: "SUDO_PASSWORD"`, `inputPrompt: "System password for sudo"`,
and command:
`sudo -k && sudo true`

The harness will cancel this prompt. Do NOT retry `sudo`.

Step 2 — verify the persistent shell is clean and usable after cancellation:

Call `workspace__runInPersistentShell` (without `requireUserInput`) with command:
`echo AFTER_CANCEL_OK > after_cancel.txt`

Then stop.
