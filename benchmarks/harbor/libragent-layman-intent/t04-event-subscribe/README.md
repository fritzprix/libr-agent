# T04 — call-me-back (event subscription)

- Category: **P**
- Prompt: 릴리스 게이트가 통과하면 알려줘. 이벤트는 /app/relayctl 로 받을 수 있어. 여기서 붙잡고 있지 말고, 통과하면 나를 깨워서 남은 작업을 이어서 하게 해줘.
- Fixture: `/app/relayctl`, a mock event CLI (`topics`, `subscribe --topic --exec [--once]`, `list`). It only records subscriptions to `/app/.relay/subscriptions.json`; nothing ever fires.
- Expected: learn the CLI from `--help` / `topics`, subscribe `release.gate.passed` with an `--exec` that calls `$LIBRAGENT_WAKE_URL`, then go idle.
- Graded: the recorded subscription (topic + wake call) and a `__runShell` call running `relayctl subscribe`. `--exec` may hold the literal `$LIBRAGENT_WAKE_URL` or the expanded `/wake/<token>` URL (a double-quoted variable is expanded by the shell before `relayctl` stores it); `--once` is not graded. Harbor ends the trial at idle, so the wake itself is not judged.
- Network: `no-network`
