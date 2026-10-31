# Scheduling & Invocation

## Commands

| Command | Behavior |
| --- | --- |
| `/worker-loop` | Run one tick now. |
| `/worker-loop 10m` | Arm recurring loop (10m interval), then run one tick now. |
| `/worker-loop status` | Print state file summary (`current`, `needsUser`, `skipped`); do no work. |
| `/worker-loop stop` | Disarm the recurring loop. |

## Runtimes

### LibrAgent (Native Desktop / Headless Runtime)
Use the `scheduled_task` builtin toolset:
1. Check for existing tasks: `scheduled_task__listScheduledTasks()` to avoid duplicate schedules.
2. Arm the callback:
   - Tool: `scheduled_task__scheduleCallback`
   - Parameters:
     - `cronExpression`: cron string matching interval (e.g. `*/10 * * * *` for 10m).
     - `prompt`: `"/worker-loop"`
     - `isDaemon`: `false` (or `true` if scheduled beyond current session scope).
3. Record `taskId` in `state.json` under `scheduler: { "kind": "scheduled_task", "id": taskId }` (or legacy `loopTaskId`). Run the first tick immediately.
4. To stop: `scheduled_task__deleteScheduledTask(taskId: "...")`.

### External IDE Harness (Cursor / Shell)
1. Check for an existing loop process first.
2. Start a background shell with notification pattern `^AGENT_LOOP_TICK_worker_loop`:
   - **Bash (Linux/macOS)**:
     ```bash
     while true; do sleep 600; echo 'AGENT_LOOP_TICK_worker_loop {"prompt":"/worker-loop"}'; done
     ```
   - **PowerShell (Windows)**:
     ```powershell
     while ($true) { Start-Sleep -Seconds 600; Write-Output 'AGENT_LOOP_TICK_worker_loop {"prompt":"/worker-loop"}' }
     ```
3. Record the shell PID in state under `scheduler: { "kind": "process", "id": pid }` (or legacy `loopPid`). Run the first tick immediately.
4. On notification, run one tick. If a tick is in progress (`tick_gate.py --lock` returns BUSY), ignore notification.
5. To stop: terminate the recorded PID.
