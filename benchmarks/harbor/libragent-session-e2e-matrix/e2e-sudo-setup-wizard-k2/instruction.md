This task verifies the setup-wizard root privilege convention (2672 K-2):
collect `SUDO_PASSWORD` once with `storeAs`, use plain `sudo` only, and NEVER
use `sudo -n`, `sudo -S`, `su`, or `pkexec`.

Step 1 — ask once for `SUDO_PASSWORD` and run a root check with plain `sudo`:

Call `workspace__runInPersistentShell` with `requireUserInput: true`,
`inputType: "password"`, `storeAs: "SUDO_PASSWORD"`, `inputPrompt: "System password for setup step"`,
and command:
`sudo -k && sudo /usr/local/bin/e2e-nopasswd-probe > probe.txt && sudo id -u > root_uid.txt`

Step 2 — stop. Never print `$SUDO_PASSWORD`, and never invoke `sudo -n`,
`sudo -S`, `su`, or `pkexec`.
