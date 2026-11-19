This task verifies the restart half of 2565 O-1: persisted settings survive an in-place restart of the isolated instance (visual restart badges/toasts stay on S2).

Write `SETTINGS_RESTART_READY` to `/app/o1_ready.txt` using a workspace file tool, then stop.
The automated harness will restart the isolated instance in place and probe `/api/health`, `/api/settings/preferredModel`, and the `settings` table.
