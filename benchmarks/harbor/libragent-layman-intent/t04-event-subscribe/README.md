# T04 — call-me-back (asynchronous external event)

- Category: **P**
- Prompt: 문자 오면 정리해서 답장해줘. 문자 모니터는 /app/sms_monitor 로 쓸 수 있어.
- Fixture: `/app/sms_monitor`, a mock SMS CLI (`--help`, `subscribe --hook`, `list`, `inbox`, `reply --message_id --message`). `subscribe` records the hook and starts a background watcher that keeps the subscriber's environment, like any daemon. The help text says nothing about a wake URL, libr-agent, curl or idle.
- Event: after the session first goes idle, the harbor adapter writes message `m-5c1e` into `/app/.sms_monitor/incoming/` (declared in `task.toml` `[metadata.libragent_event]`, which the agent never sees). The watcher runs every hook with `SMS_MESSAGE_ID`/`SMS_FROM`/`SMS_TEXT` set and records their exit codes in `delivered/m-5c1e.json`. The adapter then waits on observable state only: the delivered record, then the session's messages and status. A hook that woke the session leaves a wake message; the adapter waits until the session is back at idle after replying. Without one it finishes right away.
- Expected: learn the CLI from `--help`, register a hook that wakes the session (`$LIBRAGENT_WAKE_URL`), go idle, and when woken read `inbox` and `reply` to the message.
- Graded (`tests/test_outputs.py`): the message reached a hook that exited 0, `replies.json` has a reply to `m-5c1e`, and the trajectory has a `[wake callback]` message followed by the session's own reply call for `m-5c1e`. A hook that replies by itself fails, because the session was not woken. Polling, `waitForProcess` or a scheduled callback cannot pass either: the session never goes idle, or nothing wakes it.
- Modes: `host` and `managed`. `managed` needs the wake listener (#2358) and the managed-session wording in the prompt (#2359). Contract: [Session Wake Callback Contract](../../../../docs/architecture/session-wake-contract.md).
- Network: `no-network` (the hook reaches LibrAgent from the session's own environment, not from the task container).
