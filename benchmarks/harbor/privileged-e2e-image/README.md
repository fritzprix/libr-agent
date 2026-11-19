# Privileged-Path E2E Test Container (`#2904`)

> [!CAUTION]
> **TEST-ONLY IMAGE.** The `NOPASSWD` sudo rule, synthetic password (`e2e-sudo-fixture`), fake `pkexec`, fake `e2e-askpass`, and debug-only mock keychain exist **only** for unattended E2E / Harbor verification.
>
> - **Never** use this image as a production LibrAgent session image.
> - **Never** install these shims or `NOPASSWD` sudoers rules on a developer host or CI runner host.
> - **Never** pass real credentials; use synthetic fixture values only.

## What this image provides

1. **Dedicated non-root user (`e2etest`) with controlled `sudo`**:
   - Synthetic password `e2e-sudo-fixture` in group `sudo`, so `sudo -k && sudo true` with `SUDO_ASKPASS` succeeds with the synthetic fixture password (`2565 B-1`, `2672 S-2`, `2672 K-2`) and fails cleanly with any wrong password (`2565 B-2`) without hanging.
   - Scoped `NOPASSWD` rule (`/etc/sudoers.d/99-e2e-test-only`) for `/usr/local/bin/e2e-nopasswd-probe` only.
2. **Fake `pkexec` on `PATH` (`/usr/local/bin/pkexec`)**:
   - Appends JSONL records to `${LIBRAGENT_E2E_PRIVILEGED_LOG_DIR:-/app/.e2e-privileged-logs}/pkexec.log` so verifiers (such as `2672 K-2`) can assert `pkexec` was never called.
   - Mode controlled by `LIBRAGENT_E2E_FAKE_PKEXEC_MODE=fail|succeed` (default `fail`, exit 126).
3. **Fake askpass shim (`/usr/local/bin/e2e-askpass`)**:
   - Appends JSONL records (never logging the password value) to `${LIBRAGENT_E2E_PRIVILEGED_LOG_DIR:-/app/.e2e-privileged-logs}/askpass.log`.
   - Mode controlled by `LIBRAGENT_E2E_FAKE_ASKPASS_MODE=succeed|fail` (default `succeed`) and `LIBRAGENT_E2E_FAKE_ASKPASS_VALUE` (default `e2e-sudo-fixture`).
4. **Debug-only Mock Keychain (`LIBRAGENT_E2E_MOCK_KEYCHAIN`)**:
   - Implemented in `src-tauri/src/mcp/keychain.rs` behind `#[cfg(debug_assertions)]` and gated on the same guard as the control plane (`LIBRAGENT_E2E_CONTROL=1` **and** an explicit `LIBRAGENT_DATA_DIR`). Without a data dir the mock is inactive and the real keychain is used; there is no temp-dir fallback. Never compiled into release builds.
   - Modes:
     - `succeed`: stores/reads/deletes secrets in `$LIBRAGENT_DATA_DIR/e2e_mock_keychain.json`.
     - `fail-write`: `available()` returns `true` while `set()` errors (`2565 M-7` / `2672 M-4`).
     - `fail-read`: `available()` returns `true` while `get()` errors.
     - `unavailable`: `available()` returns `false`, causing `LayeredSecretStore` to fall back to `FileSecretStore` (`2672 M-5`).
   - Every operation appends a JSONL entry (without secret values) to `$LIBRAGENT_DATA_DIR/e2e_mock_keychain.log` (owner-only 0600 store; the harness copies both files into `/logs/agent/` for the verifier) (or `LIBRAGENT_E2E_MOCK_KEYCHAIN_LOG`).

## Build

```bash
docker build -t libragent-e2e-privileged:test benchmarks/harbor/privileged-e2e-image
```
