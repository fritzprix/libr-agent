This task verifies wrong `SUDO_PASSWORD` handling without hanging (2565 B-2).
An automated harness intentionally supplies an incorrect synthetic password.
Never echo, cat, or print `$SUDO_PASSWORD`.

Step 1 — run `sudo -k && sudo true` with `storeAs: "SUDO_PASSWORD"`:

Call `workspace__runInPersistentShell` with `requireUserInput: true`,
`inputType: "password"`, `storeAs: "SUDO_PASSWORD"`, `inputPrompt: "System password for sudo"`,
and command:
`sudo -k && sudo true`

This command is expected to fail with a non-zero exit code. Do NOT retry `sudo`.

Step 2 — record that the failure returned cleanly without hanging:

Write exactly `WRONG_PASSWORD_HANDLED` to `result.txt` with `workspace__writeFile`, then stop.
