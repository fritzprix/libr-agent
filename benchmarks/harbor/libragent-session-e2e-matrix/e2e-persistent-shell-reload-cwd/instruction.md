This task verifies that the persistent shell keeps its current working directory across a shell reload (2685 P-4).

Step 1 — Using `workspace__runInPersistentShell`, create and enter a subdirectory:
`mkdir -p sub_p4_dir && cd sub_p4_dir && pwd`

Step 2 — Trigger a persistent shell reload by calling `workspace__runInPersistentShell` with:
- `command`: `pwd > ../p4_cwd.txt && test -n "$P4_RELOAD_TOKEN" && echo RELOAD_OK`
- `storeAs`: `"P4_RELOAD_TOKEN"`
(The automated harness answers the secret prompt, which reloads the persistent shell while preserving its working directory.)

Step 3 — Stop once `/app/p4_cwd.txt` is written.
