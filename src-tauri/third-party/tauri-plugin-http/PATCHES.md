# LibrAgent patches on `tauri-plugin-http` 2.6.1

Upstream: [tauri-plugin-http 2.6.1](https://crates.io/crates/tauri-plugin-http/2.6.1)

## Delta vs registry

| File | Change |
| --- | --- |
| `Cargo.toml` | Add optional `http-proxy` path dep and feature `libragent-pac-proxy`; keep `system-proxy` in defaults so macOS/Linux stay unchanged. |
| `src/commands.rs` | When `libragent-pac-proxy` is on: `ensure_proxy_resolved` (warn + continue on failure) then `configure_client_builder`. |
| `src/error.rs` | Only if needed for Message mapping (check on upgrade). |

## Upgrade checklist

1. Copy upstream 2.6.x sources over this tree.
2. Re-apply the three rows above.
3. `pnpm rust:check` and a Windows PAC smoke if available.
