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
(cd src-tauri && cargo build)
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

## Coverage manifest

See [`coverage-manifest.json`](./coverage-manifest.json) for all **65 checklist IDs** from issue #2565 (`implemented` | `deferred` | `ui-only` | `platform-specific` | `needs-product`). The issue's **74 result slots** are pass/fail/skip/open tallies, not extra checklist rows.

## In-app orchestration (preferred)

For **last release → HEAD** planning and **internal delegate self-E2E** (not Harbor-only), use the repo skill `release-head-self-e2e` (`.agents/skills/release-head-self-e2e/`). Harbor tasks remain the verifier-backed case library this skill pulls from.

## B-6 logging contract (#2631)

Task `e2e-b6-log-variable-ref` keeps the known **variable-reference vs literal** conflict visible; it does not redefine B-6 as pass when command text containing `$env:VAR` appears in logs.
