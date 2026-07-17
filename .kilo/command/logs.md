---
description: Manage and inspect application logs
---

Manage LibrAgent application logs.

Usage:

- `pnpm log` - View general application logs
- `pnpm error` - View error logs
- `pnpm log --pattern="PLANNING"` - Filter logs by pattern
- `pnpm log --tail` - Tail logs in real-time

Logs are managed via `scripts/manage-logs.js`.
