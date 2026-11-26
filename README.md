# PR #3024 evidence screenshots

Throwaway asset branch for pull-request evidence only — never merged.

Rendered with the REAL `ServerCard` component (real Tailwind/shadcn styles) in
headless Chrome against the Vite dev build; only the MCP server list and the
Tauri IPC calls are mocked. Grid width is controlled by the browser window,
exactly like the Extensions page (`grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4`).

| File | Scenario |
|---|---|
| `card-oauth.png` | single card, OAuth server without a token (Authorize inline with the status line) |
| `card-plain.png` | single card, connected + verified |
| `card-long.png` | single card, multi-line description (owner item 2) |
| `card-failed.png` | single card, failed verification (Retry beside the status line) |
| `card-wide.png` | wide grid (1440 px, 3 columns): long description next to short ones, failed + OAuth cards |
| `card-narrow.png` | narrow grid (460 px, 1 column) |
| `card-grid-3col.png` | tightest three-column width (1024 px) |

## Round 6 (head `b4d65be5f`, G1.5 verification)

| File | Scenario |
|---|---|
| `card-1024-three-col.png` | 1024 px, 3 columns incl. `notion-workspace` OAuth card; red outline = measured status-text rect, blue outline = measured button rect |
| `card-long-status.png` | same grid with a deliberately long unauthorized-status string: text truncates with ellipsis (`truncate` + `title` tooltip), Authorize button stays fully visible on the same row |
