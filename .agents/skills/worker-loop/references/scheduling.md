# Scheduling & Invocation

## Commands

| Command | Behavior |
| --- | --- |
| `/worker-loop` | Run one tick now. |
| `/worker-loop 10m` | Arm recurring loop (10m interval), then run one tick now. |
| `/worker-loop status` | Print `scripts/worker_state.py status` (every worker and schedule), then the open owner questions: `gh issue list --label needs-owner` and `gh pr list --label needs-owner`. Do no work. |
| `/worker-loop stop` | Disarm every loop listed by `worker_state.py scheduler clear`. Running ticks finish normally. |

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
3. Record it: `worker_state.py scheduler add --kind scheduled_task --id <taskId>`. Schedules belong to the repo, not to a slot, because each tick may get a different slot. Run the first tick immediately.
4. To stop: `worker_state.py scheduler clear`, then `scheduled_task__deleteScheduledTask(taskId: "...")` for each entry it prints.

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
3. Record it: `worker_state.py scheduler add --kind process --id <pid>`. Run the first tick immediately.
4. On notification, run one tick. `BUSY reason=same-session` means this session's previous tick is still running: ignore the notification. Without a stable session ID (see the Setup section of SKILL.md), a repeated notification cannot be told apart from a new session, and starts a second worker.
5. To stop: `worker_state.py scheduler clear`, then terminate each PID it prints.
