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

## Windows (host-mode, manual)

`--isolated` does not run on Windows. To exercise the matrix there, start a
**debug desktop instance** yourself with the opt-in flags and run Harbor in
host mode against it:

```powershell
$env:LIBRAGENT_E2E_CONTROL = "1"
$env:LIBRAGENT_DATA_DIR = "C:\path\to\disposable\profile"
pnpm tauri build --debug   # then launch that binary
```

Caveat: the harness only answers prompts and approvals for **its own
isolated instance**. Host-mode Windows runs therefore do not auto-resolve
`requireUserInput` prompts yet — `e2e-interactive-echo` needs the isolated
runner (Linux) until a Windows instance manager lands.

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

A task may pin its own session mode with `execution_mode` under
`[metadata.libragent_session_e2e]`; otherwise it inherits
`LIBRAGENT_EXECUTION_MODE` (unchanged behaviour for existing tasks).

## Coverage manifest

See [`coverage-manifest.json`](./coverage-manifest.json) for all **65 checklist IDs** from issue #2565 (`implemented` | `deferred` | `ui-only` | `platform-specific` | `needs-product`). The issue's **74 result slots** are pass/fail/skip/open tallies, not extra checklist rows.

## Linux CI job (#2901)

`ci.yml` carries a `Session API E2E (Linux isolated, debug)` job: it builds the
debug binary, seeds the disposable instance, and runs the full matrix under
Xvfb — the job fails the PR when a task fails. It is inert until the owner
configures the secrets it reads: `SESSION_E2E_ASSISTANT_ID`, one of
`SESSION_E2E_SEED_DB` / `SESSION_E2E_SEED_DB_B64` (the pruned seed profile),
optionally `SESSION_E2E_MODEL`, `LIBRAGENT_SESSION_API_KEY`, and
`LIBRAGENT_E2E_SYNTHETIC_SECRET` (synthetic fixture value only). LLM keys come
from the seeded profile / CI secrets — never from a production profile.

## In-app orchestration (preferred)

For **last release → HEAD** planning and **internal delegate self-E2E** (not Harbor-only), use the repo skill `release-head-self-e2e` (`.agents/skills/release-head-self-e2e/`). Harbor tasks remain the verifier-backed case library this skill pulls from.

## B-6 logging contract (#2631)

Task `e2e-b6-log-variable-ref` keeps the known **variable-reference vs literal** conflict visible; it does not redefine B-6 as pass when command text containing `$env:VAR` appears in logs.

## B-11 Windows host requirement (#2655)

`e2e-b11-no-runshell` only has a passing path on a **Windows host** (the task exists because `workspace__runShell` is absent there). Its `task.toml` declares `requires_host_os = "windows"`; the harbor runners (`scripts/run-harbor-bench.sh` / `.ps1`, `path` preset) auto-add `-x e2e-b11-no-runshell` when the host OS does not match. The verifier asserts **tool calls and observations** (no successful `runShell`, no `submitInteractiveShellInput`, a successful PowerShell call), not blob string matching.
