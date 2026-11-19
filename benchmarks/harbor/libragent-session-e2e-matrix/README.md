# LibrAgent Session API E2E matrix (Harbor)

Unattended Harbor tasks for the [74-case matrix](https://github.com/fritzprix/libr-agent/issues/2565).
Verifiers grade **API messages, workspace files, and harness evidence** — never assistant prose.

## Supported run path (Linux, isolated)

The unattended path is the **isolated Linux runner**. `pnpm bench:session-e2e`
already passes `--isolated --debug-app-bin`, so it launches the **debug**
bench binary (the only build that contains the E2E control-plane routes) and
enables the control plane on that disposable instance automatically.

```bash
# once: build the debug binary and seed the disposable bench instance
# (run-rust-command.cjs holds the machine-wide Rust build slot, see #2636)
node scripts/run-rust-command.cjs build
pnpm bench:instance:seed --assistant-id <uuid>

# run the matrix
export LIBRAGENT_E2E_SYNTHETIC_SECRET="synthetic-e2e-fixture-only"
pnpm bench:session-e2e:n1     # first task only
pnpm bench:session-e2e        # full matrix
```

`--isolated` is Linux-only (stopping detached sidecars relies on
`/proc/<pid>/environ`), and the release binary never contains these routes —
so this is the only fully unattended combination today. If a task declares
`[metadata.libragent_session_e2e]` but the control plane is not reachable,
the trial log prints an explicit warning and the evidence file records
`"controlPlaneConfigured": false` instead of silently passing or hanging.

Release builds and default dev runs (without `LIBRAGENT_E2E_CONTROL`) keep
E2E control **disabled** (404).

## Windows (isolated, unattended) (#2905)

`--isolated` runs on Windows too. The same commands seed, launch, stop, and
relaunch a disposable instance, and the harness answers prompts and approvals
for **its own** instance exactly like on Linux:

```powershell
python -m benchmarks.harbor.isolated_instance seed --assistant-id <uuid>
pnpm bench:session-e2e   # launches the debug binary under the disposable profile
```

Windows has no `/proc`, so the manager identifies the instance tree only by
things it controls itself: a launch manifest (`instance.json`: PID, process
creation time, per-launch job name) plus a **named job object** every process
of the tree joins at launch. Stop first tries `taskkill /PID` (graceful), then
terminates the job object — which also kills detached sidecars. A recycled PID
is rejected via the creation-time stamp, and a stop can only reach the job
whose random per-launch token is in the manifest. Processes are never selected
by the `libragent` image name.

Profile isolation mirrors the Linux XDG setup: `APPDATA`, `LOCALAPPDATA`,
`USERPROFILE`, `HOME`, `TEMP`/`TMP` are redirected into the run dir, and the
primary profile (`%APPDATA%\com.fritzprix.libragent`) is refused just like the
Linux one. The bubblewrap host-mode workspace sandbox is Linux-only; on
Windows the flag means the isolated instance itself is the safety condition
(job object + redirected profiles), and the adapter refuses Windows host mode
outside disposable CI runners (`CI=true`).

The control-plane contract is shared across platforms: the debug-only,
loopback, token-guarded S1/S3 HTTP routes for interaction polling and prompt
and approval resolution behave the same on Windows — they are plain HTTP
against the same debug binary. The file-picker injection and in-place restart
routes land with #2903 (PR #2916) and are not part of this change. Only the S3
*window* size/visibility helper is Linux-side (X11); it has no Windows
equivalent in the manager.

For interactive debugging the manual host-mode path still works: start a
debug desktop instance yourself with `$env:LIBRAGENT_E2E_CONTROL = "1"` and a
disposable `$env:LIBRAGENT_DATA_DIR`, then run Harbor in host mode against it.

## Circuit-breaker Resume (#2901)

`ui__circuitBreak` stops the workflow and waits for a human Resume click. Debug
builds additionally expose `POST /api/e2e-control/sessions/:ref/circuit-break-resume`
(loopback, token-gated), which injects the exact `ui__resumeCircuitBreak`
tool-call the GUI button sends. `POST /api/sessions/:id/resume` is a different
reload path (#2898) and never touches the dialog. The poller sees the waiting
break in `GET …/interactions` (`pendingCircuitBreak`) and resumes it when the
task spec sets `resume_circuit_break = true` (the default).

## New tasks (#2901)

- `e2e-askuser-answer` — one `ui__askUser` question answered through the control
  plane; the agent must write the delivered answer verbatim to `answer.txt`.
- `e2e-circuit-break-resume` — trips the tool-loop guard with six identical
  `workspace__writeFile` calls, gets resumed through the new endpoint, and must
  finish by writing `break_marker.txt`.
- `e2e-tool-approval-normal` — pins `execution_mode = "normal"` so a workspace
  write really opens a tool-approval request; the harness grants it through the
  control plane and the verifier confirms the approved call executed.
- `e2e-storeas-masked` — the agent runs a `storeAs` command; the harness answers
  the prompt with the synthetic secret, the value must reach the shell env, and
  the verifier fails if the secret literal appears anywhere in the trajectory.
- `e2e-bool-string-coercion` — the agent sends `include_checked` as the string
  `"true"`; the shared args helper must coerce it with its warning and the call
  must run, and an `ui__askUser` question with the reserved-sounding key
  `options` must reach the harness instead of being rejected by key-name rules.
- `e2e-mcp-secret-template` — the agent registers an MCP server that needs a
  key; the verifier requires the `{{secret:E2E_WEATHER_KEY}}` template in the
  trajectory (tool-call arguments included), rejects any chat prompt for the
  secret value, and fails if any secret literal appears anywhere.
- `e2e-clarify-requirements` — a vague `기획해줘` request must trigger the
  clarify flow; the verifier requires delivered clarification answers, the
  clarify skill loaded, and a structured `requirements.md` maintained in the
  workspace.
- `e2e-askuser-cancel` — the harness cancels a waiting `ui__askUser` prompt
  through the control plane (the same path the Cancel/Stop buttons drive,
  `cancel_interactive = true`); the verifier requires the delivered cancel
  record, an informational cancel tool result, and the agent continuing to
  completion afterwards.
- `e2e-askuser-options` — one `ui__askUser` call with an options question, a
  `multiSelect` question, and a free-text question; the harness answers with
  the task's `answer_value` (a JSON object keyed by question id), and the
  verifier grades the keyed result shapes in the delivered tool result.
- `e2e-answer-masked-logs` — a `storeAs` prompt is answered with the synthetic
  secret; the verifier proves the value reached the shell, then byte-scans
  every harness artifact under `/logs/agent/` (trajectory, compaction summary,
  instance log) and fails if the recorded `injectedValue` appears in any of
  them — only the evidence record itself may hold the exact value.
- `e2e-two-session-prompts` — 2565 X-2: the parent spawns a child session and
  both hold waiting `storeAs` prompts simultaneously. With
  `resolve_child_interactions = true` and
  `wait_for_overlapping_child_prompt = true`, the harness holds answers until
  both sessions have a `pendingUserPrompt` in the same poll cycle
  (`simultaneousPending: true`), answers the child first with a distinct value,
  and answers the primary second. Each session writes the SHA-256 hex digest of
  its env var so the verifier proves per-session answer isolation without
  leaking literals.
- `e2e-storeas-replace` — 2565 B-8: re-stores the same `storeAs` variable in
  the persistent shell with a distinct second synthetic value
  (`distinct_secret_per_prompt = true`), proving via SHA-256 digests that the
  second value replaced the first while keeping `cwd` unchanged and leaking
  neither literal.
- `e2e-askuser-terminate-while-waiting` — 2565 P-6 (prompt half): a child
  session blocks on `ui__askUser`; with `terminate_child_on_prompt = true`, the
  harness terminates the child via `POST /api/sessions/:id/terminate`, verifies
  `pendingUserPrompt` is cleared, and the parent completes cleanly.
- `e2e-mcp-approval-preview` — 2672 M-3 + 2565 M-4: `tool__registerServer`
  with a literal query param, URL fragment, and `{{secret:NAME}}` param; the
  verifier asserts the `toolApproval` `commandPreview` masks literal values as
  `***` while keeping `{{secret:NAME}}` visible, and that cancelling the secret
  prompt writes no server row.
- `e2e-mcp-secret-lifecycle` — 2565 M-2, M-6, M-8, X-6: registers an HTTP MCP
  server with a URL `{{secret:NAME}}` template, updates it with
  `reenterSecrets` (receiving a second distinct secret), verifies the MCP
  secret store is separate from shell `storeAs` env, and byte-scans `/logs/agent/`
  for zero leaks.
- `e2e-skill-storeas-flows` — 2565 S-1, S-2, S-5, S-6 & 2685 E-3: exercises the
  generic `storeAs` mechanism under the four skills' variable names
  (`TELEGRAM_API_HASH`, `X_PASSWORD`,
  `LIBRAGENT_EMAIL_PASSWORD`, `LIBRAGENT_GOOGLE_CLIENT_SECRET`) with distinct
  synthetic values, verifying each via SHA-256 digest and byte-scanning
  `/logs/agent/` for zero leaks. The skills' own instructions are not exercised.

A task may pin its own session mode with `execution_mode` under
`[metadata.libragent_session_e2e]`; otherwise it inherits
`LIBRAGENT_EXECUTION_MODE` (unchanged behaviour for existing tasks).

## Coverage manifest

See [`coverage-manifest.json`](./coverage-manifest.json) for all **65 checklist IDs** from issue #2565 (`implemented` | `deferred` | `ui-only` | `platform-specific` | `needs-product`). The issue's **74 result slots** are pass/fail/skip/open tallies, not extra checklist rows.

## Release-build gating (2672 E-2)

Release binaries cannot serve `/api/e2e-control/*` at all: the module
declaration, the capability-token init, and both route-merge sites in
`src-tauri/src/server/` are compile-time gated behind
`#[cfg(debug_assertions)]`. `e2e_control_stays_compile_time_gated`
(`src-tauri/tests/e2e_control_token_tests.rs`) pins every gate site, and
`disabled_control_plane_answers_404_even_with_a_token` covers the debug
runtime path where the control plane is off.

## Linux CI job (#2901)

`ci.yml` carries a `Session API E2E (Linux isolated, debug)` job: it builds the
debug binary, seeds the disposable instance, and runs the full matrix under
Xvfb — the job fails the PR when a task fails. It is inert until the owner
configures the secrets it reads: `SESSION_E2E_ASSISTANT_ID`, one of
`SESSION_E2E_SEED_DB` / `SESSION_E2E_SEED_DB_B64` (the pruned seed profile),
optionally `SESSION_E2E_MODEL`, `LIBRAGENT_SESSION_API_KEY`, and
`LIBRAGENT_E2E_SYNTHETIC_SECRET` (synthetic fixture value only). LLM keys come
from the seeded profile / CI secrets — never from a production profile.

A sibling `Session API E2E (Windows isolated, debug)` job (#2905) runs the same
matrix on `windows-latest` against the Windows isolated manager described
above, reading the same secrets. `requires_host_os = "windows"` tasks
(e.g. `e2e-b11-no-runshell`) run there and are auto-excluded on Linux; tasks
gated to other hosts are auto-excluded here the same way.

## In-app orchestration (preferred)

For **last release → HEAD** planning and **internal delegate self-E2E** (not Harbor-only), use the repo skill `release-head-self-e2e` (`.agents/skills/release-head-self-e2e/`). Harbor tasks remain the verifier-backed case library this skill pulls from.

## B-6 logging contract (#2631)

Task `e2e-b6-log-variable-ref` keeps the known **variable-reference vs literal** conflict visible; it does not redefine B-6 as pass when command text containing `$env:VAR` appears in logs.

## B-11 Windows host requirement (#2655)

`e2e-b11-no-runshell` only has a passing path on a **Windows host** (the task exists because `workspace__runShell` is absent there). Its `task.toml` declares `requires_host_os = "windows"`; the harbor runners (`scripts/run-harbor-bench.sh` / `.ps1`, `path` preset) auto-add `-x e2e-b11-no-runshell` when the host OS does not match. The verifier asserts **tool calls and observations** (no successful `runShell`, no `submitInteractiveShellInput`, a successful PowerShell call), not blob string matching.

## Privileged-path test container & mock keychain (#2904)

See [`../privileged-e2e-image/README.md`](../privileged-e2e-image/README.md) for the **test-only** privileged container (`e2etest` with synthetic password `e2e-sudo-fixture`, scoped `NOPASSWD` probe, fake `pkexec`, fake `e2e-askpass`) and the debug-only `LIBRAGENT_E2E_MOCK_KEYCHAIN` backend (`succeed` | `fail-write` | `fail-read` | `unavailable`). Never use this image for real work and never enable `NOPASSWD` sudo or fake `pkexec` on a host machine.

- `e2e-sudo-askpass-flow` — 2565 P-1, B-1 & 2672 S-2 (`SUDO_PASSWORD` via `storeAs` + follow-up plain `sudo` via askpass).
- `e2e-sudo-wrong-password` — 2565 B-2 (wrong `SUDO_PASSWORD` fails cleanly without hanging or leaking; the sudo tool result itself must report the rejection).
- `e2e-sudo-setup-wizard-k2` — 2672 K-2 (asks once for `SUDO_PASSWORD`, plain `sudo` only; the verifier tokenizes commands and rejects `sudo -n`/`-S`/`--stdin`/`--non-interactive` in any spelling, combined short flags, `env` prefixes, and any `su`/`pkexec`).
- `e2e-sudo-cancel-x1` — 2565 X-1 (cancelling a waiting `SUDO_PASSWORD` prompt cleans up the prompt and leaves the persistent shell usable).
- `e2e-asroot-daemon-o3` — 2565 O-3 (`runShell(asRoot=true)` background daemon must be stopped by the product with the stopped-process notice; the daemon's own echo is not accepted as evidence).
- `e2e-keychain-fail-and-fallback` — 2565 M-7 / 2672 M-4 (fail-write half only): the task pins `mock_keychain_mode = "fail-write"`, the harness copies the mock's log and store into `/logs/agent`, and the verifier reads them directly - a set error was recorded and no secret persisted. The 2672 M-5 unavailable→file-fallback half stays `deferred` until a task runs `unavailable` mode.

**Required invocation for the privileged tasks (G1.2).** Their fakes (`e2etest`, fake `pkexec`/`askpass`) live in the task image, so they declare `requires_workspace_mode = "managed"` and the adapter fails fast in any other mode (host mode would run `sudo` against this machine). Run each with:

```bash
node scripts/run-harbor-bench.cjs --isolated --debug-app-bin --workspace-mode managed \
  --preset path --path benchmarks/harbor/libragent-session-e2e-matrix/<task-dir>
```

The shim logs live under the shared workspace at `/app/.e2e-privileged-logs/` (`pkexec.log`, `askpass.log`), created empty in the image build, so verifiers distinguish "never invoked" (exists, empty) from "unreachable" (missing). The product's own askpass helper lives in a per-session `mktemp` dir by design (the password never touches argv), so the image `askpass.log` is a bypass detector: the product never calls it, and any record there means something went around the product shim.
