# LibrAgent Session API E2E matrix (Harbor)

Unattended Harbor tasks for the [74-case matrix](https://github.com/fritzprix/libr-agent/issues/2565).
Verifiers grade **API messages, workspace files, and harness evidence** — never assistant prose.

## Prerequisites

1. **Debug** LibrAgent build (`pnpm tauri dev` or `pnpm tauri build --debug`).
2. Opt in on the **disposable** instance only:

```powershell
$env:LIBRAGENT_E2E_CONTROL = "1"
$env:LIBRAGENT_E2E_SYNTHETIC_SECRET = "synthetic-e2e-fixture-only"
pnpm bench:session-e2e:n1
```

Release builds and default dev (without `LIBRAGENT_E2E_CONTROL`) keep E2E control **disabled** (404).

## Windows native batch (primary)

```powershell
cd C:\path\to\libr-agent
$env:LIBRAGENT_E2E_CONTROL = "1"
$env:LIBRAGENT_E2E_SYNTHETIC_SECRET = "synthetic-e2e-fixture-only"
pip install harbor httpx
pnpm bench:session-e2e
```

Uses an **isolated** bench profile (`benchmarks/harbor/isolated_instance.py`) — not your production `%LOCALAPPDATA%` app data.

## Cloud / Linux limitation

These tasks exercise the real Session API and (on Windows) PowerShell workspace tools. Linux cloud agents can run **verifier unit tests** and the **fake control client** tests only; full matrix E2E requires a **Windows or Linux desktop debug instance** beside the harness.

## Coverage manifest

See [`coverage-manifest.json`](./coverage-manifest.json) for all **65 checklist IDs** from issue #2565 (`implemented` | `deferred` | `ui-only` | `platform-specific` | `needs-product`). The issue’s **74 result slots** are pass/fail/skip/open tallies, not extra checklist rows.

## B-6 logging contract (#2631)

Task `e2e-b6-log-variable-ref` keeps the known **variable-reference vs literal** conflict visible; it does not redefine B-6 as pass when command text containing `$env:VAR` appears in logs.
