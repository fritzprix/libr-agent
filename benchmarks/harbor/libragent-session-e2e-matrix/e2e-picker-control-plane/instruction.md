This task verifies the debug E2E file-picker control plane (#2903).

Write the exact text `PICKER_PROBE_READY` to `/app/picker_probe.txt` using a workspace file tool, then stop.
The automated harness probes `POST /api/e2e-control/picker` (path injection and cancel) against the isolated debug instance and records the evidence.
