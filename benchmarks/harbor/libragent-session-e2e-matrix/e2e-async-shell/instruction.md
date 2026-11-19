Start a background shell command using `workspace__spawnProcess` that prints `ASYNC_E2E_MARKER_$((40+2))` after a short pause (`sleep 0.5; echo ASYNC_E2E_MARKER_$((40+2))`).

Then wait for that process to complete with `workspace__waitForProcess` and read its stdout with `workspace__readProcessOutput`.

Do not print or echo `LIBRAGENT_E2E_SYNTHETIC_SECRET`. Never claim success unless the tool result contains the computed marker.
