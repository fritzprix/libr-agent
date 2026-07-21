---
description: Start the full development environment (Vite + Tauri hot reload)
---

Start the LibrAgent development environment with hot reload.

Steps:

1. Run pre-start sync scripts if present: `node scripts/sync-builtin-services.cjs` and `node scripts/sync-execution-mode.cjs`
2. Start Tauri dev server: `pnpm tauri dev`
3. The app will be available at `http://localhost:1420` for frontend-only inspection if needed.

Use `pnpm dev` for frontend-only development (Vite without Tauri).
