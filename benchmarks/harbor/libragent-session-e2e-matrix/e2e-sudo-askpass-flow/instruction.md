This task verifies the `SUDO_PASSWORD` `storeAs` + askpass flow in a test-only
privileged container (2565 P-1, 2565 B-1, 2672 S-2). An automated harness
answers the `SUDO_PASSWORD` prompt with the container's synthetic test password.
Never echo, cat, or print `$SUDO_PASSWORD`, and never call `sudo -S`, `sudo -n`,
`su`, or `pkexec`.

Step 1 — collect `SUDO_PASSWORD` via `storeAs` and run `sudo -k; sudo true`:

Call `workspace__runInPersistentShell` with `requireUserInput: true`,
`inputType: "password"`, `storeAs: "SUDO_PASSWORD"`, `inputPrompt: "System password for sudo"`,
and command:
`sudo -k && sudo true && echo SUDO_FIRST_OK > sudo_first.txt`

Step 2 — run a second plain `sudo` command in the same persistent shell without
asking for the password again:

Call `workspace__runInPersistentShell` (without `requireUserInput`) with command:
`sudo id -u > sudo_uid.txt`

Then stop. Never print `$SUDO_PASSWORD`.
