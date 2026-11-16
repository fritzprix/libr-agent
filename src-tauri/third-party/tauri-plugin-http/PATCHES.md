# LibrAgent patches on `tauri-plugin-http` 2.6.1

Upstream: [tauri-plugin-http 2.6.1](https://crates.io/crates/tauri-plugin-http/2.6.1)

## Delta vs registry

| File | Change |
| --- | --- |
| `Cargo.toml` | Add optional `http-proxy` path dep and feature `libragent-pac-proxy`; keep `system-proxy` in defaults so macOS/Linux stay unchanged. |
| `src/commands.rs` | When `libragent-pac-proxy` is on: `ensure_proxy_resolved` (warn + continue on failure) then `configure_client_builder`. |
| `src/error.rs` | Keep `Error::Message` — used by `commands.rs` for resolve/config errors. |

Also keep unmodified upstream files: `api-iife.js`, `LICENSE_MIT`, `LICENSE_APACHE-2.0`, `LICENSE.spdx` (required by `build.rs` / license notice).

## Upgrade checklist

1. Copy upstream 2.6.x sources over this tree (including `api-iife.js` and LICENSE*).
2. Re-apply the three rows above.
3. `pnpm rust:check` and a Windows PAC smoke if available.
