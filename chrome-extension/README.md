# LibrAgent Browser Bridge (Chrome MV3)

Local **Load unpacked** extension that drives your everyday Chrome profile via a
loopback WebSocket to LibrAgent. This is an MVP — not published to the Chrome
Web Store.

## Load unpacked

1. Start LibrAgent (desktop app). The extension bridge listens on
   `ws://127.0.0.1:3847/extension-bridge` by default.
2. Open Chrome → `chrome://extensions` → enable **Developer mode**.
3. Click **Load unpacked** and select this folder
   (`chrome-extension/` in the LibrAgent repo).
4. Confirm the extension connects (service worker console should log
   `Connected to ws://127.0.0.1:3847/...`).

You can also copy the absolute path from **Settings → System → Agent browser →
Chrome extension bridge**.

## Defaults

| Setting | Default | Override |
| --- | --- | --- |
| Port | `3847` | Extension Options, or `LIBRAGENT_EXTENSION_BRIDGE_PORT` in the app |
| Token | `libragent-dev` | Extension Options, or `LIBRAGENT_EXTENSION_BRIDGE_TOKEN` in the app |

MVP auth: when the app env token is unset, LibrAgent uses the fixed dev token
`libragent-dev`. Production will use a rotating token.

## Backend selection (app)

| `LIBRAGENT_BROWSER_BACKEND` | Behavior |
| --- | --- |
| unset / `auto` | Prefer the extension when connected; otherwise sticky CDP sidecar |
| `extension` | Require the extension bridge |
| `sidecar` | Always use the sticky agent Chromium sidecar |

Supported via extension today: `createSession`, `navigate`, `closeSession`,
`getState`. Screenshot / evaluate / click / etc. still need the sidecar (or a
future extension capability).

## Manual smoke test

1. Load the extension and confirm Settings shows **Connected**.
2. Ask an agent to `browser__createSession` with a normal HTTPS URL.
3. A new tab should open in your everyday Chrome (not the sticky agent profile).
4. Disconnect the extension (disable it) → new sessions fall back to the sidecar.
