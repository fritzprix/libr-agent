# system-proxy-pac (PoC)

Standalone proof-of-concept: discover the OS system proxy configuration (including PAC on Windows), resolve the proxy for a given URL the same way WinHTTP / CFNetwork does, and compare a **direct** `reqwest` GET with a GET routed through the resolved proxy.

This crate is **not** part of the Tauri workspace and is **not** intended to merge into the app.

## Requirements

- Rust 1.83+ (same era as the main app’s `reqwest` 0.12 / `rustls-tls` stack)
- Windows 10+ (primary target)
- macOS 11+ (optional; uses CFNetwork)
- Linux: environment-variable fallback only (for CI / cross-check builds)

## Build (Windows PowerShell)

```powershell
cd poc\system-proxy-pac
cargo build --release
```

Run:

```powershell
.\target\release\system-proxy-pac.exe "http://YOUR_HOST:YOUR_PORT/v1/models" --timeout-secs 45
```

Replace `YOUR_HOST` and `YOUR_PORT` with your corporate LLM or OpenAI-compatible endpoint (no credentials required; any HTTP status, including `401`, counts as reachable).

## Build (developer Linux VM — native smoke test)

```bash
cd poc/system-proxy-pac
cargo build
cargo run -- https://example.com --timeout-secs 15
```

On Linux, section 1 reads `HTTP_PROXY` / `HTTPS_PROXY`; PAC is not evaluated.

## Cross-compile check (Windows GNU target)

```bash
cd poc/system-proxy-pac
rustup target add x86_64-pc-windows-gnu
cargo check --target x86_64-pc-windows-gnu
```

If linking fails, install `mingw-w64` (Debian/Ubuntu: `gcc-mingw-w64-x86-64`).

## What output proves PAC works

On a corporate Windows profile where **only** a PAC URL is set (no `HTTP(S)_PROXY` env vars):

1. **Discovery** shows your `pac_url` (and optionally `auto_detect`).
2. **Resolved proxy** for the LLM URL is **not** `DIRECT` — e.g. `http://corp-proxy:8080`.
3. **Request A (direct)** fails with `connect` or `timeout` within the timeout window.
4. **Request B (proxied)** succeeds with `HTTP 200`, `HTTP 401`, or another status (reachable).

Example pattern:

```text
=== Request A: direct reqwest (no proxy applied) ===
status: failed — error (connect): ... (elapsed 30.00s)

=== Request B: reqwest via resolved proxy ===
Using proxy: http://proxy.corp.example:8080
status: success — HTTP 401 (elapsed 1.23s)
```

## Implementation notes

- **Windows**: `WinHttpGetIEProxyConfigForCurrentUser` + `WinHttpGetProxyForUrl` via the `windows` crate (same PAC engine WebView2 / WinINET use for autoproxy).
- **macOS**: `CFNetworkCopySystemProxySettings` + `CFNetworkCopyProxiesForURL`.
- **Other**: `HTTP_PROXY` / `HTTPS_PROXY` / `NO_PROXY` only.
- **HTTP client**: `reqwest` 0.12 with `rustls-tls`, matching `llm-client`. Direct client uses `.no_proxy()` so env vars do not mask the comparison.