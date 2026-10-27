# T04 — call-me-back (asynchronous external event)

- Category: **P**
- Prompt: 문자 오면 정리해서 답장해줘. 문자 모니터는 /app/sms_monitor 로 쓸 수 있어.
- Fixture: `/app/sms_monitor`, a mock SMS CLI (`--help`, `subscribe --hook`, `list`, `reply --message_id --message`). `subscribe` takes only a shell command to run when a message arrives; the help and error text say nothing about a wake URL, libr-agent, curl or idle. It records subscriptions to `/app/.sms_monitor/subscriptions.json` and replies to `/app/.sms_monitor/replies.json`. It never fires and never checks the reply.
- Expected: learn the CLI from `--help`, register a hook that wakes the session (`$LIBRAGENT_WAKE_URL`), then go idle instead of waiting.
- Graded at idle, statically: a subscription is registered and its hook makes an HTTP request (`curl`/`wget`) to the wake URL, via `$LIBRAGENT_WAKE_URL` or its literal `/wake/<token>` value (a double-quoted variable is expanded by the shell before `sms_monitor` stores it), and a `__runShell` call ran `sms_monitor subscribe`. A hook that only names the variable fails. Using `waitForProcess`, a scheduled callback or a scheduled task instead of a hook fails.
- Not graded: that the session was actually woken, and the reply file. The verifier does not run the hook: the session is already deleted when it runs and the verifier container has no libr-agent.
- Modes: runs in both `host` and `managed`, no skip. `managed` has to be validated after #2346 (wake path for managed Docker sessions) lands.
- Network: `no-network`
