# Chrome Web Store — listing & review kit

Companion notes for publishing **LibrAgent Browser Bridge**. Package with
`pnpm chrome-extension:pack` (or `./scripts/package-store-zip.sh`).

## Recommended first publish

Use **Unlisted** distribution until review feedback is stable. Broad
`<all_urls>` + `scripting` attracts Privacy scrutiny.

## Developer Dashboard fields

### Store listing

| Field | Suggested value |
| --- | --- |
| Name | LibrAgent Browser Bridge |
| Summary (short) | Connect LibrAgent to everyday Chrome for local agent browser control. |
| Description | See **Detailed description** below |
| Category | Productivity |
| Language | English (add Korean later if needed) |

**Detailed description (draft):**

```
LibrAgent Browser Bridge lets the LibrAgent desktop app drive tabs in your
everyday Chrome profile on this computer.

• Open and navigate tabs for research and automation
• Read page content and interact (click / type) when you ask the agent
• Capture viewport screenshots for the agent
• Talks only to LibrAgent on localhost (loopback WebSocket)

Requires the LibrAgent desktop app running on the same machine. Without the
app, the toolbar popup shows “LibrAgent not running” and the extension does
nothing useful.

This extension does not send your browsing data to a LibrAgent cloud by itself.
```

### Privacy tab

| Field | Suggested value |
| --- | --- |
| Single purpose | Bridge the local LibrAgent desktop app to Chrome so agents can control tabs on this machine. |
| Remote code | No |
| User data | See `PRIVACY.md`. Hosted URL after merge: `https://github.com/fritzprix/libr-agent/blob/dev/0.9.x/chrome-extension/PRIVACY.md` (update branch/tag for the release you ship). |
| Certifications | Not sold; not used for unrelated purposes; disclosed permissions match behavior |

### Distribution

- Visibility: **Unlisted** (first ship) or Public later
- Regions: all (or restrict if desired)

### Test instructions (for reviewers)

Paste into the Dashboard **Test instructions** field:

```
This extension only works with the LibrAgent desktop app on the same machine.

1) Install / run LibrAgent (desktop). Confirm it listens on
   ws://127.0.0.1:3847/extension-bridge (default token: libragent-dev).
2) Install this extension from the uploaded package.
3) Open the toolbar popup — status should become “Connected” within a few seconds.
4) In LibrAgent, ask an agent to open https://example.com with browser tools
   (or call browser__createSession). A tab should open in the reviewer’s Chrome.
5) Quit LibrAgent — popup should show “LibrAgent not running” / Reconnecting.
   Restart LibrAgent — popup returns to Connected without chrome://extensions reload.

Credentials: none (local loopback only).
If LibrAgent binaries are unavailable to reviewers, contact the publisher via
the GitHub issue tracker for a build or screen recording.
```

## Asset checklist (Dashboard uploads)

| Asset | Status |
| --- | --- |
| Extension icons 16/48/128 in package | Provided (`icons/icon-*.png`) |
| Store icon 128×128 | Same as `icons/icon-128.png` |
| Small promo tile 440×280 | TODO — design before public listing |
| Screenshots (1280×800 or 640×400), ≥1 | TODO — popup Connected + Settings card |
| Marquee (optional) | Skip for unlisted |

## Package & upload steps

1. From repo root: `pnpm chrome-extension:pack`
2. Upload `dist/chrome-extension/libragent-browser-bridge-<version>.zip`
3. Fill Store Listing / Privacy / Distribution / Test instructions
4. Submit for review (unlisted recommended)

## Follow-ups (not blocking first unlisted upload)

- Rotating bridge token (replace fixed `libragent-dev` for production)
- Promo tiles + polished screenshots
- Host privacy policy on the user docs site with a stable URL
- Narrow host permissions if Chrome review requires it
