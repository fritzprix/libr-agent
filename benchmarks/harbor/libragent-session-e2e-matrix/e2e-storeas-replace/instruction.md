This task verifies `storeAs` value replacement in the persistent shell (2565 B-8).
An automated harness answers each `storeAs` prompt with a distinct synthetic
value. You must NEVER echo, cat, or print `$B8_SECRET` or its value anywhere —
only its SHA-256 hex digest.

Step 1 — enter a subdirectory in the persistent shell:

Call `workspace__runInPersistentShell` with command:
`mkdir -p subdir && cd subdir && pwd > ../cwd0.txt`

Step 2 — store the first value for `B8_SECRET`:

Call `workspace__runInPersistentShell` with `requireUserInput: true`,
`inputType: "password"`, `storeAs: "B8_SECRET"`, `inputPrompt: "Enter first B8_SECRET"`,
and command:
`printf '%s' "$B8_SECRET" | sha256sum | awk '{print $1}' > ../hash1.txt && pwd > ../cwd1.txt`

Step 3 — re-store `B8_SECRET` with a second value:

Call `workspace__runInPersistentShell` with `requireUserInput: true`,
`inputType: "password"`, `storeAs: "B8_SECRET"`, `inputPrompt: "Enter replacement B8_SECRET"`,
and command:
`printf '%s' "$B8_SECRET" | sha256sum | awk '{print $1}' > ../hash2.txt && pwd > ../cwd2.txt`

Then stop. Never print `$B8_SECRET`.
