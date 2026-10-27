# T04 — call-me-back (event subscription)

- Category: **P**
- Prompt: 릴리스 게이트가 통과하면 알려줘. 이벤트는 /app/relayctl 로 받을 수 있어. 여기서 붙잡고 있지 말고, 통과하면 나를 깨워서 남은 작업을 이어서 하게 해줘.
- Fixture: `/app/relayctl`, a mock event CLI (`topics`, `subscribe --topic --exec [--once]`, `list`). It only records subscriptions to `/app/.relay/subscriptions.json`; nothing ever fires.
- Expected: learn the CLI from `--help` / `topics`, subscribe `release.gate.passed` with an `--exec` that calls `$LIBRAGENT_WAKE_URL`, then go idle.
- Graded: the recorded subscription (topic + a `curl`/`wget` call to the wake URL; an `--exec` that only names the variable fails) and a `__runShell` call running `relayctl subscribe`. `--exec` may hold the literal `$LIBRAGENT_WAKE_URL` or the expanded `/wake/<token>` URL (a double-quoted variable is expanded by the shell before `relayctl` stores it); `--once` is not graded. Harbor ends the trial at idle, so the wake itself is not judged.
- Modes: T04 holds in `host` mode (the default, `--isolated`), where `$LIBRAGENT_WAKE_URL` is exported. `managed` (Docker) sessions do not get the variable until #2343 item 1 lands, so T04 cannot pass there; that matches the `call-me-back` rule "no variable, don't use it" and is not a regression. The dependency is tracked in #2343 item 4.
- Network: `no-network`
